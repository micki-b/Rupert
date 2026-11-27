//! Read details… (#108): who documents are from, their amounts and due
//! dates, proposed by Local AI from each document's indexed text (nothing
//! is downloaded), then written after the user reviewed them. The work is
//! `rupert_core::llm::read_details` and `rupert_core::details::apply`.

use rupert_core::llm::{self, Details, Engine};
use rupert_core::{Job, tag_edit::DocRef};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{AppState, CmdResult, err};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    /// What the document has now.
    pub current: Details,
    /// What its text shows (None without text).
    pub proposed: Option<Details>,
}

/// Reads one document's details from its indexed text.
#[tauri::command]
pub async fn details_propose(
    source_id: String,
    rel_path: String,
    app: AppHandle,
) -> CmdResult<Proposal> {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap().get().clone();
    let engine = Engine::from_settings(&settings).ok_or_else(|| {
        rupert_core::i18n::tr("Local AI isn't set up: choose it in Settings ▸ Local AI.")
            .to_string()
    })?;
    let (current, text, name) = {
        let index = state.index.lock().unwrap();
        let info = index.document(&source_id, &rel_path).map_err(err)?;
        let text = index.document_text(&source_id, &rel_path).map_err(err)?;
        let (current, name) = info
            .map(|i| {
                (
                    Details {
                        sender: i.hit.sender,
                        amount: i.hit.amount,
                        currency: i.hit.currency,
                        due_date: i.hit.due_date,
                    },
                    i.hit.name,
                )
            })
            .unwrap_or_default();
        (current, text.filter(|t| !t.trim().is_empty()), name)
    };
    let Some(text) = text else {
        return Ok(Proposal {
            current,
            proposed: None,
        });
    };
    let proposed =
        tauri::async_runtime::spawn_blocking(move || llm::read_details(&engine, &text, &name))
            .await
            .map_err(err)?
            .map_err(err)?;
    Ok(Proposal {
        current,
        proposed: Some(proposed),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reviewed {
    #[serde(flatten)]
    pub doc: DocRef,
    pub details: Details,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Applied {
    pub done: usize,
    /// Documents that couldn't be saved, with why.
    pub failed: Vec<(String, String)>,
}

/// Writes the reviewed details, then has the indexer read each document
/// again.
#[tauri::command]
pub async fn details_apply(items: Vec<Reviewed>, app: AppHandle) -> CmdResult<Applied> {
    let state = app.state::<AppState>();
    let mut done = 0;
    let mut failed = Vec::new();
    for item in items {
        let source = state.source(&item.doc.source_id)?;
        let rel = item.doc.rel_path.clone();
        let root = source.root.clone();
        let details = item.details;
        let result = tauri::async_runtime::spawn_blocking(move || {
            let path = rupert_core::resolve(&root, &rel)?;
            rupert_core::details::apply(&path, &details)
        })
        .await
        .map_err(err)?;
        match result {
            Ok(_) => {
                done += 1;
                state.indexer.send(Job::File {
                    source,
                    rel_path: item.doc.rel_path,
                });
            }
            Err(e) => failed.push((item.doc.rel_path, e.to_string())),
        }
    }
    Ok(Applied { done, failed })
}
