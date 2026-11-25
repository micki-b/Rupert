//! The Settings window (#80): Rupert ▸ Settings… (⌘,), the Settings button
//! in the status bar, and shortcuts such as the chat's gear. The page itself
//! is the `/settings` route, with the tabs in a sidebar.

use rupert_core::i18n::tr;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{CmdResult, err};

pub const LABEL: &str = "settings";
/// Sent to an open Settings window to show a tab (the payload).
const SHOW_TAB: &str = "settings-show";

// MARK: - Window

/// Opens the Settings window, or brings it forward, at `tab` if given (a
/// tab's id, or "updates" for General with a check for updates).
pub fn show(app: &AppHandle, tab: Option<&str>) -> tauri::Result<()> {
    // Tab ids are plain words; anything else is ignored, so the id is safe
    // to put in a script.
    let tab = tab.filter(|t| {
        !t.is_empty() && t.len() <= 32 && t.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    });
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Some(t) = tab {
            app.emit_to(LABEL, SHOW_TAB, t)?;
        }
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let start = tab
        .map(|t| format!("window.__rupertSettingsTab = '{t}';"))
        .unwrap_or_default();
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings".into()))
        .title(tr("Settings"))
        .inner_size(780.0, 580.0)
        .min_inner_size(620.0, 420.0)
        .initialization_script(start)
        .build()?;
    Ok(())
}

/// After a language change: the window's title in the new language.
pub fn retitle(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.set_title(tr("Settings"));
    }
}

// MARK: - Command

/// Async: creating a window from a synchronous command can deadlock on
/// Windows.
#[tauri::command]
pub async fn open_settings(tab: Option<String>, app: AppHandle) -> CmdResult<()> {
    show(&app, tab.as_deref()).map_err(err)
}
