//! Dragging documents out of Rupert (#84): into Mail, Messages, a browser's
//! upload area, the Finder or File Explorer. The page starts the drag; this
//! hands the files to the system's own drag, always as a **copy**, so a drop
//! on a folder elsewhere never takes the file out of the library.
//!
//! The web view doesn't pass a drag it started itself back to the page (no
//! dragover, no drop), so Rupert follows the pointer: while the drag lasts
//! the page hears where it is over the window (`drag-out-pointer`) and
//! highlights the folder there, and when it ends (`drag-out-ended`) where it
//! was let go, so a drop on Rupert's own folder moves the document.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::{AppState, CmdResult, err};

/// Sent when the system's drag has ended (dropped or cancelled).
const DRAG_OUT_ENDED: &str = "drag-out-ended";
/// Sent while dragging: where the pointer is over the window.
const DRAG_OUT_POINTER: &str = "drag-out-pointer";

/// Where the pointer is, in the page's coordinates; `inside` the window or
/// not. `dropped`: the drag ended with a drop somewhere (only at the end).
#[derive(Clone, Serialize)]
struct Pointer {
    x: f64,
    y: f64,
    inside: bool,
    dropped: bool,
}

fn pointer(w: &WebviewWindow, dropped: bool) -> Option<Pointer> {
    let at = w.cursor_position().ok()?;
    let origin = w.inner_position().ok()?;
    let size = w.inner_size().ok()?;
    let scale = w.scale_factor().ok()?;
    let (px, py) = (at.x - f64::from(origin.x), at.y - f64::from(origin.y));
    let inside =
        px >= 0.0 && py >= 0.0 && px < f64::from(size.width) && py < f64::from(size.height);
    Some(Pointer {
        x: px / scale,
        y: py / scale,
        inside,
        dropped,
    })
}

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

    // Follow the pointer until the drag ends.
    let active = Arc::new(AtomicBool::new(true));
    {
        let (w, active) = (window.clone(), active.clone());
        std::thread::spawn(move || {
            while active.load(Ordering::SeqCst) {
                if let Some(p) = pointer(&w, false) {
                    let _ = w.emit_to(w.label(), DRAG_OUT_POINTER, p);
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        });
    }

    let (w, ended) = (window.clone(), window.clone());
    let failed = active.clone();
    window
        .run_on_main_thread(move || {
            let item = drag::DragItem::Files(files);
            let image = drag::Image::Raw(icon);
            let on_end = move |result: drag::DragResult, _at: drag::CursorPosition| {
                active.store(false, Ordering::SeqCst);
                let dropped = matches!(result, drag::DragResult::Dropped);
                let end = pointer(&ended, dropped).unwrap_or(Pointer {
                    x: -1.0,
                    y: -1.0,
                    inside: false,
                    dropped,
                });
                let _ = ended.emit_to(ended.label(), DRAG_OUT_ENDED, end);
            };
            let options = drag::Options {
                mode: drag::DragMode::Copy,
                // Let go over Rupert, the web view refuses the drop; no
                // sliding back, since the page moves it.
                skip_animatation_on_cancel_or_failure: true,
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
                failed.store(false, Ordering::SeqCst);
                let _ = w.emit_to(
                    w.label(),
                    DRAG_OUT_ENDED,
                    Pointer {
                        x: -1.0,
                        y: -1.0,
                        inside: false,
                        dropped: false,
                    },
                );
            }
        })
        .map_err(err)
}
