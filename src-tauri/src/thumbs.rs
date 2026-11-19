//! The `thumb:` URL scheme behind the large-icon and gallery views:
//! `thumb://localhost/<source id>/<path>?w=<pixels>&p=<page>` (as built by the UI's
//! `convertFileSrc`, percent-encoded; `http://thumb.localhost/…` on
//! Windows). Pictures come from `rupert_core::thumbs`; anything that has
//! none, including online-only files, is a 404 and the view shows an icon.

use tauri::http::{Request, Response, StatusCode, Uri, header};
use tauri::{AppHandle, Manager, UriSchemeResponder};

use crate::AppState;
use rupert_core::thumbs::Picture;

pub const SCHEME: &str = "thumb";
const DEFAULT_SIDE: u32 = 256;

// MARK: - Serving

pub fn serve(app: &AppHandle, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let response = match picture(&app, request.uri()) {
            Some((bytes, mime)) => Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                // The URL changes with the file (`m=`), so it can be kept.
                .header(header::CACHE_CONTROL, "max-age=31536000")
                .body(bytes),
            None => Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Vec::new()),
        };
        responder.respond(response.unwrap_or_else(|_| Response::new(Vec::new())));
    });
}

fn picture(app: &AppHandle, uri: &Uri) -> Option<Picture> {
    let state = app.try_state::<AppState>()?;
    let target = percent_decode(uri.path().trim_start_matches('/'))?;
    let (source_id, rel_path) = target.split_once('/')?;
    let source = state.source(source_id).ok()?;
    // `resolve` refuses `..` and absolute parts: only files in a source.
    let path = rupert_core::resolve(&source.root, rel_path).ok()?;
    let param = |name: &str| {
        uri.query()?
            .split('&')
            .find_map(|kv| kv.strip_prefix(name)?.strip_prefix('='))
    };
    let side = param("w")
        .and_then(|w| w.parse().ok())
        .unwrap_or(DEFAULT_SIDE);
    let page = param("p").and_then(|p| p.parse().ok()).unwrap_or(0);
    state
        .thumbs
        .get(&state.extractor, &path, page, side)
        .inspect_err(|e| eprintln!("rupert: thumbnail of {}: {e}", path.display()))
        .ok()?
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
    #[test]
    fn decodes_paths() {
        assert_eq!(
            super::percent_decode("src1%2FBoat%2FCaf%C3%A9%20bill.pdf").as_deref(),
            Some("src1/Boat/Café bill.pdf")
        );
        assert_eq!(super::percent_decode("a%2").as_deref(), Some("a%2"));
        assert_eq!(super::percent_decode("%FF"), None);
    }
}
