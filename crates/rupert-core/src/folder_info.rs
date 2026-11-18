//! Details about a folder itself, such as being a favorite. Kept in the
//! folder's own hidden `.rupert/.folder.json`, so they follow the folder
//! when it is renamed or moved (in Rupert or elsewhere) and sync to other
//! machines with it. The leading dot keeps the name apart from document
//! sidecars, which are named after their (never hidden) documents.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::sidecar;

const VERSION: u32 = 1;
/// The file's name inside a folder's `.rupert` directory.
pub const FILE_NAME: &str = ".folder.json";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderInfo {
    #[serde(default)]
    pub favorite: bool,
}

impl FolderInfo {
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Serialize, Deserialize)]
struct FolderFile {
    version: u32,
    #[serde(flatten)]
    info: FolderInfo,
}

/// Where a folder's details live.
pub fn path_for(dir: &Path) -> PathBuf {
    dir.join(sidecar::DIR_NAME).join(FILE_NAME)
}

/// The same, relative to a source root, for a folder's relative path.
pub fn rel_path_for(rel_dir: &str) -> String {
    let rel_dir = rel_dir.trim_matches('/');
    if rel_dir.is_empty() {
        format!("{}/{FILE_NAME}", sidecar::DIR_NAME)
    } else {
        format!("{rel_dir}/{}/{FILE_NAME}", sidecar::DIR_NAME)
    }
}

/// A folder's details; `None` if it has none (or the file is damaged).
pub fn read(dir: &Path) -> Option<FolderInfo> {
    parse(&fs::read(path_for(dir)).ok()?)
}

/// Decodes details fetched by other means (e.g. from OneDrive).
pub fn parse(bytes: &[u8]) -> Option<FolderInfo> {
    serde_json::from_slice::<FolderFile>(bytes)
        .ok()
        .map(|f| f.info)
        .filter(|i| !i.is_empty())
}

/// Saves a folder's details; empty details remove the file (and the
/// `.rupert` folder if nothing else is in it).
pub fn write(dir: &Path, info: &FolderInfo) -> Result<()> {
    let path = path_for(dir);
    let hidden = path.parent().expect("has a parent");
    if info.is_empty() {
        match fs::remove_file(&path) {
            Ok(()) => {
                let _ = fs::remove_dir(hidden); // only if empty
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        return Ok(());
    }
    if !hidden.exists() {
        fs::create_dir(hidden)?;
        sidecar::hide(hidden);
    }
    let file = FolderFile {
        version: VERSION,
        info: info.clone(),
    };
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(&file)?)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorite_round_trip_and_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("Boat");
        fs::create_dir(&folder).unwrap();
        assert_eq!(read(&folder), None);

        write(&folder, &FolderInfo { favorite: true }).unwrap();
        assert_eq!(read(&folder), Some(FolderInfo { favorite: true }));
        let json = fs::read_to_string(path_for(&folder)).unwrap();
        assert!(
            json.contains("\"favorite\": true") && json.contains("\"version\": 1"),
            "{json}"
        );

        // Moving the folder takes the favorite along.
        let moved = dir.path().join("Boating");
        fs::rename(&folder, &moved).unwrap();
        assert_eq!(read(&moved), Some(FolderInfo { favorite: true }));

        // Unfavoriting removes the file and the then-empty hidden folder.
        write(&moved, &FolderInfo::default()).unwrap();
        assert_eq!(read(&moved), None);
        assert!(!moved.join(sidecar::DIR_NAME).exists());
    }

    #[test]
    fn keeps_document_sidecars_when_unfavoriting() {
        let dir = tempfile::tempdir().unwrap();
        let doc = dir.path().join("lease.txt");
        fs::write(&doc, "lease").unwrap();
        sidecar::write_text(&doc, "lease text", "test", false).unwrap();
        write(dir.path(), &FolderInfo { favorite: true }).unwrap();
        write(dir.path(), &FolderInfo::default()).unwrap();
        assert!(sidecar::read(&doc).unwrap().is_some(), "sidecar untouched");
        assert!(dir.path().join(sidecar::DIR_NAME).is_dir());
    }

    #[test]
    fn relative_paths_and_parsing() {
        assert_eq!(rel_path_for(""), ".rupert/.folder.json");
        assert_eq!(
            rel_path_for("Boat/Engine/"),
            "Boat/Engine/.rupert/.folder.json"
        );
        assert_eq!(
            parse(br#"{"version":1,"favorite":true}"#),
            Some(FolderInfo { favorite: true })
        );
        assert_eq!(parse(br#"{"version":1}"#), None, "nothing set");
        assert_eq!(parse(b"garbage"), None);
    }
}
