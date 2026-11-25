//! Thumbnails for the large-icon and gallery views: the first page of a PDF,
//! rendered once and kept in the app's cache folder (never in the library;
//! the cache can be deleted at any time), or an image file as it is.
//!
//! Online-only files are never read (that would download them), but they
//! show what's cached for them (#62): pictures rendered while the file was on
//! this computer, and a **kept picture** of the first page that Rupert makes
//! for every PDF in a cloud folder while it's here anyway (indexing, a
//! download to index it, or showing it). Kept pictures are small JPEGs with
//! their own, larger budget, since they can't be made again without a
//! download.

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
/// A kept picture's longest side and JPEG quality: sharp enough for the
/// large icons and readable in the gallery, about 50 KB.
const KEEP_SIDE: u32 = 720;
const KEEP_QUALITY: u8 = 75;
/// The kept pictures' folder inside the cache.
const KEPT_DIR: &str = "kept";

/// A picture to show: its bytes and MIME type.
pub type Picture = (Vec<u8>, &'static str);

#[derive(Debug, Clone)]
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
    /// (the view scales them). Online-only files get only what's cached for
    /// them. `None` for kinds without a picture.
    ///
    /// `keep`: the file is in a cloud folder, so it may become online-only;
    /// showing its first page also makes its kept picture.
    pub fn get(
        &self,
        extractor: &TextExtractor,
        path: &Path,
        page: usize,
        side: u32,
        keep: bool,
    ) -> Result<Option<Picture>> {
        let online_only = cloud_state(path)? == CloudState::CloudOnly;
        match FileKind::from_path(path) {
            FileKind::Pdf => {
                let side = side.clamp(MIN_SIDE, MAX_SIDE);
                // Size and date come from the file system; they don't
                // download anything, and evicting a file changes neither.
                let stamp = Stamp::of(path)?;
                let cached = self
                    .dir
                    .join(format!("{}.png", stamp.key(path, page, side)));
                if let Ok(bytes) = fs::read(&cached) {
                    return Ok(Some((bytes, "image/png")));
                }
                if online_only {
                    return Ok((page == 0).then(|| self.kept(path, &stamp)).flatten());
                }
                let png = extractor
                    .render_page(path, page, side, 2.0, true)?
                    .to_png()?;
                self.store(&cached, &png);
                if keep && page == 0 {
                    let _ = self.keep(extractor, path);
                }
                Ok(Some((png, "image/png")))
            }
            _ if online_only => Ok(None),
            FileKind::Image => match image_mime(path) {
                Some(mime) => Ok(Some((fs::read(path)?, mime))),
                None => Ok(None),
            },
            _ => Ok(None),
        }
    }

    /// Saves a rendered picture. A cache that can't be written only costs
    /// a render next time, so failures are ignored.
    fn store(&self, path: &Path, bytes: &[u8]) {
        let Some(dir) = path.parent() else {
            return;
        };
        if fs::create_dir_all(dir).is_err() {
            return;
        }
        let tmp = path.with_extension("tmp");
        if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, path).is_err() {
            let _ = fs::remove_file(&tmp);
        }
    }

    // MARK: - Kept pictures

    /// Makes the kept picture of the PDF at `path` if it has none yet: its
    /// first page, small. Only while the file is on this computer; an
    /// online-only file is left alone. `Ok(true)` if one was made.
    pub fn keep(&self, extractor: &TextExtractor, path: &Path) -> Result<bool> {
        if FileKind::from_path(path) != FileKind::Pdf
            || !extractor.pdf_supported()
            || cloud_state(path)? == CloudState::CloudOnly
        {
            return Ok(false);
        }
        let stamp = Stamp::of(path)?;
        if self.has_kept_stamp(path, &stamp) {
            return Ok(false);
        }
        let file = self.kept_path(path, &stamp);
        match extractor
            .render_page(path, 0, KEEP_SIDE, 2.0, true)
            .and_then(|page| to_jpeg(&page))
        {
            Ok(jpeg) => {
                self.store(&file, &jpeg);
                Ok(true)
            }
            Err(e) => {
                // Noted, so this version of the file isn't downloaded again
                // for a picture that can't be made.
                self.store(&file.with_extension("none"), b"");
                Err(e)
            }
        }
    }

    /// Whether the PDF at `path` has a kept picture for its current version
    /// (or one was tried and couldn't be made). Reads only the file
    /// system's size and date, so it's safe for online-only files.
    pub fn has_kept(&self, path: &Path) -> bool {
        Stamp::of(path).is_ok_and(|stamp| self.has_kept_stamp(path, &stamp))
    }

    fn has_kept_stamp(&self, path: &Path, stamp: &Stamp) -> bool {
        let file = self.kept_path(path, stamp);
        file.exists() || file.with_extension("none").exists()
    }

    /// The kept picture of a PDF, if it has one for this version of the
    /// file. Marks it as just used, so pruning keeps it longer.
    fn kept(&self, path: &Path, stamp: &Stamp) -> Option<Picture> {
        let file = self.kept_path(path, stamp);
        let bytes = fs::read(&file).ok()?;
        if let Ok(f) = fs::File::options().write(true).open(&file) {
            let _ = f.set_modified(std::time::SystemTime::now());
        }
        Some((bytes, "image/jpeg"))
    }

    fn kept_path(&self, path: &Path, stamp: &Stamp) -> PathBuf {
        self.dir
            .join(KEPT_DIR)
            .join(format!("{}.jpg", stamp.key(path, 0, KEEP_SIDE)))
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
    /// thumbnails until it's down to `keep_bytes`. Kept pictures have
    /// their own budget: see [`Self::prune_kept`].
    pub fn prune(&self, max_bytes: u64, keep_bytes: u64) {
        prune_dir(&self.dir, max_bytes, keep_bytes);
    }

    /// [`Self::prune`] for the kept pictures, the least recently shown first.
    pub fn prune_kept(&self, max_bytes: u64, keep_bytes: u64) {
        prune_dir(&self.dir.join(KEPT_DIR), max_bytes, keep_bytes);
    }
}

fn prune_dir(dir: &Path, max_bytes: u64, keep_bytes: u64) {
    let Ok(entries) = fs::read_dir(dir) else {
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

/// A file's size and modification time, which change whenever it does.
struct Stamp {
    len: u64,
    modified_ms: u128,
}

impl Stamp {
    fn of(path: &Path) -> Result<Self> {
        let meta = fs::metadata(path)?;
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis());
        Ok(Self {
            len: meta.len(),
            modified_ms,
        })
    }

    fn key(&self, path: &Path, page: usize, side: u32) -> String {
        cache_key(path, self.len, self.modified_ms, page, side)
    }
}

/// A rendered page as a JPEG (no transparency: pages are rendered on white).
fn to_jpeg(page: &crate::text::RenderedPage) -> Result<Vec<u8>> {
    use jpeg_encoder::{ColorType, Encoder};
    let mut out = Vec::new();
    let color = if page.channels == 1 {
        ColorType::Luma
    } else {
        ColorType::Rgba
    };
    let (w, h) = (
        u16::try_from(page.width)
            .map_err(|_| crate::error::Error::InvalidValue("page too wide".into()))?,
        u16::try_from(page.height)
            .map_err(|_| crate::error::Error::InvalidValue("page too tall".into()))?,
    );
    Encoder::new(&mut out, KEEP_QUALITY)
        .encode(&page.pixels, w, h, color)
        .map_err(|e| crate::error::Error::InvalidValue(e.to_string()))?;
    Ok(out)
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
    fn kept_pictures_are_jpegs() {
        for channels in [1u8, 4] {
            let page = crate::text::RenderedPage {
                width: 30,
                height: 40,
                channels,
                pixels: vec![200; 30 * 40 * channels as usize],
                to_page: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            };
            let jpeg = to_jpeg(&page).unwrap();
            assert_eq!(&jpeg[..2], &[0xFF, 0xD8], "{channels} channels");
        }
    }

    #[test]
    fn kept_pictures_follow_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = Thumbnails::new(dir.path().join("cache"));
        let doc = dir.path().join("bill.pdf");
        fs::write(&doc, b"%PDF-1.7 not really").unwrap();
        let stamp = Stamp::of(&doc).unwrap();
        assert!(thumbs.kept(&doc, &stamp).is_none());
        assert!(!thumbs.has_kept(&doc));

        thumbs.store(&thumbs.kept_path(&doc, &stamp), b"jpeg");
        let old = UNIX_EPOCH + std::time::Duration::from_secs(1_000);
        fs::File::options()
            .write(true)
            .open(thumbs.kept_path(&doc, &stamp))
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(
            thumbs.kept(&doc, &stamp),
            Some((b"jpeg".to_vec(), "image/jpeg"))
        );
        // Shown just now, so pruning keeps it longest.
        let used = fs::metadata(thumbs.kept_path(&doc, &stamp))
            .unwrap()
            .modified()
            .unwrap();
        assert!(used > old);

        assert!(thumbs.has_kept(&doc));

        // A changed file doesn't show the old picture, and needs a new one.
        fs::write(&doc, b"%PDF-1.7 changed, longer").unwrap();
        let stamp = Stamp::of(&doc).unwrap();
        assert!(thumbs.kept(&doc, &stamp).is_none());
        assert!(!thumbs.has_kept(&doc));
        // One that couldn't be made isn't asked for again (no new download).
        thumbs.store(&thumbs.kept_path(&doc, &stamp).with_extension("none"), b"");
        assert!(thumbs.has_kept(&doc));
        assert!(thumbs.kept(&doc, &stamp).is_none());
    }

    #[test]
    fn kept_pictures_have_their_own_budget() {
        let dir = tempfile::tempdir().unwrap();
        let thumbs = Thumbnails::new(dir.path().to_path_buf());
        let kept = dir.path().join(KEPT_DIR);
        fs::create_dir_all(&kept).unwrap();
        fs::write(dir.path().join("a.png"), vec![0u8; 300]).unwrap();
        fs::write(kept.join("b.jpg"), vec![0u8; 300]).unwrap();
        thumbs.prune(100, 0);
        assert!(!dir.path().join("a.png").exists());
        assert!(kept.join("b.jpg").exists());
        thumbs.prune_kept(100, 0);
        assert!(!kept.join("b.jpg").exists());
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
