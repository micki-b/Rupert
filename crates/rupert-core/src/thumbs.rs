//! Thumbnails for the large-icon and gallery views: the first page of a PDF,
//! rendered once and kept in the app's cache folder (never in the library;
//! the cache can be deleted at any time), or an image file as it is.
//! Online-only files get none: reading them would download them.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::file_kind::{FileKind, image_mime};
use crate::listing::{CloudState, cloud_state};
use crate::text::TextExtractor;

/// Smallest and largest picture asked for (pixels, longest side).
const MIN_SIDE: u32 = 64;
const MAX_SIDE: u32 = 1600;

/// A picture to show: its bytes and MIME type.
pub type Picture = (Vec<u8>, &'static str);

pub struct Thumbnails {
    dir: PathBuf,
}

impl Thumbnails {
    /// Thumbnails cached in `dir` (created when first needed).
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    // MARK: - Getting one

    /// A picture of the document at `path`: for a PDF, its `page` (0-based)
    /// at most `side` pixels on its longest side; images come as they are
    /// (the view scales them). `None` for online-only files and kinds
    /// without a picture.
    pub fn get(
        &self,
        extractor: &TextExtractor,
        path: &Path,
        page: usize,
        side: u32,
    ) -> Result<Option<Picture>> {
        if cloud_state(path)? == CloudState::CloudOnly {
            return Ok(None);
        }
        match FileKind::from_path(path) {
            FileKind::Pdf => {
                let side = side.clamp(MIN_SIDE, MAX_SIDE);
                let meta = fs::metadata(path)?;
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_millis());
                let cached = self.dir.join(format!(
                    "{}.png",
                    cache_key(path, meta.len(), modified, page, side)
                ));
                if let Ok(bytes) = fs::read(&cached) {
                    return Ok(Some((bytes, "image/png")));
                }
                let png = extractor
                    .render_page(path, page, side, 2.0, true)?
                    .to_png()?;
                self.store(&cached, &png);
                Ok(Some((png, "image/png")))
            }
            FileKind::Image => match image_mime(path) {
                Some(mime) => Ok(Some((fs::read(path)?, mime))),
                None => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// Saves a rendered thumbnail. A cache that can't be written only costs
    /// a render next time, so failures are ignored.
    fn store(&self, path: &Path, png: &[u8]) {
        if fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, png).is_ok() && fs::rename(&tmp, path).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }

    /// How many pages the document at `path` has: a PDF's page count, 1 for
    /// other files. `None` for online-only files (it would download them).
    pub fn page_count(&self, extractor: &TextExtractor, path: &Path) -> Result<Option<usize>> {
        if cloud_state(path)? == CloudState::CloudOnly {
            return Ok(None);
        }
        match FileKind::from_path(path) {
            FileKind::Pdf => extractor.page_count(path).map(Some),
            _ => Ok(Some(1)),
        }
    }

    // MARK: - Keeping the cache small

    /// If the cache holds more than `max_bytes`, deletes the oldest
    /// thumbnails until it's down to `keep_bytes`.
    pub fn prune(&self, max_bytes: u64, keep_bytes: u64) {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
            .flatten()
            .filter_map(|e| {
                let meta = e.metadata().ok()?;
                meta.is_file()
                    .then(|| (meta.modified().unwrap_or(UNIX_EPOCH), meta.len(), e.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        if total <= max_bytes {
            return;
        }
        files.sort();
        for (_, len, path) in files {
            if total <= keep_bytes {
                break;
            }
            if fs::remove_file(&path).is_ok() {
                total -= len;
            }
        }
    }
}

/// Changes whenever the file does (size, modification time) or another
/// page or size is asked for, so a stale thumbnail is never shown.
fn cache_key(path: &Path, len: u64, modified_ms: u128, page: usize, side: u32) -> String {
    let mut h = Sha256::new();
    h.update(path.to_string_lossy().as_bytes());
    h.update(format!("|{len}|{modified_ms}|{page}|{side}").as_bytes());
    h.finalize()
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_change_with_the_file_and_size() {
        let p = Path::new("/a/b.pdf");
        let k = cache_key(p, 10, 5, 0, 256);
        assert_eq!(k.len(), 32);
        assert_eq!(k, cache_key(p, 10, 5, 0, 256));
        assert_ne!(k, cache_key(p, 11, 5, 0, 256));
        assert_ne!(k, cache_key(p, 10, 6, 0, 256));
        assert_ne!(k, cache_key(p, 10, 5, 1, 256));
        assert_ne!(k, cache_key(p, 10, 5, 0, 1400));
        assert_ne!(k, cache_key(Path::new("/a/c.pdf"), 10, 5, 0, 256));
    }

    #[test]
    fn prune_keeps_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = Thumbnails::new(dir.path().to_path_buf());
        for (i, name) in ["old", "mid", "new"].iter().enumerate() {
            let p = dir.path().join(format!("{name}.png"));
            fs::write(&p, vec![0u8; 100]).unwrap();
            let t = UNIX_EPOCH + std::time::Duration::from_secs(1_000 + i as u64);
            fs::File::options()
                .write(true)
                .open(&p)
                .unwrap()
                .set_modified(t)
                .unwrap();
        }
        thumbs.prune(1_000, 200); // under the limit: nothing goes
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 3);
        thumbs.prune(250, 200);
        assert!(!dir.path().join("old.png").exists());
        assert!(dir.path().join("mid.png").exists() && dir.path().join("new.png").exists());
    }
}
