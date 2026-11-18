//! OneDrive folders on the phone, through Microsoft Graph. The iPhone's
//! Files app can't hand OneDrive folders to other apps, so these sources
//! are listed through the web API instead (`rupert_core::onedrive`) and
//! indexed from Rupert's catalog and sidecars (`rupert_core::remote`).
//!
//! The refresh token lives in the Keychain; the access token only in
//! memory. Documents are downloaded only when opened, into the cache.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use rupert_core::onedrive::{self, Graph, Pkce, Tokens};
use rupert_core::{Index, Progress, Provider, Source, remote};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, CmdResult, INDEX_CHANGED, blocking, device, err};

/// The app's registration with Microsoft (a public client: not a secret).
/// Read from `mobile/onedrive-client-id` at build time.
const CLIENT_ID: Option<&str> = option_env!("RUPERT_ONEDRIVE_CLIENT_ID");
const REDIRECT_URI: &str = "msauth.io.github.micki-b.rupert://auth";
const CALLBACK_SCHEME: &str = "msauth.io.github.micki-b.rupert";
const REFRESH_KEY: &str = "onedrive.refresh-token";
/// Opened documents are kept this long, then removed from the cache.
const CACHE_DAYS: u64 = 14;

/// OneDrive indexing progress. Payload: `Progress`.
pub const REMOTE_PROGRESS: &str = "remote-progress";

fn client_id() -> Option<&'static str> {
    CLIENT_ID.map(str::trim).filter(|s| !s.is_empty())
}

// MARK: - Saved

/// A OneDrive folder in the library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFolder {
    /// Rupert's source id.
    pub id: String,
    pub name: String,
    /// OneDrive's item id.
    pub item_id: String,
}

impl RemoteFolder {
    pub fn source(&self) -> Source {
        Source {
            id: self.id.clone(),
            name: self.name.clone(),
            root: PathBuf::from(format!("onedrive:{}", self.item_id)),
            provider: Provider::OneDrive,
            show_all_types: false,
            download_online_only: false,
        }
    }
}

/// A OneDrive folder scans are uploaded to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInbox {
    pub item_id: String,
    pub name: String,
}

// MARK: - State

#[derive(Default)]
pub struct OneDrive {
    tokens: Mutex<Option<Tokens>>,
    account: Mutex<Option<String>>,
    /// A refresh token is stored.
    signed_in: AtomicBool,
    syncing: AtomicBool,
    again: AtomicBool,
    progress: Mutex<Progress>,
    last_error: Mutex<Option<String>>,
}

impl OneDrive {
    pub fn is_signed_in(&self) -> bool {
        self.signed_in.load(Ordering::SeqCst)
    }

    /// A connection with a current access token, refreshing it if needed.
    pub fn graph(&self, app: &AppHandle) -> Result<Graph, String> {
        let client = client_id().ok_or("OneDrive isn't set up in this build of Rupert.")?;
        let mut tokens = self.tokens.lock().unwrap();
        if let Some(t) = tokens.as_ref().filter(|t| t.is_fresh()) {
            return Ok(Graph::new(&t.access_token));
        }
        let refresh = device(app)
            .keychain_get(REFRESH_KEY)
            .map_err(err)?
            .ok_or("Sign in to OneDrive first (Settings).")?;
        let fresh = onedrive::refresh(client, &refresh).map_err(err)?;
        if let Some(r) = &fresh.refresh_token {
            device(app).keychain_set(REFRESH_KEY, r).map_err(err)?;
        }
        let graph = Graph::new(&fresh.access_token);
        *tokens = Some(fresh);
        Ok(graph)
    }

    /// Checks the Keychain once at launch.
    pub fn restore(&self, app: &AppHandle) {
        let stored = device(app).keychain_get(REFRESH_KEY).ok().flatten();
        self.signed_in.store(stored.is_some(), Ordering::SeqCst);
    }
}

fn remote_folders(app: &AppHandle) -> Vec<RemoteFolder> {
    app.state::<AppState>()
        .saved
        .lock()
        .unwrap()
        .saved
        .remote
        .clone()
}

// MARK: - Commands: account

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    configured: bool,
    signed_in: bool,
    account: Option<String>,
    error: Option<String>,
    progress: Progress,
}

#[tauri::command]
pub fn onedrive_status(od: State<OneDrive>) -> Status {
    Status {
        configured: client_id().is_some(),
        signed_in: od.is_signed_in(),
        account: od.account.lock().unwrap().clone(),
        error: od.last_error.lock().unwrap().clone(),
        progress: od.progress.lock().unwrap().clone(),
    }
}

/// Shows Microsoft's sign-in page. `false` if cancelled.
#[tauri::command]
pub async fn onedrive_sign_in(app: AppHandle) -> CmdResult<bool> {
    let handle = app.clone();
    let signed_in = blocking(move || {
        let client = client_id().ok_or("OneDrive isn't set up in this build of Rupert.")?;
        let pkce = Pkce::new();
        let url = onedrive::authorize_url(client, REDIRECT_URI, &pkce);
        let Some(answer) = device(&handle)
            .sign_in(&url, CALLBACK_SCHEME)
            .map_err(err)?
        else {
            return Ok(false);
        };
        let code = onedrive::code_from_redirect(&answer, &pkce).map_err(err)?;
        let tokens = onedrive::redeem_code(client, REDIRECT_URI, &code, &pkce).map_err(err)?;
        let refresh = tokens
            .refresh_token
            .clone()
            .ok_or("Microsoft didn't allow staying signed in.")?;
        device(&handle)
            .keychain_set(REFRESH_KEY, &refresh)
            .map_err(err)?;
        let od = handle.state::<OneDrive>();
        let account = Graph::new(&tokens.access_token).account_name().ok();
        *od.tokens.lock().unwrap() = Some(tokens);
        *od.account.lock().unwrap() = account;
        *od.last_error.lock().unwrap() = None;
        od.signed_in.store(true, Ordering::SeqCst);
        Ok(true)
    })
    .await?;
    if signed_in {
        start_sync(&app);
    }
    Ok(signed_in)
}

/// Forgets the account on this phone. OneDrive folders stay in the list
/// (unavailable) until removed or signed in again.
#[tauri::command]
pub fn onedrive_sign_out(app: AppHandle, od: State<OneDrive>) -> CmdResult<()> {
    device(&app).keychain_delete(REFRESH_KEY).map_err(err)?;
    *od.tokens.lock().unwrap() = None;
    *od.account.lock().unwrap() = None;
    od.signed_in.store(false, Ordering::SeqCst);
    Ok(())
}

// MARK: - Commands: folders

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderItem {
    id: String,
    name: String,
}

/// Subfolders of a OneDrive folder (`None`: the top level), for choosing.
#[tauri::command]
pub async fn onedrive_folders(id: Option<String>, app: AppHandle) -> CmdResult<Vec<FolderItem>> {
    blocking(move || {
        let graph = app.state::<OneDrive>().graph(&app)?;
        Ok(graph
            .folders(id.as_deref())
            .map_err(err)?
            .into_iter()
            .map(|i| FolderItem {
                id: i.id,
                name: i.name,
            })
            .collect())
    })
    .await
}

#[tauri::command]
pub fn add_onedrive_folder(item_id: String, name: String, app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    {
        let mut saved = state.saved.lock().unwrap();
        if saved.saved.remote.iter().any(|f| f.item_id == item_id) {
            return Err(format!("“{name}” is already in the library."));
        }
        saved.saved.remote.push(RemoteFolder {
            id: uuid_like(),
            name,
            item_id,
        });
        saved.save();
    }
    start_sync(&app);
    Ok(())
}

#[tauri::command]
pub fn set_onedrive_inbox(item_id: String, name: String, state: State<AppState>) {
    let mut saved = state.saved.lock().unwrap();
    saved.saved.onedrive_inbox = Some(RemoteInbox { item_id, name });
    saved.saved.inbox = None;
    saved.save();
}

fn uuid_like() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("onedrive-{nanos:x}")
}

// MARK: - Indexing

/// Brings the index up to date with every OneDrive folder, in the
/// background. A request while one runs makes it run once more.
pub fn start_sync(app: &AppHandle) {
    let od = app.state::<OneDrive>();
    if !od.is_signed_in() || remote_folders(app).is_empty() {
        return;
    }
    od.again.store(true, Ordering::SeqCst);
    if od.syncing.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("rupert-onedrive".into())
        .spawn(move || {
            let od = app.state::<OneDrive>();
            while od.again.swap(false, Ordering::SeqCst) {
                let result = sync_all(&app);
                *od.last_error.lock().unwrap() = result.err();
            }
            od.syncing.store(false, Ordering::SeqCst);
            report(&app, Progress::default());
        });
    if spawned.is_err() {
        od.syncing.store(false, Ordering::SeqCst);
    }
}

fn report(app: &AppHandle, p: Progress) {
    *app.state::<OneDrive>().progress.lock().unwrap() = p.clone();
    let _ = app.emit(REMOTE_PROGRESS, p);
}

fn sync_all(app: &AppHandle) -> Result<(), String> {
    let od = app.state::<OneDrive>();
    let graph = od.graph(app)?;
    if od.account.lock().unwrap().is_none() {
        *od.account.lock().unwrap() = graph.account_name().ok();
    }
    let index = Index::open(&app.state::<AppState>().index_path).map_err(err)?;
    for folder in remote_folders(app) {
        report(
            app,
            Progress {
                active: true,
                current: Some(folder.name.clone()),
                ..Default::default()
            },
        );
        let files = graph.list_all(&folder.item_id).map_err(err)?;
        let fetch = |rel: &str| graph.read(&folder.item_id, rel);
        let mut on_progress = |done: usize, total: usize| {
            report(
                app,
                Progress {
                    active: true,
                    done,
                    total,
                    current: Some(folder.name.clone()),
                    ..Default::default()
                },
            )
        };
        let result = remote::sync(&index, &folder.source(), &files, &fetch, &mut on_progress)
            .map_err(err)?;
        if result.indexed + result.removed > 0 {
            let _ = app.emit(INDEX_CHANGED, ());
        }
    }
    Ok(())
}

// MARK: - Opening and uploading

/// Downloads a document (unless cached and unchanged) and returns its path.
pub fn fetch_document(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_path: &str,
    size: u64,
) -> Result<PathBuf, String> {
    let cache = cache_dir(app)?.join(&folder.id);
    let dest = rupert_core::resolve(&cache, rel_path).map_err(err)?;
    let cached = std::fs::metadata(&dest).is_ok_and(|m| m.len() == size);
    if !cached {
        let graph = app.state::<OneDrive>().graph(app)?;
        graph
            .download(&folder.item_id, rel_path, &dest)
            .map_err(err)?;
    }
    Ok(dest)
}

/// Uploads a finished scan; the name OneDrive gave it.
pub fn upload_scan(
    app: &AppHandle,
    inbox: &RemoteInbox,
    name: &str,
    pdf: &[u8],
) -> Result<String, String> {
    let graph = app.state::<OneDrive>().graph(app)?;
    graph
        .upload(&inbox.item_id, &format!("{name}.pdf"), pdf)
        .map_err(err)
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_cache_dir().map_err(err)?.join("onedrive"))
}

/// Removes opened documents not used for a while.
pub fn tidy_cache(app: &AppHandle) {
    let Ok(dir) = cache_dir(app) else { return };
    let max_age = Duration::from_secs(CACHE_DAYS * 24 * 3600);
    fn walk(dir: &Path, max_age: Duration) {
        for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = e.path();
            if path.is_dir() {
                walk(&path, max_age);
                let _ = std::fs::remove_dir(&path); // only if empty
            } else if e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age > max_age)
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    walk(&dir, max_age);
}
