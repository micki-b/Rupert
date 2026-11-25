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

/// Where the drag started, as the page saw the pointer: in its own
/// coordinates and the screen's. Their difference is where the page sits on
/// the screen (the window's reported position isn't: on macOS it's the
/// title bar's height off).
#[derive(Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Start {
    client_x: f64,
    client_y: f64,
    screen_x: f64,
    screen_y: f64,
}

fn pointer(w: &WebviewWindow, start: Start, dropped: bool) -> Option<Pointer> {
    let at = w.cursor_position().ok()?;
    let scale = w.scale_factor().ok()?;
    let size = w.inner_size().ok()?;
    let x = at.x / scale - (start.screen_x - start.client_x);
    let y = at.y / scale - (start.screen_y - start.client_y);
    let (width, height) = (
        f64::from(size.width) / scale,
        f64::from(size.height) / scale,
    );
    Some(Pointer {
        x,
        y,
        inside: x >= 0.0 && y >= 0.0 && x < width && y < height,
        dropped,
    })
}

/// Whether the window under the pointer is this one, not another app's
/// lying over it. (The web view accepts a drop it can't use, so "dropped"
/// alone doesn't tell.) Where it can't be asked (Linux): assumed so.
#[cfg(target_os = "macos")]
fn ours_under_pointer(w: &WebviewWindow) -> bool {
    use objc2_app_kit::{NSEvent, NSWindow};
    let (Ok(ptr), Some(mtm)) = (w.ns_window(), objc2::MainThreadMarker::new()) else {
        return true;
    };
    // SAFETY: Tauri's NSWindow, alive as long as its window; main thread.
    let ours = unsafe { &*(ptr as *const NSWindow) };
    let at = NSEvent::mouseLocation();
    NSWindow::windowNumberAtPoint_belowWindowWithWindowNumber(at, 0, mtm) == ours.windowNumber()
}

#[cfg(target_os = "windows")]
fn ours_under_pointer(w: &WebviewWindow) -> bool {
    use windows_sys::Win32::Foundation::POINT;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GA_ROOT, GetAncestor, GetCursorPos, WindowFromPoint,
    };
    let Ok(ours) = w.hwnd() else {
        return true;
    };
    let mut at = POINT { x: 0, y: 0 };
    // SAFETY: plain Win32 calls with a valid out-pointer.
    unsafe {
        if GetCursorPos(&mut at) == 0 {
            return true;
        }
        GetAncestor(WindowFromPoint(at), GA_ROOT) as isize == ours.0 as isize
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn ours_under_pointer(_w: &WebviewWindow) -> bool {
    true
}

/// The picture under the pointer when there's no page to show.
const FALLBACK_ICON: &[u8] = include_bytes!("../icons/128x128.png");

/// Starts dragging the documents (or folders) at `rel_paths` of a source out
/// of the window. The page hears `drag-out-ended` when it's over.
#[tauri::command]
pub async fn drag_out(
    source_id: String,
    rel_paths: Vec<String>,
    start: Start,
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
                if let Some(p) = pointer(&w, start, false) {
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
                // Taken by something other than this window.
                let dropped =
                    matches!(result, drag::DragResult::Dropped) && !ours_under_pointer(&ended);
                let end = pointer(&ended, start, dropped).unwrap_or(Pointer {
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
