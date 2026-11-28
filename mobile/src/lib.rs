//! Rupert for iPhone: browse and search the same folders as the computer,
//! open documents, and scan paper into the inbox folder.
//!
//! The phone only reads the library. Folders come from the Files picker
//! (iCloud Drive, On My iPhone) and are reopened at launch from bookmarks;
//! OneDrive and Google Drive folders, which the Files app can't hand to
//! other apps, are read through their web APIs (`cloud`, `onedrive`,
//! `gdrive`). Text and details come from the shared catalog and sidecars
//! that Rupert on a computer writes; documents are downloaded only when
//! opened. Scans go to the inbox folder, where Rupert on the computer runs
//! OCR and suggests how to file them.

use rupert_core::i18n::{self, tr};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Duration;

mod assistant;
mod cloud;
mod gdrive;
mod onedrive;
mod share;
mod thumbs;

use cloud::{RemoteFolder, RemoteInbox, Service};
use gdrive::GoogleDrive;
use onedrive::OneDrive;

use rupert_core::camera;
use rupert_core::indexer::{self, Mode, Options, PrepareHidden};
use rupert_core::reminders::{self, Dated, Day, Group, ReminderSettings, Show};
use rupert_core::saved_search::{self, SavedSearch};
use rupert_core::text::TextExtractor;
use rupert_core::{
    DocInfo, Entry, FolderMeta, Hit, Index, IndexStats, IndexerHandle, Job, Progress, Provider,
    SearchQuery, Source, SourceStore, TagCount, TagList, TagMode,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State, Wry};
use tauri_plugin_rupert_phone::{Device, Error as DeviceError};

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
    /// Library folders read through a cloud service's web API.
    #[serde(default)]
    remote: Vec<RemoteFolder>,
    /// Scans go here (OneDrive or Google Drive) instead of `inbox` when set.
    #[serde(default, alias = "onedriveInbox")]
    remote_inbox: Option<RemoteInbox>,
    /// The source added so the inbox shows in the library when it isn't
    /// inside a library folder: a Files source in the store, or a cloud
    /// folder in `remote`. Not listed as a library folder.
    #[serde(default)]
    inbox_source: Option<String>,
    /// Rupert's language ("en", "es"); `None` follows the phone's.
    #[serde(default)]
    language: Option<String>,
    /// Sharing a PDF: include the notes kept inside it (off: a copy without
    /// them is shared, as on the computer).
    #[serde(default)]
    send_notes: bool,
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

    /// Local folders only; cloud folders are synced by `cloud`. (Not
    /// `Job::SyncAll`, which would drop the cloud folders' entries.)
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
        locate_in(&self.sources(), path)
    }

    /// Where the inbox can be browsed in the library: the folder it is in a
    /// library folder, or the source added for it.
    fn inbox_place(&self) -> Option<Place> {
        let (files, remote, own, remotes) = {
            let saved = self.saved.lock().unwrap();
            let s = &saved.saved;
            (
                s.inbox.clone(),
                s.remote_inbox.clone(),
                s.inbox_source.clone(),
                s.remote.clone(),
            )
        };
        let not_own = |id: &String| Some(id) != own.as_ref();
        let (source_id, rel_path) = if let Some(r) = remote {
            let library: Vec<RemoteFolder> =
                remotes.iter().filter(|f| not_own(&f.id)).cloned().collect();
            remote_place(&r, &library).or_else(|| {
                let id = own.filter(|id| remotes.iter().any(|f| &f.id == id))?;
                Some((id, String::new()))
            })?
        } else {
            let path = files?.path;
            let sources = self.sources();
            let library: Vec<Source> = sources.iter().filter(|s| not_own(&s.id)).cloned().collect();
            match locate_in(&library, &path) {
                Some((s, rel)) => (s.id, rel),
                None => (
                    own.filter(|id| sources.iter().any(|s| &s.id == id))?,
                    String::new(),
                ),
            }
        };
        Some(Place {
            source_id,
            rel_path,
        })
    }
}

/// The source (and relative folder) `path` lies in, if any.
fn locate_in(sources: &[Source], path: &Path) -> Option<(Source, String)> {
    sources.iter().find_map(|s| {
        let rel = path.strip_prefix(&s.root).ok()?;
        Some((s.clone(), rel.to_string_lossy().replace('\\', "/")))
    })
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

// MARK: - Commands: language

/// Rupert's language now, the phone's (what "Automatic" means), and the
/// setting (`None`: automatic).
#[derive(Serialize)]
struct Language {
    current: i18n::Lang,
    system: i18n::Lang,
    setting: Option<String>,
}

#[tauri::command]
fn language(state: State<AppState>) -> Language {
    Language {
        current: i18n::current(),
        system: i18n::system(),
        setting: state.saved.lock().unwrap().saved.language.clone(),
    }
}

/// Settings ▸ Language. `None` follows the phone.
#[tauri::command]
fn set_language(language: Option<String>, app: AppHandle, state: State<AppState>) -> Language {
    {
        let mut saved = state.saved.lock().unwrap();
        saved.saved.language = language.filter(|l| i18n::Lang::from_code(l).is_some());
        saved.save();
        i18n::set(i18n::resolve(saved.saved.language.as_deref()));
    }
    share::publish(&app);
    self::language(state)
}

// MARK: - Commands: about

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppVersion {
    /// "0.1.1".
    version: String,
    /// Apple's build number ("2611200404"), when it differs from the version.
    build: Option<String>,
    /// A debug build, not one from the App Store.
    development: bool,
}

/// The app's version and build number, for Settings ▸ About (and for bug
/// reports). The build number comes from the app's own Info.plist.
#[tauri::command]
fn app_version(app: AppHandle) -> AppVersion {
    let version = app.package_info().version.to_string();
    let build = bundle_build().filter(|b| *b != version);
    AppVersion {
        version,
        build,
        development: cfg!(debug_assertions),
    }
}

/// `CFBundleVersion` from the Info.plist beside the app's executable (the
/// app bundle on iOS); `None` elsewhere.
fn bundle_build() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let info = plist::Value::from_file(exe.parent()?.join("Info.plist")).ok()?;
    info.as_dictionary()?
        .get("CFBundleVersion")?
        .as_string()
        .map(str::to_string)
}

// MARK: - The scan link

/// Sent when the scan link opened Rupert while it was running (iPhone).
#[cfg_attr(
    not(any(target_os = "macos", target_os = "ios", target_os = "android")),
    allow(dead_code)
)]
const SCAN_LINK: &str = "scan-link";

/// The scan link (#124) opened Rupert and the page hasn't shown the Scan tab
/// yet (iPhone: `RunEvent::Opened`; Android keeps its own, in the plugin).
static SCAN_LINK_OPENED: AtomicBool = AtomicBool::new(false);

/// Whether `url` is the scan link: the desktop app's Scan QR code
/// (https://rupertlibrarian.com/scan/, a universal link / App Link), or
/// rupert://scan from that page when the link opened in the browser.
#[cfg_attr(
    not(any(target_os = "macos", target_os = "ios", target_os = "android")),
    allow(dead_code)
)]
fn is_scan_link(url: &tauri::Url) -> bool {
    match url.scheme() {
        "https" => {
            url.host_str()
                .is_some_and(|h| h.eq_ignore_ascii_case("rupertlibrarian.com"))
                && url.path().trim_end_matches('/') == "/scan"
        }
        "rupert" => url
            .host_str()
            .is_some_and(|h| h.eq_ignore_ascii_case("scan")),
        _ => false,
    }
}

/// Whether the scan link opened Rupert since the page last asked; the page
/// then shows the Scan tab. Asked at start, when the app comes back, and on
/// `scan-link`.
#[tauri::command]
async fn take_scan_link(app: AppHandle) -> bool {
    let opened = SCAN_LINK_OPENED.swap(false, Ordering::SeqCst);
    if cfg!(target_os = "android") {
        let android = blocking(move || Ok(device(&app).take_scan_link().unwrap_or(false)))
            .await
            .unwrap_or(false);
        return opened || android;
    }
    opened
}

/// Rupert's privacy policy on the web (Google Play wants it reachable from
/// inside the app).
const PRIVACY_POLICY_URL: &str = "https://rupertlibrarian.com/privacy/";

/// Opens the privacy policy in the phone's browser, from Settings ▸ About.
/// Only this one page: the web view can't ask to open other addresses.
#[tauri::command]
async fn open_privacy_policy(app: AppHandle) -> CmdResult<()> {
    blocking(move || device(&app).open_url(PRIVACY_POLICY_URL).map_err(err)).await
}

// MARK: - Commands: overview

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SourceView {
    id: String,
    name: String,
    provider: Provider,
    available: bool,
    /// Added only to show the inbox (not a library folder).
    inbox: bool,
}

/// A folder in the library.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Place {
    source_id: String,
    rel_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    ready: bool,
    sources: Vec<SourceView>,
    /// Name of the folder scans are saved to.
    inbox: Option<String>,
    /// The service it's on; `None`: a folder in Files.
    inbox_service: Option<Service>,
    /// Where to browse the inbox.
    inbox_place: Option<Place>,
    stats: IndexStats,
    progress: Progress,
}

#[tauri::command]
fn overview(app: AppHandle, state: State<AppState>) -> CmdResult<Overview> {
    let inbox_place = state.inbox_place();
    let local = state.sources();
    let saved = state.saved.lock().unwrap();
    let own = saved.saved.inbox_source.as_deref();
    let mut sources: Vec<SourceView> = local
        .into_iter()
        .map(|s| SourceView {
            available: s.is_available(),
            inbox: Some(s.id.as_str()) == own,
            id: s.id,
            name: s.name,
            provider: s.provider,
        })
        .collect();
    sources.extend(saved.saved.remote.iter().map(|f| SourceView {
        id: f.id.clone(),
        name: f.name.clone(),
        provider: f.service.provider(),
        available: f.service.signed_in(&app),
        inbox: Some(f.id.as_str()) == own,
    }));
    let (inbox, inbox_service) = match (&saved.saved.remote_inbox, &saved.saved.inbox) {
        (Some(r), _) => (Some(r.name.clone()), Some(r.service)),
        (None, Some(f)) => (Some(f.name.clone()), None),
        (None, None) => (None, None),
    };
    Ok(Overview {
        ready: state.ready.load(Ordering::SeqCst),
        sources,
        inbox,
        inbox_service,
        inbox_place,
        stats: state.index.lock().unwrap().stats().map_err(err)?,
        progress: state.progress.lock().unwrap().clone(),
    })
}

#[tauri::command]
fn refresh(app: AppHandle, state: State<AppState>) {
    state.sync_local();
    cloud::start_all(&app);
}

// MARK: - Commands: folders

/// Picks a folder in Files and adds it to the library. `false` if cancelled.
#[tauri::command]
async fn add_folder(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    let app_handle = app.clone();
    let picked = blocking(move || device(&app).pick_folder().map_err(err)).await?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    // A folder holding the inbox replaces the inbox's own source (sources
    // can't nest); `reconcile_inbox` brings it back if still needed.
    let own = state.saved.lock().unwrap().saved.inbox_source.clone();
    let own_inside = own.filter(|id| {
        state
            .source(id)
            .is_ok_and(|s| s.root.starts_with(&picked.path))
    });
    if let Some(id) = own_inside {
        let _ = state.store.lock().unwrap().remove(&id);
        state.saved.lock().unwrap().saved.inbox_source = None;
        state.indexer.send(Job::RemoveSource(id));
    }
    let added = state
        .store
        .lock()
        .unwrap()
        .add(&picked.path, Some(picked.name));
    let source = match added {
        Ok(source) => source,
        Err(e) => {
            reconcile_inbox(&app_handle);
            return Err(err(e));
        }
    };
    {
        let mut saved = state.saved.lock().unwrap();
        saved
            .saved
            .bookmarks
            .insert(source.id.clone(), picked.bookmark);
        saved.save();
    }
    state.indexer.send(Job::SyncSource(source));
    reconcile_inbox(&app_handle);
    Ok(true)
}

/// Removes a folder from the library (nothing on disk changes).
#[tauri::command]
fn remove_folder(id: String, app: AppHandle, state: State<AppState>) -> CmdResult<()> {
    if state.saved.lock().unwrap().saved.inbox_source.as_deref() == Some(id.as_str()) {
        return Err(tr("That's the inbox; choose another inbox in Settings instead.").into());
    }
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
    reconcile_inbox(&app);
    Ok(())
}

/// Picks the folder scans are saved to. `false` if cancelled.
#[tauri::command]
async fn choose_inbox(app: AppHandle, state: State<'_, AppState>) -> CmdResult<bool> {
    let handle = app.clone();
    let picked = blocking(move || device(&handle).pick_folder().map_err(err)).await?;
    let Some(picked) = picked else {
        return Ok(false);
    };
    {
        let mut saved = state.saved.lock().unwrap();
        saved.saved.inbox = Some(SavedFolder {
            name: picked.name,
            path: picked.path.into(),
            bookmark: picked.bookmark,
        });
        saved.saved.remote_inbox = None;
        saved.save();
    }
    reconcile_inbox(&app);
    Ok(true)
}

// MARK: - The inbox in the library
//
// Scans should be easy to find on the phone. When the inbox folder lies
// inside a library folder it is browsed there; otherwise it is added as a
// source of its own (`Saved::inbox_source`), which the library shows as
// "Inbox" rather than as a folder. Read-only, like every source here.

/// Where a cloud inbox lies inside one of `folders` on its service (by the path it was
/// chosen by): that folder's id and the inbox's path in it.
fn remote_place(inbox: &RemoteInbox, folders: &[RemoteFolder]) -> Option<(String, String)> {
    folders
        .iter()
        .filter(|f| f.service == inbox.service)
        .find_map(|f| {
            if f.item_id == inbox.item_id {
                return Some((f.id.clone(), String::new()));
            }
            let at = inbox.path.iter().position(|p| p.id == f.item_id)?;
            let rel: Vec<&str> = inbox.path[at + 1..]
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            Some((f.id.clone(), rel.join("/")))
        })
}

/// Adds the inbox's own source when the inbox isn't inside a library
/// folder, and removes it when it is (or there's no inbox any more). Called
/// whenever the inbox or the library folders change, and at launch.
pub(crate) fn reconcile_inbox(app: &AppHandle) {
    share::publish(app);
    let state = app.state::<AppState>();
    let (files, remote, own, remotes) = {
        let saved = state.saved.lock().unwrap();
        let s = &saved.saved;
        (
            s.inbox.clone(),
            s.remote_inbox.clone(),
            s.inbox_source.clone(),
            s.remote.clone(),
        )
    };
    let not_own = |id: &String| Some(id) != own.as_ref();
    let library_remote: Vec<RemoteFolder> =
        remotes.iter().filter(|f| not_own(&f.id)).cloned().collect();
    let library_local: Vec<Source> = state
        .sources()
        .into_iter()
        .filter(|s| not_own(&s.id))
        .collect();
    let need_remote = remote
        .clone()
        .filter(|r| remote_place(r, &library_remote).is_none());
    let need_files = files
        .filter(|_| remote.is_none())
        .filter(|f| locate_in(&library_local, &f.path).is_none());

    // Keep the inbox's source if it is still the right one.
    if let Some(id) = &own {
        let keep = if let Some(f) = &need_files {
            match state.source(id) {
                Ok(s) if s.root == f.path => true,
                Ok(_) => {
                    // The inbox folder moved (its bookmark resolved elsewhere).
                    let moved = state.store.lock().unwrap().set_root(id, &f.path).is_ok();
                    if moved && let Ok(s) = state.source(id) {
                        state.indexer.send(Job::SyncSource(s));
                    }
                    moved
                }
                Err(_) => false,
            }
        } else if let Some(r) = &need_remote {
            remotes
                .iter()
                .any(|f| &f.id == id && f.item_id == r.item_id && f.service == r.service)
        } else {
            false
        };
        if keep {
            return;
        }
        let _ = state.store.lock().unwrap().remove(id);
        let mut saved = state.saved.lock().unwrap();
        saved.saved.remote.retain(|f| &f.id != id);
        saved.saved.inbox_source = None;
        saved.save();
        drop(saved);
        state.indexer.send(Job::RemoveSource(id.clone()));
    }

    if let Some(f) = need_files {
        let added = state
            .store
            .lock()
            .unwrap()
            .add(&f.path, Some(f.name.clone()));
        match added {
            Ok(source) => {
                let mut saved = state.saved.lock().unwrap();
                saved.saved.inbox_source = Some(source.id.clone());
                saved.save();
                drop(saved);
                state.indexer.send(Job::SyncSource(source));
            }
            // E.g. the inbox holds a library folder (sources can't nest).
            Err(e) => eprintln!("rupert: couldn't show the inbox in the library: {e}"),
        }
    } else if let Some(r) = need_remote {
        let folder = RemoteFolder::new(r.service, r.item_id.clone(), r.name.clone());
        let mut saved = state.saved.lock().unwrap();
        saved.saved.inbox_source = Some(folder.id.clone());
        saved.saved.remote.push(folder);
        saved.save();
        drop(saved);
        cloud::start_sync(app, r.service);
    }
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

/// A cloud folder's contents, from the index (nothing is on the phone).
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
            sender: h.sender,
            amount: h.amount,
            currency: h.currency,
            due_date: h.due_date,
            done: h.done,
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

/// Stars or unstars a folder: the phone's one write to the library, and
/// only to Rupert's own `.rupert/.folder.json` in that folder.
#[tauri::command]
async fn set_folder_favorite(
    source_id: String,
    rel_path: String,
    favorite: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    use rupert_core::folder_info::{self, FolderInfo};
    let info = FolderInfo { favorite };
    let rel = rel_path.trim_matches('/').to_string();
    if let Some(folder) = state.remote(&source_id) {
        let (handle, info2, rel2) = (app.clone(), info.clone(), rel.clone());
        blocking(move || cloud::write_folder_info(&handle, &folder, &rel2, &info2)).await?;
    } else {
        let source = state.source(&source_id)?;
        let dir = rupert_core::resolve(&source.root, &rel).map_err(err)?;
        let (handle, info2) = (app.clone(), info.clone());
        blocking(move || {
            let path = folder_info::path_for(&dir).to_string_lossy().into_owned();
            let device = device(&handle);
            let result = if info2.is_empty() {
                device.remove_file(&path)
            } else {
                let text = String::from_utf8(folder_info::to_bytes(&info2)).map_err(err)?;
                device.write_file(&path, &text)
            };
            match result {
                // Not on iOS (development on a computer): write directly.
                Err(DeviceError::Unsupported) => folder_info::write(&dir, &info2).map_err(err),
                other => other.map_err(err),
            }
        })
        .await?;
    }
    state
        .index
        .lock()
        .unwrap()
        .set_folder(&source_id, &rel, Some(&info))
        .map_err(err)?;
    Ok(())
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
        // The desktop's date, kind and place filters (#103) aren't on the phone yet.
        ..Default::default()
    };
    state.index.lock().unwrap().search(&q).map_err(err)
}

// MARK: - Commands: saved searches and Coming up (#162)

/// A saved search and the source it's kept with (see `rupert_core::saved_search`).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListedSearch {
    source_id: String,
    #[serde(flatten)]
    search: SavedSearch,
}

/// The saved searches in the library's folders, by name: read from each
/// folder on the phone (fetched with Rupert's other files while indexing)
/// and from the copies kept of the cloud folders'. Read-only.
#[tauri::command]
async fn saved_searches(app: AppHandle) -> CmdResult<Vec<ListedSearch>> {
    blocking(move || {
        let state = app.state::<AppState>();
        let local = state
            .sources()
            .into_iter()
            .map(|s| (s.id, saved_search::list(&s.root)));
        let remote: Vec<String> = state
            .saved
            .lock()
            .unwrap()
            .saved
            .remote
            .iter()
            .map(|f| f.id.clone())
            .collect();
        let remote = remote.into_iter().map(|id| {
            let found = cloud::saved_searches(&app, &id);
            (id, found)
        });
        let mut all: Vec<ListedSearch> = local
            .chain(remote)
            .flat_map(|(source_id, found)| {
                found.into_iter().map(move |search| ListedSearch {
                    source_id: source_id.clone(),
                    search,
                })
            })
            .collect();
        all.sort_by_key(|l| l.search.name.to_lowercase());
        Ok(all)
    })
    .await
}

/// Runs a saved search with all its filters, as the computer does.
#[tauri::command]
fn run_saved_search(
    source_id: String,
    search: SavedSearch,
    state: State<AppState>,
) -> CmdResult<Vec<Hit>> {
    let q = search.query(&source_id, Day::today(), 200);
    state.index.lock().unwrap().search(&q).map_err(err)
}

/// Coming up on the phone: every document with a due date that isn't done
/// (or, with `done`, those marked done), and how many there are of each.
/// Lead times are each computer's own setting, so the phone doesn't use them.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ComingUp {
    items: Vec<reminders::Item>,
    /// Not done, and overdue.
    open: usize,
    overdue: usize,
    done: usize,
}

#[tauri::command]
async fn coming_up(done: bool, app: AppHandle) -> CmdResult<ComingUp> {
    blocking(move || {
        let state = app.state::<AppState>();
        let mut ids: Vec<String> = state.sources().into_iter().map(|s| s.id).collect();
        ids.extend(
            state
                .saved
                .lock()
                .unwrap()
                .saved
                .remote
                .iter()
                .map(|f| f.id.clone()),
        );
        let docs: Vec<Dated> = state
            .index
            .lock()
            .unwrap()
            .dated()
            .map_err(err)?
            .into_iter()
            .filter(|d| ids.contains(&d.source_id))
            .collect();
        let today = Day::today();
        let settings = ReminderSettings::default();
        let open = reminders::coming_up(docs.clone(), &settings, today, Show::All);
        let done_count = docs.iter().filter(|d| d.done.is_some()).count();
        Ok(ComingUp {
            overdue: open.iter().filter(|i| i.group == Group::Overdue).count(),
            open: open.len(),
            done: done_count,
            items: if done {
                reminders::coming_up(docs, &settings, today, Show::Done)
            } else {
                open
            },
        })
    })
    .await
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
            let path = cloud::fetch_document(&app, &folder, &rel_path, size)?;
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

// MARK: - Commands: sharing (#69)

/// Share: downloads the document if needed and hands it to the phone's
/// share sheet. A PDF goes as a copy without its notes (in the app's
/// cache) unless Settings ▸ Sharing includes them; the library is only
/// read. Only ever on an explicit tap.
#[tauri::command]
async fn share_document(
    source_id: String,
    rel_path: String,
    at: [f64; 4],
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<()> {
    let include_notes = state.saved.lock().unwrap().saved.send_notes;
    let info = state
        .index
        .lock()
        .unwrap()
        .document(&source_id, &rel_path)
        .map_err(err)?;
    let (size, title) = info.map(|d| (d.hit.size, d.hit.title)).unwrap_or((0, None));
    let cache = app.path().app_cache_dir().map_err(err)?;
    let remote = state.remote(&source_id);
    let local = match &remote {
        Some(_) => None,
        None => Some(state.source(&source_id)?),
    };
    let (handle, rel, from) = (app.clone(), rel_path.clone(), local.clone());
    blocking(move || {
        let device = device(&handle);
        let path = match (&remote, &from) {
            (Some(folder), _) => cloud::fetch_document(&handle, folder, &rel, size)?,
            (None, Some(source)) => {
                let path = rupert_core::resolve(&source.root, &rel).map_err(err)?;
                // iCloud Drive and Files providers fetch on demand; a file
                // on an Android phone is simply there.
                #[cfg(not(target_os = "android"))]
                device.download(&path.to_string_lossy()).map_err(err)?;
                path
            }
            (None, None) => return Err("unknown folder".to_string()),
        };
        let sent = rupert_core::share::prepare(&path, None, include_notes, &cache).map_err(err)?;
        // Android hands out files only from the app's cache (FileProvider).
        #[cfg(target_os = "android")]
        let sent = if sent.starts_with(&cache) {
            sent
        } else {
            let folder = cache.join("sending").join(format!(
                "{:x}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default()
            ));
            fs::create_dir_all(&folder).map_err(err)?;
            let copy = folder.join(sent.file_name().unwrap_or_default());
            fs::copy(&sent, &copy).map_err(err)?;
            copy
        };
        let name = sent
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let subject = title.unwrap_or_else(|| rupert_core::rename::split_name(&name).0.to_string());
        device
            .share(&sent.to_string_lossy(), &subject, at)
            .map_err(err)
    })
    .await?;
    if let Some(source) = local {
        // Downloaded to share it: the index should know.
        state.indexer.send(Job::File { source, rel_path });
    }
    Ok(())
}

/// Settings ▸ Sharing: whether a PDF's notes go along.
#[tauri::command]
fn send_notes(state: State<AppState>) -> bool {
    state.saved.lock().unwrap().saved.send_notes
}

#[tauri::command]
fn set_send_notes(on: bool, state: State<AppState>) {
    let mut saved = state.saved.lock().unwrap();
    saved.saved.send_notes = on;
    saved.save();
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
        (saved.saved.inbox.clone(), saved.saved.remote_inbox.clone())
    };
    if inbox.is_none() && remote_inbox.is_none() {
        return Err(tr("Choose the inbox folder first.").into());
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
            let result = cloud::upload(&handle, &remote, &format!("{name}.pdf"), &pdf);
            if let Some(dir) = tmp_dir {
                let _ = fs::remove_dir_all(dir);
            }
            // Show it in the inbox once the service lists it.
            cloud::start_sync(&handle, remote.service);
            return result.map(|name| {
                Some(tauri_plugin_rupert_phone::Placed {
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
    // Nothing to reopen (always so on Android, which has no Files folders).
    let resolved = if bookmarks.is_empty() {
        Ok(Vec::new())
    } else {
        device(app).resolve_bookmarks(&bookmarks)
    };
    match resolved {
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
    reconcile_inbox(app);
    state.ready.store(true, Ordering::SeqCst);
    let _ = app.emit(FOLDERS_READY, ());
    state.sync_local();
    app.state::<OneDrive>().restore(app);
    app.state::<GoogleDrive>().restore(app);
    cloud::start_all(app);
    cloud::tidy_cache(app);
    thumbs::tidy(app);
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
    i18n::set(i18n::resolve(saved.saved.language.as_deref()));
    let index_path = data_dir.join("index.sqlite");
    let index = Index::open(&index_path)?;
    app.manage(assistant::AssistantState::default());
    app.manage(OneDrive::default());
    app.manage(GoogleDrive::default());

    let progress_handle = app.handle().clone();
    let changed_handle = app.handle().clone();
    let indexer = indexer::spawn_with(
        index_path.clone(),
        // No PDFium on the phone: text comes from the catalog and sidecars.
        TextExtractor::new(None),
        Options {
            mode: Mode::Viewer,
            prepare_hidden: Some(hidden_folder_fetcher(app.handle().clone())),
            // No PDFium to draw pages with.
            pictures: None,
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
/// Android can't pass environment variables to an app, so there the same
/// names come from `KEY=value` lines in `rupert-test.env` in the app's data
/// folder (written with `adb shell run-as`).
#[cfg(debug_assertions)]
mod test_hooks {
    use super::*;

    fn var(app: &tauri::App, name: &str) -> Option<String> {
        if let Ok(value) = std::env::var(name) {
            return Some(value);
        }
        let file = app.path().app_data_dir().ok()?.join("rupert-test.env");
        let text = std::fs::read_to_string(file).ok()?;
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .map(str::to_owned)
    }

    pub fn install(app: &tauri::App) {
        let state = app.state::<AppState>();
        if let Some(folder) = var(app, "RUPERT_TEST_FOLDER") {
            let mut store = state.store.lock().unwrap();
            let known = store.sources().iter().any(|s| s.root == Path::new(&folder));
            if !known && let Err(e) = store.add(&folder, None) {
                eprintln!("rupert: test folder: {e}");
            }
        }
        if let Some(folder) = var(app, "RUPERT_TEST_INBOX") {
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
        if let Some(js) = var(app, "RUPERT_TEST_JS") {
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

/// Test scripts (`RUPERT_TEST_JS`) report what they're doing through this, to
/// the app's console (`simctl launch --console-pty`), or on Android to logcat
/// (`adb logcat -s Rupert`). Release builds ignore it.
#[tauri::command]
fn test_log(message: String) {
    if cfg!(debug_assertions) {
        #[cfg(target_os = "android")]
        android_log(&format!("rupert-test: {message}"));
        #[cfg(not(target_os = "android"))]
        eprintln!("rupert-test: {message}");
    }
}

/// Android sends an app's stderr nowhere; this writes an info line to logcat.
#[cfg(target_os = "android")]
fn android_log(text: &str) {
    use std::ffi::{CString, c_char, c_int};
    #[link(name = "log")]
    unsafe extern "C" {
        fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) -> c_int;
    }
    const INFO: c_int = 4;
    if let Ok(text) = CString::new(text) {
        // SAFETY: both strings are NUL-terminated and outlive the call.
        unsafe { __android_log_write(INFO, c"Rupert".as_ptr(), text.as_ptr()) };
    }
}

// MARK: - Entry point

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_rupert_phone::init())
        // Pictures for the large-icon and gallery views.
        .register_asynchronous_uri_scheme_protocol(thumbs::SCHEME, |ctx, request, responder| {
            thumbs::serve(ctx.app_handle(), request, responder)
        })
        .setup(setup)
        .invoke_handler(tauri::generate_handler![
            overview,
            refresh,
            app_version,
            open_privacy_policy,
            take_scan_link,
            language,
            set_language,
            share_document,
            send_notes,
            set_send_notes,
            add_folder,
            remove_folder,
            choose_inbox,
            list_dir,
            search,
            list_tags,
            favorite_folders,
            set_folder_favorite,
            tag_suggestions,
            saved_searches,
            run_saved_search,
            coming_up,
            document,
            open_document,
            scan,
            share::send_shared,
            onedrive::onedrive_status,
            onedrive::onedrive_sign_in,
            onedrive::onedrive_sign_out,
            onedrive::onedrive_folders,
            onedrive::add_onedrive_folder,
            onedrive::set_onedrive_inbox,
            gdrive::gdrive_status,
            gdrive::gdrive_sign_in,
            gdrive::gdrive_sign_out,
            gdrive::gdrive_folders,
            gdrive::add_gdrive_folder,
            gdrive::set_gdrive_inbox,
            assistant::assistant_status,
            assistant::assistant_send,
            assistant::assistant_cancel,
            assistant::assistant_reset,
            test_log,
        ])
        .build(tauri::generate_context!())
        .expect("error while running Rupert")
        .run(|_app, _event| {
            // Tauri has `Opened` on the phones and macOS only (not on the
            // Linux runner that checks this crate). Android's plugin also
            // reads the link from the activity's intent.
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            if let tauri::RunEvent::Opened { urls } = _event
                && urls.iter().any(is_scan_link)
            {
                SCAN_LINK_OPENED.store(true, Ordering::SeqCst);
                let _ = _app.emit(SCAN_LINK, ());
            }
        });
}

// MARK: - Tests

#[cfg(test)]
mod scan_link_tests {
    use super::is_scan_link;

    #[test]
    fn knows_the_scan_link() {
        let yes = [
            "https://rupertlibrarian.com/scan/",
            "https://rupertlibrarian.com/scan",
            "https://RupertLibrarian.com/scan/?from=qr",
            "rupert://scan",
        ];
        let no = [
            "https://rupertlibrarian.com/",
            "https://rupertlibrarian.com/scanner/",
            "https://example.com/scan/",
            "msauth.io.github.micki-b.rupert://auth",
        ];
        for u in yes {
            assert!(is_scan_link(&u.parse().unwrap()), "{u}");
        }
        for u in no {
            assert!(!is_scan_link(&u.parse().unwrap()), "{u}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cloud::FolderItem;

    fn item(id: &str, name: &str) -> FolderItem {
        FolderItem {
            id: id.into(),
            name: name.into(),
        }
    }

    fn folder(id: &str, item_id: &str) -> RemoteFolder {
        RemoteFolder {
            id: id.into(),
            name: item_id.into(),
            item_id: item_id.into(),
            service: Service::OneDrive,
        }
    }

    #[test]
    fn a_onedrive_inbox_is_found_inside_a_library_folder() {
        let inbox = RemoteInbox {
            item_id: "IN".into(),
            name: "Rupert Inbox".into(),
            path: vec![
                item("DOCS", "Documents"),
                item("PAPER", "Paper"),
                item("IN", "Rupert Inbox"),
            ],
            service: Service::OneDrive,
        };
        let library = [folder("a", "BOAT"), folder("b", "DOCS")];
        assert_eq!(
            remote_place(&inbox, &library),
            Some(("b".into(), "Paper/Rupert Inbox".into()))
        );
        // The inbox is itself a library folder.
        assert_eq!(
            remote_place(&inbox, &[folder("c", "IN")]),
            Some(("c".into(), String::new()))
        );
        // Elsewhere, or chosen before its path was kept.
        assert_eq!(remote_place(&inbox, &[folder("a", "BOAT")]), None);
        // A folder on the other service never holds it, even with the same id.
        let google = RemoteFolder {
            service: Service::GoogleDrive,
            ..folder("g", "DOCS")
        };
        assert_eq!(remote_place(&inbox, std::slice::from_ref(&google)), None);
        let old = RemoteInbox {
            path: Vec::new(),
            ..inbox
        };
        assert_eq!(remote_place(&old, &library), None);
    }

    #[test]
    fn folders_saved_before_google_drive_load_as_onedrive() {
        let json = r#"{"bookmarks":{},"inbox":null,
            "remote":[{"id":"onedrive-1","name":"Boating","itemId":"B"}],
            "onedriveInbox":{"itemId":"IN","name":"Rupert Inbox"}}"#;
        let saved: Saved = serde_json::from_str(json).unwrap();
        assert_eq!(saved.remote[0].service, Service::OneDrive);
        let inbox = saved.remote_inbox.unwrap();
        assert_eq!(inbox.service, Service::OneDrive);
        assert_eq!(inbox.item_id, "IN");
        // Saved again, it uses the new names.
        let again = Saved {
            remote_inbox: Some(inbox),
            ..Default::default()
        };
        let text = serde_json::to_string(&again).unwrap();
        assert!(text.contains(r#""remoteInbox""#) && text.contains(r#""service":"onedrive""#));
    }

    #[test]
    fn google_drive_folders_are_their_own_sources() {
        let f = RemoteFolder::new(Service::GoogleDrive, "XYZ".into(), "Paperwork".into());
        let s = f.source();
        assert!(f.id.starts_with("googledrive-"));
        assert_eq!(s.provider, Provider::GoogleDrive);
        assert_eq!(s.root, Path::new("googledrive:XYZ"));
    }

    #[test]
    fn a_files_inbox_is_found_inside_a_library_folder() {
        let source = |id: &str, root: &str| Source {
            id: id.into(),
            name: id.into(),
            root: root.into(),
            provider: Provider::ICloud,
            show_all_types: false,
            download_online_only: false,
            offline: false,
            bookmark: None,
        };
        let library = [
            source("docs", "/icloud/Documents"),
            source("boat", "/icloud/Boat"),
        ];
        let found = locate_in(&library, Path::new("/icloud/Documents/Scans/Inbox"))
            .map(|(s, rel)| (s.id, rel));
        assert_eq!(found, Some(("docs".into(), "Scans/Inbox".into())));
        let at_root = locate_in(&library, Path::new("/icloud/Boat")).map(|(s, rel)| (s.id, rel));
        assert_eq!(at_root, Some(("boat".into(), String::new())));
        assert!(locate_in(&library, Path::new("/icloud/Rupert Inbox")).is_none());
        // A sibling whose name starts the same isn't inside.
        assert!(locate_in(&library, Path::new("/icloud/Boating")).is_none());
    }
}
