//! Dragging documents out of Rupert (#84): into Mail, Messages, a browser's
//! upload area, the Finder or File Explorer. The page starts the drag; this
//! hands the files to the system's own drag, always as a **copy**, so a drop
//! on a folder elsewhere never takes the file out of the library. Drops back
//! on Rupert's own folders are handled by the page, which moves them.

use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{AppState, CmdResult, err};

/// Sent when the system's drag has ended (dropped or cancelled).
const DRAG_OUT_ENDED: &str = "drag-out-ended";

/// The picture under the pointer when there's no page to show.
const FALLBACK_ICON: &[u8] = include_bytes!("../icons/128x128.png");

/// Starts dragging the documents (or folders) at `rel_paths` of a source out
/// of the window. The page hears `drag-out-ended` when it's over.
#[tauri::command]
pub async fn drag_out(
    source_id: String,
    rel_paths: Vec<String>,
    window: WebviewWindow,
    app: AppHandle,
) -> CmdResult<()> {
    let state = app.state::<AppState>();
    let source = state.source(&source_id)?;
    let files: Vec<PathBuf> = rel_paths
        .iter()
        .map(|rel| rupert_core::resolve(&source.root, rel))
        .collect::<rupert_core::Result<_>>()
        .map_err(err)?;
    if files.is_empty() {
        return Ok(());
    }
    // A PDF's first page as the picture, small and from the cache (an
    // online-only file isn't read). Anything else shows Rupert's icon.
    let (thumbs, extractor, first) = (
        state.thumbs.clone(),
        state.extractor.clone(),
        files[0].clone(),
    );
    let icon = tauri::async_runtime::spawn_blocking(move || {
        if rupert_core::FileKind::from_path(&first) != rupert_core::FileKind::Pdf {
            return None;
        }
        thumbs
            .get(&extractor, &first, 0, 96, false)
            .ok()
            .flatten()
            .map(|(bytes, _)| bytes)
    })
    .await
    .map_err(err)?
    .unwrap_or_else(|| FALLBACK_ICON.to_vec());

    let (w, ended) = (window.clone(), app.clone());
    window
        .run_on_main_thread(move || {
            let item = drag::DragItem::Files(files);
            let image = drag::Image::Raw(icon);
            let on_end = move |result: drag::DragResult, _at: drag::CursorPosition| {
                let dropped = matches!(result, drag::DragResult::Dropped);
                let _ = ended.emit(DRAG_OUT_ENDED, dropped);
            };
            let options = drag::Options {
                mode: drag::DragMode::Copy,
                ..Default::default()
            };
            #[cfg(target_os = "linux")]
            let started = match w.gtk_window() {
                Ok(gtk) => drag::start_drag(&gtk, item, image, on_end, options),
                Err(_) => Err(drag::Error::FailedToStartDrag),
            };
            #[cfg(not(target_os = "linux"))]
            let started = drag::start_drag(&w, item, image, on_end, options);
            if let Err(e) = started {
                eprintln!("rupert: couldn't start dragging out: {e}");
                let _ = w.emit(DRAG_OUT_ENDED, false);
            }
        })
        .map_err(err)
}
