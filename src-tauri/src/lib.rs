//! Tauri shell: wires `rupert-core` to the UI through commands.
//! Keep logic in `rupert-core`; commands here should stay thin.

use std::path::PathBuf;
use std::sync::Mutex;

use rupert_core::{
    CloudState, DirChange, DocMeta, Entry, FileKind, FolderMeta, Hit, Index, IndexStats,
    IndexerHandle, Job, LibraryWatcher, MetaRecord, Progress, SearchQuery, Source, SourceStore,
    TagCount,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Folders changed on disk. Payload: `Vec<DirChange>`.
const DIR_CHANGED: &str = "dir-changed";
/// Indexing progress. Payload: `Progress`.
const INDEX_PROGRESS: &str = "index-progress";
/// The index changed; refresh searches and tag lists. No payload.
const INDEX_CHANGED: &str = "index-changed";

// MARK: - State

struct AppState {
    store: Mutex<SourceStore>,
    /// `None` if the OS refused to start a watcher; browsing still works.
    watcher: Mutex<Option<LibraryWatcher>>,
    /// Read connection for searches (the indexer thread has its own).
    index: Mutex<Index>,
    indexer: IndexerHandle,
    progress: Mutex<Progress>,
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

    fn sources(&self) -> Vec<Source> {
        self.store.lock().unwrap().sources().to_vec()
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
    state.sources().into_iter().map(Into::into).collect()
}

#[tauri::command]
fn add_source(path: String, state: State<AppState>) -> CmdResult<SourceView> {
    let source = state.store.lock().unwrap().add(path, None).map_err(err)?;
    state.rewatch();
    state.indexer.send(Job::SyncSource(source.clone()));
    Ok(source.into())
}

#[tauri::command]
fn remove_source(id: String, state: State<AppState>) -> CmdResult<()> {
    state.store.lock().unwrap().remove(&id).map_err(err)?;
    state.rewatch();
    state.indexer.send(Job::RemoveSource(id));
    Ok(())
}

#[tauri::command]
fn update_source(
    id: String,
    name: Option<String>,
    show_all_types: Option<bool>,
    state: State<AppState>,
) -> CmdResult<SourceView> {
    let updated = state
        .store
        .lock()
        .unwrap()
        .update(&id, name, show_all_types)
        .map_err(err)?;
    if show_all_types.is_some() {
        state.indexer.send(Job::SyncSource(updated.clone()));
    }
    Ok(updated.into())
}

/// Re-checks which sources are reachable and re-watches them (e.g. after a
/// sync folder or drive comes back).
#[tauri::command]
fn refresh_sources(state: State<AppState>) -> Vec<SourceView> {
    state.rewatch();
    list_sources(state)
}

// MARK: - Commands: browsing

/// A folder entry plus what the index knows about it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedEntry {
    #[serde(flatten)]
    entry: Entry,
    #[serde(flatten)]
    meta: Option<FolderMeta>,
}

#[tauri::command]
async fn list_dir(
    source_id: String,
    rel_path: String,
    state: State<'_, AppState>,
) -> CmdResult<Vec<ListedEntry>> {
    let source = state.source(&source_id)?;
    let rel = rel_path.clone();
    let entries = tauri::async_runtime::spawn_blocking(move || {
        rupert_core::list_dir(&source.root, &rel, source.show_all_types)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    let mut indexed = state
        .index
        .lock()
        .unwrap()
        .folder_meta(&source_id, &rel_path)
        .unwrap_or_default();
    Ok(entries
        .into_iter()
        .map(|entry| ListedEntry {
            meta: indexed.remove(&entry.rel_path),
            entry,
        })
        .collect())
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

// MARK: - Commands: metadata

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetaResponse {
    /// `false` when the details live inside an online-only file and the
    /// caller didn't allow downloading it.
    available: bool,
    record: Option<MetaRecord>,
}

/// Reads a document's metadata. For online-only PDFs this would download
/// the file, so it only happens when `allow_download` is set.
#[tauri::command]
async fn read_meta(
    source_id: String,
    rel_path: String,
    allow_download: bool,
    state: State<'_, AppState>,
) -> CmdResult<MetaResponse> {
    let source = state.source(&source_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
        let needs_file = FileKind::from_path(&path) == FileKind::Pdf;
        if needs_file
            && !allow_download
            && rupert_core::cloud_state(&path).map_err(err)? == CloudState::CloudOnly
        {
            return Ok(MetaResponse {
                available: false,
                record: None,
            });
        }
        let record = rupert_core::read_meta(&path).map_err(err)?;
        Ok(MetaResponse {
            available: true,
            record: Some(record),
        })
    })
    .await
    .map_err(err)?
}

#[tauri::command]
async fn write_meta(
    source_id: String,
    rel_path: String,
    meta: DocMeta,
    state: State<'_, AppState>,
) -> CmdResult<MetaRecord> {
    let source = state.source(&source_id)?;
    let (src, rel) = (source.clone(), rel_path.clone());
    let record = tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&src.root, &rel).map_err(err)?;
        let local = rupert_core::cloud_state(&path).map_err(err)? == CloudState::Local;
        rupert_core::write_meta(&path, meta, local).map_err(err)
    })
    .await
    .map_err(err)??;
    state.indexer.send(Job::File { source, rel_path });
    Ok(record)
}

// MARK: - Commands: search

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    #[serde(default)]
    text: String,
    source_id: Option<String>,
    tag: Option<String>,
    #[serde(default)]
    favorites_only: bool,
}

#[tauri::command]
async fn search(query: SearchArgs, state: State<'_, AppState>) -> CmdResult<Vec<Hit>> {
    let q = SearchQuery {
        text: query.text,
        source_id: query.source_id,
        tag: query.tag,
        favorites_only: query.favorites_only,
        limit: 300,
    };
    state.index.lock().unwrap().search(&q).map_err(err)
}

#[tauri::command]
fn list_tags(state: State<AppState>) -> CmdResult<Vec<TagCount>> {
    state.index.lock().unwrap().tags().map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexStatus {
    stats: IndexStats,
    progress: Progress,
}

#[tauri::command]
fn index_status(state: State<AppState>) -> CmdResult<IndexStatus> {
    Ok(IndexStatus {
        stats: state.index.lock().unwrap().stats().map_err(err)?,
        progress: state.progress.lock().unwrap().clone(),
    })
}

// MARK: - Setup

/// Where PDFium lives: bundled resources in a built app, `src-tauri/pdfium`
/// when running from the source tree.
fn pdfium_dir(app: &tauri::App) -> Option<PathBuf> {
    let bundled = app.path().resource_dir().ok().map(|d| d.join("pdfium"));
    let dev = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdfium"));
    [bundled, dev].into_iter().flatten().find(|d| d.is_dir())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = SourceStore::load(config_dir.join("sources.json"))?;
            let index_path = data_dir.join("index.sqlite");

            // Indexer first: it creates the database the read connection uses.
            let progress_handle = app.handle().clone();
            let changed_handle = app.handle().clone();
            let indexer = rupert_core::indexer::spawn(
                index_path.clone(),
                pdfium_dir(app),
                move |p: Progress| {
                    if let Some(state) = progress_handle.try_state::<AppState>() {
                        *state.progress.lock().unwrap() = p.clone();
                    }
                    let _ = progress_handle.emit(INDEX_PROGRESS, p);
                },
                move || {
                    let _ = changed_handle.emit(INDEX_CHANGED, ());
                },
            )?;
            let index = Index::open(&index_path)?;

            // Watcher: tell the UI, and re-index the changed folders.
            let handle = app.handle().clone();
            let watcher = LibraryWatcher::new(move |changes: Vec<DirChange>| {
                if let Some(state) = handle.try_state::<AppState>() {
                    for change in &changes {
                        if let Ok(source) = state.source(&change.source_id) {
                            state.indexer.send(Job::SyncDir {
                                source,
                                rel_dir: change.rel_dir.clone(),
                            });
                        }
                    }
                }
                let _ = handle.emit(DIR_CHANGED, changes);
            })
            .inspect_err(|e| eprintln!("rupert: file watching unavailable: {e}"))
            .ok();

            let state = AppState {
                store: Mutex::new(store),
                watcher: Mutex::new(watcher),
                index: Mutex::new(index),
                indexer,
                progress: Mutex::new(Progress::default()),
            };
            state.rewatch();
            state.indexer.send(Job::SyncAll(state.sources()));
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
            read_meta,
            write_meta,
            search,
            list_tags,
            index_status,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rupert");
}
