//! Sidecar files: metadata for files that can't hold it themselves, kept in
//! a hidden `.rupert/` folder in the same directory as the file:
//! `Taxes/lease.docx` → `Taxes/.rupert/lease.docx.json`.
//!
//! Each sidecar records the file's size and (when the file is local) a
//! SHA-256 of its contents, so a sidecar separated from its file by a move
//! outside Rupert can be matched up again later.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::meta::DocMeta;

pub const DIR_NAME: &str = ".rupert";
const VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SidecarFile {
    version: u32,
    /// The file this describes (its name at the time of writing).
    file: String,
    size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sha256: Option<String>,
    meta: DocMeta,
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

pub fn read(file: &Path) -> Result<Option<DocMeta>> {
    let path = path_for(file)?;
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice::<SidecarFile>(&bytes)?.meta)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Writes the sidecar atomically. Hashes the file only if `hash_contents`
/// (never for online-only files: reading them would download them).
pub fn write(file: &Path, meta: &DocMeta, hash_contents: bool) -> Result<()> {
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
        meta: meta.clone(),
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&record)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
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
fn hide(dir: &Path) {
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
fn hide(_dir: &Path) {}

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
        write(&file, &meta, true).unwrap();
        assert_eq!(
            path_for(&file).unwrap(),
            dir.path().join(".rupert/photo.jpg.json")
        );
        assert_eq!(read(&file).unwrap(), Some(meta));

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
        write(&file, &DocMeta::default(), false).unwrap();
        let raw: SidecarFile =
            serde_json::from_slice(&fs::read(path_for(&file).unwrap()).unwrap()).unwrap();
        assert_eq!(raw.sha256, None);
    }
}
