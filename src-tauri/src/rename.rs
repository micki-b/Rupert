//! Rename documents… (#64): the dialog's commands. The logic is in
//! `rupert_core::rename`; this hands it the index and settings, runs Local
//! AI's proposals on a thread (one document at a time, each sent to the
//! page as it arrives, with Stop), and has the indexer follow the renames.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use rupert_core::Job;
use rupert_core::llm::Engine;
use rupert_core::rename::{self, Checked, Details, Example, Outcome, Planned, Renamed, Row, Step};
use rupert_core::{DocMeta, Source};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{AppState, CmdResult, err};

/// Sent for each proposal from Local AI.
const PROPOSAL: &str = "rename-proposal";
/// Sent when Local AI has proposed for every document, or was stopped.
const PROPOSALS_DONE: &str = "rename-proposals-done";

/// The running proposals' Stop switch, and the latest job's number.
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);
static JOB: AtomicU64 = AtomicU64::new(0);

// MARK: - The documents

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rows {
    rows: Vec<Row>,
    /// Whether Local AI can be used (Apple Intelligence, or a server on
    /// this computer or the local network).
    ai: bool,
}

/// The folder's documents, with names from the name pattern.
#[tauri::command]
pub async fn rename_rows(source_id: String, rel_dir: String, app: AppHandle) -> CmdResult<Rows> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let settings = state.settings.lock().unwrap().get().clone();
    let ai = Engine::from_settings(&settings).is_some();
    let app2 = app.clone();
    let rows = tauri::async_runtime::spawn_blocking(move || {
        let state = app2.state::<AppState>();
        let index = state.index.lock().unwrap();
        rename::rows(&source, &index, &settings, &rel_dir)
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    Ok(Rows { rows, ai })
}

/// What the names would really be: allowed, taken (" (2)"), unchanged.
#[tauri::command]
pub async fn rename_check(
    source_id: String,
    rel_dir: String,
    planned: Vec<Planned>,
    state: State<'_, AppState>,
) -> CmdResult<Vec<Checked>> {
    let source = state.source(&source_id)?;
    tauri::async_runtime::spawn_blocking(move || rename::check(&source, &rel_dir, &planned))
        .await
        .map_err(err)?
        .map_err(err)
}

/// A typed name and the document it's for.
#[derive(Deserialize)]
pub struct Typed {
    details: Details,
    name: String,
}

/// Names for `targets` the same way as the typed ones, if they follow a
/// pattern of details (`null` when they don't: ask Local AI instead).
#[tauri::command]
pub fn rename_same_way(typed: Vec<Typed>, targets: Vec<Details>) -> Option<Vec<Option<String>>> {
    let examples: Vec<(Details, String)> = typed.into_iter().map(|t| (t.details, t.name)).collect();
    rename::same_way(&examples, &targets)
}

// MARK: - Local AI

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Proposal {
    job: u64,
    rel_path: String,
    /// The new name, without extension.
    name: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProposalsDone {
    job: u64,
    stopped: bool,
}

/// Starts asking Local AI for new names of `rel_paths` (in that order),
/// following `instruction` and the typed `examples`. Returns the job's
/// number, which its events carry. A job still running is stopped.
#[tauri::command]
pub fn rename_propose(
    source_id: String,
    instruction: String,
    examples: Vec<Example>,
    rel_paths: Vec<String>,
    app: AppHandle,
) -> CmdResult<u64> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let settings = state.settings.lock().unwrap().get().clone();
    let engine = Engine::from_settings(&settings).ok_or_else(|| {
        rupert_core::i18n::tr("Local AI isn't set up: choose it in Settings ▸ Local AI.")
            .to_string()
    })?;
    let stop = Arc::new(AtomicBool::new(false));
    if let Some(old) = RUNNING.lock().unwrap().replace(stop.clone()) {
        old.store(true, Ordering::SeqCst);
    }
    let job = JOB.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        for rel_path in rel_paths {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let (name, error) =
                match propose_one(&app, &source, &engine, &instruction, &examples, &rel_path) {
                    Ok(n) => (Some(n), None),
                    Err(e) => (None, Some(e)),
                };
            if stop.load(Ordering::SeqCst) {
                break;
            }
            let _ = app.emit(
                PROPOSAL,
                Proposal {
                    job,
                    rel_path,
                    name,
                    error,
                },
            );
        }
        let _ = app.emit(
            PROPOSALS_DONE,
            ProposalsDone {
                job,
                stopped: stop.load(Ordering::SeqCst),
            },
        );
    });
    Ok(job)
}

fn propose_one(
    app: &AppHandle,
    source: &Source,
    engine: &Engine,
    instruction: &str,
    examples: &[Example],
    rel_path: &str,
) -> Result<String, String> {
    let state = app.state::<AppState>();
    let (info, text) = {
        let index = state.index.lock().unwrap();
        (
            index.document(&source.id, rel_path).map_err(err)?,
            index.document_text(&source.id, rel_path).map_err(err)?,
        )
    };
    let name = rel_path.rsplit('/').next().unwrap_or(rel_path);
    let (stem, ext) = rename::split_name(name);
    let target = Details {
        stem: stem.to_string(),
        title: info.as_ref().and_then(|i| i.hit.title.clone()),
        document_date: info.as_ref().and_then(|i| i.hit.document_date.clone()),
        tags: info.map(|i| i.hit.tags).unwrap_or_default(),
    };
    rename::propose(
        engine,
        &rename::Ask {
            instruction,
            examples,
            target: &target,
            text: text.as_deref().unwrap_or(""),
            ext: ext.trim_start_matches('.'),
        },
    )
    .map_err(err)
}

/// Stops asking Local AI (the proposals so far stay).
#[tauri::command]
pub fn rename_stop() {
    if let Some(stop) = RUNNING.lock().unwrap().take() {
        stop.store(true, Ordering::SeqCst);
    }
}

// MARK: - Renaming and undoing

/// Has the indexer follow renamed documents.
fn reindex(state: &AppState, source: &Source, outcome: &Outcome) {
    for r in &outcome.renamed {
        for rel_path in [&r.from, &r.to] {
            state.indexer.send(Job::File {
                source: source.clone(),
                rel_path: rel_path.clone(),
            });
        }
    }
}

/// What the index knows about each document, by path.
fn metas(state: &AppState, source_id: &str, paths: &[&str]) -> Vec<(String, DocMeta)> {
    let index = state.index.lock().unwrap();
    paths
        .iter()
        .filter_map(|p| {
            index
                .doc_meta(source_id, p)
                .ok()
                .flatten()
                .map(|m| (p.to_string(), m))
        })
        .collect()
}

/// Renames the documents; with `set_title`, each one's title becomes its
/// new name too. What was done comes back, for Undo.
#[tauri::command]
pub async fn rename_apply(
    source_id: String,
    steps: Vec<Step>,
    set_title: bool,
    app: AppHandle,
) -> CmdResult<Outcome> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let known = if set_title {
        let paths: Vec<&str> = steps.iter().map(|s| s.rel_path.as_str()).collect();
        metas(&state, &source_id, &paths)
    } else {
        Vec::new()
    };
    let src = source.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let meta_of = |rel: &str| known.iter().find(|(p, _)| p == rel).map(|(_, m)| m.clone());
        rename::apply(
            &src,
            &steps,
            set_title.then_some(&meta_of as rename::MetaOf),
        )
    })
    .await
    .map_err(err)?;
    reindex(&state, &source, &outcome);
    Ok(outcome)
}

/// Gives the documents of a batch their old names (and titles) back.
#[tauri::command]
pub async fn rename_undo(
    source_id: String,
    renamed: Vec<Renamed>,
    app: AppHandle,
) -> CmdResult<Outcome> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let paths: Vec<&str> = renamed
        .iter()
        .filter(|r| r.title_changed)
        .flat_map(|r| [r.to.as_str(), r.from.as_str()])
        .collect();
    let known = metas(&state, &source_id, &paths);
    let src = source.clone();
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let meta_of = |rel: &str| known.iter().find(|(p, _)| p == rel).map(|(_, m)| m.clone());
        rename::undo(&src, &renamed, &meta_of)
    })
    .await
    .map_err(err)?;
    reindex(&state, &source, &outcome);
    Ok(outcome)
}
