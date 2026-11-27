//! System notifications for reminders (#109), handed to the system ahead of
//! time so they arrive even when Rupert is closed.
//!
//! - macOS: the User Notifications framework, with Mark done / Show /
//!   Tomorrow buttons on one-per-document notifications. Needs Rupert to run
//!   as an app bundle (not under `tauri dev`).
//! - Windows: scheduled toasts under Rupert's app id (set on the Start menu
//!   shortcut by the installer), without buttons.
//! - Elsewhere: not available.

use std::sync::{Mutex, OnceLock};

use rupert_core::reminders::Planned;
use serde::Serialize;

/// What a click on a notification asked for.
// Clicks and buttons come only on macOS.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    /// The notification's id ([`Planned::id`]).
    pub id: String,
    pub action: Action,
}

// Clicks and buttons come only on macOS.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// The notification itself was clicked.
    Open,
    Done,
    Show,
    Tomorrow,
}

/// Whether Rupert may notify.
// Clicks and buttons come only on macOS.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    Granted,
    Denied,
    NotDetermined,
    /// This system (or this way of running Rupert) has no notifications.
    Unavailable,
}

/// The buttons' titles, in Rupert's language.
// Clicks and buttons come only on macOS.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct Titles {
    pub done: String,
    pub show: String,
    pub tomorrow: String,
}

// MARK: - Responses

type Handler = Box<dyn Fn(Response) + Send + Sync>;
static HANDLER: OnceLock<Handler> = OnceLock::new();
/// Responses that came before the handler was set (a click that launched
/// Rupert).
static WAITING: Mutex<Vec<Response>> = Mutex::new(Vec::new());

/// Where clicks go from now on (and those that came before).
pub fn set_handler(handler: impl Fn(Response) + Send + Sync + 'static) {
    if HANDLER.set(Box::new(handler)).is_err() {
        return;
    }
    let waiting = std::mem::take(&mut *WAITING.lock().unwrap());
    if let Some(h) = HANDLER.get() {
        for r in waiting {
            h(r);
        }
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn deliver(r: Response) {
    match HANDLER.get() {
        Some(h) => h(r),
        None => WAITING.lock().unwrap().push(r),
    }
}

// MARK: - Platform

#[cfg(target_os = "macos")]
pub use mac::{install, open_settings_url, permission, request, schedule};
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub use other::{install, open_settings_url, permission, request, schedule};
#[cfg(target_os = "windows")]
pub use win::{install, open_settings_url, permission, request, schedule};

#[cfg(target_os = "macos")]
mod mac {
    use std::ptr::NonNull;
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, ProtocolObject};
    use objc2::{AllocAnyThread, define_class, msg_send};
    use objc2_foundation::{
        NSArray, NSBundle, NSDateComponents, NSError, NSObject, NSObjectProtocol, NSSet, NSString,
    };
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNCalendarNotificationTrigger,
        UNMutableNotificationContent, UNNotification, UNNotificationAction,
        UNNotificationActionOptions, UNNotificationCategory, UNNotificationCategoryOptions,
        UNNotificationDefaultActionIdentifier, UNNotificationPresentationOptions,
        UNNotificationRequest, UNNotificationResponse, UNNotificationSettings, UNNotificationSound,
        UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    use super::{Action, Permission, Planned, Response, Titles, deliver};

    const DOC: &str = "doc";

    define_class!(
        /// Shows reminders while Rupert is in front too, and passes clicks on.
        #[unsafe(super(NSObject))]
        #[name = "RupertNotificationDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                handler.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                handler: &DynBlock<dyn Fn()>,
            ) {
                let id = response.notification().request().identifier().to_string();
                let action = response.actionIdentifier();
                // SAFETY: a framework constant.
                let default = unsafe { UNNotificationDefaultActionIdentifier };
                let action = if action.isEqualToString(default) {
                    Some(Action::Open)
                } else {
                    match action.to_string().as_str() {
                        "done" => Some(Action::Done),
                        "show" => Some(Action::Show),
                        "tomorrow" => Some(Action::Tomorrow),
                        _ => None,
                    }
                };
                if let Some(action) = action {
                    deliver(Response { id, action });
                }
                handler.call(());
            }
        }
    );

    /// Only an app bundle has a notification center: asking for it from a
    /// bare binary (`tauri dev`) throws.
    fn center() -> Option<Retained<UNUserNotificationCenter>> {
        let bundle = NSBundle::mainBundle();
        bundle.bundleIdentifier()?;
        bundle
            .bundlePath()
            .to_string()
            .ends_with(".app")
            .then(UNUserNotificationCenter::currentNotificationCenter)
    }

    /// Sets the delegate (as early as possible, so a click that launched
    /// Rupert is received) and the buttons.
    pub fn install(titles: Titles) {
        let Some(center) = center() else {
            return;
        };
        // Once; later calls (a new language) only rename the buttons.
        static DELEGATE: std::sync::Once = std::sync::Once::new();
        DELEGATE.call_once(|| {
            let delegate: Retained<Delegate> = {
                let this = Delegate::alloc().set_ivars(());
                // SAFETY: NSObject's initializer.
                unsafe { msg_send![super(this), init] }
            };
            center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
            // The center keeps only a weak reference.
            std::mem::forget(delegate);
        });
        let action = |id: &str, title: &str, options| {
            UNNotificationAction::actionWithIdentifier_title_options(
                &NSString::from_str(id),
                &NSString::from_str(title),
                options,
            )
        };
        let actions = NSArray::from_retained_slice(&[
            action("done", &titles.done, UNNotificationActionOptions::empty()),
            action(
                "show",
                &titles.show,
                UNNotificationActionOptions::Foreground,
            ),
            action(
                "tomorrow",
                &titles.tomorrow,
                UNNotificationActionOptions::empty(),
            ),
        ]);
        let category =
            UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
                &NSString::from_str(DOC),
                &actions,
                &NSArray::new(),
                UNNotificationCategoryOptions::empty(),
            );
        center.setNotificationCategories(&NSSet::from_retained_slice(&[category]));
    }

    pub fn permission() -> Permission {
        let Some(center) = center() else {
            return Permission::Unavailable;
        };
        let (tx, rx) = mpsc::channel();
        let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            // SAFETY: the framework passes a valid object.
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            let _ = tx.send(if status == UNAuthorizationStatus::NotDetermined {
                Permission::NotDetermined
            } else if status == UNAuthorizationStatus::Denied {
                Permission::Denied
            } else {
                Permission::Granted
            });
        });
        center.getNotificationSettingsWithCompletionHandler(&block);
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or(Permission::Unavailable)
    }

    /// Asks once (macOS shows its own prompt); later it only reports.
    pub fn request() -> Permission {
        let Some(center) = center() else {
            return Permission::Unavailable;
        };
        let (tx, rx) = mpsc::channel();
        let block = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            // SAFETY: the framework passes a valid error or null.
            if let Some(e) = unsafe { error.as_ref() } {
                eprintln!(
                    "rupert: notifications refused: {}",
                    e.localizedDescription()
                );
            }
            let _ = tx.send(granted.as_bool());
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &block,
        );
        // The prompt waits for the person.
        match rx.recv_timeout(Duration::from_secs(300)) {
            Ok(true) => Permission::Granted,
            Ok(false) => Permission::Denied,
            Err(_) => permission(),
        }
    }

    /// Replaces every reminder handed to macOS with `planned`, each on its
    /// day at `hour:minute`.
    pub fn schedule(planned: &[Planned], hour: u32, minute: u32) {
        let Some(center) = center() else {
            return;
        };
        center.removeAllPendingNotificationRequests();
        for p in planned {
            let Some(day) = rupert_core::reminders::Day::parse(&p.day) else {
                continue;
            };
            let (y, m, d) = day.ymd();
            let content = UNMutableNotificationContent::new();
            content.setTitle(&NSString::from_str(&p.title));
            content.setBody(&NSString::from_str(&p.body));
            content.setThreadIdentifier(&NSString::from_str("reminders"));
            content.setSound(Some(&UNNotificationSound::defaultSound()));
            if p.source_id.is_some() {
                content.setCategoryIdentifier(&NSString::from_str(DOC));
            }
            let when = NSDateComponents::new();
            when.setYear(y as isize);
            when.setMonth(m as isize);
            when.setDay(d as isize);
            when.setHour(hour as isize);
            when.setMinute(minute as isize);
            let trigger = UNCalendarNotificationTrigger::triggerWithDateMatchingComponents_repeats(
                &when, false,
            );
            let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
                &NSString::from_str(&p.id),
                &content,
                Some(&trigger),
            );
            center.addNotificationRequest_withCompletionHandler(&request, None);
        }
    }

    /// System Settings ▸ Notifications ▸ Rupert.
    pub fn open_settings_url() -> Option<String> {
        let id = NSBundle::mainBundle().bundleIdentifier()?;
        Some(format!(
            "x-apple.systempreferences:com.apple.Notifications-Settings.extension?id={id}"
        ))
    }
}

#[cfg(target_os = "windows")]
mod win {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::Foundation::DateTime;
    use windows::UI::Notifications::{ScheduledToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    use super::{Permission, Planned, Titles};

    /// Rupert's app id: the installer puts the bundle identifier on the
    /// Start menu shortcut, which is what lets an app show toasts.
    const APP_ID: &str = "io.github.micki-b.rupert";
    const GROUP: &str = "reminders";

    pub fn install(_titles: Titles) {}

    pub fn permission() -> Permission {
        // People turn them off in Windows Settings; Rupert can't tell.
        Permission::Granted
    }

    pub fn request() -> Permission {
        Permission::Granted
    }

    pub fn schedule(planned: &[Planned], hour: u32, minute: u32) {
        if let Err(e) = try_schedule(planned, hour, minute) {
            eprintln!("rupert: couldn't schedule reminders: {e}");
        }
    }

    fn try_schedule(planned: &[Planned], hour: u32, minute: u32) -> windows::core::Result<()> {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_ID))?;
        let existing = notifier.GetScheduledToastNotifications()?;
        for i in 0..existing.Size()? {
            let toast = existing.GetAt(i)?;
            if toast.Group()? == GROUP {
                notifier.RemoveFromSchedule(&toast)?;
            }
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        for (n, p) in planned.iter().enumerate() {
            let Some(secs) = rupert_core::reminders::Day::parse(&p.day)
                .and_then(|d| rupert_core::reminders::local_epoch(d, hour, minute))
            else {
                continue;
            };
            if secs <= now + 5 {
                continue;
            }
            let xml = XmlDocument::new()?;
            xml.LoadXml(&HSTRING::from(format!(
                "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
                escape(&p.title),
                escape(&p.body)
            )))?;
            // 100-nanosecond steps since 1601.
            let when = DateTime {
                UniversalTime: (secs + 11_644_473_600) * 10_000_000,
            };
            let toast = ScheduledToastNotification::CreateScheduledToastNotification(&xml, when)?;
            toast.SetGroup(&HSTRING::from(GROUP))?;
            toast.SetTag(&HSTRING::from(format!("r{n}")))?;
            notifier.AddToSchedule(&toast)?;
        }
        Ok(())
    }

    fn escape(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }

    pub fn open_settings_url() -> Option<String> {
        Some("ms-settings:notifications".into())
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
mod other {
    use super::{Permission, Planned, Titles};

    pub fn install(_titles: Titles) {}

    pub fn permission() -> Permission {
        Permission::Unavailable
    }

    pub fn request() -> Permission {
        Permission::Unavailable
    }

    pub fn schedule(_planned: &[Planned], _hour: u32, _minute: u32) {}

    pub fn open_settings_url() -> Option<String> {
        None
    }
}
