//! Tauri shell: wires `rupert-core` to the UI through commands.
//! Keep logic in `rupert-core`; commands here should stay thin.

use std::sync::Mutex;

use rupert_core::{DirChange, Entry, LibraryWatcher, Source, SourceStore};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Event sent to the UI when folders change on disk. Payload: `Vec<DirChange>`.
const DIR_CHANGED: &str = "dir-changed";

// MARK: - State

struct AppState {
    store: Mutex<SourceStore>,
    /// `None` if the OS refused to start a watcher; browsing still works.
    watcher: Mutex<Option<LibraryWatcher>>,
}

impl AppState {
    fn rewatch(&self) {
        let store = self.store.lock().unwrap();
        if let Some(watcher) = self.watcher.lock().unwrap().as_mut()
            && let Err(e) = watcher.set_sources(store.sources())
        {
            eprintln!("rupert: couldn't watch sources: {e}");
        }
    }

    fn source(&self, id: &str) -> Result<Source, String> {
        let store = self.store.lock().unwrap();
        store.get(id).cloned().map_err(err)
    }
}

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// MARK: - Commands: app

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
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

// MARK: - Commands: sources

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceView {
    #[serde(flatten)]
    source: Source,
    available: bool,
}

impl From<Source> for SourceView {
    fn from(source: Source) -> Self {
        let available = source.is_available();
        Self { source, available }
    }
}

#[tauri::command]
fn list_sources(state: State<AppState>) -> Vec<SourceView> {
    let store = state.store.lock().unwrap();
    store.sources().iter().cloned().map(Into::into).collect()
}

#[tauri::command]
fn add_source(path: String, state: State<AppState>) -> CmdResult<SourceView> {
    let source = state.store.lock().unwrap().add(path, None).map_err(err)?;
    state.rewatch();
    Ok(source.into())
}

#[tauri::command]
fn remove_source(id: String, state: State<AppState>) -> CmdResult<()> {
    state.store.lock().unwrap().remove(&id).map_err(err)?;
    state.rewatch();
    Ok(())
}

#[tauri::command]
fn update_source(
    id: String,
    name: Option<String>,
    show_all_types: Option<bool>,
    state: State<AppState>,
) -> CmdResult<SourceView> {
    let mut store = state.store.lock().unwrap();
    Ok(store.update(&id, name, show_all_types).map_err(err)?.into())
}

/// Re-checks which sources are reachable and re-watches them (e.g. after a
/// sync folder or drive comes back).
#[tauri::command]
fn refresh_sources(state: State<AppState>) -> Vec<SourceView> {
    state.rewatch();
    list_sources(state)
}

// MARK: - Commands: browsing

#[tauri::command]
async fn list_dir(
    source_id: String,
    rel_path: String,
    state: State<'_, AppState>,
) -> CmdResult<Vec<Entry>> {
    let source = state.source(&source_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        rupert_core::list_dir(&source.root, &rel_path, source.show_all_types)
    })
    .await
    .map_err(err)?
    .map_err(err)
}

/// Opens a file in its default app. For online-only files this is what
/// triggers the download — only ever on an explicit user action.
#[tauri::command]
fn open_entry(
    source_id: String,
    rel_path: String,
    app: AppHandle,
    state: State<AppState>,
) -> CmdResult<()> {
    let source = state.source(&source_id)?;
    let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(err)
}

#[tauri::command]
fn reveal_entry(
    source_id: String,
    rel_path: String,
    app: AppHandle,
    state: State<AppState>,
) -> CmdResult<()> {
    let source = state.source(&source_id)?;
    let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
    app.opener().reveal_item_in_dir(path).map_err(err)
}

// MARK: - Entry point

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let store = SourceStore::load(config_dir.join("sources.json"))?;

            let handle = app.handle().clone();
            let watcher = LibraryWatcher::new(move |changes: Vec<DirChange>| {
                let _ = handle.emit(DIR_CHANGED, changes);
            })
            .inspect_err(|e| eprintln!("rupert: file watching unavailable: {e}"))
            .ok();

            let state = AppState {
                store: Mutex::new(store),
                watcher: Mutex::new(watcher),
            };
            state.rewatch();
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_sources,
            add_source,
            remove_source,
            update_source,
            refresh_sources,
            list_dir,
            open_entry,
            reveal_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rupert");
}
