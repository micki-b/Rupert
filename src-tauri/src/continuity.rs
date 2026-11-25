//! Continuity Camera (#124, macOS): "Scan Documents" with a nearby iPhone or
//! iPad, straight into the inbox. No Rupert app is needed on the phone; it
//! only has to be signed in to the same Apple Account, with Wi-Fi and
//! Bluetooth on.
//!
//! AppKit takes over a menu item with the identifier
//! `NSMenuItemImportFromDeviceIdentifier` (its own title, and a submenu with
//! the nearby devices and what each can do), but only one that's in the menu
//! bar when the menu bar is installed: not one added later, nor one in any
//! other menu (tried 2026-11-25 with a bare AppKit app). So the item goes
//! into File and the menu bar is installed again; the Scan panel's button
//! borrows that same item for its own menu. AppKit then offers what the
//! responder chain accepts, of the types the app registered: [`Receiver`],
//! right after the web view, takes PDFs only (so only Scan Documents is
//! offered, not Take Photo or Add Sketch), and only while there's an inbox.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSMenu, NSMenuItem, NSMenuItemImportFromDeviceIdentifier, NSPasteboard,
    NSPasteboardTypePDF, NSResponder, NSUserInterfaceItemIdentification, NSView, NSWindow,
};
use objc2_foundation::{NSArray, NSPoint, NSString};
use rupert_core::fileops;
use rupert_core::i18n::tr;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{AppState, CmdResult, err};

/// Sent when a scan arrived (or couldn't be saved).
pub const CONTINUITY_SCAN: &str = "continuity-scan";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Scanned {
    /// The file's name in the inbox, when it was saved.
    name: Option<String>,
    error: Option<String>,
}

// MARK: - Receiving

struct Ivars {
    app: AppHandle,
}

define_class!(
    /// Accepts a PDF from Continuity Camera and saves it in the inbox.
    #[unsafe(super(NSResponder))]
    #[thread_kind = MainThreadOnly]
    #[name = "RupertContinuityReceiver"]
    #[ivars = Ivars]
    struct Receiver;

    impl Receiver {
        #[unsafe(method(validRequestorForSendType:returnType:))]
        fn valid_requestor(
            &self,
            send: Option<&NSString>,
            ret: Option<&NSString>,
        ) -> *mut AnyObject {
            // SAFETY: an AppKit constant.
            let pdf = unsafe { NSPasteboardTypePDF };
            if send.is_none()
                && ret.is_some_and(|t| t.isEqualToString(pdf))
                && inbox(&self.ivars().app).is_ok()
            {
                return self as *const Self as *mut AnyObject;
            }
            // SAFETY: NSResponder's own answer (it asks the next responder).
            unsafe { msg_send![super(self), validRequestorForSendType: send, returnType: ret] }
        }

        #[unsafe(method(readSelectionFromPasteboard:))]
        fn read_selection(&self, pasteboard: &NSPasteboard) -> bool {
            // SAFETY: an AppKit constant.
            let pdf = unsafe { NSPasteboardTypePDF };
            // (No early `return` in these methods: the macro wraps the body.)
            pasteboard
                .dataForType(pdf)
                .is_some_and(|data| save(&self.ivars().app, &data.to_vec()))
        }
    }
);

impl Receiver {
    fn new(mtm: MainThreadMarker, app: AppHandle) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { app });
        // SAFETY: NSResponder's designated initializer.
        unsafe { msg_send![super(this), init] }
    }
}

/// The inbox folder, if one is set and there.
fn inbox(app: &AppHandle) -> CmdResult<std::path::PathBuf> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().get().clone();
    let dir = crate::print_entry::inbox_dir(&state, &settings)?;
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(tr("Choose the inbox folder first.").into())
    }
}

/// Saves a scan in the inbox as "Scan 2026-11-25 14.03.07.pdf" (as the
/// phones name theirs) and tells the window.
fn save(app: &AppHandle, pdf: &[u8]) -> bool {
    let result = inbox(app).and_then(|dir| {
        fileops::copy_in(&dir, &format!("{}.pdf", scan_name()), pdf, None, true).map_err(err)
    });
    let scanned = match &result {
        Ok(path) => Scanned {
            name: path.file_name().map(|n| n.to_string_lossy().into_owned()),
            error: None,
        },
        Err(e) => Scanned {
            name: None,
            error: Some(e.clone()),
        },
    };
    let _ = app.emit(CONTINUITY_SCAN, scanned);
    result.is_ok()
}

/// "Scan 2026-11-25 14.03.07", in local time.
fn scan_name() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: localtime_r fills the struct it's given.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&now, &mut tm) };
    format!(
        "Scan {:04}-{:02}-{:02} {:02}.{:02}.{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

// MARK: - Setting up

/// Puts the receiver in the main window's responder chain, right after the
/// web view: the web view passes on what it doesn't take itself to its next
/// responder, while the window hands such questions to its delegate and the
/// application instead (so a receiver after the window is never asked).
pub fn install(app: &AppHandle) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // AppKit only asks the responder chain about types registered here.
    // SAFETY: an AppKit constant.
    let pdf = unsafe { NSPasteboardTypePDF };
    NSApplication::sharedApplication(mtm)
        .registerServicesMenuSendTypes_returnTypes(&NSArray::new(), &NSArray::from_slice(&[pdf]));
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let handle = app.clone();
    let _ = window.with_webview(move |webview| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        // SAFETY: the WKWebView, alive as long as its window; main thread.
        let view = unsafe { &*(webview.inner() as *const NSView) };
        let receiver = Receiver::new(mtm, handle);
        // SAFETY: the web view's next responder (its superview) stays after
        // ours; the receiver lives as long as the app (never released).
        unsafe {
            receiver.setNextResponder(view.nextResponder().as_deref());
            view.setNextResponder(Some(&receiver));
        }
        std::mem::forget(receiver);
    });
}

thread_local! {
    /// The item in File, once AppKit has taken it over; the Scan panel's
    /// button borrows it.
    static ITEM: std::cell::RefCell<Option<Retained<NSMenuItem>>> =
        const { std::cell::RefCell::new(None) };
}

/// Index of the item in File: after Import into Inbox….
const FILE_INDEX: isize = 1;

/// File (the menu bar's second menu).
fn file_menu(mtm: MainThreadMarker) -> Option<Retained<NSMenu>> {
    NSApplication::sharedApplication(mtm)
        .mainMenu()
        .and_then(|m| m.itemAtIndex(1))
        .and_then(|i| i.submenu())
}

/// Adds File ▸ Import from iPhone or iPad (the menu bar is built with
/// Tauri's menu, which can't set an item's identifier), then installs the
/// menu bar again so AppKit takes the item over. Call after each
/// `set_menu`.
pub fn add_to_file_menu(app: &AppHandle) {
    let _ = app.run_on_main_thread(|| {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let ns_app = NSApplication::sharedApplication(mtm);
        let (Some(main), Some(file)) = (ns_app.mainMenu(), file_menu(mtm)) else {
            return;
        };
        let item = NSMenuItem::new(mtm);
        item.setTitle(&NSString::from_str(tr("Import from iPhone or iPad")));
        // SAFETY: an AppKit constant.
        item.setIdentifier(Some(unsafe { NSMenuItemImportFromDeviceIdentifier }));
        file.insertItem_atIndex(&item, FILE_INDEX);
        // Installing the same menu again does nothing; without one between,
        // it counts.
        ns_app.setMainMenu(None);
        ns_app.setMainMenu(Some(&main));
        // AppKit's own title is English unless the app bundles its
        // language; Rupert's follows Rupert's language.
        item.setTitle(&NSString::from_str(tr("Import from iPhone or iPad")));
        ITEM.with(|i| *i.borrow_mut() = Some(item));
    });
}

// MARK: - Commands

/// The Scan panel's "Scan with iPhone…": a menu at the button with the
/// nearby devices (x, y: the button's bottom left, in the page's points).
#[tauri::command]
pub fn continuity_menu(window: WebviewWindow, x: f64, y: f64) -> CmdResult<()> {
    let w = window.clone();
    window
        .run_on_main_thread(move || {
            let Some(mtm) = MainThreadMarker::new() else {
                return;
            };
            let Ok(ptr) = w.ns_window() else { return };
            // SAFETY: Tauri's NSWindow, alive as long as its window; main thread.
            let ns_window = unsafe { &*(ptr as *const NSWindow) };
            let Some(view) = ns_window.contentView() else {
                return;
            };
            // The page's y runs down; the view's up, unless it's flipped.
            let y = if view.isFlipped() {
                y
            } else {
                view.bounds().size.height - y
            };
            let Some(item) = ITEM.with(|i| i.borrow().clone()) else {
                return;
            };
            // Borrowed from File for as long as the menu is open (an item
            // is in one menu at a time), then put back.
            let file = file_menu(mtm).filter(|f| f.indexOfItem(&item) >= 0);
            if let Some(file) = &file {
                file.removeItem(&item);
            }
            let menu = NSMenu::new(mtm);
            menu.addItem(&item);
            menu.popUpMenuPositioningItem_atLocation_inView(None, NSPoint::new(x, y), Some(&view));
            menu.removeItem(&item);
            if let Some(file) = file {
                file.insertItem_atIndex(&item, FILE_INDEX.min(file.numberOfItems()));
            }
        })
        .map_err(err)
}
