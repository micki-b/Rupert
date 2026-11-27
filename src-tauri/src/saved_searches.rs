//! Saved searches (#110): listed from every source's `.rupert/searches.json`,
//! saved with the source they search (see `rupert_core::saved_search`).

use rupert_core::saved_search::{self, SavedSearch};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::{AppState, CmdResult, err};

/// A saved search and the source it's kept with.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listed {
    pub source_id: String,
    #[serde(flatten)]
    pub search: SavedSearch,
}

/// Every saved search, by name.
#[tauri::command]
pub async fn saved_searches(app: AppHandle) -> CmdResult<Vec<Listed>> {
    tauri::async_runtime::spawn_blocking(move || {
        let sources = app.state::<AppState>().sources();
        let mut all: Vec<Listed> = sources
            .iter()
            .filter(|s| s.root.is_dir())
            .flat_map(|source| {
                saved_search::list(&source.root)
                    .into_iter()
                    .map(|search| Listed {
                        source_id: source.id.clone(),
                        search,
                    })
            })
            .collect();
        all.sort_by_key(|l| l.search.name.to_lowercase());
        all
    })
    .await
    .map_err(err)
}

/// Saves a search with a source (a new one, or one renamed or updated).
/// `was_in`: the source it was kept with before, when that's another one
/// (its search now looks in a different source): it's taken out there.
#[tauri::command]
pub async fn save_search(
    source_id: String,
    search: SavedSearch,
    was_in: Option<String>,
    app: AppHandle,
) -> CmdResult<Listed> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let source = state.source(&source_id)?;
        let saved = saved_search::save(&source.root, search).map_err(err)?;
        if let Some(old) = was_in.filter(|old| *old != source_id)
            && let Ok(old) = state.source(&old)
        {
            saved_search::remove(&old.root, &saved.id).map_err(err)?;
        }
        Ok(Listed {
            source_id,
            search: saved,
        })
    })
    .await
    .map_err(err)?
}

/// Forgets a saved search (never any documents).
#[tauri::command]
pub async fn remove_saved_search(source_id: String, id: String, app: AppHandle) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let source = app.state::<AppState>().source(&source_id)?;
        saved_search::remove(&source.root, &id).map_err(err)?;
        Ok(())
    })
    .await
    .map_err(err)?
}
