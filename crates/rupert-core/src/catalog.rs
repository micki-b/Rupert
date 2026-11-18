//! The shared catalog: a copy of each document's metadata and extracted text
//! in `<source root>/.rupert/catalog/`, synced with the documents, so other
//! machines can show and search files that are online-only there.
//!
//! One small gzip'd JSON file per document, named after a hash of its path,
//! so machines never write the same file unless describing the same
//! document — and then only when something actually changed. The catalog
//! is a cache: entries are trusted only if the file's size and modification
//! time still match.

use std::collections::HashSet;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::Result;
use crate::meta::{DocMeta, Storage};
use crate::sidecar;

const VERSION: u32 = 1;
const EXT: &str = "json.gz";
/// Sync services round modification times differently; allow a little slack.
const MTIME_SLACK_MS: i64 = 2_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub version: u32,
    pub rel_path: String,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub meta: DocMeta,
    pub storage: StorageTag,
    pub text: Option<String>,
}

/// Serializable mirror of [`Storage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StorageTag {
    Embedded,
    Sidecar,
    None,
}

impl From<Storage> for StorageTag {
    fn from(s: Storage) -> Self {
        match s {
            Storage::Embedded => Self::Embedded,
            Storage::Sidecar => Self::Sidecar,
            Storage::None => Self::None,
        }
    }
}

impl From<StorageTag> for Storage {
    fn from(s: StorageTag) -> Self {
        match s {
            StorageTag::Embedded => Self::Embedded,
            StorageTag::Sidecar => Self::Sidecar,
            StorageTag::None => Self::None,
        }
    }
}

impl CatalogEntry {
    pub fn new(
        rel_path: &str,
        size: u64,
        modified_ms: Option<i64>,
        meta: DocMeta,
        storage: Storage,
        text: Option<String>,
    ) -> Self {
        Self {
            version: VERSION,
            rel_path: rel_path.into(),
            size,
            modified_ms,
            meta,
            storage: storage.into(),
            text,
        }
    }

    /// Whether this entry still describes a file with this size and time.
    pub fn matches(&self, size: u64, modified_ms: Option<i64>) -> bool {
        self.size == size
            && match (self.modified_ms, modified_ms) {
                (Some(a), Some(b)) => (a - b).abs() <= MTIME_SLACK_MS,
                _ => false,
            }
    }
}

// MARK: - Paths

pub fn dir(root: &Path) -> PathBuf {
    root.join(sidecar::DIR_NAME).join("catalog")
}

fn key(rel_path: &str) -> String {
    Sha256::digest(rel_path.as_bytes())
        .iter()
        .take(16)
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn entry_path(root: &Path, rel_path: &str) -> PathBuf {
    let k = key(rel_path);
    dir(root).join(&k[..2]).join(format!("{k}.{EXT}"))
}

// MARK: - Read / write

pub fn read(root: &Path, rel_path: &str) -> Result<Option<CatalogEntry>> {
    let path = entry_path(root, rel_path);
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut json = Vec::new();
    if GzDecoder::new(file).read_to_end(&mut json).is_err() {
        return Ok(None); // half-synced or damaged: treat as missing
    }
    Ok(serde_json::from_slice::<CatalogEntry>(&json)
        .ok()
        .filter(|e| e.rel_path == rel_path))
}

/// Writes the entry unless an identical one is already there (so machines
/// that agree don't keep re-uploading it). Returns whether it wrote.
pub fn write(root: &Path, entry: &CatalogEntry) -> Result<bool> {
    if read(root, &entry.rel_path)?.as_ref() == Some(entry) {
        return Ok(false);
    }
    let path = entry_path(root, &entry.rel_path);
    let parent = path.parent().expect("entry has a parent");
    fs::create_dir_all(parent)?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&serde_json::to_vec(entry)?)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, gz.finish()?)?;
    fs::rename(&tmp, &path)?;
    Ok(true)
}

pub fn remove(root: &Path, rel_path: &str) -> Result<()> {
    match fs::remove_file(entry_path(root, rel_path)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Deletes entries for documents that no longer exist, plus leftovers such
/// as sync-conflict copies ("…-MacBook.json.gz") and temp files.
pub fn prune(root: &Path, live_rel_paths: &HashSet<String>) -> Result<usize> {
    let live: HashSet<String> = live_rel_paths.iter().map(|p| key(p)).collect();
    let mut removed = 0;
    let Ok(shards) = fs::read_dir(dir(root)) else {
        return Ok(0);
    };
    for shard in shards.flatten() {
        let Ok(files) = fs::read_dir(shard.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            let keep = name
                .strip_suffix(&format!(".{EXT}"))
                .is_some_and(|k| k.len() == 32 && live.contains(k));
            if !keep && fs::remove_file(file.path()).is_ok() {
                removed += 1;
            }
        }
        let _ = fs::remove_dir(shard.path()); // only if now empty
    }
    Ok(removed)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str) -> CatalogEntry {
        CatalogEntry::new(
            path,
            100,
            Some(1_700_000_000_000),
            DocMeta {
                title: Some("Lease".into()),
                ..Default::default()
            },
            Storage::Embedded,
            Some("The tenant agrees".into()),
        )
    }

    #[test]
    fn write_read_skip_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let e = entry("Home/lease.pdf");
        assert!(write(dir.path(), &e).unwrap());
        assert!(
            !write(dir.path(), &e).unwrap(),
            "identical entry not rewritten"
        );
        assert_eq!(read(dir.path(), "Home/lease.pdf").unwrap(), Some(e));
        assert_eq!(read(dir.path(), "Home/other.pdf").unwrap(), None);
    }

    #[test]
    fn matches_with_mtime_slack() {
        let e = entry("a.pdf");
        assert!(e.matches(100, Some(1_700_000_001_500)));
        assert!(!e.matches(100, Some(1_700_000_005_000)));
        assert!(!e.matches(101, Some(1_700_000_000_000)));
    }

    #[test]
    fn prune_removes_dead_entries_and_conflict_copies() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &entry("keep.pdf")).unwrap();
        write(dir.path(), &entry("gone.pdf")).unwrap();
        let shard = entry_path(dir.path(), "keep.pdf")
            .parent()
            .unwrap()
            .to_path_buf();
        fs::write(shard.join("abc-MacBook.json.gz"), b"x").unwrap();

        let live = HashSet::from(["keep.pdf".to_string()]);
        assert_eq!(prune(dir.path(), &live).unwrap(), 2);
        assert!(read(dir.path(), "keep.pdf").unwrap().is_some());
        assert!(read(dir.path(), "gone.pdf").unwrap().is_none());
    }

    #[test]
    fn damaged_entry_reads_as_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = entry_path(dir.path(), "a.pdf");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not gzip").unwrap();
        assert_eq!(read(dir.path(), "a.pdf").unwrap(), None);
    }
}
