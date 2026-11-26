//! Export… (#107): a set of documents copied into a folder or a zip, with a
//! list. The work is `rupert_core::export`; here the documents are looked up
//! and the page hears `export-progress` while it runs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use rupert_core::export::{self, ExportDoc, Options};
use rupert_core::tag_edit::DocRef;
use rupert_core::{CloudState, FileKind};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult, err};

/// Sent before each document: how far along, and its name.
const EXPORT_PROGRESS: &str = "export-progress";

/// Set by Cancel; checked between documents.
static STOP: AtomicBool = AtomicBool::new(false);

// MARK: - What would be exported

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub documents: usize,
    pub size: u64,
    /// Online-only documents, which copying downloads.
    pub online: usize,
    /// Notes go along by default as Share… sends them (Settings).
    pub include_notes: bool,
}

/// The documents in a folder and all its subfolders, as the index knows
/// them (for Export folder…).
#[tauri::command]
pub async fn export_folder_docs(
    source_id: String,
    rel_path: String,
    app: AppHandle,
) -> CmdResult<Vec<DocRef>> {
    let state = app.state::<AppState>();
    let stamps = state
        .index
        .lock()
        .unwrap()
        .stamps_under(&source_id, &rel_path)
        .map_err(err)?;
    let mut docs: Vec<DocRef> = stamps
        .into_keys()
        .map(|rel_path| DocRef {
            source_id: source_id.clone(),
            rel_path,
        })
        .collect();
    docs.sort_by_key(|d| d.rel_path.to_lowercase());
    Ok(docs)
}

#[tauri::command]
pub async fn export_plan(docs: Vec<DocRef>, app: AppHandle) -> CmdResult<Plan> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().get().clone();
    let index = state.index.lock().unwrap();
    let mut plan = Plan {
        documents: docs.len(),
        size: 0,
        online: 0,
        include_notes: settings.send_notes,
    };
    for d in &docs {
        if let Some(info) = index.document(&d.source_id, &d.rel_path).map_err(err)? {
            plan.size += info.hit.size;
            if info.hit.cloud == CloudState::CloudOnly {
                plan.online += 1;
            }
        }
    }
    Ok(plan)
}

// MARK: - Exporting

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
    name: String,
    /// It's online-only: copying it downloads it first.
    downloading: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Done {
    pub path: String,
    pub copied: usize,
    pub failed: Vec<(String, String)>,
}

/// Exports `docs` to `dest` (from the save dialog): a new folder, or a zip.
/// `title` heads the list.
#[tauri::command]
pub async fn export_run(
    docs: Vec<DocRef>,
    options: Options,
    dest: String,
    title: String,
    app: AppHandle,
) -> CmdResult<Done> {
    STOP.store(false, Ordering::SeqCst);
    let state = app.state::<AppState>();
    let cache = app.path().app_cache_dir().map_err(err)?;
    let mut list: Vec<ExportDoc> = Vec::with_capacity(docs.len());
    {
        let index = state.index.lock().unwrap();
        for d in &docs {
            let source = state.source(&d.source_id)?;
            let path = rupert_core::resolve(&source.root, &d.rel_path).map_err(err)?;
            let meta = index
                .doc_meta(&d.source_id, &d.rel_path)
                .map_err(err)?
                .unwrap_or_default();
            list.push(ExportDoc {
                source_name: source.name.clone(),
                rel_path: d.rel_path.clone(),
                path,
                meta,
            });
        }
    }
    let dest = PathBuf::from(dest);
    let out = dest.clone();
    let total = list.len();
    let include_notes = options.include_notes;
    let emitter = app.clone();
    let report = tauri::async_runtime::spawn_blocking(move || {
        export::write(
            &dest,
            &list,
            &options,
            &title,
            |i| {
                let d = &list[i];
                let downloading =
                    rupert_core::cloud_state(&d.path).ok() == Some(CloudState::CloudOnly);
                let _ = emitter.emit(
                    EXPORT_PROGRESS,
                    Progress {
                        done: i,
                        total,
                        name: d
                            .rel_path
                            .rsplit('/')
                            .next()
                            .unwrap_or_default()
                            .to_string(),
                        downloading,
                    },
                );
            },
            // A PDF without its notes unless they go along, as Share…
            // sends it; reading an online-only file downloads it.
            |d| {
                if FileKind::from_path(&d.path) == FileKind::Pdf {
                    rupert_core::share::prepare(&d.path, None, include_notes, &cache)
                } else {
                    Ok(d.path.clone())
                }
            },
            || STOP.load(Ordering::SeqCst),
        )
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    Ok(Done {
        path: out.to_string_lossy().to_string(),
        copied: report.copied,
        failed: report.failed,
    })
}

/// Show in Finder / File Explorer for the finished export.
#[tauri::command]
pub fn export_reveal(path: String, app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(path).map_err(err)
}

/// Stops the export in progress after the document being copied.
#[tauri::command]
pub fn export_cancel() {
    STOP.store(true, Ordering::SeqCst);
}
