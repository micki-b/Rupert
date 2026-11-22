//! The `thumb:` URL scheme on the phone, behind the large-icon and gallery
//! views (#41). The URLs are the computer's (`thumb://localhost/<source
//! id>/<path>?w=<pixels>&m=<modified>`, built by `Thumb.svelte`); the
//! pictures come from elsewhere, since the phone has no PDFium:
//!
//! - **OneDrive and Google Drive folders:** the service's own thumbnails,
//!   so the document isn't downloaded.
//! - **Files folders (iPhone):** the system's thumbnail, for documents that
//!   are on the phone. Online-only ones get none: making one would fetch
//!   the file.
//!
//! Pictures (and "has none") are kept in the cache, by source, path, size
//! and modification time, so a changed document gets a new one. Anything
//! without a picture is a 404, and the view shows its icon.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use rupert_core::{CloudState, cloud_state};
use tauri::http::{Request, Response, StatusCode, Uri, header};
use tauri::{AppHandle, Manager, UriSchemeResponder};

use crate::{AppState, cloud, device};

pub const SCHEME: &str = "thumb";
const DEFAULT_SIDE: u32 = 320;
/// Pictures not shown for this long are removed.
const KEEP_DAYS: u64 = 30;
/// "Has no picture" is believed this long, then asked again.
const NONE_HOURS: u64 = 24;

// MARK: - Serving

pub fn serve(app: &AppHandle, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = match picture(&app, request.uri()) {
            Some(bytes) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                // The URL changes with the document (`m=`), so it can be kept.
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(bytes),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
    });
}

fn picture(app: &AppHandle, uri: &Uri) -> Option<Vec<u8>> {
    let target = percent_decode(uri.path().trim_start_matches('/'))?;
    let (source_id, rel_path) = target.split_once('/')?;
    let param = |name: &str| {
        uri.query()?
            .split('&')
            .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
    };
    let side = param("w")
        .and_then(|w| w.parse::<u32>().ok())
        .unwrap_or(DEFAULT_SIDE)
        .clamp(64, 1600);
    let modified = param("m").unwrap_or("0");

    let dir = cache_dir(app)?;
    let key = format!(
        "{:016x}",
        fnv1a(&format!("{source_id}\n{rel_path}\n{side}\n{modified}"))
    );
    let file = dir.join(format!("{key}.jpg"));
    let none = dir.join(format!("{key}.none"));
    if let Ok(bytes) = fs::read(&file) {
        return Some(bytes);
    }
    let fresh = |p: &PathBuf, max: Duration| {
        fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age < max)
    };
    if fresh(&none, Duration::from_secs(NONE_HOURS * 3600)) {
        return None;
    }
    match make(app, source_id, rel_path, side) {
        Ok(Some(bytes)) => {
            let _ = fs::create_dir_all(&dir);
            let tmp = file.with_extension("part");
            if fs::write(&tmp, &bytes).is_ok() {
                let _ = fs::rename(&tmp, &file);
            }
            Some(bytes)
        }
        Ok(None) => {
            let _ = fs::create_dir_all(&dir);
            let _ = fs::write(&none, b"");
            None
        }
        // Not kept: offline, signed out… may work next time.
        Err(e) => {
            eprintln!("rupert: thumbnail of {rel_path}: {e}");
            None
        }
    }
}

/// A picture made now, `None` if the document has none.
fn make(
    app: &AppHandle,
    source_id: &str,
    rel_path: &str,
    side: u32,
) -> Result<Option<Vec<u8>>, String> {
    let state = app.state::<AppState>();
    if let Some(folder) = state.remote(source_id) {
        return cloud::thumbnail(app, &folder, rel_path, side);
    }
    let source = state.source(source_id)?;
    // `resolve` refuses `..` and absolute parts: only files in a source.
    let path = rupert_core::resolve(&source.root, rel_path).map_err(crate::err)?;
    if cloud_state(&path).map_err(crate::err)? != CloudState::Local {
        return Ok(None);
    }
    match device(app).thumbnail(&path.to_string_lossy(), side) {
        Ok(Some(made)) => {
            let bytes = fs::read(&made).map_err(crate::err)?;
            let _ = fs::remove_file(&made);
            Ok(Some(bytes))
        }
        // None, or not on this platform (Android has no Files folders).
        _ => Ok(None),
    }
}

// MARK: - Cache

fn cache_dir(app: &AppHandle) -> Option<PathBuf> {
    Some(app.path().app_cache_dir().ok()?.join("thumbs"))
}

/// Removes pictures not shown for a while (reading one doesn't change its
/// time, so this is by when it was made).
pub fn tidy(app: &AppHandle) {
    let Some(dir) = cache_dir(app) else { return };
    let max_age = Duration::from_secs(KEEP_DAYS * 24 * 3600);
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > max_age);
        if old {
            let _ = fs::remove_file(e.path());
        }
    }
}

/// A stable 64-bit hash for cache file names.
fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// `%2F` → `/` etc.; `None` if it isn't valid UTF-8 afterwards.
fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_paths() {
        assert_eq!(
            percent_decode("src%2FTaxes%202025%2FW-2.pdf").as_deref(),
            Some("src/Taxes 2025/W-2.pdf")
        );
        assert_eq!(percent_decode("M%C3%A4rz").as_deref(), Some("März"));
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(fnv1a(""), 0xcbf2_9ce4_8422_2325);
        assert_ne!(fnv1a("a\nb"), fnv1a("a\nc"));
    }
}
