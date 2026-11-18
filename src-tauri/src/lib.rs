//! Tauri shell: wires `rupert-core` to the UI through commands.
//! Keep logic in `rupert-core`; commands here should stay thin.

use std::path::PathBuf;
use std::sync::Mutex;

mod assistant;

use rupert_core::escl::{self, Capabilities, ScanRequest, Scanner};
use rupert_core::folder_info::{self, FolderInfo};
use rupert_core::intake::{self, Filed, Filing, FolderChoice, IntakeHandle, IntakeItem};
use rupert_core::llm::LmStudio;
use rupert_core::scansnap::{self, ImportPlan, ImportReport, ImportSummary};
use rupert_core::settings::{Settings, SettingsStore};
use rupert_core::tag_edit::{self, Preview, TagEdit};
use rupert_core::text::TextExtractor;
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
/// Rupert ▸ Settings… was chosen (macOS menu). No payload.
#[cfg(target_os = "macos")]
const MENU_SETTINGS: &str = "menu-settings";
/// File ▸ Import from ScanSnap Home… was chosen (macOS menu). No payload.
#[cfg(target_os = "macos")]
const MENU_IMPORT: &str = "menu-import-scansnap";

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
    state.reconfigure_intake();
    state.indexer.send(Job::SyncSource(source.clone()));
    Ok(source.into())
}

#[tauri::command]
fn remove_source(id: String, state: State<AppState>) -> CmdResult<()> {
    state.store.lock().unwrap().remove(&id).map_err(err)?;
    state.rewatch();
    state.reconfigure_intake();
    state.indexer.send(Job::RemoveSource(id));
    Ok(())
}

#[tauri::command]
fn update_source(
    id: String,
    name: Option<String>,
    show_all_types: Option<bool>,
    download_online_only: Option<bool>,
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

// MARK: - Test hook (debug builds only)

/// Runs `RUPERT_TEST_JS` in the page once it has loaded, so a test
/// instance can be driven without clicking (macOS won't bring a window
/// forward while another app is in use).
#[cfg(debug_assertions)]
fn run_test_script(app: &AppHandle) {
    let Ok(js) = std::env::var("RUPERT_TEST_JS") else {
        return;
    };
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        if let Some(w) = app.get_webview_window("main") {
            let _ = w.eval(&js);
        }
    });
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

// MARK: - Commands: settings

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().get().clone()
}

#[tauri::command]
fn set_settings(settings: Settings, state: State<AppState>) -> CmdResult<Settings> {
    state
        .settings
        .lock()
        .unwrap()
        .set(settings.clone())
        .map_err(err)?;
    state.reconfigure_intake();
    Ok(settings)
}

/// The OCR engine on this machine, if any (e.g. "Apple Vision").
#[tauri::command]
fn ocr_engine() -> Option<&'static str> {
    rupert_core::ocr::Engine::detect().map(|e| e.name())
}

/// Chat models LM Studio offers at `url` (fails if it isn't running).
#[tauri::command]
async fn lm_models(url: String) -> CmdResult<Vec<String>> {
    tauri::async_runtime::spawn_blocking(move || LmStudio { url, model: None }.models())
        .await
        .map_err(err)?
        .map_err(err)
}

// MARK: - Commands: inbox

#[tauri::command]
fn inbox_items(state: State<AppState>) -> Vec<IntakeItem> {
    state.intake.items()
}

#[tauri::command]
fn inbox_redo(rel_path: String, state: State<AppState>) {
    state.intake.redo(rel_path);
}

/// Every folder in the sources that holds documents, for the folder picker.
#[tauri::command]
fn all_folders(state: State<AppState>) -> CmdResult<Vec<FolderChoice>> {
    let sources = state.sources();
    let folders = state.index.lock().unwrap().folders().map_err(err)?;
    Ok(folders
        .into_iter()
        .filter_map(|(id, dir)| {
            let source = sources.iter().find(|s| s.id == id)?;
            Some(intake::folder_choice(source, &dir))
        })
        .collect())
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
        .ok_or("set up the inbox first: scans are saved there")?;
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
        .ok_or("nothing to import; review the import first")?;
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

/// macOS: the standard menu bar plus File ▸ Import from ScanSnap Home….
/// (Windows and Linux keep their window chrome menu-free; the sidebar has
/// the same action.)
#[cfg(target_os = "macos")]
fn install_menu(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    let menu = Menu::default(app.handle())?;
    let import = MenuItem::with_id(
        app,
        MENU_IMPORT,
        "Import from ScanSnap Home…",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(app, MENU_SETTINGS, "Settings…", true, Some("CmdOrCtrl+,"))?;
    let scan = MenuItem::with_id(app, MENU_SCAN, "Scan…", true, None::<&str>)?;
    for (i, item) in menu.items()?.into_iter().enumerate() {
        let Some(sub) = item.as_submenu() else {
            continue;
        };
        if i == 0 {
            // The app menu: Settings… after "About Rupert".
            sub.insert(&PredefinedMenuItem::separator(app)?, 1)?;
            sub.insert(&settings, 2)?;
        } else if sub.text()? == "File" {
            sub.prepend(&PredefinedMenuItem::separator(app)?)?;
            sub.prepend(&import)?;
            sub.prepend(&scan)?;
        }
    }
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| {
        for id in [MENU_IMPORT, MENU_SETTINGS, MENU_SCAN] {
            if event.id() == id {
                let _ = app.emit(id, ());
            }
        }
    });
    Ok(())
}

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
            #[cfg(target_os = "macos")]
            install_menu(app)?;
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = SourceStore::load(config_dir.join("sources.json"))?;
            let index_path = data_dir.join("index.sqlite");

            // Indexer first: it creates the database the read connection uses.
            let progress_handle = app.handle().clone();
            let changed_handle = app.handle().clone();
            // PDFium lives on one service thread; everyone shares this handle.
            let extractor = TextExtractor::new(pdfium_dir(app).as_deref());
            let settings = SettingsStore::load(config_dir.join("settings.json"))?;
            let indexer = rupert_core::indexer::spawn(
                index_path.clone(),
                extractor.clone(),
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
            tag_suggestions,
            tag_edit_preview,
            assistant::assistant_info,
            assistant::assistant_send,
            assistant::assistant_reset,
            assistant::assistant_set_key,
            assistant::assistant_has_key,
            assistant::assistant_cli_status,
            assistant::assistant_cli_login,
            favorite_folders,
            folder_favorite,
            set_folder_favorite,
            tag_edit_apply,
            index_status,
            scansnap_library,
            scansnap_plan,
            scansnap_apply,
            get_settings,
            set_settings,
            ocr_engine,
            lm_models,
            inbox_items,
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
