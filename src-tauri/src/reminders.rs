//! Reminders (#109): the Coming up list, done and "remind me later", Add to
//! Calendar, and keeping the system's notifications and the Dock badge in
//! step with the library. The logic is in `rupert_core::reminders`.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rupert_core::i18n::tr;
use rupert_core::indexer::Job;
use rupert_core::meta::DocMeta;
use rupert_core::reminders::{self, Counts, Dated, Day, Item, Lead, Show};
use rupert_core::{CloudState, cloud_state, resolve};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::notify::{self, Action, Permission, Response};
use crate::{AppState, CmdResult, INDEX_CHANGED, err};

/// Sent when a notification asked to show Coming up (or one document in
/// it); the window also asks with `reminders_take_shown` once it's loaded.
const REMINDERS_SHOW: &str = "reminders-show";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowTarget {
    pub source_id: Option<String>,
    pub rel_path: Option<String>,
}

static SHOWN: Mutex<Option<ShowTarget>> = Mutex::new(None);
static POKE: OnceLock<Mutex<Sender<()>>> = OnceLock::new();

/// Plans notifications and the badge again soon (documents or settings
/// changed).
pub fn poke() {
    if let Some(tx) = POKE.get() {
        let _ = tx.lock().unwrap().send(());
    }
}

// MARK: - Reading

/// Documents with a due date in the sources Rupert has now.
fn dated(state: &AppState) -> Vec<Dated> {
    let ids: HashSet<String> = state.sources().into_iter().map(|s| s.id).collect();
    let docs = state.index.lock().unwrap().dated().unwrap_or_default();
    docs.into_iter()
        .filter(|d| ids.contains(&d.source_id))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct List {
    pub items: Vec<Item>,
    pub counts: Counts,
}

#[tauri::command]
pub async fn reminders_list(show: Show, app: AppHandle) -> CmdResult<List> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().get().reminders.clone();
        if !settings.enabled {
            return List {
                items: Vec::new(),
                counts: Counts::default(),
            };
        }
        let docs = dated(&state);
        let today = Day::today();
        List {
            counts: reminders::counts(&docs, &settings, today),
            items: reminders::coming_up(docs, &settings, today, show),
        }
    })
    .await
    .map_err(err)
}

#[tauri::command]
pub async fn reminders_counts(app: AppHandle) -> CmdResult<Counts> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let settings = state.settings.lock().unwrap().get().reminders.clone();
        if !settings.enabled {
            return Counts::default();
        }
        reminders::counts(&dated(&state), &settings, Day::today())
    })
    .await
    .map_err(err)
}

// MARK: - Done and remind me later

/// Writes done / "remind me later" into the document (see
/// `rupert_core::meta::set_reminder`) and updates the index at once.
fn update(
    app: &AppHandle,
    source_id: &str,
    rel_path: &str,
    done: Option<Option<String>>,
    snoozed: Option<Option<String>>,
) -> CmdResult<DocMeta> {
    let state = app.state::<AppState>();
    let source = state.source(source_id)?;
    let indexed = state
        .index
        .lock()
        .unwrap()
        .doc_meta(source_id, rel_path)
        .map_err(err)?;
    let path = resolve(&source.root, rel_path).map_err(err)?;
    let local = cloud_state(&path).map_err(err)? == CloudState::Local;
    let meta =
        rupert_core::meta::set_reminder(&path, done, snoozed, local, indexed).map_err(err)?;
    state
        .index
        .lock()
        .unwrap()
        .set_doc_reminder(
            source_id,
            rel_path,
            meta.done.as_deref(),
            meta.snoozed.as_deref(),
        )
        .map_err(err)?;
    let _ = app.emit(INDEX_CHANGED, ());
    state.indexer.send(Job::File {
        source,
        rel_path: rel_path.to_string(),
    });
    poke();
    Ok(meta)
}

/// Marks a document done (today) or not done.
#[tauri::command]
pub async fn reminders_set_done(
    source_id: String,
    rel_path: String,
    done: bool,
    app: AppHandle,
) -> CmdResult<DocMeta> {
    tauri::async_runtime::spawn_blocking(move || {
        let value = done.then(|| Day::today().iso());
        update(&app, &source_id, &rel_path, Some(value), None)
    })
    .await
    .map_err(err)?
}

/// Hides a document from Coming up for `days` days (`None`: shows it again).
#[tauri::command]
pub async fn reminders_snooze(
    source_id: String,
    rel_path: String,
    days: Option<i64>,
    app: AppHandle,
) -> CmdResult<DocMeta> {
    tauri::async_runtime::spawn_blocking(move || {
        let until = days.map(|d| Day::today().plus_days(d.clamp(1, 366)).iso());
        update(&app, &source_id, &rel_path, None, Some(until))
    })
    .await
    .map_err(err)?
}

// MARK: - Add to Calendar

/// Writes an iCalendar file for the document and opens it, so the
/// calendar app offers to add the event (with its alarm).
#[tauri::command]
pub async fn reminders_add_to_calendar(
    source_id: String,
    rel_path: String,
    app: AppHandle,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().get().reminders.clone();
    let doc = dated(&state)
        .into_iter()
        .find(|d| d.source_id == source_id && d.rel_path == rel_path)
        .ok_or_else(|| tr("This document has no due date.").to_string())?;
    let lead = settings.lead_for(&doc);
    let lead = if lead == Lead::Never {
        Lead::OnTheDay
    } else {
        lead
    };
    let stamp = utc_stamp();
    let ics = reminders::calendar_event(&doc, lead, &stamp)
        .ok_or_else(|| tr("This document has no due date.").to_string())?;
    let dir = std::env::temp_dir().join("Rupert");
    std::fs::create_dir_all(&dir).map_err(err)?;
    let name: String = doc
        .label()
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    let path = dir.join(format!("{name}.ics"));
    std::fs::write(&path, ics).map_err(err)?;
    tauri_plugin_opener::open_path(&path, None::<&str>).map_err(err)
}

/// Now in UTC as `YYYYMMDDTHHMMSSZ`.
fn utc_stamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let day = Day::parse("1970-01-01")
        .unwrap()
        .plus_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!(
        "{}T{:02}{:02}{:02}Z",
        day.iso().replace('-', ""),
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

// MARK: - Notifications

#[tauri::command]
pub async fn notifications_permission() -> CmdResult<Permission> {
    tauri::async_runtime::spawn_blocking(notify::permission)
        .await
        .map_err(err)
}

/// Asks the system once whether Rupert may notify (turning them on).
#[tauri::command]
pub async fn notifications_request() -> CmdResult<Permission> {
    let p = tauri::async_runtime::spawn_blocking(notify::request)
        .await
        .map_err(err)?;
    poke();
    Ok(p)
}

#[tauri::command]
pub fn notifications_open_settings() -> CmdResult<()> {
    let url = notify::open_settings_url().ok_or_else(|| tr("Not available here.").to_string())?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(err)
}

/// What a notification asked the window to show, once (for a click that
/// launched Rupert, before the window was there to hear it).
#[tauri::command]
pub fn reminders_take_shown() -> Option<ShowTarget> {
    SHOWN.lock().unwrap().take()
}

fn respond(app: &AppHandle, r: Response) {
    let doc = reminders::doc_of(&r.id);
    match (r.action, doc) {
        (Action::Done, Some((source, rel))) => {
            if let Err(e) = update(app, &source, &rel, Some(Some(Day::today().iso())), None) {
                eprintln!("rupert: couldn't mark done: {e}");
            }
        }
        (Action::Tomorrow, Some((source, rel))) => {
            let until = Day::today().plus_days(1).iso();
            if let Err(e) = update(app, &source, &rel, None, Some(Some(until))) {
                eprintln!("rupert: couldn't snooze: {e}");
            }
        }
        (_, doc) => {
            let target = ShowTarget {
                source_id: doc.as_ref().map(|d| d.0.clone()),
                rel_path: doc.map(|d| d.1),
            };
            *SHOWN.lock().unwrap() = Some(target.clone());
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
            let _ = app.emit(REMINDERS_SHOW, target);
        }
    }
}

// MARK: - Keeping in step

/// The buttons' titles; call before the app finishes launching.
pub fn install() {
    notify::install(notify::Titles {
        done: tr("Mark done").into(),
        show: tr("Show").into(),
        tomorrow: tr("Tomorrow").into(),
    });
}

/// Starts the thread that hands notifications to the system and sets the
/// badge: now, whenever [`poke`] is called, and every minute (a new day,
/// the notification time passing).
pub fn spawn(app: &AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    if POKE.set(Mutex::new(tx)).is_err() {
        return;
    }
    let responder = app.clone();
    notify::set_handler(move |r| respond(&responder, r));
    let app = app.clone();
    std::thread::spawn(move || {
        let mut last = None;
        loop {
            refresh(&app, &mut last);
            match rx.recv_timeout(Duration::from_secs(60)) {
                Ok(()) => {
                    // Let a burst of changes settle (an indexing pass).
                    std::thread::sleep(Duration::from_secs(2));
                    while rx.try_recv().is_ok() {}
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn refresh(app: &AppHandle, last: &mut Option<u64>) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let settings = state.settings.lock().unwrap().get().reminders.clone();
    let docs = if settings.enabled {
        dated(&state)
    } else {
        Vec::new()
    };
    let today = Day::today();
    let (hour, minute) = settings.time();
    let passed = reminders::local_minutes() >= hour * 60 + minute;
    let planned = reminders::plan(&docs, &settings, today, passed);
    let counts = reminders::counts(&docs, &settings, today);
    let badge = (settings.enabled && settings.dock_badge && counts.soon > 0).then(|| {
        let decimal = match rupert_core::i18n::current() {
            rupert_core::i18n::Lang::En => '.',
            _ => ',',
        };
        reminders::short_count(counts.soon, decimal)
    });

    let mut h = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&planned)
        .unwrap_or_default()
        .hash(&mut h);
    (&badge, hour, minute).hash(&mut h);
    let signature = h.finish();
    if *last == Some(signature) {
        return;
    }
    *last = Some(signature);
    // With notifications off, this takes back any handed over before.
    notify::schedule(&planned, hour, minute);
    #[cfg(target_os = "macos")]
    if let Some(w) = app.get_webview_window("main") {
        // "1K" past 999, as beside Coming up.
        let _ = w.set_badge_label(badge);
    }
}
