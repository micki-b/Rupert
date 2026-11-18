//! Rupert for iPhone: browse and search the same folders as the computer,
//! open documents, and scan paper into the inbox folder.
//!
//! The phone only reads the library. Folders come from the Files picker
//! (iCloud Drive, On My iPhone) and are reopened at launch from bookmarks;
//! OneDrive folders, which the Files app can't hand to other apps, are read
//! through Microsoft Graph (`onedrive`). Text and details come from the shared catalog and sidecars
//! that Rupert on a computer writes; documents are downloaded only when
//! opened. Scans go to the inbox folder, where Rupert on the computer runs
//! OCR and suggests how to file them.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

mod onedrive;

use onedrive::{OneDrive, RemoteFolder, RemoteInbox};

use rupert_core::camera;
use rupert_core::indexer::{self, Mode, Options, PrepareHidden};
use rupert_core::text::TextExtractor;
use rupert_core::{
    DocInfo, Entry, FolderMeta, Hit, Index, IndexStats, IndexerHandle, Job, Progress, Provider,
    SearchQuery, Source, SourceStore, TagCount, TagList, TagMode,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_rupert_ios::{Device, Error as DeviceError};

/// Indexing progress. Payload: `Progress`.
const INDEX_PROGRESS: &str = "index-progress";
/// The index changed; refresh lists. No payload.
pub(crate) const INDEX_CHANGED: &str = "index-changed";
/// Saved folders were reopened (or couldn't be). No payload.
const FOLDERS_READY: &str = "folders-ready";
/// Waiting longer than this for the sync service to fetch a hidden folder
/// just means indexing goes on with what is already on the phone.
const FETCH_TIMEOUT: Duration = Duration::from_secs(90);

// MARK: - Saved folders

/// Bookmarks for the picked folders, and the inbox folder for scans.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Saved {
    /// Source id → bookmark.
    bookmarks: HashMap<String, String>,
    inbox: Option<SavedFolder>,
    /// Library folders read through OneDrive's web API.
    #[serde(default)]
    remote: Vec<RemoteFolder>,
    /// Scans go here instead of `inbox` when set.
    #[serde(default)]
    onedrive_inbox: Option<RemoteInbox>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedFolder {
    name: String,
    path: PathBuf,
    bookmark: String,
}

pub(crate) struct SavedStore {
    path: PathBuf,
    saved: Saved,
}

impl SavedStore {
    fn load(path: PathBuf) -> Self {
        let saved = fs::read(&path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        Self { path, saved }
    }

    fn save(&self) {
        let tmp = self.path.with_extension("json.tmp");
        let written = serde_json::to_vec_pretty(&self.saved)
            .map_err(|e| e.to_string())
            .and_then(|json| fs::write(&tmp, json).map_err(|e| e.to_string()))
            .and_then(|_| fs::rename(&tmp, &self.path).map_err(|e| e.to_string()));
        if let Err(e) = written {
            eprintln!("rupert: couldn't save folders: {e}");
        }
    }
}

// MARK: - State

pub(crate) struct AppState {
    store: Mutex<SourceStore>,
    saved: Mutex<SavedStore>,
    index_path: PathBuf,
    /// Read connection for lists and searches (the indexer has its own).
    index: Mutex<Index>,
    indexer: IndexerHandle,
    progress: Mutex<Progress>,
    /// Saved folders have been reopened.
    ready: AtomicBool,
}

impl AppState {
    fn source(&self, id: &str) -> Result<Source, String> {
        self.store.lock().unwrap().get(id).cloned().map_err(err)
    }

    fn sources(&self) -> Vec<Source> {
        self.store.lock().unwrap().sources().to_vec()
    }

    /// Local folders only; OneDrive folders are synced by `onedrive`. (Not
    /// `Job::SyncAll`, which would drop the OneDrive folders' entries.)
    fn sync_local(&self) {
        for source in self.sources() {
            self.indexer.send(Job::SyncSource(source));
        }
    }

    fn remote(&self, id: &str) -> Option<RemoteFolder> {
        let saved = self.saved.lock().unwrap();
        saved.saved.remote.iter().find(|f| f.id == id).cloned()
    }

    /// The source (and relative folder) a path lies in, if any.
    fn locate(&self, path: &Path) -> Option<(Source, String)> {
        self.sources().into_iter().find_map(|s| {
            let rel = path.strip_prefix(&s.root).ok()?;
            Some((s.clone(), rel.to_string_lossy().replace('\\', "/")))
        })
    }
}

pub(crate) type CmdResult<T> = Result<T, String>;

pub(crate) fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

pub(crate) fn device(app: &AppHandle) -> tauri::State<'_, Device<Wry>> {
    app.state::<Device<Wry>>()
}

/// Runs blocking work (iOS calls, file reads) off the main thread.
pub(crate) async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

// MARK: - Commands: overview

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceView {
    id: String,
    name: String,
    provider: Provider,
    available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    ready: bool,
    sources: Vec<SourceView>,
    /// Name of the folder scans are saved to.
    inbox: Option<String>,
    inbox_on_onedrive: bool,
    stats: IndexStats,
    progress: Progress,
}

#[tauri::command]
fn overview(state: State<AppState>, od: State<OneDrive>) -> CmdResult<Overview> {
    let saved = state.saved.lock().unwrap();
    let mut sources: Vec<SourceView> = state
        .sources()
        .into_iter()
        .map(|s| SourceView {
            available: s.is_available(),
            id: s.id,
            name: s.name,
            provider: s.provider,
        })
        .collect();
    sources.extend(saved.saved.remote.iter().map(|f| SourceView {
        id: f.id.clone(),
        name: f.name.clone(),
        provider: Provider::OneDrive,
        available: od.is_signed_in(),
    }));
    let (inbox, inbox_on_onedrive) = match (&saved.saved.onedrive_inbox, &saved.saved.inbox) {
        (Some(r), _) => (Some(r.name.clone()), true),
        (None, Some(f)) => (Some(f.name.clone()), false),
        (None, None) => (None, false),
    };
    Ok(Overview {
        ready: state.ready.load(Ordering::SeqCst),
        sources,
        inbox,
        inbox_on_onedrive,
        stats: state.index.lock().unwrap().stats().map_err(err)?,
        progress: state.progress.lock().unwrap().clone(),
    })
}

#[tauri::command]
fn refresh(app: AppHandle, state: State<AppState>) {
    state.sync_local();
    onedrive::start_sync(&app);
}

// MARK: - Commands: folders

/// Picks a folder in Files and adds it to the library. `false` if cancelled.
#[tauri::command]
async fn add_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    let picked = blocking(move || device(&app).pick_folder().map_err(err)).await?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    let source = state
        .store
        .lock()
        .unwrap()
        .add(&picked.path, Some(picked.name))
        .map_err(err)?;
    {
        let mut saved = state.saved.lock().unwrap();
        saved
            .saved
            .bookmarks
            .insert(source.id.clone(), picked.bookmark);
        saved.save();
    }
    state.indexer.send(Job::SyncSource(source));
    Ok(true)
}

/// Removes a folder from the library (nothing on disk changes).
#[tauri::command]
fn remove_folder(id: String, state: State<AppState>) -> CmdResult<()> {
    if state.remote(&id).is_none() {
        state.store.lock().unwrap().remove(&id).map_err(err)?;
    }
    {
        let mut saved = state.saved.lock().unwrap();
        saved.saved.bookmarks.remove(&id);
        saved.saved.remote.retain(|f| f.id != id);
        saved.save();
    }
    state.indexer.send(Job::RemoveSource(id));
    Ok(())
}

/// Picks the folder scans are saved to. `false` if cancelled.
#[tauri::command]
async fn choose_inbox(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    let picked = blocking(move || device(&app).pick_folder().map_err(err)).await?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    let mut saved = state.saved.lock().unwrap();
    saved.saved.inbox = Some(SavedFolder {
        name: picked.name,
        path: picked.path.into(),
        bookmark: picked.bookmark,
    });
    saved.saved.onedrive_inbox = None;
    saved.save();
    Ok(true)
}

// MARK: - Commands: browsing and search

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedEntry {
    #[serde(flatten)]
    entry: Entry,
    meta: Option<FolderMeta>,
}

#[tauri::command]
async fn list_dir(
    source_id: String,
    rel_path: String,
    state: State<'_, AppState>,
) -> CmdResult<Vec<ListedEntry>> {
    if state.remote(&source_id).is_some() {
        return list_remote(&source_id, &rel_path, &state);
    }
    let source = state.source(&source_id)?;
    let rel = rel_path.clone();
    let entries = blocking(move || {
        rupert_core::list_dir(&source.root, &rel, source.show_all_types).map_err(err)
    })
    .await?;
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

/// A OneDrive folder's contents, from the index (nothing is on the phone).
fn list_remote(source_id: &str, rel_path: &str, state: &AppState) -> CmdResult<Vec<ListedEntry>> {
    let (dirs, docs) = state
        .index
        .lock()
        .unwrap()
        .list_folder(source_id, rel_path)
        .map_err(err)?;
    let join = |name: &str| {
        if rel_path.is_empty() {
            name.to_string()
        } else {
            format!("{rel_path}/{name}")
        }
    };
    let folders = dirs.into_iter().map(|name| ListedEntry {
        entry: Entry {
            rel_path: join(&name),
            name,
            is_dir: true,
            kind: None,
            size: 0,
            modified_ms: None,
            cloud: rupert_core::CloudState::CloudOnly,
        },
        meta: None,
    });
    let files = docs.into_iter().map(|h| ListedEntry {
        meta: Some(FolderMeta {
            title: h.title,
            tags: h.tags,
            favorite: h.favorite,
            document_date: h.document_date,
        }),
        entry: Entry {
            name: h.name,
            rel_path: h.rel_path,
            is_dir: false,
            kind: Some(h.kind),
            size: h.size,
            modified_ms: h.modified_ms,
            cloud: h.cloud,
        },
    });
    Ok(folders.chain(files).collect())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FavoriteFolder {
    source_id: String,
    rel_path: String,
    name: String,
    source_name: String,
}

/// Folders marked as favorites (on a computer), in any library folder.
#[tauri::command]
fn favorite_folders(state: State<AppState>) -> CmdResult<Vec<FavoriteFolder>> {
    let mut names: Vec<(String, String)> = state
        .sources()
        .into_iter()
        .map(|s| (s.id, s.name))
        .collect();
    names.extend(
        state
            .saved
            .lock()
            .unwrap()
            .saved
            .remote
            .iter()
            .map(|f| (f.id.clone(), f.name.clone())),
    );
    let favs = state
        .index
        .lock()
        .unwrap()
        .favorite_folders()
        .map_err(err)?;
    let mut out: Vec<FavoriteFolder> = favs
        .into_iter()
        .filter_map(|(source_id, rel_path)| {
            let source_name = names.iter().find(|(id, _)| *id == source_id)?.1.clone();
            let name = rel_path
                .rsplit('/')
                .next()
                .filter(|n| !n.is_empty())
                .unwrap_or(&source_name)
                .to_string();
            Some(FavoriteFolder {
                source_id,
                rel_path,
                name,
                source_name,
            })
        })
        .collect();
    out.sort_by_key(|f| f.name.to_lowercase());
    Ok(out)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    text: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    tag_mode: TagMode,
    #[serde(default)]
    favorites_only: bool,
}

#[tauri::command]
fn search(query: SearchArgs, state: State<AppState>) -> CmdResult<Vec<Hit>> {
    let q = SearchQuery {
        text: query.text,
        source_id: None,
        tags: query.tags,
        tag_mode: query.tag_mode,
        favorites_only: query.favorites_only,
        limit: 200,
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

#[tauri::command]
fn document(
    source_id: String,
    rel_path: String,
    state: State<AppState>,
) -> CmdResult<Option<DocInfo>> {
    state
        .index
        .lock()
        .unwrap()
        .document(&source_id, &rel_path)
        .map_err(err)
}

/// Downloads the document if needed and shows it in Quick Look. Only ever
/// on an explicit tap.
#[tauri::command]
async fn open_document(
    source_id: String,
    rel_path: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    if let Some(folder) = state.remote(&source_id) {
        let size = state
            .index
            .lock()
            .unwrap()
            .document(&source_id, &rel_path)
            .map_err(err)?
            .map(|d| d.hit.size)
            .unwrap_or(0);
        return blocking(move || {
            let path = onedrive::fetch_document(&app, &folder, &rel_path, size)?;
            device(&app).preview(&path.to_string_lossy()).map_err(err)
        })
        .await;
    }
    let source = state.source(&source_id)?;
    let path = rupert_core::resolve(&source.root, &rel_path).map_err(err)?;
    let path = path.to_string_lossy().into_owned();
    let handle = app.clone();
    blocking(move || {
        let device = device(&handle);
        device.download(&path).map_err(err)?;
        device.preview(&path).map_err(err)
    })
    .await?;
    // It's on the phone now; the index should know.
    state.indexer.send(Job::File { source, rel_path });
    Ok(())
}

// MARK: - Commands: scanning

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanSettings {
    /// "color", "gray" or "bw".
    mode: String,
    dpi: u32,
}

/// Scans with the camera and saves the PDF to the inbox as `name`. The
/// saved file name, or `None` if the scan was cancelled.
#[tauri::command]
async fn scan(
    name: String,
    settings: ScanSettings,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Option<String>> {
    let (inbox, remote_inbox) = {
        let saved = state.saved.lock().unwrap();
        (
            saved.saved.inbox.clone(),
            saved.saved.onedrive_inbox.clone(),
        )
    };
    if inbox.is_none() && remote_inbox.is_none() {
        return Err("Choose the inbox folder first.".into());
    }
    let handle = app.clone();
    let placed = blocking(move || {
        let device = device(&handle);
        let mode = match settings.mode.as_str() {
            m @ ("color" | "gray" | "bw") => m,
            _ => "gray",
        };
        let pages = device
            .scan(mode, settings.dpi.clamp(100, 600))
            .map_err(err)?;
        let Some(first) = pages.first() else {
            return Ok(None);
        };
        let jpegs = pages
            .iter()
            .map(fs::read)
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(err)?;
        let pdf = camera::pages_to_pdf(&jpegs).map_err(err)?;
        let tmp_dir = Path::new(first).parent().map(Path::to_path_buf);
        if let Some(remote) = remote_inbox {
            let result = onedrive::upload_scan(&handle, &remote, &name, &pdf);
            if let Some(dir) = tmp_dir {
                let _ = fs::remove_dir_all(dir);
            }
            return result.map(|name| {
                Some(tauri_plugin_rupert_ios::Placed {
                    path: String::new(),
                    name,
                })
            });
        }
        let inbox = inbox.expect("checked above");
        let tmp = Path::new(first).with_file_name("scan.pdf");
        fs::write(&tmp, pdf).map_err(err)?;
        let placed = device
            .place(&tmp.to_string_lossy(), &inbox.path.to_string_lossy(), &name)
            .map_err(err)?;
        Ok(Some(placed))
    })
    .await?;
    let Some(placed) = placed else {
        return Ok(None);
    };
    // Show it right away if the inbox is inside a library folder.
    if let Some((source, rel)) = state.locate(Path::new(&placed.path)) {
        state.indexer.send(Job::File {
            source,
            rel_path: rel,
        });
    }
    Ok(Some(placed.name))
}

// MARK: - Startup

/// Reopens the saved folders (their paths can change between launches),
/// then brings the index up to date.
fn open_saved_folders(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (ids, mut bookmarks): (Vec<Option<String>>, Vec<String>) = {
        let saved = state.saved.lock().unwrap();
        let mut ids = Vec::new();
        let mut marks = Vec::new();
        for (id, mark) in &saved.saved.bookmarks {
            ids.push(Some(id.clone()));
            marks.push(mark.clone());
        }
        if let Some(inbox) = &saved.saved.inbox {
            ids.push(None);
            marks.push(inbox.bookmark.clone());
        }
        (ids, marks)
    };
    match device(app).resolve_bookmarks(&bookmarks) {
        Ok(resolved) => {
            let mut store = state.store.lock().unwrap();
            let mut saved = state.saved.lock().unwrap();
            for ((id, r), mark) in ids.iter().zip(resolved).zip(bookmarks.iter_mut()) {
                if let Some(e) = &r.error {
                    eprintln!("rupert: couldn't reopen a folder: {e}");
                }
                let Some(path) = r.path else { continue };
                if let Some(fresh) = r.bookmark {
                    *mark = fresh;
                }
                match id {
                    Some(id) => {
                        let _ = store.set_root(id, &path);
                        saved.saved.bookmarks.insert(id.clone(), mark.clone());
                    }
                    None => {
                        if let Some(inbox) = saved.saved.inbox.as_mut() {
                            inbox.path = path.into();
                            inbox.bookmark = mark.clone();
                        }
                    }
                }
            }
            saved.save();
        }
        Err(DeviceError::Unsupported) => {} // not on iOS: paths are stable
        Err(e) => eprintln!("rupert: couldn't reopen folders: {e}"),
    }
    state.ready.store(true, Ordering::SeqCst);
    let _ = app.emit(FOLDERS_READY, ());
    state.sync_local();
    app.state::<OneDrive>().restore(app);
    onedrive::start_sync(app);
    onedrive::tidy_cache(app);
}

/// Lets the sync service fetch one of Rupert's hidden folders (sidecars,
/// catalog) before the indexer reads it. Gives up after a while, e.g.
/// offline, and indexes what is there.
fn hidden_folder_fetcher(app: AppHandle) -> PrepareHidden {
    Box::new(move |dir: &Path| {
        let (tx, rx) = mpsc::channel();
        let handle = app.clone();
        let path = dir.to_string_lossy().into_owned();
        std::thread::spawn(move || {
            let _ = tx.send(device(&handle).fetch_folder(&path));
        });
        match rx.recv_timeout(FETCH_TIMEOUT) {
            Ok(Ok(())) | Ok(Err(DeviceError::Unsupported)) => {}
            Ok(Err(e)) => eprintln!("rupert: couldn't fetch {}: {e}", dir.display()),
            Err(_) => eprintln!("rupert: still fetching {}; going on", dir.display()),
        }
    })
}

fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let config_dir = app.path().app_config_dir()?;
    let data_dir = app.path().app_data_dir()?;
    fs::create_dir_all(&config_dir)?;
    fs::create_dir_all(&data_dir)?;
    let store = SourceStore::load(config_dir.join("sources.json"))?;
    let saved = SavedStore::load(config_dir.join("phone.json"));
    let index_path = data_dir.join("index.sqlite");
    let index = Index::open(&index_path)?;
    app.manage(OneDrive::default());

    let progress_handle = app.handle().clone();
    let changed_handle = app.handle().clone();
    let indexer = indexer::spawn_with(
        index_path.clone(),
        // No PDFium on the phone: text comes from the catalog and sidecars.
        TextExtractor::new(None),
        Options {
            mode: Mode::Viewer,
            prepare_hidden: Some(hidden_folder_fetcher(app.handle().clone())),
        },
        move |p| {
            if let Some(state) = progress_handle.try_state::<AppState>() {
                *state.progress.lock().unwrap() = p.clone();
            }
            let _ = progress_handle.emit(INDEX_PROGRESS, p);
        },
        move || {
            let _ = changed_handle.emit(INDEX_CHANGED, ());
        },
    )?;

    app.manage(AppState {
        store: Mutex::new(store),
        saved: Mutex::new(saved),
        index_path,
        index: Mutex::new(index),
        indexer,
        progress: Mutex::new(Progress::default()),
        ready: AtomicBool::new(false),
    });

    #[cfg(debug_assertions)]
    test_hooks::install(app);

    let handle = app.handle().clone();
    std::thread::Builder::new()
        .name("rupert-open-folders".into())
        .spawn(move || open_saved_folders(&handle))?;
    Ok(())
}

// MARK: - Test hooks (debug builds only)

/// Lets a simulator be set up and driven from the command line, since
/// the Files picker can't be scripted: `SIMCTL_CHILD_RUPERT_TEST_FOLDER`
/// adds a library folder, `…_RUPERT_TEST_INBOX` sets the inbox, and
/// `…_RUPERT_TEST_JS` runs a script in the page once it has loaded.
#[cfg(debug_assertions)]
mod test_hooks {
    use super::*;

    pub fn install(app: &tauri::App) {
        let state = app.state::<AppState>();
        if let Ok(folder) = std::env::var("RUPERT_TEST_FOLDER") {
            let mut store = state.store.lock().unwrap();
            let known = store.sources().iter().any(|s| s.root == Path::new(&folder));
            if !known && let Err(e) = store.add(&folder, None) {
                eprintln!("rupert: test folder: {e}");
            }
        }
        if let Ok(folder) = std::env::var("RUPERT_TEST_INBOX") {
            let mut saved = state.saved.lock().unwrap();
            saved.saved.inbox = Some(SavedFolder {
                name: Path::new(&folder)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                path: folder.into(),
                bookmark: String::new(),
            });
        }
        if let Ok(js) = std::env::var("RUPERT_TEST_JS") {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(4));
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.eval(&js);
                }
            });
        }
    }
}

// MARK: - Entry point

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_rupert_ios::init())
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            overview,
            refresh,
            add_folder,
            remove_folder,
            choose_inbox,
            list_dir,
            search,
            list_tags,
            favorite_folders,
            tag_suggestions,
            document,
            open_document,
            scan,
            onedrive::onedrive_status,
            onedrive::onedrive_sign_in,
            onedrive::onedrive_sign_out,
            onedrive::onedrive_folders,
            onedrive::add_onedrive_folder,
            onedrive::set_onedrive_inbox,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Rupert");
}
