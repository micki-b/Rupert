//! Sidecar files: what Rupert records about a file that the file can't
//! hold itself, kept in a hidden `.rupert/` folder in the same directory:
//! `Taxes/lease.docx` → `Taxes/.rupert/lease.docx.json`.
//!
//! A sidecar can hold:
//! - **metadata** for formats without embedded metadata (and PDFs that can't
//!   be changed safely). This copy is the master.
//! - **pending metadata** for a PDF that was online-only when the details
//!   were recorded (e.g. by the ScanSnap Home import). It fills in what the
//!   PDF lacks and moves into the PDF the next time it's saved in Rupert.
//! - **text** from elsewhere (e.g. ScanSnap Home's OCR) for files whose own
//!   text can't be extracted, so they can be searched.
//!
//! Each sidecar records the file's size and (when the file is local) a
//! SHA-256 of its contents, so a sidecar separated from its file by a move
//! outside Rupert can be matched up again later.

use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::meta::DocMeta;

pub const DIR_NAME: &str = ".rupert";
const VERSION: u32 = 1;

/// The contents of a sidecar.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sidecar {
    pub meta: Option<DocMeta>,
    /// `meta` is waiting to be written into the file (see module docs).
    pub pending: bool,
    pub text: Option<String>,
    /// Where `text` came from, e.g. "ScanSnap Home".
    pub text_source: Option<String>,
}

impl Sidecar {
    pub fn is_empty(&self) -> bool {
        self.meta.is_none() && self.text.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SidecarFile {
    version: u32,
    /// The file this describes (its name at the time of writing).
    file: String,
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    meta: Option<DocMeta>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pending: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_source: Option<String>,
}

// MARK: - Paths

pub fn path_for(file: &Path) -> Result<PathBuf> {
    let name = file
        .file_name()
        .ok_or_else(|| Error::InvalidPath(file.display().to_string()))?;
    let dir = file
        .parent()
        .ok_or_else(|| Error::InvalidPath(file.display().to_string()))?;
    let mut sidecar = name.to_os_string();
    sidecar.push(".json");
    Ok(dir.join(DIR_NAME).join(sidecar))
}

// MARK: - Read / write

pub fn read(file: &Path) -> Result<Option<Sidecar>> {
    let path = path_for(file)?;
    match fs::read(&path) {
        Ok(bytes) => {
            let f: SidecarFile = serde_json::from_slice(&bytes)?;
            Ok(Some(Sidecar {
                meta: f.meta,
                pending: f.pending,
                text: f.text,
                text_source: f.text_source,
            }))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Writes the sidecar atomically, or removes it if `sidecar` is empty.
/// Hashes the file only if `hash_contents` (never for online-only files:
/// reading them would download them).
pub fn write(file: &Path, sidecar: &Sidecar, hash_contents: bool) -> Result<()> {
    if sidecar.is_empty() {
        return remove(file);
    }
    let path = path_for(file)?;
    let dir = path.parent().expect("sidecar path has a parent");
    if !dir.exists() {
        fs::create_dir(dir)?;
        hide(dir);
    }
    let record = SidecarFile {
        version: VERSION,
        file: file
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        size: fs::metadata(file)?.len(),
        sha256: if hash_contents {
            Some(sha256(file)?)
        } else {
            None
        },
        meta: sidecar.meta.clone(),
        pending: sidecar.pending && sidecar.meta.is_some(),
        text: sidecar.text.clone(),
        text_source: sidecar.text.as_ref().and(sidecar.text_source.clone()),
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&record)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Replaces the metadata part, keeping any text.
pub fn write_meta(file: &Path, meta: Option<DocMeta>, pending: bool, hash: bool) -> Result<()> {
    let current = read(file)?.unwrap_or_default();
    write(
        file,
        &Sidecar {
            meta,
            pending,
            ..current
        },
        hash,
    )
}

/// Replaces the text part, keeping any metadata. Returns `false` if the
/// sidecar already held exactly this text.
pub fn write_text(file: &Path, text: &str, source: &str, hash: bool) -> Result<bool> {
    let current = read(file)?.unwrap_or_default();
    if current.text.as_deref() == Some(text) {
        return Ok(false);
    }
    write(
        file,
        &Sidecar {
            text: Some(text.to_string()),
            text_source: Some(source.to_string()),
            ..current
        },
        hash,
    )?;
    Ok(true)
}

/// Deletes the file's sidecar if there is one, and the `.rupert/` folder if
/// that leaves it empty.
pub fn remove(file: &Path) -> Result<()> {
    let path = path_for(file)?;
    match fs::remove_file(&path) {
        Ok(()) => {
            if let Some(dir) = path.parent() {
                let _ = fs::remove_dir(dir); // fails harmlessly if not empty
            }
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Modification times (ms since the epoch) of the sidecars in `dir`'s
/// `.rupert/` folder, keyed by the name of the file each one describes.
/// One directory read, so the indexer can notice sidecar changes cheaply.
pub fn mtimes(dir: &Path) -> HashMap<String, i64> {
    let mut out = HashMap::new();
    let Ok(entries) = fs::read_dir(dir.join(DIR_NAME)) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(file) = name.strip_suffix(".json") else {
            continue;
        };
        let ms = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64);
        if let Some(ms) = ms {
            out.insert(file.to_string(), ms);
        }
    }
    out
}

pub fn sha256(file: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut f = fs::File::open(file)?;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

// MARK: - Hidden folder

/// Dot-folders are already hidden on macOS and Linux; Windows needs the
/// hidden attribute.
#[cfg(windows)]
pub(crate) fn hide(dir: &Path) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_HIDDEN, GetFileAttributesW, INVALID_FILE_ATTRIBUTES, SetFileAttributesW,
    };
    let wide: Vec<u16> = dir.as_os_str().encode_wide().chain([0]).collect();
    unsafe {
        let attrs = GetFileAttributesW(wide.as_ptr());
        if attrs != INVALID_FILE_ATTRIBUTES {
            SetFileAttributesW(wide.as_ptr(), attrs | FILE_ATTRIBUTE_HIDDEN);
        }
    }
}

#[cfg(not(windows))]
pub(crate) fn hide(_dir: &Path) {}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_read_remove() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("photo.jpg");
        fs::write(&file, b"jpeg").unwrap();
        let meta = DocMeta {
            title: Some("Receipt".into()),
            ..Default::default()
        };

        assert_eq!(read(&file).unwrap(), None);
        write_meta(&file, Some(meta.clone()), false, true).unwrap();
        assert_eq!(
            path_for(&file).unwrap(),
            dir.path().join(".rupert/photo.jpg.json")
        );
        assert_eq!(read(&file).unwrap().unwrap().meta, Some(meta));

        let raw: SidecarFile =
            serde_json::from_slice(&fs::read(path_for(&file).unwrap()).unwrap()).unwrap();
        assert_eq!(raw.size, 4);
        assert_eq!(raw.file, "photo.jpg");
        // `printf jpeg | shasum -a 256`
        assert_eq!(
            raw.sha256.as_deref(),
            Some("41e5787e9f28562d07b891b1816b492309d646c0f2829743fa4963a9f9cc1d61")
        );

        remove(&file).unwrap();
        assert_eq!(read(&file).unwrap(), None);
        assert!(!dir.path().join(".rupert").exists(), "empty folder removed");
    }

    #[test]
    fn no_hash_for_online_only() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.png");
        fs::write(&file, b"png").unwrap();
        write_meta(&file, Some(DocMeta::default()), false, false).unwrap();
        let raw: SidecarFile =
            serde_json::from_slice(&fs::read(path_for(&file).unwrap()).unwrap()).unwrap();
        assert_eq!(raw.sha256, None);
    }

    #[test]
    fn text_and_meta_are_kept_separately() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("scan.pdf");
        fs::write(&file, b"%PDF").unwrap();

        assert!(write_text(&file, "invoice total", "ScanSnap Home", false).unwrap());
        assert!(!write_text(&file, "invoice total", "ScanSnap Home", false).unwrap());
        let meta = DocMeta {
            favorite: true,
            ..Default::default()
        };
        write_meta(&file, Some(meta.clone()), true, false).unwrap();
        let s = read(&file).unwrap().unwrap();
        assert_eq!(s.meta, Some(meta));
        assert!(s.pending);
        assert_eq!(s.text.as_deref(), Some("invoice total"));
        assert_eq!(s.text_source.as_deref(), Some("ScanSnap Home"));

        // Dropping the metadata keeps the text; dropping both removes the file.
        write_meta(&file, None, false, false).unwrap();
        let s = read(&file).unwrap().unwrap();
        assert_eq!((s.meta, s.pending), (None, false));
        assert!(s.text.is_some());
        write(&file, &Sidecar::default(), false).unwrap();
        assert!(!path_for(&file).unwrap().exists());
    }

    #[test]
    fn reads_version_1_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.png");
        fs::write(&file, b"png").unwrap();
        fs::create_dir(dir.path().join(DIR_NAME)).unwrap();
        fs::write(
            path_for(&file).unwrap(),
            r#"{"version":1,"file":"a.png","size":3,"meta":{"title":"Old"}}"#,
        )
        .unwrap();
        let s = read(&file).unwrap().unwrap();
        assert_eq!(s.meta.unwrap().title.as_deref(), Some("Old"));
        assert_eq!(mtimes(dir.path()).len(), 1);
        assert!(mtimes(dir.path()).contains_key("a.png"));
    }
}
