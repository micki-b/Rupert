//! Print to Rupert (#101): Settings ▸ Inbox's switch for the entry that
//! saves into the inbox from outside Rupert (the Print dialog's PDF menu on
//! a Mac, File Explorer's Send to on Windows). The entry itself is
//! `rupert_core::print_entry`; this turns it on and off and keeps it
//! pointing at the inbox when the inbox (or the language) changes.
//!
//! The Mac App Store edition can't write to `~/Library/PDF Services` on its
//! own: turning the switch on asks where to save the entry, in a save panel
//! opened there, and keeps a bookmark to it for later changes.

use std::path::PathBuf;

use rupert_core::i18n::tr;
use rupert_core::print_entry;
use rupert_core::settings::Settings;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult, SETTINGS_CHANGED, err};

/// The inbox folder on disk, if one is set.
fn inbox_dir(state: &AppState, settings: &Settings) -> CmdResult<PathBuf> {
    let inbox = settings
        .inbox
        .as_ref()
        .ok_or(tr("Choose the inbox folder first."))?;
    let source = state.source(&inbox.source_id)?;
    rupert_core::resolve(&source.root, &inbox.rel_path).map_err(err)
}

/// Turns the entry on or off. Resolves to the settings as saved (unchanged
/// when the Mac App Store edition's save panel was cancelled).
#[tauri::command]
pub async fn set_print_entry(on: bool, app: AppHandle) -> CmdResult<Settings> {
    let state = app.state::<AppState>();
    let mut settings = state.settings.lock().unwrap().get().clone();
    if on {
        let inbox = inbox_dir(&state, &settings)?;
        match place(&app, &settings, &inbox).await? {
            Some(bookmark) => {
                settings.print_entry = true;
                settings.print_entry_bookmark = bookmark;
            }
            None => return Ok(settings), // cancelled
        }
    } else {
        take_away(&settings)?;
        settings.print_entry = false;
        settings.print_entry_bookmark = None;
    }
    state
        .settings
        .lock()
        .unwrap()
        .set(settings.clone())
        .map_err(err)?;
    let _ = app.emit(SETTINGS_CHANGED, &settings);
    Ok(settings)
}

/// Points the entry at the inbox again (after the inbox or the language
/// changed). Quietly does nothing when it's off; a failure is only logged.
pub fn refresh(state: &AppState, settings: &Settings) {
    if !settings.print_entry {
        return;
    }
    let result = inbox_dir(state, settings).and_then(|inbox| renew(settings, &inbox));
    if let Err(e) = result {
        eprintln!("rupert: couldn't update the Print to Rupert entry: {e}");
    }
}

// MARK: - Outside the sandbox

/// Makes the entry where it belongs. `Some(bookmark)` once it's there.
#[cfg(not(feature = "app-store"))]
async fn place(
    _app: &AppHandle,
    _settings: &Settings,
    inbox: &std::path::Path,
) -> CmdResult<Option<Option<String>>> {
    let folder = print_entry::folder().ok_or(tr("there's no such entry on Linux"))?;
    print_entry::install(&folder, inbox).map_err(err)?;
    Ok(Some(None))
}

#[cfg(not(feature = "app-store"))]
fn renew(_settings: &Settings, inbox: &std::path::Path) -> CmdResult<()> {
    let folder = print_entry::folder().ok_or(tr("there's no such entry on Linux"))?;
    print_entry::install(&folder, inbox).map(drop).map_err(err)
}

#[cfg(not(feature = "app-store"))]
fn take_away(_settings: &Settings) -> CmdResult<()> {
    match print_entry::folder() {
        Some(folder) => print_entry::remove(&folder).map_err(err),
        None => Ok(()),
    }
}

// MARK: - Mac App Store edition

/// Asks where to save the entry (a save panel opened in PDF Services), makes
/// it there and returns a bookmark to it. `None` if cancelled.
#[cfg(feature = "app-store")]
async fn place(
    app: &AppHandle,
    _settings: &Settings,
    inbox: &std::path::Path,
) -> CmdResult<Option<Option<String>>> {
    let window = app
        .get_webview_window(crate::settings_window::LABEL)
        .or_else(|| app.get_webview_window("main"))
        .ok_or("no window")?;
    let (tx, rx) = std::sync::mpsc::channel();
    window
        .run_on_main_thread(move || {
            let _ = tx.send(ask_where());
        })
        .map_err(err)?;
    let Some(entry) = rx.recv().map_err(err)? else {
        return Ok(None);
    };
    print_entry::write_at(&entry, inbox).map_err(err)?;
    let bookmark = rupert_core::bookmark::create(&entry).map_err(err)?;
    Ok(Some(bookmark))
}

/// The save panel, in `~/Library/PDF Services` (or `~/Library`, to make
/// that folder, if it isn't there yet).
#[cfg(feature = "app-store")]
fn ask_where() -> Option<PathBuf> {
    use objc2_app_kit::{NSModalResponseOK, NSSavePanel};
    use objc2_foundation::{NSString, NSURL};
    let mtm = objc2::MainThreadMarker::new()?;
    let folder = print_entry::folder()?;
    let start = if folder.is_dir() {
        folder.clone()
    } else {
        folder.parent()?.to_path_buf()
    };
    let panel = NSSavePanel::savePanel(mtm);
    panel.setDirectoryURL(NSURL::from_path(&start, true, None).as_deref());
    panel.setNameFieldStringValue(&NSString::from_str(print_entry::name()));
    panel.setCanCreateDirectories(true);
    panel.setMessage(Some(&NSString::from_str(tr(
        "Save the entry in the PDF Services folder (in your Library folder; make it if it isn't there). The Print dialog's PDF menu then offers it.",
    ))));
    if panel.runModal() != NSModalResponseOK {
        return None;
    }
    panel.URL()?.to_file_path()
}

#[cfg(feature = "app-store")]
fn renew(settings: &Settings, inbox: &std::path::Path) -> CmdResult<()> {
    let bookmark = settings
        .print_entry_bookmark
        .as_deref()
        .ok_or("no bookmark")?;
    let entry = rupert_core::bookmark::open(bookmark).map_err(err)?.path;
    print_entry::write_at(&entry, inbox).map_err(err)
}

#[cfg(feature = "app-store")]
fn take_away(settings: &Settings) -> CmdResult<()> {
    if let Some(bookmark) = settings.print_entry_bookmark.as_deref()
        && let Ok(opened) = rupert_core::bookmark::open(bookmark)
        && opened.path.is_file()
    {
        std::fs::remove_file(&opened.path).map_err(err)?;
    }
    Ok(())
}
