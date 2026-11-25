//! Listing one folder of a source, without ever reading file contents.
//!
//! Everything here comes from directory enumeration and file attributes, so
//! online-only files (iCloud "Optimize Storage", OneDrive Files On-Demand)
//! are never downloaded just because Rupert looked at them.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::file_kind::{FileKind, is_ignored_name};

// MARK: - Types

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CloudState {
    /// The file's contents are on this disk.
    Local,
    /// Only a placeholder is on disk; opening the file downloads it.
    CloudOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    /// Path relative to the source root, `/`-separated on every platform.
    pub rel_path: String,
    pub is_dir: bool,
    /// `None` for folders.
    pub kind: Option<FileKind>,
    /// Bytes; 0 for folders and when unknown.
    pub size: u64,
    /// Milliseconds since the Unix epoch.
    pub modified_ms: Option<i64>,
    pub cloud: CloudState,
}

// MARK: - Relative paths

/// Joins a `/`-separated path (from the UI) onto a source root, refusing
/// anything that could escape the root.
pub fn resolve(root: &Path, rel: &str) -> Result<PathBuf> {
    let mut path = root.to_path_buf();
    for part in rel.split('/').filter(|p| !p.is_empty()) {
        let mut components = Path::new(part).components();
        match (components.next(), components.next()) {
            // A backslash is a separator on Windows but an ordinary
            // character in macOS and Linux file names.
            (Some(Component::Normal(c)), None) if !(cfg!(windows) && part.contains('\\')) => {
                path.push(c)
            }
            _ => return Err(Error::InvalidPath(rel.to_string())),
        }
    }
    Ok(path)
}

/// The inverse of [`resolve`]: `root/a/b` → `"a/b"`. `None` if `path` is not
/// inside `root`.
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?;
    let parts: Vec<_> = rest
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

fn join_rel(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

// MARK: - Listing

/// Every folder under `root` (its own hidden `.rupert` folders aside), as
/// paths relative to it, from directory listings only: nothing is opened,
/// so online-only files stay where they are. Unreadable folders are skipped.
pub fn all_dirs(root: &Path) -> Vec<String> {
    fn walk(root: &Path, rel: &str, out: &mut Vec<String>) {
        let Ok(entries) = list_dir(root, rel, false) else {
            return;
        };
        for e in entries.into_iter().filter(|e| e.is_dir) {
            out.push(e.rel_path.clone());
            walk(root, &e.rel_path, out);
        }
    }
    let mut out = Vec::new();
    walk(root, "", &mut out);
    out
}

/// Lists the folders and files directly inside `rel` (not recursive).
/// Folders come first, then files, each sorted case-insensitively. Unless
/// `show_all_types` is set, only document files are included.
pub fn list_dir(root: &Path, rel: &str, show_all_types: bool) -> Result<Vec<Entry>> {
    let dir = resolve(root, rel)?;
    let mut entries = Vec::new();
    // A drive that streams files without marking them (Google Drive on
    // Windows): everything on it counts as online-only.
    let streamed = crate::cloud::streams_on_demand(&dir);

    for item in fs::read_dir(&dir)? {
        let Ok(item) = item else { continue };
        let raw_name = item.file_name().to_string_lossy().into_owned();
        if is_ignored_name(&raw_name) {
            continue;
        }

        // Pre-Sonoma iCloud placeholder: ".Name.pdf.icloud" stands for "Name.pdf".
        let (name, placeholder) = match icloud_placeholder_name(&raw_name) {
            Some(real) => (real, true),
            None => (raw_name, false),
        };
        if name.starts_with('.') {
            continue;
        }

        // `DirEntry::metadata` doesn't follow symlinks and, on Windows, comes
        // from the directory listing itself, so it never opens the file.
        let Ok(mut meta) = item.metadata() else {
            continue;
        };
        if meta.file_type().is_symlink() {
            match fs::metadata(item.path()) {
                Ok(target) => meta = target,
                Err(_) => continue, // broken link
            }
        }
        if is_hidden(&meta) {
            continue;
        }

        let is_dir = meta.is_dir();
        let kind = (!is_dir).then(|| FileKind::from_path(&name));
        if let Some(kind) = kind
            && !show_all_types
            && !kind.is_document()
        {
            continue;
        }

        let cloud = if placeholder || (!is_dir && (streamed || is_cloud_only(&meta))) {
            CloudState::CloudOnly
        } else {
            CloudState::Local
        };

        entries.push(Entry {
            rel_path: join_rel(rel.trim_matches('/'), &name),
            name,
            is_dir,
            kind,
            size: if is_dir || placeholder { 0 } else { meta.len() },
            modified_ms: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64),
            cloud,
        });
    }

    entries.sort_by_cached_key(|e| (!e.is_dir, e.name.to_lowercase()));
    Ok(entries)
}

/// Whether a single file is online-only, read from its parent's directory
/// listing (the same no-download path `list_dir` uses).
pub fn cloud_state(path: &Path) -> Result<CloudState> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?;
    if crate::cloud::streams_on_demand(parent) && fs::symlink_metadata(path)?.is_file() {
        return Ok(CloudState::CloudOnly);
    }
    let name = path
        .file_name()
        .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?;
    for item in fs::read_dir(parent)?.flatten() {
        let raw = item.file_name();
        if raw == name {
            let meta = item.metadata()?;
            return Ok(if is_cloud_only(&meta) {
                CloudState::CloudOnly
            } else {
                CloudState::Local
            });
        }
        if icloud_placeholder_name(&raw.to_string_lossy()).as_deref()
            == Some(&*name.to_string_lossy())
        {
            return Ok(CloudState::CloudOnly);
        }
    }
    Err(Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("{} not found", path.display()),
    )))
}

// MARK: - Platform attributes
//
// The decoders are compiled on every platform so their tests run in CI
// everywhere; each OS only calls its own.

/// macOS `SF_DATALESS`: the file is a placeholder whose data lives in the
/// cloud (APFS dataless file, used by iCloud Drive and File Provider apps
/// such as OneDrive).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const SF_DATALESS: u32 = 0x4000_0000;

/// Windows Cloud Files attributes that mean "reading this will download it".
#[cfg_attr(not(windows), allow(dead_code))]
const FILE_ATTRIBUTE_OFFLINE: u32 = 0x0000_1000;
#[cfg_attr(not(windows), allow(dead_code))]
const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
#[cfg_attr(not(windows), allow(dead_code))]
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
#[cfg_attr(not(windows), allow(dead_code))]
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x0000_0002;
#[cfg_attr(not(windows), allow(dead_code))]
const FILE_ATTRIBUTE_SYSTEM: u32 = 0x0000_0004;

#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
fn dataless_flags(flags: u32) -> bool {
    flags & SF_DATALESS != 0
}

#[cfg_attr(not(windows), allow(dead_code))]
fn cloud_only_attributes(attrs: u32) -> bool {
    attrs
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
}

#[cfg_attr(not(windows), allow(dead_code))]
fn hidden_attributes(attrs: u32) -> bool {
    attrs & (FILE_ATTRIBUTE_HIDDEN | FILE_ATTRIBUTE_SYSTEM) != 0
}

#[cfg(target_os = "macos")]
fn is_cloud_only(meta: &fs::Metadata) -> bool {
    use std::os::macos::fs::MetadataExt;
    dataless_flags(meta.st_flags())
}

/// iOS File Provider folders (iCloud Drive, OneDrive) use the same
/// dataless files; older iCloud placeholders are `.name.icloud` files.
#[cfg(target_os = "ios")]
fn is_cloud_only(meta: &fs::Metadata) -> bool {
    use std::os::ios::fs::MetadataExt;
    dataless_flags(meta.st_flags())
}

#[cfg(windows)]
fn is_cloud_only(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    cloud_only_attributes(meta.file_attributes())
}

/// Linux sync clients keep full copies, so nothing is ever online-only.
#[cfg(not(any(target_os = "macos", target_os = "ios", windows)))]
fn is_cloud_only(_meta: &fs::Metadata) -> bool {
    false
}

#[cfg(windows)]
fn is_hidden(meta: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    hidden_attributes(meta.file_attributes())
}

/// Elsewhere "hidden" means a leading dot, which the listing already skips.
#[cfg(not(windows))]
fn is_hidden(_meta: &fs::Metadata) -> bool {
    false
}

fn icloud_placeholder_name(name: &str) -> Option<String> {
    let inner = name.strip_prefix('.')?.strip_suffix(".icloud")?;
    (!inner.is_empty()).then(|| inner.to_string())
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn names(entries: &[Entry]) -> Vec<&str> {
        entries.iter().map(|e| e.name.as_str()).collect()
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Taxes/2025")).unwrap();
        fs::create_dir(root.join("archive")).unwrap();
        fs::create_dir(root.join(".rupert")).unwrap();
        for f in [
            "b-scan.pdf",
            "A-receipt.JPG",
            "setup.exe",
            ".DS_Store",
            ".hidden.pdf",
            "Taxes/W-2.pdf",
        ] {
            fs::write(root.join(f), b"x").unwrap();
        }
        dir
    }

    #[test]
    fn lists_documents_folders_first_sorted() {
        let dir = fixture();
        let entries = list_dir(dir.path(), "", false).unwrap();
        assert_eq!(
            names(&entries),
            ["archive", "Taxes", "A-receipt.JPG", "b-scan.pdf"]
        );
        let taxes = &entries[1];
        assert!(taxes.is_dir && taxes.kind.is_none() && taxes.rel_path == "Taxes");
        let scan = &entries[3];
        assert_eq!(scan.kind, Some(FileKind::Pdf));
        assert_eq!(scan.size, 1);
        assert_eq!(scan.cloud, CloudState::Local);
        assert!(scan.modified_ms.is_some());
    }

    #[test]
    fn show_all_types_includes_non_documents() {
        let dir = fixture();
        let entries = list_dir(dir.path(), "", true).unwrap();
        assert!(names(&entries).contains(&"setup.exe"));
        assert!(!names(&entries).contains(&".DS_Store"));
    }

    #[test]
    fn nested_rel_paths() {
        let dir = fixture();
        let entries = list_dir(dir.path(), "Taxes", false).unwrap();
        assert_eq!(names(&entries), ["2025", "W-2.pdf"]);
        assert_eq!(entries[1].rel_path, "Taxes/W-2.pdf");
        assert_eq!(entries[0].rel_path, "Taxes/2025");
    }

    #[test]
    fn resolve_rejects_escapes() {
        let root = Path::new("/lib");
        assert_eq!(resolve(root, "a/b").unwrap(), Path::new("/lib/a/b"));
        assert_eq!(resolve(root, "").unwrap(), Path::new("/lib"));
        for bad in ["..", "a/../../etc", "./a"] {
            assert!(resolve(root, bad).is_err(), "{bad}");
        }
        let odd = resolve(root, "Scans/TITLE 4;\\ BOAT.pdf");
        if cfg!(windows) {
            assert!(odd.is_err());
            assert!(resolve(root, "a\\..\\b").is_err());
        } else {
            assert_eq!(odd.unwrap(), Path::new("/lib/Scans/TITLE 4;\\ BOAT.pdf"));
        }
    }

    #[test]
    fn relative_round_trips() {
        let root = Path::new("/lib");
        assert_eq!(
            relative(root, Path::new("/lib/a/b")).as_deref(),
            Some("a/b")
        );
        assert_eq!(relative(root, Path::new("/lib")).as_deref(), Some(""));
        assert_eq!(relative(root, Path::new("/other")), None);
    }

    #[test]
    fn icloud_placeholders_map_to_real_names() {
        assert_eq!(
            icloud_placeholder_name(".Lease.pdf.icloud").as_deref(),
            Some("Lease.pdf")
        );
        assert_eq!(icloud_placeholder_name("Lease.pdf"), None);
        assert_eq!(icloud_placeholder_name("..icloud"), None);

        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".Lease.pdf.icloud"), b"plist").unwrap();
        let entries = list_dir(dir.path(), "", false).unwrap();
        assert_eq!(names(&entries), ["Lease.pdf"]);
        assert_eq!(entries[0].cloud, CloudState::CloudOnly);
        assert_eq!(entries[0].size, 0);
    }

    #[test]
    fn decodes_platform_flags() {
        assert!(dataless_flags(0x4000_0000));
        assert!(!dataless_flags(0x0000_0020));
        assert!(cloud_only_attributes(0x0040_0000 | 0x20));
        assert!(cloud_only_attributes(0x1000));
        assert!(!cloud_only_attributes(0x0008_0000 | 0x20)); // pinned, local
        assert!(hidden_attributes(0x2));
        assert!(!hidden_attributes(0x20));
    }
}
