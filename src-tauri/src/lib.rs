//! Tauri shell: wires `rupert-core` to the UI through commands.
//! Keep logic in `rupert-core`; commands here should stay thin.

use serde::Serialize;

// MARK: - Commands

#[derive(Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
    core_version: &'static str,
    os: &'static str,
    arch: &'static str,
}

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Rupert",
        version: env!("CARGO_PKG_VERSION"),
        core_version: rupert_core::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

// MARK: - Entry point

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![app_info])
        .run(tauri::generate_context!())
        .expect("error while running Rupert");
}
