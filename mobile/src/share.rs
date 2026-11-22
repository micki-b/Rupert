//! Documents shared with Rupert from other apps go to the inbox (#42).
//!
//! - **iPhone:** Rupert's share extension (`RupertShare` in `gen/apple`)
//!   saves them to the inbox itself, without opening the app. It finds the
//!   inbox in a settings file in the app group, which [`publish`] keeps up
//!   to date, and uses the app's sign-ins (the keychain is shared). What it
//!   can't save (offline, say) it keeps in the app group's `Pending` folder.
//! - **Android:** sharing opens Rupert; the plugin copies what was shared
//!   into the cache's `shared` folder.
//!
//! Either way, [`send_shared`] then sends what's waiting, whenever the app
//! comes to the front.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use rupert_core::i18n::{self, tr};
use rupert_core::{FileKind, Job};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::cloud::{self, RemoteInbox, Service};
use crate::{AppState, CmdResult, SavedFolder, blocking, device, err, gdrive, onedrive};

/// The extension's settings file, in the app group.
const SETTINGS_FILE: &str = "share-settings.json";

/// One send at a time (the app can come to the front twice quickly).
static SENDING: AtomicBool = AtomicBool::new(false);

// MARK: - Settings for the share extension

/// What the share extension needs to save to the inbox.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareSettings<'a> {
    inbox: Option<Inbox<'a>>,
    /// Rupert's language ("en", "es", "de").
    language: &'a str,
    onedrive_client_id: Option<&'a str>,
    google_client_id: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum Inbox<'a> {
    /// A folder picked in Files; the bookmark reopens it.
    #[serde(rename_all = "camelCase")]
    Files { name: &'a str, bookmark: &'a str },
    #[serde(rename_all = "camelCase")]
    Onedrive { name: &'a str, item_id: &'a str },
    #[serde(rename_all = "camelCase")]
    Googledrive { name: &'a str, item_id: &'a str },
}

/// Writes the share extension's settings (iPhone only): call whenever the
/// inbox or the language changes.
pub fn publish(app: &AppHandle) {
    let Ok(folders) = device(app).shared_folders() else {
        return;
    };
    let Some(dir) = folders.settings else { return };
    let state = app.state::<AppState>();
    let saved = state.saved.lock().unwrap();
    let s = &saved.saved;
    let inbox = match (&s.remote_inbox, &s.inbox) {
        (Some(r), _) => Some(match r.service {
            Service::OneDrive => Inbox::Onedrive {
                name: &r.name,
                item_id: &r.item_id,
            },
            Service::GoogleDrive => Inbox::Googledrive {
                name: &r.name,
                item_id: &r.item_id,
            },
        }),
        (None, Some(f)) => Some(Inbox::Files {
            name: &f.name,
            bookmark: &f.bookmark,
        }),
        (None, None) => None,
    };
    let settings = ShareSettings {
        inbox,
        language: i18n::current().code(),
        onedrive_client_id: onedrive::client_id(),
        google_client_id: gdrive::client_id(),
    };
    let path = Path::new(&dir).join(SETTINGS_FILE);
    let tmp = path.with_extension("json.tmp");
    let written = serde_json::to_vec_pretty(&settings)
        .map_err(err)
        .and_then(|json| fs::write(&tmp, json).map_err(err))
        .and_then(|_| fs::rename(&tmp, &path).map_err(err));
    if let Err(e) = written {
        eprintln!("rupert: couldn't write the share extension's settings: {e}");
    }
}

// MARK: - Sending what's waiting

/// What [`send_shared`] did.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sent {
    /// The names the documents got in the inbox.
    pub sent: Vec<String>,
    /// Why some couldn't be sent; they stay and are tried again next time.
    pub problems: Vec<String>,
    /// Files that aren't documents, removed without sending.
    pub refused: Vec<String>,
}

/// Sends documents shared from other apps that are waiting on the phone to
/// the inbox. Nothing waiting (or not a phone): nothing sent.
#[tauri::command]
pub async fn send_shared(app: AppHandle) -> CmdResult<Sent> {
    if SENDING.swap(true, Ordering::SeqCst) {
        return Ok(Sent::default());
    }
    let handle = app.clone();
    let result = blocking(move || Ok(send_waiting(&handle))).await;
    SENDING.store(false, Ordering::SeqCst);
    result
}

/// The documents waiting: `Pending/<id>/<name>`, oldest first.
fn waiting(pending: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(pending)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let time = e
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            (time, e.path())
        })
        .collect();
    dirs.sort();
    let mut files = Vec::new();
    for (_, dir) in dirs {
        let file = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_file() && !is_hidden(p));
        match file {
            Some(f) => files.push(f),
            // Left empty by an interrupted save.
            None => {
                let _ = fs::remove_dir_all(&dir);
            }
        }
    }
    files
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().starts_with('.'))
}

fn send_waiting(app: &AppHandle) -> Sent {
    let mut out = Sent::default();
    let Ok(folders) = device(app).shared_folders() else {
        return out;
    };
    let files = waiting(Path::new(&folders.pending));
    if files.is_empty() {
        return out;
    }
    let state = app.state::<AppState>();
    let (inbox, remote) = {
        let saved = state.saved.lock().unwrap();
        (saved.saved.inbox.clone(), saved.saved.remote_inbox.clone())
    };
    if inbox.is_none() && remote.is_none() {
        out.problems
            .push(tr("Choose the inbox folder first.").into());
        return out;
    }
    for file in files {
        let name = file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !FileKind::from_path(&file).is_document() {
            let _ = fs::remove_dir_all(file.parent().unwrap_or(&file));
            out.refused.push(name);
            continue;
        }
        match send_one(app, &file, &name, remote.as_ref(), inbox.as_ref()) {
            Ok(saved_as) => out.sent.push(saved_as),
            Err(e) => out.problems.push(format!("{name}: {e}")),
        }
    }
    if let Some(r) = &remote
        && !out.sent.is_empty()
    {
        // Show them in the inbox once the service lists them.
        cloud::start_sync(app, r.service);
    }
    out
}

/// Sends one waiting document and removes it (with its folder) from the
/// waiting place. The name it got in the inbox.
fn send_one(
    app: &AppHandle,
    file: &Path,
    name: &str,
    remote: Option<&RemoteInbox>,
    inbox: Option<&SavedFolder>,
) -> Result<String, String> {
    if let Some(remote) = remote {
        let bytes = fs::read(file).map_err(err)?;
        let saved_as = cloud::upload(app, remote, name, &bytes)?;
        let _ = fs::remove_dir_all(file.parent().unwrap_or(file));
        return Ok(saved_as);
    }
    let inbox = inbox.expect("an inbox");
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| name.to_string());
    // `place` copies it under a free name and removes the waiting folder.
    let placed = device(app)
        .place(
            &file.to_string_lossy(),
            &inbox.path.to_string_lossy(),
            &stem,
        )
        .map_err(err)?;
    // Show it right away if the inbox is inside a library folder.
    let state = app.state::<AppState>();
    if let Some((source, rel_path)) = state.locate(Path::new(&placed.path)) {
        state.indexer.send(Job::File { source, rel_path });
    }
    Ok(placed.name)
}
