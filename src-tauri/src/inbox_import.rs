//! Import into Inbox (#122): the Import window's commands. The page walks
//! the list one document at a time (so it can show progress and stop), and
//! each call copies one document into the inbox, or says it's already in
//! the library. The logic is `rupert_core::import`.

use std::path::{Path, PathBuf};

use rupert_core::Job;
use rupert_core::i18n::tr;
use rupert_core::import::{self, Plan};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{AppState, CmdResult, err};

/// What happened to one document.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Imported {
    /// Copied into the inbox, at `rel_path` of the inbox's source.
    #[serde(rename_all = "camelCase")]
    Copied { rel_path: String },
    /// Already in the library there: not copied.
    #[serde(rename_all = "camelCase")]
    Duplicate { source_id: String, rel_path: String },
    /// Not a document Rupert takes (or hidden): left out.
    LeftOut,
}

/// The inbox's source and its folder on disk.
fn inbox(state: &AppState) -> CmdResult<(rupert_core::Source, PathBuf)> {
    let inbox = state
        .settings
        .lock()
        .unwrap()
        .get()
        .inbox
        .clone()
        .ok_or(tr("Choose the inbox folder first."))?;
    let source = state.source(&inbox.source_id)?;
    let dir = rupert_core::resolve(&source.root, &inbox.rel_path).map_err(err)?;
    Ok((source, dir))
}

/// What the chosen files and folders bring in.
#[tauri::command]
pub async fn inbox_import_plan(paths: Vec<String>) -> CmdResult<Plan> {
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    tauri::async_runtime::spawn_blocking(move || import::plan(&paths))
        .await
        .map_err(err)
}

/// Copies the document at `path` into the inbox, unless it's already in
/// the library.
#[tauri::command]
pub async fn inbox_import_file(path: String, app: AppHandle) -> CmdResult<Imported> {
    let from = PathBuf::from(&path);
    let name = from
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !import::takes(&name) {
        return Ok(Imported::LeftOut);
    }
    let size = std::fs::metadata(&from).map_err(err)?.len();
    let handle = app.clone();
    let (n, f) = (name.clone(), from.clone());
    let found = tauri::async_runtime::spawn_blocking(move || {
        already_there(&handle, size, &n, &|theirs| {
            import::same_file_contents(&f, theirs).unwrap_or(false)
        })
    })
    .await
    .map_err(err)??;
    if let Some((source_id, rel_path)) = found {
        return Ok(Imported::Duplicate {
            source_id,
            rel_path,
        });
    }
    let state = app.state::<AppState>();
    let (source, dir) = inbox(&state)?;
    let copied = tauri::async_runtime::spawn_blocking(move || import::copy_file_in(&from, &dir))
        .await
        .map_err(err)?
        .map_err(err)?;
    arrived(&state, source, &copied)
}

/// Where a dropped file goes (the `rupert-import` header, JSON with
/// non-ASCII characters escaped).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Dropped {
    name: String,
    modified_ms: Option<u64>,
}

/// Copies a file dropped on the Import window (its contents are the
/// request's body) into the inbox, unless it's already in the library.
#[tauri::command]
pub async fn inbox_import_dropped(
    request: tauri::ipc::Request<'_>,
    app: AppHandle,
) -> CmdResult<Imported> {
    let tauri::ipc::InvokeBody::Raw(contents) = request.body() else {
        return Err("expected the file's contents".into());
    };
    let dropped: Dropped = request
        .headers()
        .get("rupert-import")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| serde_json::from_str(h).ok())
        .ok_or("expected the file's name")?;
    if !import::takes(&dropped.name) {
        return Ok(Imported::LeftOut);
    }
    let contents = contents.clone();
    let handle = app.clone();
    let (name, bytes) = (dropped.name.clone(), contents.clone());
    let found = tauri::async_runtime::spawn_blocking(move || {
        already_there(&handle, bytes.len() as u64, &name, &|theirs| {
            import::same_contents(&bytes, theirs).unwrap_or(false)
        })
    })
    .await
    .map_err(err)??;
    if let Some((source_id, rel_path)) = found {
        return Ok(Imported::Duplicate {
            source_id,
            rel_path,
        });
    }
    let state = app.state::<AppState>();
    let (source, dir) = inbox(&state)?;
    let modified = dropped
        .modified_ms
        .map(|ms| std::time::UNIX_EPOCH + std::time::Duration::from_millis(ms));
    let copied = tauri::async_runtime::spawn_blocking(move || {
        rupert_core::fileops::copy_in(&dir, &dropped.name, &contents, modified, true)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    arrived(&state, source, &copied)
}

fn already_there(
    app: &AppHandle,
    size: u64,
    name: &str,
    same: &dyn Fn(&Path) -> bool,
) -> CmdResult<Option<(String, String)>> {
    let state = app.state::<AppState>();
    let sources = state.sources();
    let candidates = import::same_size(&state.index.lock().unwrap(), size).map_err(err)?;
    Ok(import::find_in_library(candidates, &sources, name, same))
}

/// Tells the indexer and the inbox about a document that just arrived.
fn arrived(state: &AppState, source: rupert_core::Source, path: &Path) -> CmdResult<Imported> {
    let rel_path =
        rupert_core::listing::relative(&source.root, path).ok_or_else(|| err(path.display()))?;
    state.indexer.send(Job::File {
        source,
        rel_path: rel_path.clone(),
    });
    state.intake.refresh();
    Ok(Imported::Copied { rel_path })
}
