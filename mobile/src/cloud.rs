//! Library folders the phone reaches through a cloud service's web API,
//! because the service's own app doesn't hand its folders to other apps
//! through Files: OneDrive (`onedrive`, Microsoft Graph) and Google Drive
//! (`gdrive`, the Drive API). Both are indexed from Rupert's catalog and
//! sidecars (`rupert_core::remote`); documents download only when opened.
//!
//! This module holds what both share and sends each call to the right one.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use rupert_core::folder_info::FolderInfo;
use rupert_core::{Provider, Source};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{gdrive, onedrive};

/// Opened documents are kept this long, then removed from the cache.
const CACHE_DAYS: u64 = 14;

// MARK: - Types

/// The service a remote folder lives on. Saved without one means OneDrive
/// (the only one before Google Drive).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Service {
    #[default]
    OneDrive,
    GoogleDrive,
}

impl Service {
    fn key(self) -> &'static str {
        match self {
            Self::OneDrive => "onedrive",
            Self::GoogleDrive => "googledrive",
        }
    }

    pub fn provider(self) -> Provider {
        match self {
            Self::OneDrive => Provider::OneDrive,
            Self::GoogleDrive => Provider::GoogleDrive,
        }
    }

    /// Whether this phone is signed in to the service.
    pub fn signed_in(self, app: &AppHandle) -> bool {
        match self {
            Self::OneDrive => app.state::<onedrive::OneDrive>().is_signed_in(),
            Self::GoogleDrive => app.state::<gdrive::GoogleDrive>().is_signed_in(),
        }
    }
}

/// A folder in the library, on a cloud service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFolder {
    /// Rupert's source id.
    pub id: String,
    pub name: String,
    /// The service's id for the folder.
    pub item_id: String,
    #[serde(default)]
    pub service: Service,
}

impl RemoteFolder {
    pub fn new(service: Service, item_id: String, name: String) -> Self {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        Self {
            id: format!("{}-{nanos:x}", service.key()),
            name,
            item_id,
            service,
        }
    }

    pub fn source(&self) -> Source {
        Source {
            id: self.id.clone(),
            name: self.name.clone(),
            root: PathBuf::from(format!("{}:{}", self.service.key(), self.item_id)),
            provider: self.service.provider(),
            show_all_types: false,
            download_online_only: false,
            offline: false,
            bookmark: None,
        }
    }
}

/// A folder on a cloud service that scans are uploaded to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInbox {
    pub item_id: String,
    pub name: String,
    /// The folders from the top of the drive down to the inbox itself, as
    /// chosen (empty if chosen before Rupert kept it): tells whether it lies
    /// inside one of the library's folders on the same service.
    #[serde(default)]
    pub path: Vec<FolderItem>,
    #[serde(default)]
    pub service: Service,
}

/// A folder while choosing one.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderItem {
    pub id: String,
    pub name: String,
}

// MARK: - Calls by service

/// Brings the index up to date with the folders on one service, in the
/// background.
pub fn start_sync(app: &AppHandle, service: Service) {
    match service {
        Service::OneDrive => onedrive::start_sync(app),
        Service::GoogleDrive => gdrive::start_sync(app),
    }
}

/// [`start_sync`] for every service.
pub fn start_all(app: &AppHandle) {
    onedrive::start_sync(app);
    gdrive::start_sync(app);
}

/// Downloads a document (unless cached and unchanged) and returns its path.
pub fn fetch_document(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_path: &str,
    size: u64,
) -> Result<PathBuf, String> {
    let dest =
        rupert_core::resolve(&cache_dir(app)?.join(&folder.id), rel_path).map_err(crate::err)?;
    if std::fs::metadata(&dest).is_ok_and(|m| m.len() == size) {
        return Ok(dest);
    }
    match folder.service {
        Service::OneDrive => onedrive::download(app, folder, rel_path, &dest)?,
        Service::GoogleDrive => gdrive::download(app, folder, rel_path, &dest)?,
    }
    Ok(dest)
}

/// The service's picture of a document (fitting in `side` pixels), or
/// `None` if it has none. Doesn't download the document.
pub fn thumbnail(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_path: &str,
    side: u32,
) -> Result<Option<Vec<u8>>, String> {
    match folder.service {
        Service::OneDrive => onedrive::thumbnail(app, folder, rel_path, side),
        Service::GoogleDrive => gdrive::thumbnail(app, folder, rel_path, side),
    }
}

/// Writes (or removes, if empty) a folder's own details file.
pub fn write_folder_info(
    app: &AppHandle,
    folder: &RemoteFolder,
    rel_dir: &str,
    info: &FolderInfo,
) -> Result<(), String> {
    match folder.service {
        Service::OneDrive => onedrive::write_folder_info(app, folder, rel_dir, info),
        Service::GoogleDrive => gdrive::write_folder_info(app, folder, rel_dir, info),
    }
}

/// Uploads a file to the inbox (never replacing one); the name the service
/// gave it.
pub fn upload(
    app: &AppHandle,
    inbox: &RemoteInbox,
    name: &str,
    bytes: &[u8],
) -> Result<String, String> {
    match inbox.service {
        Service::OneDrive => onedrive::upload(app, inbox, name, bytes),
        Service::GoogleDrive => gdrive::upload(app, inbox, name, bytes),
    }
}

// MARK: - Cache

/// Where opened documents are kept (per source, by path).
fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_cache_dir()
        .map_err(crate::err)?
        .join("onedrive"))
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
