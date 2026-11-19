//! The help window: Help ▸ Rupert Help on macOS, F1, or the Help button in
//! the status bar. The page itself is the `/help` route.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{CmdResult, err};

const LABEL: &str = "help";

// MARK: - Window

/// Opens the help window, or brings it forward, at `section` (an id on the
/// help page) if given.
pub fn show(app: &AppHandle, section: Option<&str>) -> tauri::Result<()> {
    // Section ids are plain words; anything else is ignored, so the id is
    // safe to put in a script.
    let section = section.filter(|s| {
        !s.is_empty() && s.len() <= 64 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    });
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Some(s) = section {
            window.eval(format!("location.hash = '{s}'"))?;
        }
        let _ = window.unminimize();
        window.show()?;
        window.set_focus()?;
        return Ok(());
    }
    let start = section
        .map(|s| format!("window.__rupertHelpSection = '{s}';"))
        .unwrap_or_default();
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("help".into()))
        .title("Rupert Help")
        .inner_size(920.0, 720.0)
        .min_inner_size(520.0, 400.0)
        .initialization_script(start)
        .build()?;
    Ok(())
}

// MARK: - Command

/// Async: creating a window from a synchronous command can deadlock on
/// Windows.
#[tauri::command]
pub async fn open_help(section: Option<String>, app: AppHandle) -> CmdResult<()> {
    show(&app, section.as_deref()).map_err(err)
}
