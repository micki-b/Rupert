//! Tauri shell: wires `rupert-core` to the UI through commands.
//! Keep logic in `rupert-core`; commands here should stay thin.

use rupert_core::i18n::{self, tr, trf};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod assistant;
mod drag_out;
mod help;
mod settings_window;
mod thumbs;

use rupert_core::escl::{self, Capabilities, ScanRequest, Scanner};
use rupert_core::folder_info::{self, FolderInfo};
use rupert_core::intake::{self, Filed, Filing, FolderChoice, IntakeHandle, IntakeItem};
use rupert_core::llm::{self, LocalAi, Reach};
use rupert_core::scansnap::{self, ImportPlan, ImportReport, ImportSummary};
use rupert_core::settings::{Settings, SettingsStore};
use rupert_core::tag_edit::{self, Preview, TagEdit};
use rupert_core::text::TextExtractor;
use rupert_core::thumbs::Thumbnails;
use rupert_core::{
    CloudState, DirChange, DocMeta, Entry, FileKind, FolderMeta, Hit, Index, IndexStats,
    IndexerHandle, Job, LibraryWatcher, MetaRecord, Progress, SearchQuery, Source, SourceStore,
    TagCount, TagList, TagMode,
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
/// ScanSnap Home import progress. Payload: `ImportProgress`.
const IMPORT_PROGRESS: &str = "import-progress";
/// ScanSnap Home import finished. Payload: `ImportReport`.
const IMPORT_DONE: &str = "import-done";
/// A page arrived from the scanner. Payload: number of pages so far.
const SCAN_PROGRESS: &str = "scan-progress";
/// A scan finished. Payload: `ScanDone`.
const SCAN_DONE: &str = "scan-done";
/// The tag manager changed a document. Payload: `TagEditProgress`.
const TAG_EDIT_PROGRESS: &str = "tag-edit-progress";
/// File ▸ Scan… was chosen (macOS menu). No payload.
#[cfg(target_os = "macos")]
const MENU_SCAN: &str = "menu-scan";
/// The inbox list or an item in it changed. No payload.
const INTAKE_CHANGED: &str = "intake-changed";
/// Settings were saved (any window). Payload: the settings.
const SETTINGS_CHANGED: &str = "settings-changed";
/// Sources were added, removed, renamed or changed (any window). No payload.
const SOURCES_CHANGED: &str = "sources-changed";
/// Rupert ▸ Settings… was chosen (macOS menu). No payload.
#[cfg(target_os = "macos")]
const MENU_SETTINGS: &str = "menu-settings";
/// File ▸ Import from ScanSnap Home… was chosen (macOS menu). No payload.
#[cfg(target_os = "macos")]
const MENU_IMPORT: &str = "menu-import-scansnap";
/// Help ▸ Rupert Help (macOS menu); handled here, not in the window.
#[cfg(target_os = "macos")]
const MENU_HELP: &str = "menu-help";
/// Rupert ▸ Check for Updates… was chosen (macOS menu). No payload.
#[cfg(target_os = "macos")]
const MENU_UPDATES: &str = "menu-check-updates";

// MARK: - State

struct AppState {
    store: Mutex<SourceStore>,
    /// For extra read connections (e.g. the assistant's).
    index_path: PathBuf,
    /// `None` if the OS refused to start a watcher; browsing still works.
    watcher: Mutex<Option<LibraryWatcher>>,
    /// Read connection for searches (the indexer thread has its own).
    index: Mutex<Index>,
    indexer: IndexerHandle,
    progress: Mutex<Progress>,
    /// The ScanSnap Home import last reviewed, waiting to be applied.
    import_plan: Mutex<Option<ImportPlan>>,
    settings: Mutex<SettingsStore>,
    intake: IntakeHandle,
    extractor: TextExtractor,
    /// Pictures for the large-icon and gallery views (`thumbs.rs`).
    thumbs: Arc<Thumbnails>,
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

    /// Hands the intake the current settings and sources.
    fn reconfigure_intake(&self) {
        let settings = self.settings.lock().unwrap().get().clone();
        self.intake.configure(settings, self.sources());
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
    /// "direct" (installers from GitHub, self-updating) or "app-store".
    edition: &'static str,
}

// The App Store edition doesn't register the updater (the dependency
// stays, so both editions build from one manifest).
#[cfg(feature = "app-store")]
use tauri_plugin_updater as _;

const EDITION: &str = if cfg!(feature = "app-store") {
    "app-store"
} else {
    "direct"
};

#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Rupert",
        version: env!("CARGO_PKG_VERSION"),
        core_version: rupert_core::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        edition: EDITION,
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
    fn from(mut source: Source) -> Self {
        let available = source.is_available();
        source.bookmark = None; // opaque, and the window doesn't need it
        Self { source, available }
    }
}

#[tauri::command]
fn list_sources(state: State<AppState>) -> Vec<SourceView> {
    state.sources().into_iter().map(Into::into).collect()
}

#[tauri::command]
fn add_source(path: String, app: AppHandle, state: State<AppState>) -> CmdResult<SourceView> {
    let source = {
        let mut store = state.store.lock().unwrap();
        let added = store.add(&path, None).map_err(err)?;
        // The user just picked it: remember access (macOS bookmark).
        store.remember_pick(&added.id, &path).unwrap_or(added)
    };
    state.rewatch();
    state.reconfigure_intake();
    state.indexer.send(Job::SyncSource(source.clone()));
    let _ = app.emit(SOURCES_CHANGED, ());
    Ok(source.into())
}

/// The user picked an unavailable source's folder again (it moved, or a
/// sandboxed app lost access): remember the pick and index it.
#[tauri::command]
fn relocate_source(
    id: String,
    path: String,
    app: AppHandle,
    state: State<AppState>,
) -> CmdResult<SourceView> {
    let path = PathBuf::from(path);
    if !path.is_dir() {
        return Err(err(trf("not a folder: {0}", &[&path.display()])));
    }
    let source = state
        .store
        .lock()
        .unwrap()
        .remember_pick(&id, &path)
        .map_err(err)?;
    state.rewatch();
    state.reconfigure_intake();
    state.indexer.send(Job::SyncSource(source.clone()));
    let _ = app.emit(SOURCES_CHANGED, ());
    Ok(source.into())
}

#[tauri::command]
fn remove_source(id: String, app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    state.store.lock().unwrap().remove(&id).map_err(err)?;
    state.rewatch();
    state.reconfigure_intake();
    state.indexer.send(Job::RemoveSource(id));
    let _ = app.emit(SOURCES_CHANGED, ());
    Ok(())
}

#[tauri::command]
fn update_source(
    id: String,
    name: Option<String>,
    show_all_types: Option<bool>,
    download_online_only: Option<bool>,
    app: AppHandle,
    state: State<AppState>,
) -> CmdResult<SourceView> {
    let updated = state
        .store
        .lock()
        .unwrap()
        .update(&id, name, show_all_types, download_online_only)
        .map_err(err)?;
    // Re-sync: the file filter changed, or downloads start/stop.
    if show_all_types.is_some() || download_online_only.is_some() {
        state.indexer.send(Job::SyncSource(updated.clone()));
    }
    let _ = app.emit(SOURCES_CHANGED, ());
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

/// Moves a document (with its sidecar) or a folder to the Trash / Recycle
/// Bin, then lets the index catch up.
#[tauri::command]
async fn trash_entry(source_id: String, rel_path: String, app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let (root, rel) = (source.root.clone(), rel_path.clone());
    let was_dir =
        tauri::async_runtime::spawn_blocking(move || rupert_core::fileops::trash(&root, &rel))
            .await
            .map_err(err)?
            .map_err(err)?;
    if was_dir {
        let rel_dir = rel_path
            .rsplit_once('/')
            .map(|(dir, _)| dir.to_string())
            .unwrap_or_default();
        state.indexer.send(Job::SyncDir { source, rel_dir });
    } else {
        state.indexer.send(Job::File { source, rel_path });
    }
    Ok(())
}

/// Moves a document (with its sidecar) or a folder into the folder
/// `dest_dir` of `dest_source_id` (possibly another source), then lets the
/// index catch up. Returns its new path there.
#[tauri::command]
async fn move_entry(
    source_id: String,
    rel_path: String,
    dest_source_id: String,
    dest_dir: String,
    app: AppHandle,
) -> CmdResult<String> {
    let state = app.state::<AppState>();
    let from = state.source(&source_id)?;
    let to = state.source(&dest_source_id)?;
    // The inbox is kept by its path; moving it would lose it.
    if let Some(inbox) = &state.settings.lock().unwrap().get().inbox
        && inbox.source_id == source_id
        && rupert_core::fileops::is_within(&inbox.rel_path, &rel_path)
    {
        return Err(
            "This folder holds Rupert's inbox. Choose another inbox folder in \
                    Settings ▸ Inbox first, then move it."
                .into(),
        );
    }
    let (f, t, rel, dest) = (from.clone(), to.clone(), rel_path.clone(), dest_dir.clone());
    let moved = tauri::async_runtime::spawn_blocking(move || {
        rupert_core::fileops::move_entry(&f, &rel, &t, &dest)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    if moved.rel_path != rel_path || source_id != dest_source_id {
        if moved.is_dir {
            let old_dir = rel_path
                .rsplit_once('/')
                .map(|(dir, _)| dir.to_string())
                .unwrap_or_default();
            state.indexer.send(Job::SyncDir {
                source: from,
                rel_dir: old_dir,
            });
            state.indexer.send(Job::SyncDir {
                source: to,
                rel_dir: dest_dir,
            });
        } else {
            state.indexer.send(Job::File {
                source: from,
                rel_path,
            });
            state.indexer.send(Job::File {
                source: to,
                rel_path: moved.rel_path.clone(),
            });
        }
    }
    Ok(moved.rel_path)
}

/// Makes a folder (File it's New folder…, #63); the index picks it up.
/// Returns its path.
#[tauri::command]
async fn create_folder(
    source_id: String,
    parent: String,
    name: String,
    app: AppHandle,
) -> CmdResult<String> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let (s, p) = (source.clone(), parent.clone());
    let rel = tauri::async_runtime::spawn_blocking(move || {
        rupert_core::fileops::create_folder(&s, &p, &name)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    state.indexer.send(Job::SyncDir {
        source,
        rel_dir: parent,
    });
    Ok(rel)
}

/// Renames a document or folder in place (#65). The index catches up, and
/// if the inbox is in a renamed folder, the setting follows it. Returns the
/// new path.
#[tauri::command]
async fn rename_entry(
    source_id: String,
    rel_path: String,
    new_name: String,
    app: AppHandle,
) -> CmdResult<String> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let (s, rel, name) = (source.clone(), rel_path.clone(), new_name.clone());
    let moved = tauri::async_runtime::spawn_blocking(move || {
        rupert_core::fileops::rename_entry(&s, &rel, &name)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    if moved.rel_path == rel_path {
        return Ok(moved.rel_path);
    }
    let parent = rel_path
        .rsplit_once('/')
        .map(|(dir, _)| dir.to_string())
        .unwrap_or_default();
    if moved.is_dir {
        state.indexer.send(Job::SyncDir {
            source,
            rel_dir: parent,
        });
        // The inbox is kept by its path.
        let changed = {
            let mut store = state.settings.lock().unwrap();
            let mut settings = store.get().clone();
            match &mut settings.inbox {
                Some(inbox)
                    if inbox.source_id == source_id
                        && rupert_core::fileops::is_within(&inbox.rel_path, &rel_path) =>
                {
                    inbox.rel_path =
                        format!("{}{}", moved.rel_path, &inbox.rel_path[rel_path.len()..]);
                    store.set(settings.clone()).map_err(err)?;
                    Some(settings)
                }
                _ => None,
            }
        };
        if let Some(settings) = changed {
            state.reconfigure_intake();
            let _ = app.emit(SETTINGS_CHANGED, &settings);
        }
    } else {
        state.indexer.send(Job::File {
            source: source.clone(),
            rel_path,
        });
        state.indexer.send(Job::File {
            source,
            rel_path: moved.rel_path.clone(),
        });
    }
    Ok(moved.rel_path)
}

/// Where a file dropped onto the window goes (the `rupert-copy` header,
/// JSON with non-ASCII characters escaped).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopyInto {
    source_id: String,
    dest_dir: String,
    name: String,
    modified_ms: Option<u64>,
}

/// Copies a file dropped from another app (its contents are the request's
/// body) into the folder `dest_dir`, keeping its name. In the inbox, it's
/// then processed like a new scan. Returns its path in the source.
#[tauri::command]
async fn copy_in(request: tauri::ipc::Request<'_>, app: AppHandle) -> CmdResult<String> {
    let tauri::ipc::InvokeBody::Raw(contents) = request.body() else {
        return Err("expected the file's contents".into());
    };
    let into: CopyInto = request
        .headers()
        .get("rupert-copy")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| serde_json::from_str(h).ok())
        .ok_or("expected where to copy the file")?;
    let state = app.state::<AppState>();
    let source = state.source(&into.source_id)?;
    let contents = contents.clone();
    let (root, dir, all_types) = (
        source.root.clone(),
        into.dest_dir.clone(),
        source.show_all_types,
    );
    let modified = into
        .modified_ms
        .map(|ms| std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms));
    let path = tauri::async_runtime::spawn_blocking(move || {
        let dest = rupert_core::resolve(&root, &dir)?;
        rupert_core::fileops::copy_in(&dest, &into.name, &contents, modified, !all_types)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    let rel_path =
        rupert_core::listing::relative(&source.root, &path).ok_or_else(|| err(path.display()))?;
    let to_inbox = state
        .settings
        .lock()
        .unwrap()
        .get()
        .inbox
        .as_ref()
        .is_some_and(|i| {
            i.source_id == source.id
                && i.rel_path.trim_matches('/') == into.dest_dir.trim_matches('/')
        });
    state.indexer.send(Job::File {
        source,
        rel_path: rel_path.clone(),
    });
    if to_inbox {
        state.intake.refresh();
    }
    Ok(rel_path)
}

/// A Markdown document's text for the reader (or its first `limit` bytes,
/// for a tile), or `None` for an online-only file.
#[tauri::command]
async fn read_markdown(
    source_id: String,
    rel_path: String,
    limit: Option<usize>,
    app: AppHandle,
) -> CmdResult<Option<String>> {
    let source = app.state::<AppState>().source(&source_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
        if !rupert_core::markdown::is_markdown(&path) {
            return Err(tr("Not a Markdown file.").into());
        }
        rupert_core::markdown::read(&path, limit.unwrap_or(rupert_core::markdown::MAX_BYTES))
            .map_err(err)
    })
    .await
    .map_err(err)?
}

/// How many pages a document has (for the gallery), or `None` for an
/// online-only file.
#[tauri::command]
async fn page_count(
    source_id: String,
    rel_path: String,
    app: AppHandle,
) -> CmdResult<Option<usize>> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let (thumbs, extractor) = (state.thumbs.clone(), state.extractor.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
        thumbs.page_count(&extractor, &path).map_err(err)
    })
    .await
    .map_err(err)?
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
    /// caller didn't allow downloading it. `record` then holds only what the
    /// sidecar knows (e.g. imported details waiting to move into the PDF).
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
            let beside = rupert_core::meta::read_meta_beside(&path).map_err(err)?;
            return Ok(MetaResponse {
                available: false,
                record: beside.map(|(meta, _)| MetaRecord {
                    meta,
                    storage: rupert_core::Storage::Sidecar,
                    note: None,
                }),
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

/// Stars or unstars a document from its star in a list (see
/// `rupert_core::meta::set_favorite`: an online-only PDF is downloaded only
/// to unstar it).
#[tauri::command]
async fn set_favorite(
    source_id: String,
    rel_path: String,
    favorite: bool,
    app: AppHandle,
) -> CmdResult<DocMeta> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let indexed = state
        .index
        .lock()
        .unwrap()
        .doc_meta(&source_id, &rel_path)
        .map_err(err)?;
    let (root, rel) = (source.root.clone(), rel_path.clone());
    let meta = tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&root, &rel).map_err(err)?;
        let local = rupert_core::cloud_state(&path).map_err(err)? == CloudState::Local;
        rupert_core::meta::set_favorite(&path, favorite, local, indexed).map_err(err)
    })
    .await
    .map_err(err)??;
    state
        .index
        .lock()
        .unwrap()
        .set_doc_favorite(&source_id, &rel_path, meta.favorite)
        .map_err(err)?;
    let _ = app.emit(INDEX_CHANGED, ());
    state.indexer.send(Job::File { source, rel_path });
    Ok(meta)
}

/// Downloads one online-only document (its cloud badge was clicked), then
/// has the indexer read it.
#[tauri::command]
async fn download_file(
    source_id: String,
    rel_path: String,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let source = state.source(&source_id)?;
    let (root, rel) = (source.root.clone(), rel_path.clone());
    tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&root, &rel).map_err(err)?;
        rupert_core::cloud::download(&path).map_err(err)
    })
    .await
    .map_err(err)??;
    state.indexer.send(Job::File { source, rel_path });
    Ok(())
}

// MARK: - Test hook (debug builds only)

/// Runs `RUPERT_TEST_JS` in the page once it has loaded, so a test
/// instance can be driven without clicking (macOS won't bring a window
/// forward while another app is in use).
///
/// `RUPERT_TEST_WINDOW_SIZE=<width>x<height>` sets the window's outer size
/// in points first (title bar included), for App Store screenshots and
/// previews at exact pixel sizes.
#[cfg(debug_assertions)]
fn run_test_script(app: &AppHandle) {
    if let Ok(size) = std::env::var("RUPERT_TEST_WINDOW_SIZE")
        && let Some((w, h)) = size.split_once('x')
        && let (Ok(w), Ok(h)) = (w.parse::<f64>(), h.parse::<f64>())
        && let Some(win) = app.get_webview_window("main")
    {
        let scale = win.scale_factor().unwrap_or(1.0);
        let (outer, inner) = (win.outer_size(), win.inner_size());
        let bar = match (outer, inner) {
            (Ok(o), Ok(i)) => f64::from(o.height.saturating_sub(i.height)) / scale,
            _ => 0.0,
        };
        let _ = win.set_size(tauri::LogicalSize::new(w, h - bar));
    }
    let Ok(js) = std::env::var("RUPERT_TEST_JS") else {
        return;
    };
    // Once the page has loaded and shows the window's layout (a large
    // library can take a while), and only once: asked every second.
    let guarded = format!(
        "if (!window.__rupertTest && document.readyState === 'complete' && \
         document.querySelector('.shell')) {{ window.__rupertTest = true; {js} }}"
    );
    let app = app.clone();
    std::thread::spawn(move || {
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.eval(&guarded);
            }
        }
    });
}

/// Test scripts (`RUPERT_TEST_JS`) report what they found through this, to
/// the dev log. Release builds ignore it.
#[tauri::command]
fn test_log(message: String) {
    if cfg!(debug_assertions) {
        eprintln!("rupert-test: {message}");
    }
}

// MARK: - Commands: favorite folders

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteFolder {
    source_id: String,
    rel_path: String,
    /// The folder's name (the source's name for a source root).
    name: String,
    source_name: String,
}

/// Favorite folders whose source is still in the library.
fn favorite_folder_list(state: &AppState) -> CmdResult<Vec<FavoriteFolder>> {
    let sources = state.sources();
    let favs = state
        .index
        .lock()
        .unwrap()
        .favorite_folders()
        .map_err(err)?;
    let mut out: Vec<FavoriteFolder> = favs
        .into_iter()
        .filter_map(|(source_id, rel_path)| {
            let source = sources.iter().find(|s| s.id == source_id)?;
            let name = rel_path
                .rsplit('/')
                .next()
                .filter(|n| !n.is_empty())
                .unwrap_or(&source.name)
                .to_string();
            Some(FavoriteFolder {
                source_name: source.name.clone(),
                source_id,
                rel_path,
                name,
            })
        })
        .collect();
    out.sort_by_key(|f| f.name.to_lowercase());
    Ok(out)
}

#[tauri::command]
fn favorite_folders(state: State<AppState>) -> CmdResult<Vec<FavoriteFolder>> {
    favorite_folder_list(&state)
}

#[tauri::command]
fn folder_favorite(source_id: String, rel_path: String, state: State<AppState>) -> CmdResult<bool> {
    let source = state.source(&source_id)?;
    let dir = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
    Ok(folder_info::read(&dir).is_some_and(|i| i.favorite))
}

/// Marks a folder as a favorite (or not), in the folder's own `.rupert`.
#[tauri::command]
fn set_folder_favorite(
    source_id: String,
    rel_path: String,
    favorite: bool,
    app: AppHandle,
    state: State<AppState>,
) -> CmdResult<()> {
    let source = state.source(&source_id)?;
    let dir = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
    let info = FolderInfo { favorite };
    folder_info::write(&dir, &info).map_err(err)?;
    state
        .index
        .lock()
        .unwrap()
        .set_folder(&source_id, &rel_path, Some(&info))
        .map_err(err)?;
    let _ = app.emit(INDEX_CHANGED, ());
    Ok(())
}

// MARK: - Commands: tag manager

/// What a rename, merge or delete would change.
#[tauri::command]
fn tag_edit_preview(edit: TagEdit, state: State<AppState>) -> CmdResult<Preview> {
    let sources = state.sources();
    let index = state.index.lock().unwrap();
    tag_edit::plan(&index, &sources, &edit)
        .map(|(_, preview)| preview)
        .map_err(err)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TagEditProgress {
    done: usize,
    total: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TagEditFailure {
    path: String,
    error: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TagEditReport {
    changed: usize,
    failed: Vec<TagEditFailure>,
}

/// Renames, merges or deletes tags in every document that has them.
#[tauri::command]
async fn tag_edit_apply(edit: TagEdit, app: AppHandle) -> CmdResult<TagEditReport> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sources = state.sources();
        let (targets, _) = {
            let index = state.index.lock().unwrap();
            tag_edit::plan(&index, &sources, &edit).map_err(err)?
        };
        let total = targets.len();
        let mut report = TagEditReport {
            changed: 0,
            failed: Vec::new(),
        };
        for (i, target) in targets.iter().enumerate() {
            let result = {
                let index = state.index.lock().unwrap();
                tag_edit::apply_one(&index, target, &edit)
            };
            match result {
                Ok(true) => report.changed += 1,
                Ok(false) => {}
                Err(e) => report.failed.push(TagEditFailure {
                    path: format!("{}/{}", target.source.name, target.rel_path),
                    error: e.to_string(),
                }),
            }
            state.indexer.send(Job::File {
                source: target.source.clone(),
                rel_path: target.rel_path.clone(),
            });
            let _ = app.emit(TAG_EDIT_PROGRESS, TagEditProgress { done: i + 1, total });
        }
        Ok(report)
    })
    .await
    .map_err(err)?
}

// MARK: - Commands: search

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    #[serde(default)]
    text: String,
    source_id: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    tag_mode: TagMode,
    #[serde(default)]
    favorites_only: bool,
}

#[tauri::command]
async fn search(query: SearchArgs, state: State<'_, AppState>) -> CmdResult<Vec<Hit>> {
    let q = SearchQuery {
        text: query.text,
        source_id: query.source_id,
        tags: query.tags,
        tag_mode: query.tag_mode,
        favorites_only: query.favorites_only,
        limit: 300,
    };
    state.index.lock().unwrap().search(&q).map_err(err)
}

/// Tags for the tag filter (see `Index::tag_suggestions`).
#[tauri::command]
fn tag_suggestions(
    filter: String,
    selected: Vec<String>,
    mode: TagMode,
    limit: usize,
    state: State<AppState>,
) -> CmdResult<TagList> {
    state
        .index
        .lock()
        .unwrap()
        .tag_suggestions(&filter, &selected, mode, limit.clamp(1, 500))
        .map_err(err)
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

/// Stops indexing and downloading after the file in hand (#78).
#[tauri::command]
fn stop_indexing(app: AppHandle) {
    let state = app.state::<AppState>();
    state.indexer.stop();
    let progress = {
        let mut p = state.progress.lock().unwrap();
        p.stopped = true;
        p.clone()
    };
    let _ = app.emit(INDEX_PROGRESS, progress);
}

/// "Index this folder": re-reads every document in it and its subfolders
/// and downloads its online-only ones to read them (#78). Asking for this
/// also ends a Stop.
#[tauri::command]
fn index_folder(source_id: String, rel_path: String, app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let was_stopped = state.progress.lock().unwrap().stopped;
    if was_stopped {
        state.indexer.resume();
        let progress = {
            let mut p = state.progress.lock().unwrap();
            p.stopped = false;
            p.clone()
        };
        let _ = app.emit(INDEX_PROGRESS, progress);
    }
    state.indexer.send(Job::IndexFolder {
        source,
        rel_dir: rel_path,
    });
    Ok(())
}

/// Lets indexing and downloading go on, checking every folder again.
#[tauri::command]
fn resume_indexing(app: AppHandle) {
    let state = app.state::<AppState>();
    state.indexer.resume();
    let progress = {
        let mut p = state.progress.lock().unwrap();
        p.stopped = false;
        p.clone()
    };
    let _ = app.emit(INDEX_PROGRESS, progress);
    state.indexer.send(Job::SyncAll(state.sources()));
}

// MARK: - Commands: settings

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().get().clone()
}

#[tauri::command]
fn set_settings(app: AppHandle, settings: Settings, state: State<AppState>) -> CmdResult<Settings> {
    state
        .settings
        .lock()
        .unwrap()
        .set(settings.clone())
        .map_err(err)?;
    state.reconfigure_intake();
    let lang = i18n::resolve(settings.language.as_deref());
    if lang != i18n::current() {
        i18n::set(lang);
        #[cfg(target_os = "macos")]
        install_menu(&app).map_err(err)?;
        settings_window::retitle(&app);
    }
    let _ = app.emit(SETTINGS_CHANGED, &settings);
    Ok(settings)
}

/// Rupert's language now, and the system's (what "Automatic" means).
#[derive(Serialize)]
struct Language {
    current: i18n::Lang,
    system: i18n::Lang,
}

#[tauri::command]
fn language() -> Language {
    Language {
        current: i18n::current(),
        system: i18n::system(),
    }
}

/// The OCR engine on this machine, if any (e.g. "Apple Vision").
#[tauri::command]
fn ocr_engine() -> Option<&'static str> {
    rupert_core::ocr::Engine::detect().map(|e| e.name())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalAiPlace {
    reach: Reach,
    host: String,
}

/// Whether Apple Intelligence can be used on this Mac, and why not.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppleAiInfo {
    #[serde(flatten)]
    status: rupert_core::apple_ai::Status,
    /// Why it can't be used, to show (None when available).
    message: Option<String>,
}

#[tauri::command]
fn apple_ai_status() -> AppleAiInfo {
    let status = rupert_core::apple_ai::status();
    let message = status.reason.map(|r| r.message().to_string());
    AppleAiInfo { status, message }
}

/// Where a Local AI address points (this computer, the local network or
/// elsewhere), so Settings can say where documents' text goes.
#[tauri::command]
fn local_ai_place(url: String) -> LocalAiPlace {
    LocalAiPlace {
        reach: llm::reach(&url),
        host: llm::host(&url).unwrap_or_default().to_string(),
    }
}

/// Chat models the Local AI server at `url` offers (fails if it isn't
/// running).
#[tauri::command]
async fn lm_models(url: String) -> CmdResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || LocalAi { url, model: None }.models())
        .await
        .map_err(err)?
        .map_err(err)
}

// MARK: - Commands: inbox

#[tauri::command]
fn inbox_items(state: State<AppState>) -> Vec<IntakeItem> {
    state.intake.items()
}

/// The inbox items whose name or text match what was typed in the inbox's
/// search field (#59), as relative paths.
#[tauri::command]
fn inbox_search(text: String, state: State<AppState>) -> CmdResult<Vec<String>> {
    let Some(inbox) = state.settings.lock().unwrap().get().inbox.clone() else {
        return Ok(Vec::new());
    };
    state
        .index
        .lock()
        .unwrap()
        .matching_in_folder(&inbox.source_id, &inbox.rel_path, &text)
        .map_err(err)
}

#[tauri::command]
fn inbox_redo(rel_path: String, state: State<AppState>) {
    state.intake.redo(rel_path);
}

/// Every folder in the sources, empty ones too, for the folder picker.
#[tauri::command]
async fn all_folders(app: AppHandle) -> CmdResult<Vec<FolderChoice>> {
    let state = app.state::<AppState>();
    let sources = state.sources();
    let indexed = state.index.lock().unwrap().folders().map_err(err)?;
    // Empty folders too (e.g. one just made for filing): from the disk.
    tauri::async_runtime::spawn_blocking(move || {
        let mut all: std::collections::BTreeSet<(String, String)> = indexed.into_iter().collect();
        for s in sources.iter().filter(|s| s.is_available()) {
            all.insert((s.id.clone(), String::new()));
            for dir in rupert_core::all_dirs(&s.root) {
                all.insert((s.id.clone(), dir));
            }
        }
        all.into_iter()
            .filter_map(|(id, dir)| {
                let source = sources.iter().find(|s| s.id == id)?;
                Some(intake::folder_choice(source, &dir))
            })
            .collect()
    })
    .await
    .map_err(err)
}

/// Writes the reviewed details, then renames and moves the document.
#[tauri::command]
async fn file_document(
    source_id: String,
    rel_path: String,
    filing: Filing,
    app: AppHandle,
) -> CmdResult<Filed> {
    let state = app.state::<AppState>();
    let sources = state.sources();
    let (sid, rel, f) = (source_id.clone(), rel_path.clone(), filing.clone());
    let filed = tauri::async_runtime::spawn_blocking(move || {
        intake::file_document(&sources, &sid, &rel, &f)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    // Re-index where it was and where it is now.
    let parent = |p: &str| {
        p.rsplit_once('/')
            .map(|(d, _)| d.to_string())
            .unwrap_or_default()
    };
    for (id, dir) in [
        (source_id, parent(&rel_path)),
        (filed.source_id.clone(), parent(&filed.rel_path)),
    ] {
        if let Ok(source) = state.source(&id) {
            state.indexer.send(Job::SyncDir {
                source,
                rel_dir: dir,
            });
        }
    }
    state.intake.refresh();
    Ok(filed)
}

/// A page image for the review: a `data:` URL, or `None` if there's
/// nothing to show without downloading the file.
#[tauri::command]
async fn preview(
    source_id: String,
    rel_path: String,
    page: Option<usize>,
    app: AppHandle,
) -> CmdResult<Option<String>> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let extractor = state.extractor.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
        let image = intake::preview(&extractor, &path, page.unwrap_or(0)).map_err(err)?;
        Ok(image.map(|(bytes, mime)| format!("data:{mime};base64,{}", base64(&bytes))))
    })
    .await
    .map_err(err)?
}

/// A sharp page for the page viewer's zoom (#83): a `data:` URL, or `None`
/// if there's nothing to show without downloading the file.
#[tauri::command]
async fn page_image(
    source_id: String,
    rel_path: String,
    page: usize,
    side: u32,
    app: AppHandle,
) -> CmdResult<Option<String>> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let extractor = state.extractor.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
        let image = rupert_core::thumbs::large_page(&extractor, &path, page, side).map_err(err)?;
        Ok(image.map(|(bytes, mime)| format!("data:{mime};base64,{}", base64(&bytes))))
    })
    .await
    .map_err(err)?
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

// MARK: - Commands: scanning

/// eSCL scanners on the network (takes a few seconds).
#[tauri::command]
async fn scanners_discover() -> CmdResult<Vec<Scanner>> {
    tauri::async_runtime::spawn_blocking(|| escl::discover(std::time::Duration::from_secs(3)))
        .await
        .map_err(err)?
        .map_err(err)
}

/// A scanner by address, checked by reading its capabilities.
#[tauri::command]
async fn scanner_capabilities(scanner: Scanner) -> CmdResult<Capabilities> {
    tauri::async_runtime::spawn_blocking(move || escl::capabilities(&scanner))
        .await
        .map_err(err)?
        .map_err(err)
}

#[tauri::command]
fn scanner_at(address: String) -> Scanner {
    escl::scanner_at(&address)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanDone {
    /// Where the PDF was saved (in the inbox's source), if it worked.
    source_id: Option<String>,
    rel_path: Option<String>,
    error: Option<String>,
}

/// Scans into the inbox in the background; progress and the result arrive
/// as events. `name` is the file name without extension.
#[tauri::command]
fn scan_start(
    scanner: Scanner,
    request: ScanRequest,
    name: String,
    app: AppHandle,
    state: State<AppState>,
) -> CmdResult<()> {
    let inbox = state
        .settings
        .lock()
        .unwrap()
        .get()
        .inbox
        .clone()
        .ok_or(tr("set up the inbox first: scans are saved there"))?;
    let source = state.source(&inbox.source_id)?;
    let dest = rupert_core::resolve(&source.root, &inbox.rel_path).map_err(err)?;
    let stem = rupert_core::fileops::clean_name(&name, "pdf");
    std::thread::Builder::new()
        .name("rupert-scan".into())
        .spawn(move || {
            let result = escl::capabilities(&scanner).and_then(|caps| {
                escl::scan(&scanner, &caps, &request, &dest, &stem, |pages| {
                    let _ = app.emit(SCAN_PROGRESS, pages);
                })
            });
            let done = match result {
                Ok(path) => ScanDone {
                    source_id: Some(source.id.clone()),
                    rel_path: rupert_core::listing::relative(&source.root, &path),
                    error: None,
                },
                Err(e) => ScanDone {
                    source_id: None,
                    rel_path: None,
                    error: Some(e.to_string()),
                },
            };
            if let Some(state) = app.try_state::<AppState>() {
                state.intake.refresh();
            }
            let _ = app.emit(SCAN_DONE, done);
        })
        .map_err(err)?;
    Ok(())
}

// MARK: - Commands: ScanSnap Home import

/// ScanSnap Home's library folder, if it's in the usual place.
#[tauri::command]
fn scansnap_library() -> Option<String> {
    scansnap::default_library().map(|p| p.display().to_string())
}

/// Reads the library and works out what an import would do. Writes nothing;
/// the plan is kept for `scansnap_apply`.
#[tauri::command]
async fn scansnap_plan(library: String, app: AppHandle) -> CmdResult<ImportSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let sources = state.sources();
        let plan = {
            let index = state.index.lock().unwrap();
            scansnap::plan(std::path::Path::new(&library), &sources, Some(&index))
        }
        .map_err(err)?;
        let summary = plan.summary.clone();
        *state.import_plan.lock().unwrap() = Some(plan);
        Ok(summary)
    })
    .await
    .map_err(err)?
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportProgress {
    done: usize,
    total: usize,
}

/// Applies the reviewed plan in the background. Progress and the final
/// report arrive as events; changed folders are then re-indexed.
#[tauri::command]
fn scansnap_apply(app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    let plan = state
        .import_plan
        .lock()
        .unwrap()
        .take()
        .ok_or(tr("nothing to import; review the import first"))?;
    let sources = state.sources();
    std::thread::Builder::new()
        .name("rupert-import".into())
        .spawn(move || {
            let mut last = std::time::Instant::now();
            let (report, touched): (ImportReport, _) =
                scansnap::apply(&plan, &sources, |done, total| {
                    if done == total || last.elapsed().as_millis() >= 100 {
                        last = std::time::Instant::now();
                        let _ = app.emit(IMPORT_PROGRESS, ImportProgress { done, total });
                    }
                });
            let state = app.state::<AppState>();
            for (source_id, rel_dir) in touched {
                if let Ok(source) = state.source(&source_id) {
                    state.indexer.send(Job::SyncDir { source, rel_dir });
                }
            }
            let _ = app.emit(IMPORT_DONE, report);
        })
        .map_err(err)?;
    Ok(())
}

// MARK: - Setup

/// macOS: the standard menu bar plus File ▸ Scan… and Import from ScanSnap
/// Home…, Rupert ▸ Settings… and Help ▸ Rupert Help, in Rupert's language
/// (Tauri's default menu is English only). Built again when the language
/// changes. (Windows and Linux keep their window chrome menu-free; the
/// window has the same actions, and F1 opens the help.)
#[cfg(target_os = "macos")]
fn install_menu(app: &tauri::AppHandle) -> tauri::Result<()> {
    use tauri::menu::{
        AboutMetadata, HELP_SUBMENU_ID, Menu, MenuItem, PredefinedMenuItem, Submenu,
        WINDOW_SUBMENU_ID,
    };
    let name = app.package_info().name.clone();
    let about = AboutMetadata {
        name: Some(name.clone()),
        version: Some(app.package_info().version.to_string()),
        copyright: app.config().bundle.copyright.clone(),
        ..Default::default()
    };
    let sep = || PredefinedMenuItem::separator(app);
    let item =
        |id: &str, text: &str, accel: Option<&str>| MenuItem::with_id(app, id, text, true, accel);

    let app_menu = Submenu::with_items(
        app,
        &name,
        true,
        &[&PredefinedMenuItem::about(
            app,
            Some(&trf("About {0}", &[&name])),
            Some(about),
        )?],
    )?;
    if !cfg!(feature = "app-store") {
        // Not in the App Store edition, which the App Store updates.
        app_menu.append(&sep()?)?;
        app_menu.append(&item(MENU_UPDATES, tr("Check for Updates…"), None)?)?;
    }
    let settings = item(MENU_SETTINGS, tr("Settings…"), Some("CmdOrCtrl+,"))?;
    app_menu.append_items(&[
        &sep()?,
        &settings,
        &sep()?,
        &PredefinedMenuItem::services(app, Some(tr("Services")))?,
        &sep()?,
        &PredefinedMenuItem::hide(app, Some(&trf("Hide {0}", &[&name])))?,
        &PredefinedMenuItem::hide_others(app, Some(tr("Hide Others")))?,
        &PredefinedMenuItem::show_all(app, Some(tr("Show All")))?,
        &sep()?,
        &PredefinedMenuItem::quit(app, Some(&trf("Quit {0}", &[&name])))?,
    ])?;

    let file = Submenu::with_items(
        app,
        tr("File"),
        true,
        &[
            &item(MENU_SCAN, tr("Scan…"), None)?,
            &item(MENU_IMPORT, tr("Import from ScanSnap Home…"), None)?,
            &sep()?,
            &PredefinedMenuItem::close_window(app, Some(tr("Close Window")))?,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        tr("Edit"),
        true,
        &[
            &PredefinedMenuItem::undo(app, Some(tr("Undo")))?,
            &PredefinedMenuItem::redo(app, Some(tr("Redo")))?,
            &sep()?,
            &PredefinedMenuItem::cut(app, Some(tr("Cut")))?,
            &PredefinedMenuItem::copy(app, Some(tr("Copy")))?,
            &PredefinedMenuItem::paste(app, Some(tr("Paste")))?,
            &PredefinedMenuItem::select_all(app, Some(tr("Select All")))?,
        ],
    )?;
    let view = Submenu::with_items(
        app,
        tr("View"),
        true,
        &[&PredefinedMenuItem::fullscreen(
            app,
            Some(tr("Enter Full Screen")),
        )?],
    )?;
    let window = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        tr("Window"),
        true,
        &[
            &PredefinedMenuItem::minimize(app, Some(tr("Minimize")))?,
            &PredefinedMenuItem::maximize(app, Some(tr("Zoom")))?,
            &sep()?,
            &PredefinedMenuItem::close_window(app, Some(tr("Close Window")))?,
        ],
    )?;
    let help = Submenu::with_id_and_items(
        app,
        HELP_SUBMENU_ID,
        tr("Help"),
        true,
        &[&item(
            MENU_HELP,
            tr("Rupert Help"),
            Some("CmdOrCtrl+Shift+/"),
        )?],
    )?;
    let menu = Menu::with_items(app, &[&app_menu, &file, &edit, &view, &window, &help])?;
    app.set_menu(menu)?;
    Ok(())
}

/// The menu's own items: open the help, or tell the window.
#[cfg(target_os = "macos")]
fn on_menu(app: &tauri::AppHandle, event: tauri::menu::MenuEvent) {
    if event.id() == MENU_HELP {
        let _ = help::show(app, None);
    }
    for id in [MENU_IMPORT, MENU_SETTINGS, MENU_SCAN, MENU_UPDATES] {
        if event.id() == id {
            let _ = app.emit(id, ());
        }
    }
}

/// Where PDFium lives: bundled resources in a built app, `src-tauri/pdfium`
/// when running from the source tree.
fn pdfium_dir(app: &tauri::App) -> Option<PathBuf> {
    let resources = app.path().resource_dir().ok();
    // macOS bundles it in Contents/Frameworks (signed with the app, as
    // notarization and the App Store require; tauri.macos.conf.json).
    let frameworks = resources
        .as_ref()
        .and_then(|r| r.parent())
        .map(|c| c.join("Frameworks"));
    let bundled = resources.map(|d| d.join("pdfium"));
    let dev = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdfium"));
    [frameworks, bundled, dev]
        .into_iter()
        .flatten()
        .find(|d| has_pdfium(d))
}

/// Whether `dir` holds the PDFium library for this platform.
fn has_pdfium(dir: &std::path::Path) -> bool {
    ["libpdfium.dylib", "libpdfium.so", "pdfium.dll"]
        .iter()
        .any(|name| dir.join(name).is_file())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init());
    // The App Store updates its own edition.
    #[cfg(not(feature = "app-store"))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    builder
        .register_asynchronous_uri_scheme_protocol(thumbs::SCHEME, |ctx, request, responder| {
            thumbs::serve(ctx.app_handle(), request, responder)
        })
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let settings = SettingsStore::load(config_dir.join("settings.json"))?;
            i18n::set(i18n::resolve(settings.get().language.as_deref()));
            #[cfg(target_os = "macos")]
            {
                install_menu(app.handle())?;
                app.on_menu_event(on_menu);
            }
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let mut store = SourceStore::load(config_dir.join("sources.json"))?;
            // Open the folders' bookmarks first: a sandboxed app (Mac App
            // Store edition) can't reach them otherwise.
            for (id, e) in store.open_bookmarks() {
                eprintln!("rupert: source {id} needs to be picked again: {e}");
            }
            let index_path = data_dir.join("index.sqlite");

            // Indexer first: it creates the database the read connection uses.
            let progress_handle = app.handle().clone();
            let changed_handle = app.handle().clone();
            // PDFium lives on one service thread; everyone shares this handle.
            let extractor = TextExtractor::new(pdfium_dir(app).as_deref());

            // Thumbnails: a disposable cache, trimmed now and then. Kept
            // pictures of cloud documents (#62) get a larger budget of their
            // own: they can't be made again without a download.
            let thumbs = Arc::new(Thumbnails::new(app.path().app_cache_dir()?.join("thumbs")));
            let pruning = thumbs.clone();
            std::thread::spawn(move || {
                pruning.prune(200 << 20, 150 << 20);
                pruning.prune_kept(1 << 30, 800 << 20);
            });

            let indexer = rupert_core::indexer::spawn_with(
                index_path.clone(),
                extractor.clone(),
                rupert_core::indexer::Options {
                    pictures: Some((*thumbs).clone()),
                    ..Default::default()
                },
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

            let intake_handle = app.handle().clone();
            let intake = intake::spawn(
                index_path.clone(),
                data_dir.join("intake.json"),
                extractor.clone(),
                move || {
                    let _ = intake_handle.emit(INTAKE_CHANGED, ());
                },
            )?;

            // Watcher: tell the UI, and re-index the changed folders.
            let handle = app.handle().clone();
            let watcher = LibraryWatcher::new(move |changes: Vec<DirChange>| {
                if let Some(state) = handle.try_state::<AppState>() {
                    // New scans in the inbox?
                    let inbox = state.settings.lock().unwrap().get().inbox.clone();
                    if let Some(inbox) = inbox
                        && changes
                            .iter()
                            .any(|c| c.source_id == inbox.source_id && c.rel_dir == inbox.rel_path)
                    {
                        state.intake.refresh();
                    }
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
                index_path: index_path.clone(),
                store: Mutex::new(store),
                watcher: Mutex::new(watcher),
                index: Mutex::new(index),
                indexer,
                progress: Mutex::new(Progress::default()),
                import_plan: Mutex::new(None),
                settings: Mutex::new(settings),
                intake,
                extractor,
                thumbs: thumbs.clone(),
            };
            state.rewatch();
            state.reconfigure_intake();
            state.indexer.send(Job::SyncAll(state.sources()));
            app.manage(state);
            app.manage(assistant::AssistantState::new());
            #[cfg(debug_assertions)]
            run_test_script(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            language,
            list_sources,
            add_source,
            relocate_source,
            remove_source,
            update_source,
            refresh_sources,
            list_dir,
            open_entry,
            reveal_entry,
            trash_entry,
            move_entry,
            copy_in,
            page_count,
            read_markdown,
            test_log,
            read_meta,
            write_meta,
            set_favorite,
            download_file,
            search,
            list_tags,
            tag_suggestions,
            tag_edit_preview,
            assistant::assistant_info,
            assistant::assistant_send,
            assistant::assistant_reset,
            help::open_help,
            settings_window::open_settings,
            assistant::assistant_cancel,
            assistant::assistant_set_key,
            assistant::assistant_has_key,
            assistant::assistant_cli_status,
            assistant::assistant_cli_login,
            assistant::chatgpt_status,
            assistant::chatgpt_sign_in,
            assistant::chatgpt_sign_out,
            favorite_folders,
            folder_favorite,
            set_folder_favorite,
            tag_edit_apply,
            index_status,
            stop_indexing,
            resume_indexing,
            index_folder,
            rename_entry,
            create_folder,
            page_image,
            drag_out::drag_out,
            scansnap_library,
            scansnap_plan,
            scansnap_apply,
            get_settings,
            set_settings,
            ocr_engine,
            lm_models,
            local_ai_place,
            apple_ai_status,
            inbox_items,
            inbox_search,
            inbox_redo,
            all_folders,
            file_document,
            preview,
            scanners_discover,
            scanner_capabilities,
            scanner_at,
            scan_start,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rupert");
}
