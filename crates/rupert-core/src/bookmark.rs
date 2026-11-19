//! Security-scoped bookmarks (macOS): how a sandboxed app (the Mac App Store
//! edition) keeps access to folders the user picked, across launches. A
//! bookmark is made right after the pick; at every launch it is resolved
//! and access is started for as long as the app runs. Outside the sandbox
//! the same bookmarks also follow a folder that was moved or renamed.
//!
//! Stored as base64 in `sources.json`. Elsewhere (Windows, Linux) there are
//! no bookmarks: `create` gives `None`.

use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
use crate::error::Error;
use crate::error::Result;

/// A resolved bookmark: where the folder is now, and a fresh bookmark to
/// save when the old one went stale (e.g. the folder moved).
#[derive(Debug, Clone, PartialEq)]
pub struct Opened {
    pub path: PathBuf,
    pub refreshed: Option<String>,
}

/// A bookmark for the folder at `path`, which the user just picked. `None`
/// where bookmarks don't exist.
pub fn create(path: &Path) -> Result<Option<String>> {
    #[cfg(target_os = "macos")]
    {
        mac::create(path).map(Some)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Ok(None)
    }
}

/// Resolves a bookmark and starts access to its folder for the rest of the
/// app's run.
pub fn open(bookmark: &str) -> Result<Opened> {
    #[cfg(target_os = "macos")]
    {
        mac::open(bookmark)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bookmark;
        Err(crate::error::Error::Unsupported(
            "folder bookmarks exist only on macOS".into(),
        ))
    }
}

// MARK: - macOS

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use crate::oauth::{base64url, base64url_decode};
    use objc2::runtime::Bool;
    use objc2_foundation::{
        NSData, NSURL, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions,
    };

    fn err(what: &str, e: impl std::fmt::Display) -> Error {
        Error::InvalidValue(format!("{what}: {e}"))
    }

    pub fn create(path: &Path) -> Result<String> {
        let url = NSURL::from_path(path, true, None)
            .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?;
        let data = url
            .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                NSURLBookmarkCreationOptions::WithSecurityScope,
                None,
                None,
            )
            .map_err(|e| {
                err(
                    "couldn't remember access to the folder",
                    e.localizedDescription(),
                )
            })?;
        Ok(base64url(&data.to_vec()))
    }

    pub fn open(bookmark: &str) -> Result<Opened> {
        let bytes = base64url_decode(bookmark)
            .ok_or_else(|| Error::InvalidValue("the folder's bookmark is damaged".into()))?;
        let data = NSData::with_bytes(&bytes);
        let mut stale = Bool::NO;
        // SAFETY: `stale` outlives the call.
        let url = unsafe {
            NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                &data,
                NSURLBookmarkResolutionOptions::WithSecurityScope,
                None,
                &mut stale,
            )
        }
        .map_err(|e| err("the folder can't be found", e.localizedDescription()))?;
        // Access lasts until the app quits (never stopped on purpose).
        // Outside the sandbox this returns false and nothing is needed.
        // SAFETY: a plain Objective-C call on a valid URL.
        let _ = unsafe { url.startAccessingSecurityScopedResource() };
        let path = url
            .to_file_path()
            .ok_or_else(|| Error::InvalidValue("the folder's bookmark has no path".into()))?;
        let refreshed = if stale.as_bool() {
            create(&path).ok()
        } else {
            None
        };
        Ok(Opened { path, refreshed })
    }
}

// MARK: - Tests

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_follow_a_move() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Taxes");
        std::fs::create_dir(&folder).unwrap();
        let mark = create(&folder).unwrap().unwrap();
        let opened = open(&mark).unwrap();
        assert_eq!(
            opened.path.canonicalize().unwrap(),
            folder.canonicalize().unwrap()
        );

        // A renamed folder is still found.
        let moved = dir.path().join("Taxes 2025");
        std::fs::rename(&folder, &moved).unwrap();
        let opened = open(&mark).unwrap();
        assert_eq!(
            opened.path.canonicalize().unwrap(),
            moved.canonicalize().unwrap()
        );

        assert!(open("not a bookmark").is_err());
    }
}
