//! Google Drive folders on the phone, through the Drive API. The Google
//! Drive app doesn't hand its folders to other apps through Files (they're
//! greyed out in the picker), so these sources are listed through the web
//! API instead (`rupert_core::gdrive`) and indexed from Rupert's catalog
//! and sidecars (`rupert_core::remote`), as OneDrive's are (`onedrive`).
//!
//! A preview: Rupert's Google project isn't verified by Google (that needs
//! a yearly security assessment), so the sign-in shows Google's
//! "unverified app" warning and at most 100 people can use it.
//!
//! The refresh token lives in the Keychain; the access token and each
//! folder's last listing (Drive works by id, Rupert by path) only in memory.

use rupert_core::i18n::{tr, trf};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use rupert_core::folder_info::{self, FolderInfo};
use rupert_core::gdrive::{self, Drive, Listing, Pkce, Tokens};
use rupert_core::{Index, Progress, remote};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::cloud::{FolderItem, RemoteFolder, RemoteInbox, Service};
use crate::onedrive::REMOTE_PROGRESS;
use crate::{AppState, CmdResult, INDEX_CHANGED, blocking, device, err};

/// The app's iOS client with Google (public: not a secret). Read from
/// `mobile/google-client-id` at build time.
const CLIENT_ID: Option<&str> = option_env!("RUPERT_GOOGLE_CLIENT_ID");
const REFRESH_KEY: &str = "gdrive.refresh-token";
fn not_set_up() -> &'static str {
    tr("Google Drive isn't set up in this build of Rupert.")
}

pub(crate) fn client_id() -> Option<&'static str> {
    CLIENT_ID.map(str::trim).filter(|s| !s.is_empty())
}

/// Whether Google Drive can be used in this build. On Android, Google Play
/// services signs in (the Android OAuth client is matched by package name
/// and signing key, not an id in the app); on iOS it needs the client id.
fn configured() -> bool {
    cfg!(target_os = "android") || client_id().is_some()
}

/// Android: a marker that the person signed in (there's no refresh token
/// to keep; Play services hands out access tokens).
#[cfg(target_os = "android")]
const SIGNED_IN_KEY: &str = "gdrive-signed-in";

/// Android: an access token from Google Play services, kept for 50 minutes
/// (Google's last about an hour). `None` if the person declined; an error
/// saying why when Google refused (#53).
#[cfg(target_os = "android")]
fn android_tokens(app: &AppHandle) -> Result<Option<Tokens>, String> {
    use tauri_plugin_rupert_phone::GoogleToken;
    match device(app).google_token(gdrive::SCOPES).map_err(err)? {
        GoogleToken::Token(access_token) => Ok(Some(Tokens {
            access_token,
            refresh_token: None,
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(50 * 60),
        })),
        GoogleToken::Declined => Ok(None),
        GoogleToken::Refused { code, status } => Err(refused(code, &status)),
    }
}

/// Why Google refused a sign-in on Android, from its status code
/// (`CommonStatusCodes`) and text.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn refused(code: i32, status: &str) -> String {
    const NETWORK_ERROR: i32 = 7;
    const DEVELOPER_ERROR: i32 = 10;
    if code == DEVELOPER_ERROR || status.contains("UNREGISTERED_ON_API_CONSOLE") {
        tr("Google doesn't recognize this copy of Rupert, so it can't sign in. Please tell us at contact@rupertlibrarian.com.").into()
    } else if code == NETWORK_ERROR {
        tr("Couldn't reach Google. Check the internet connection and try again.").into()
    } else {
        trf(
            "Google didn't let Rupert sign in: {0} (code {1}).",
            &[&status.trim(), &code],
        )
    }
}

// MARK: - State

#[derive(Default)]
pub struct GoogleDrive {
    tokens: Mutex<Option<Tokens>>,
    account: Mutex<Option<String>>,
    /// A refresh token is stored.
    signed_in: AtomicBool,
    syncing: AtomicBool,
    again: AtomicBool,
    progress: Mutex<Progress>,
    last_error: Mutex<Option<String>>,
    /// Each library folder's last listing, by source id.
    listings: Mutex<HashMap<String, Listing>>,
}

impl GoogleDrive {
    pub fn is_signed_in(&self) -> bool {
        self.signed_in.load(Ordering::SeqCst)
    }

    /// A connection with a current access token, refreshing it if needed.
    pub fn drive(&self, app: &AppHandle) -> Result<Drive, String> {
        let mut tokens = self.tokens.lock().unwrap();
        if let Some(t) = tokens.as_ref().filter(|t| t.is_fresh()) {
            return Ok(Drive::new(&t.access_token));
        }
        #[cfg(target_os = "android")]
        {
            let fresh =
                android_tokens(app)?.ok_or(tr("Sign in to Google Drive first (Settings)."))?;
            let drive = Drive::new(&fresh.access_token);
            *tokens = Some(fresh);
            return Ok(drive);
        }
        #[allow(unreachable_code)]
        let client = client_id().ok_or(not_set_up())?;
        let refresh = device(app)
            .keychain_get(REFRESH_KEY)
            .map_err(err)?
            .ok_or(tr("Sign in to Google Drive first (Settings)."))?;
        let fresh = gdrive::refresh(client, &refresh).map_err(err)?;
        let drive = Drive::new(&fresh.access_token);
        *tokens = Some(fresh);
        Ok(drive)
    }

    /// Checks the Keychain once at launch.
    pub fn restore(&self, app: &AppHandle) {
        #[cfg(target_os = "android")]
        let stored = device(app).keychain_get(SIGNED_IN_KEY).ok().flatten();
        #[cfg(not(target_os = "android"))]
        let stored = device(app).keychain_get(REFRESH_KEY).ok().flatten();
        self.signed_in.store(stored.is_some(), Ordering::SeqCst);
    }

    fn listing(&self, folder: &RemoteFolder) -> Listing {
        self.listings
            .lock()
            .unwrap()
            .get(&folder.id)
            .cloned()
            .unwrap_or_default()
    }
}

/// The library's Google Drive folders.
fn drive_folders(app: &AppHandle) -> Vec<RemoteFolder> {
    let state = app.state::<AppState>();
    let saved = state.saved.lock().unwrap();
    saved
        .saved
        .remote
        .iter()
        .filter(|f| f.service == Service::GoogleDrive)
        .cloned()
        .collect()
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
pub fn gdrive_status(gd: State<GoogleDrive>) -> Status {
    Status {
        configured: configured(),
        signed_in: gd.is_signed_in(),
        account: gd.account.lock().unwrap().clone(),
        error: gd.last_error.lock().unwrap().clone(),
        progress: gd.progress.lock().unwrap().clone(),
    }
}

/// Shows Google's sign-in page. `false` if cancelled.
#[tauri::command]
pub async fn gdrive_sign_in(app: AppHandle) -> CmdResult<bool> {
    let handle = app.clone();
    let signed_in = blocking(move || {
        #[cfg(target_os = "android")]
        {
            let Some(tokens) = android_tokens(&handle)? else {
                return Ok(false);
            };
            device(&handle)
                .keychain_set(SIGNED_IN_KEY, "1")
                .map_err(err)?;
            let gd = handle.state::<GoogleDrive>();
            let account = Drive::new(&tokens.access_token).account_name().ok();
            *gd.tokens.lock().unwrap() = Some(tokens);
            *gd.account.lock().unwrap() = account;
            *gd.last_error.lock().unwrap() = None;
            gd.signed_in.store(true, Ordering::SeqCst);
            return Ok(true);
        }
        #[allow(unreachable_code)]
        let client = client_id().ok_or(not_set_up())?;
        let (redirect, scheme) = gdrive::redirect_for(client).ok_or(not_set_up())?;
        let pkce = Pkce::new();
        let url = gdrive::authorize_url(client, &redirect, &pkce);
        let Some(answer) = device(&handle).sign_in(&url, &scheme).map_err(err)? else {
            return Ok(false);
        };
        let code = gdrive::code_from_redirect(&answer, &pkce).map_err(err)?;
        let tokens = gdrive::redeem_code(client, &redirect, &code, &pkce).map_err(err)?;
        let refresh = tokens
            .refresh_token
            .clone()
            .ok_or(tr("Google didn't allow staying signed in."))?;
        device(&handle)
            .keychain_set(REFRESH_KEY, &refresh)
            .map_err(err)?;
        let gd = handle.state::<GoogleDrive>();
        let account = Drive::new(&tokens.access_token).account_name().ok();
        *gd.tokens.lock().unwrap() = Some(tokens);
        *gd.account.lock().unwrap() = account;
        *gd.last_error.lock().unwrap() = None;
        gd.signed_in.store(true, Ordering::SeqCst);
        Ok(true)
    })
    .await?;
    if signed_in {
        start_sync(&app);
    }
    Ok(signed_in)
}

/// Forgets the account on this phone. Google Drive folders stay in the list
/// (unavailable) until removed or signed in again.
#[tauri::command]
pub fn gdrive_sign_out(app: AppHandle, gd: State<GoogleDrive>) -> CmdResult<()> {
    #[cfg(target_os = "android")]
    device(&app).keychain_delete(SIGNED_IN_KEY).map_err(err)?;
    device(&app).keychain_delete(REFRESH_KEY).map_err(err)?;
    *gd.tokens.lock().unwrap() = None;
    *gd.account.lock().unwrap() = None;
    gd.listings.lock().unwrap().clear();
    gd.signed_in.store(false, Ordering::SeqCst);
    Ok(())
}

// MARK: - Commands: folders

/// Subfolders of a Google Drive folder (`None`: the top of My Drive), for
/// choosing.
#[tauri::command]
pub async fn gdrive_folders(id: Option<String>, app: AppHandle) -> CmdResult<Vec<FolderItem>> {
    blocking(move || {
        let drive = app.state::<GoogleDrive>().drive(&app)?;
        Ok(drive
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
pub fn add_gdrive_folder(item_id: String, name: String, app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    {
        let mut saved = state.saved.lock().unwrap();
        if saved
            .saved
            .remote
            .iter()
            .any(|f| f.service == Service::GoogleDrive && f.item_id == item_id)
        {
            return Err(trf("“{0}” is already in the library.", &[&name]));
        }
        saved
            .saved
            .remote
            .push(RemoteFolder::new(Service::GoogleDrive, item_id, name));
        saved.save();
    }
    crate::reconcile_inbox(&app);
    start_sync(&app);
    Ok(())
}

/// Makes a Google Drive folder the inbox. `path`: the folders from the top
/// of My Drive down to it, as browsed.
#[tauri::command]
pub fn set_gdrive_inbox(item_id: String, name: String, path: Vec<FolderItem>, app: AppHandle) {
    {
        let state = app.state::<AppState>();
        let mut saved = state.saved.lock().unwrap();
        saved.saved.remote_inbox = Some(RemoteInbox {
            item_id,
            name,
            path,
            service: Service::GoogleDrive,
        });
        saved.saved.inbox = None;
        saved.save();
    }
    crate::reconcile_inbox(&app);
}

// MARK: - Indexing

/// Brings the index up to date with every Google Drive folder, in the
/// background. A request while one runs makes it run once more.
pub fn start_sync(app: &AppHandle) {
    let gd = app.state::<GoogleDrive>();
    if !gd.is_signed_in() || drive_folders(app).is_empty() {
        return;
    }
    gd.again.store(true, Ordering::SeqCst);
    if gd.syncing.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("rupert-gdrive".into())
        .spawn(move || {
            let gd = app.state::<GoogleDrive>();
            while gd.again.swap(false, Ordering::SeqCst) {
                let result = sync_all(&app);
                *gd.last_error.lock().unwrap() = result.err();
            }
            gd.syncing.store(false, Ordering::SeqCst);
            report(&app, Progress::default());
        });
    if spawned.is_err() {
        gd.syncing.store(false, Ordering::SeqCst);
    }
}

fn report(app: &AppHandle, p: Progress) {
    *app.state::<GoogleDrive>().progress.lock().unwrap() = p.clone();
    let _ = app.emit(REMOTE_PROGRESS, p);
}

fn sync_all(app: &AppHandle) -> Result<(), String> {
    let gd = app.state::<GoogleDrive>();
    let drive = gd.drive(app)?;
    if gd.account.lock().unwrap().is_none() {
        *gd.account.lock().unwrap() = drive.account_name().ok();
    }
    let index = Index::open(&app.state::<AppState>().index_path).map_err(err)?;
    for folder in drive_folders(app) {
        let busy = |done: usize, total: usize| Progress {
            active: true,
            done,
            total,
            current: Some(folder.name.clone()),
            ..Default::default()
        };
        report(app, busy(0, 0));
        let listing = drive.list_all(&folder.item_id).map_err(err)?;
        let fetch = |rel: &str| match listing.file_ids.get(rel) {
            Some(id) => drive.read(id),
            None => Err(rupert_core::Error::InvalidPath(rel.to_string())),
        };
        let mut on_progress = |done: usize, total: usize| report(app, busy(done, total));
        let result = remote::sync(
            &index,
            &folder.source(),
            &listing.files,
            &fetch,
            &mut on_progress,
        )
        .map_err(err)?;
        gd.listings
            .lock()
            .unwrap()
            .insert(folder.id.clone(), listing);
        if result.indexed + result.removed > 0 {
            let _ = app.emit(INDEX_CHANGED, ());
        }
    }
    Ok(())
}

// MARK: - Opening and writing

/// The id of a document, from the folder's last listing (listed afresh if
/// it isn't there yet, e.g. right after launch).
fn file_id(
    app: &AppHandle,
    drive: &Drive,
    folder: &RemoteFolder,
    rel_path: &str,
) -> Result<String, String> {
    let gd = app.state::<GoogleDrive>();
    if let Some(id) = gd.listing(folder).file_ids.get(rel_path) {
        return Ok(id.clone());
    }
    let listing = drive.list_all(&folder.item_id).map_err(err)?;
    let id = listing.file_ids.get(rel_path).cloned();
    gd.listings
        .lock()
        .unwrap()
        .insert(folder.id.clone(), listing);
    id.ok_or_else(|| trf("“{0}” isn't in Google Drive any more.", &[&rel_path]))
}

/// Downloads a document the user opened into `dest`.
pub fn download(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_path: &str,
    dest: &Path,
) -> Result<(), String> {
    let drive = app.state::<GoogleDrive>().drive(app)?;
    let id = file_id(app, &drive, folder, rel_path)?;
    drive.download(&id, dest).map_err(err)
}

/// Google Drive's picture of a document; see [`crate::cloud::thumbnail`].
pub fn thumbnail(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_path: &str,
    side: u32,
) -> Result<Option<Vec<u8>>, String> {
    let drive = app.state::<GoogleDrive>().drive(app)?;
    let id = file_id(app, &drive, folder, rel_path)?;
    drive.thumbnail(&id, side).map_err(err)
}

/// Writes (or removes) a Google Drive folder's own details file.
pub fn write_folder_info(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_dir: &str,
    info: &FolderInfo,
) -> Result<(), String> {
    let gd = app.state::<GoogleDrive>();
    let drive = gd.drive(app)?;
    let listing = gd.listing(folder);
    let rel = folder_info::rel_path_for(rel_dir);
    if info.is_empty() {
        match drive.find(&folder.item_id, &listing, &rel).map_err(err)? {
            Some(id) => drive.delete_small(&id).map_err(err),
            None => Ok(()),
        }
    } else {
        drive
            .write_small(
                &folder.item_id,
                &listing,
                &rel,
                &folder_info::to_bytes(info),
            )
            .map_err(err)
    }
}

/// Uploads a file to the inbox (never replacing one); the name it got.
pub fn upload(
    app: &AppHandle,
    inbox: &RemoteInbox,
    name: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let drive = app.state::<GoogleDrive>().drive(app)?;
    drive.upload(&inbox.item_id, name, bytes).map_err(err)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    /// Google's refusals say why instead of looking like a cancel (#53).
    #[test]
    fn explains_why_google_refused() {
        rupert_core::i18n::set(rupert_core::i18n::Lang::En);
        assert!(refused(8, "UNREGISTERED_ON_API_CONSOLE").contains("doesn't recognize"));
        assert!(refused(10, "").contains("doesn't recognize"));
        assert!(refused(7, "NETWORK_ERROR").contains("Couldn't reach Google"));
        assert_eq!(
            refused(13, "ERROR "),
            "Google didn't let Rupert sign in: ERROR (code 13)."
        );
    }
}
