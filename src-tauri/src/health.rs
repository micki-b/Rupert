//! Library health (#113) and duplicates (#111): the counts and lists, and
//! their fixes (recognize text in place, suggest details, clear up copies).
//! The work is `rupert_core::health`, `ocr::recognize_in_place`,
//! `llm::Engine::suggest` and `details::apply_suggested`.

use std::sync::atomic::{AtomicBool, Ordering};

use rupert_core::details::{self, Suggested};
use rupert_core::health::{self, DupGroup};
use rupert_core::index::{HealthCounts, HealthRow, Hit};
use rupert_core::llm::{self, Engine};
use rupert_core::{Job, tag_edit::DocRef};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::{AppState, CmdResult, err};

/// Progress of a long fix: `{ done, total, name }`.
const HEALTH_PROGRESS: &str = "health-progress";

/// Set by Stop; checked between documents.
static STOP: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    done: usize,
    total: usize,
    name: String,
}

fn emit(app: &AppHandle, done: usize, total: usize, name: &str) {
    let _ = app.emit(
        HEALTH_PROGRESS,
        Progress {
            done,
            total,
            name: name.to_string(),
        },
    );
}

// MARK: - Counts and lists

#[tauri::command]
pub async fn health_counts(source_id: Option<String>, app: AppHandle) -> CmdResult<HealthCounts> {
    let state = app.state::<AppState>();
    let index = state.index.lock().unwrap();
    index.health_counts(source_id.as_deref()).map_err(err)
}

/// The documents of a row, for "Show" (at most 2,000).
#[tauri::command]
pub async fn health_docs(
    row: HealthRow,
    source_id: Option<String>,
    app: AppHandle,
) -> CmdResult<Vec<Hit>> {
    let state = app.state::<AppState>();
    let index = state.index.lock().unwrap();
    let mut hits = index.health_hits(row, source_id.as_deref()).map_err(err)?;
    hits.truncate(2000);
    Ok(hits)
}

#[tauri::command]
pub fn health_stop() {
    STOP.store(true, Ordering::SeqCst);
}

// MARK: - Duplicates

/// Looks for duplicates, hashing same-sized files with the index free.
#[tauri::command]
pub async fn duplicates_find(
    source_id: Option<String>,
    app: AppHandle,
) -> CmdResult<Vec<DupGroup>> {
    STOP.store(false, Ordering::SeqCst);
    let state = app.state::<AppState>();
    let candidates = {
        let index = state.index.lock().unwrap();
        health::candidates(&index, source_id.as_deref()).map_err(err)?
    };
    let sources = state.sources();
    let emitter = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        health::find_duplicates(
            candidates,
            &sources,
            |done, total| emit(&emitter, done, total, ""),
            || STOP.load(Ordering::SeqCst),
        )
    })
    .await
    .map_err(err)?
    .map_err(err)
}

/// Keep all (`keep`): a group of acceptable duplicates is remembered in
/// its first copy's source and skipped from then on; `keep: false` lets it
/// be reviewed again (from every source that remembers it).
#[tauri::command]
pub async fn duplicates_keep(
    source_id: String,
    key: String,
    copies: usize,
    keep: bool,
    app: AppHandle,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if keep {
        let source = state.source(&source_id)?;
        health::set_kept(&source.root, &key, copies, true).map_err(err)
    } else {
        for s in state.sources() {
            if health::kept_list(&s.root).iter().any(|k| k.key == key) {
                health::set_kept(&s.root, &key, 0, false).map_err(err)?;
            }
        }
        Ok(())
    }
}

/// How many duplicate groups a source keeps as they are (Settings).
#[tauri::command]
pub fn duplicates_kept_count(source_id: String, app: AppHandle) -> CmdResult<usize> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    Ok(health::kept_list(&source.root).len())
}

/// Forgets a source's kept groups, so they're reviewed again (Settings).
#[tauri::command]
pub fn duplicates_reset(source_id: String, app: AppHandle) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    health::reset_kept(&source.root).map_err(err)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearUp {
    pub keep: DocRef,
    pub others: Vec<DocRef>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleared {
    pub trashed: usize,
    pub failed: Vec<(String, String)>,
}

/// For each group: the copy kept gets the others' tags and notes, then the
/// others go to the Trash (never deleted outright).
#[tauri::command]
pub async fn duplicates_clear(groups: Vec<ClearUp>, app: AppHandle) -> CmdResult<Cleared> {
    let state = app.state::<AppState>();
    let mut trashed = 0;
    let mut failed = Vec::new();
    for g in groups {
        let (known, others) = {
            let index = state.index.lock().unwrap();
            let known = index
                .doc_meta(&g.keep.source_id, &g.keep.rel_path)
                .map_err(err)?
                .unwrap_or_default();
            let others: Vec<_> = g
                .others
                .iter()
                .filter_map(|o| index.doc_meta(&o.source_id, &o.rel_path).ok().flatten())
                .collect();
            (known, others)
        };
        let keep_source = state.source(&g.keep.source_id)?;
        let keep_path = rupert_core::resolve(&keep_source.root, &g.keep.rel_path).map_err(err)?;
        let carried = tauri::async_runtime::spawn_blocking(move || {
            health::keep_details(&keep_path, &others, &known)
        })
        .await
        .map_err(err)?;
        if let Err(e) = carried {
            // The others stay, so nothing of theirs is lost.
            failed.push((g.keep.rel_path.clone(), e.to_string()));
            continue;
        }
        state.indexer.send(Job::File {
            source: keep_source,
            rel_path: g.keep.rel_path.clone(),
        });
        for o in g.others {
            let source = state.source(&o.source_id)?;
            let (root, rel) = (source.root.clone(), o.rel_path.clone());
            let result = tauri::async_runtime::spawn_blocking(move || {
                rupert_core::fileops::trash(&root, &rel)
            })
            .await
            .map_err(err)?;
            match result {
                Ok(_) => {
                    trashed += 1;
                    let rel_dir = o
                        .rel_path
                        .rsplit_once('/')
                        .map_or("", |(d, _)| d)
                        .to_string();
                    state.indexer.send(Job::SyncDir { source, rel_dir });
                }
                Err(e) => failed.push((o.rel_path, e.to_string())),
            }
        }
    }
    Ok(Cleared { trashed, failed })
}

// MARK: - Recognize text in place

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Recognized {
    /// Documents that got text.
    pub done: usize,
    /// Documents where nothing could be read.
    pub nothing: usize,
    pub failed: Vec<(String, String)>,
}

/// Recognizes text in `docs` one at a time (Stop between them), then has
/// the indexer read each again.
#[tauri::command]
pub async fn health_recognize(docs: Vec<DocRef>, app: AppHandle) -> CmdResult<Recognized> {
    STOP.store(false, Ordering::SeqCst);
    let state = app.state::<AppState>();
    let engine = rupert_core::ocr::Engine::detect().ok_or_else(|| {
        rupert_core::i18n::tr("Text recognition isn't available on this computer.").to_string()
    })?;
    let mut out = Recognized {
        done: 0,
        nothing: 0,
        failed: Vec::new(),
    };
    let total = docs.len();
    for (i, d) in docs.into_iter().enumerate() {
        if STOP.load(Ordering::SeqCst) {
            break;
        }
        let name = d
            .rel_path
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .to_string();
        emit(&app, i, total, &name);
        let source = state.source(&d.source_id)?;
        let path = rupert_core::resolve(&source.root, &d.rel_path).map_err(err)?;
        let (extractor, engine) = (state.extractor.clone(), engine.clone());
        let result = tauri::async_runtime::spawn_blocking(move || {
            rupert_core::ocr::recognize_in_place(&path, &extractor, &engine, |_, _| {})
        })
        .await
        .map_err(err)?;
        match result {
            Ok(r) if r.recognized > 0 => {
                out.done += 1;
                state.indexer.send(Job::File {
                    source,
                    rel_path: d.rel_path,
                });
            }
            Ok(_) => out.nothing += 1,
            Err(e) => out.failed.push((d.rel_path, e.to_string())),
        }
    }
    emit(&app, total, total, "");
    Ok(out)
}

// MARK: - Suggest details

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestProposal {
    /// What the document has now.
    pub current: Suggested,
    /// What Local AI suggests from its text (None without text).
    pub proposed: Option<Suggested>,
}

/// Title, date and tags for one document, from its indexed text (nothing
/// is downloaded).
#[tauri::command]
pub async fn suggest_propose(
    source_id: String,
    rel_path: String,
    app: AppHandle,
) -> CmdResult<SuggestProposal> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().get().clone();
    let engine = Engine::from_settings(&settings).ok_or_else(|| {
        rupert_core::i18n::tr("Local AI isn't set up: choose it in Settings ▸ Local AI.")
            .to_string()
    })?;
    let (current, text, name, tags) = {
        let index = state.index.lock().unwrap();
        let info = index.document(&source_id, &rel_path).map_err(err)?;
        let text = index.document_text(&source_id, &rel_path).map_err(err)?;
        let tags = index.top_tags(150).unwrap_or_default();
        let (current, name) = info
            .map(|i| {
                (
                    Suggested {
                        title: i.hit.title,
                        document_date: i.hit.document_date,
                        tags: Some(i.hit.tags),
                    },
                    i.hit.name,
                )
            })
            .unwrap_or_default();
        (current, text.filter(|t| !t.trim().is_empty()), name, tags)
    };
    let Some(text) = text else {
        return Ok(SuggestProposal {
            current,
            proposed: None,
        });
    };
    let year = rupert_core::pdf_meta::utc_today()[..4].to_string();
    let s = tauri::async_runtime::spawn_blocking(move || {
        engine.suggest(&llm::Request {
            text: &text,
            file_name: &name,
            year: &year,
            folders: &[],
            tags: &tags,
        })
    })
    .await
    .map_err(err)?
    .map_err(err)?;
    Ok(SuggestProposal {
        current,
        proposed: Some(Suggested {
            title: s.title,
            document_date: s.document_date,
            tags: Some(s.tags),
        }),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewedSuggestion {
    #[serde(flatten)]
    pub doc: DocRef,
    pub suggested: Suggested,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestApplied {
    pub done: usize,
    pub failed: Vec<(String, String)>,
}

/// Writes the reviewed titles, dates and tags, then has the indexer read
/// each document again.
#[tauri::command]
pub async fn suggest_apply(
    items: Vec<ReviewedSuggestion>,
    app: AppHandle,
) -> CmdResult<SuggestApplied> {
    let state = app.state::<AppState>();
    let mut out = SuggestApplied {
        done: 0,
        failed: Vec::new(),
    };
    for item in items {
        let source = state.source(&item.doc.source_id)?;
        let (root, rel, s) = (
            source.root.clone(),
            item.doc.rel_path.clone(),
            item.suggested,
        );
        let result = tauri::async_runtime::spawn_blocking(move || {
            let path = rupert_core::resolve(&root, &rel)?;
            details::apply_suggested(&path, &s)
        })
        .await
        .map_err(err)?;
        match result {
            Ok(_) => {
                out.done += 1;
                state.indexer.send(Job::File {
                    source,
                    rel_path: item.doc.rel_path,
                });
            }
            Err(e) => out.failed.push((item.doc.rel_path, e.to_string())),
        }
    }
    Ok(out)
}
