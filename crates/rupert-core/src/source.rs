//! Sources: the folders the user has chosen to show in Rupert, each shown as
//! its own tree. The list is saved as JSON in the app's config folder.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::bookmark;
use crate::error::{Error, Result};

// MARK: - Provider

/// Which sync service (if any) a source lives in, from its path or what the
/// OS reports. Cloud sources share a catalog (see [`crate::catalog`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Local,
    ICloud,
    OneDrive,
    GoogleDrive,
    Dropbox,
}

impl Provider {
    pub fn detect(path: &Path) -> Self {
        match Self::from_path(path) {
            Self::Local => crate::cloud::provider_of(path).unwrap_or(Self::Local),
            known => known,
        }
    }

    /// From the path alone (the folder names sync services use).
    fn from_path(path: &Path) -> Self {
        for component in path.components() {
            let c = component.as_os_str().to_string_lossy();
            // macOS: ~/Library/Mobile Documents/com~apple~CloudDocs; Windows: ~/iCloudDrive
            if c == "Mobile Documents" || c == "iCloudDrive" || c == "iCloud Drive" {
                return Self::ICloud;
            }
            // macOS: ~/Library/CloudStorage/OneDrive-Personal; Windows: ~/OneDrive, ~/OneDrive - Org
            if c.starts_with("OneDrive") {
                return Self::OneDrive;
            }
            // macOS: ~/Library/CloudStorage/GoogleDrive-<account>; Windows: G:\My Drive
            if c.starts_with("GoogleDrive") || c == "Google Drive" || c == "My Drive" {
                return Self::GoogleDrive;
            }
            if c.starts_with("Dropbox") {
                return Self::Dropbox;
            }
        }
        Self::Local
    }
}

// MARK: - Source

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    pub id: String,
    pub name: String,
    pub root: PathBuf,
    pub provider: Provider,
    /// Show every file type instead of only documents.
    #[serde(default)]
    pub show_all_types: bool,
    /// Download online-only documents to read their text, then free the
    /// space again (see [`crate::cloud`]).
    #[serde(default)]
    pub download_online_only: bool,
    /// macOS: a security-scoped bookmark (base64) for the folder, so a
    /// sandboxed app keeps access across launches and a moved folder is
    /// found again (see [`crate::bookmark`]). Not used on the phone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bookmark: Option<String>,
}

impl Source {
    /// Whether the root folder can be read right now: not on an unmounted
    /// drive or a signed-out sync folder, and (sandboxed) still allowed.
    /// Reading, not just checking it's a folder: a sandbox lets an app see
    /// that a folder exists without letting it look inside.
    pub fn is_available(&self) -> bool {
        fs::read_dir(&self.root).is_ok()
    }
}

// MARK: - SourceStore

#[derive(Debug, Default, Serialize, Deserialize)]
struct SourcesFile {
    sources: Vec<Source>,
}

/// The saved list of sources. Every change is written to disk immediately.
#[derive(Debug)]
pub struct SourceStore {
    path: PathBuf,
    sources: Vec<Source>,
}

impl SourceStore {
    /// Loads the list from `path`; a missing file means no sources yet.
    pub fn load(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let sources = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<SourcesFile>(&bytes)?.sources,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.into()),
        };
        let mut store = Self { path, sources };
        store.redetect_providers()?;
        Ok(store)
    }

    /// Re-checks each reachable source's provider (detection improves over
    /// time, e.g. iCloud "Desktop & Documents" folders). Saves if any changed.
    fn redetect_providers(&mut self) -> Result<()> {
        let mut changed = false;
        for source in self.sources.iter_mut().filter(|s| s.root.is_dir()) {
            let provider = Provider::detect(&source.root);
            if provider != source.provider {
                source.provider = provider;
                changed = true;
            }
        }
        if changed {
            self.save()?;
        }
        Ok(())
    }

    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    pub fn get(&self, id: &str) -> Result<&Source> {
        self.sources
            .iter()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSource)
    }

    /// Adds a folder as a new source. Sources may not repeat or nest, so a
    /// file always belongs to exactly one source.
    pub fn add(&mut self, root: impl Into<PathBuf>, name: Option<String>) -> Result<Source> {
        let root = root.into();
        if !root.is_absolute() {
            return Err(Error::InvalidPath(root.display().to_string()));
        }
        if !root.is_dir() {
            return Err(Error::NotADirectory(root));
        }
        if let Err(e) = fs::read_dir(&root) {
            return Err(Error::InvalidValue(format!(
                "Rupert can't open “{}”: {e}",
                root.display()
            )));
        }
        let key = comparable(&root);
        for existing in &self.sources {
            let other = comparable(&existing.root);
            if other == key {
                return Err(Error::DuplicateSource);
            }
            if key.starts_with(&other) || other.starts_with(&key) {
                return Err(Error::OverlappingSource(existing.name.clone()));
            }
        }

        let provider = Provider::detect(&root);
        let name = name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| default_name(&root, provider));
        let source = Source {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            root,
            provider,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        };
        self.sources.push(source.clone());
        self.save()?;
        Ok(source)
    }

    pub fn remove(&mut self, id: &str) -> Result<()> {
        let before = self.sources.len();
        self.sources.retain(|s| s.id != id);
        if self.sources.len() == before {
            return Err(Error::UnknownSource);
        }
        self.save()
    }

    pub fn update(
        &mut self,
        id: &str,
        name: Option<String>,
        show_all_types: Option<bool>,
        download_online_only: Option<bool>,
    ) -> Result<Source> {
        let source = self
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSource)?;
        if let Some(name) = name.filter(|n| !n.trim().is_empty()) {
            source.name = name;
        }
        if let Some(all) = show_all_types {
            source.show_all_types = all;
        }
        if let Some(download) = download_online_only {
            source.download_online_only = download;
        }
        let updated = source.clone();
        self.save()?;
        Ok(updated)
    }

    /// Points a source at a new location of the same folder (on iOS, where
    /// a saved folder's path can change between launches).
    pub fn set_root(&mut self, id: &str, root: impl Into<PathBuf>) -> Result<()> {
        let root = root.into();
        let source = self
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSource)?;
        if source.root != root {
            source.root = root;
            self.save()?;
        }
        Ok(())
    }

    // MARK: Folder access (macOS)

    /// At launch: opens every source's bookmark (starting access, which a
    /// sandboxed app needs), follows folders that moved, saves refreshed
    /// bookmarks, and quietly makes bookmarks for sources that have none
    /// but are reachable. Returns the sources that couldn't be opened, with
    /// why; they show as unavailable until the user picks them again.
    pub fn open_bookmarks(&mut self) -> Vec<(String, Error)> {
        let mut failed = Vec::new();
        let mut changed = false;
        for source in &mut self.sources {
            match source.bookmark.as_deref().map(bookmark::open) {
                Some(Ok(opened)) => {
                    if opened.path != source.root {
                        source.root = opened.path;
                        changed = true;
                    }
                    if let Some(fresh) = opened.refreshed {
                        source.bookmark = Some(fresh);
                        changed = true;
                    }
                }
                Some(Err(e)) => failed.push((source.id.clone(), e)),
                None if source.root.is_dir() => {
                    if let Ok(Some(mark)) = bookmark::create(&source.root) {
                        source.bookmark = Some(mark);
                        changed = true;
                    }
                }
                None => {}
            }
        }
        if changed {
            let _ = self.save();
        }
        let _ = self.redetect_providers();
        failed
    }

    /// Records the user's pick of `root` for a source: its bookmark, and the
    /// new place if it moved. Used for new sources and for "Allow access…".
    pub fn remember_pick(&mut self, id: &str, root: impl Into<PathBuf>) -> Result<Source> {
        let root = root.into();
        let mark = bookmark::create(&root)?;
        let source = self
            .sources
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSource)?;
        source.root = root;
        source.bookmark = mark;
        let source = source.clone();
        self.save()?;
        Ok(source)
    }

    /// Writes atomically (temp file + rename) so a crash never leaves a
    /// half-written list.
    fn save(&self) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = SourcesFile {
            sources: self.sources.clone(),
        };
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&file)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

// MARK: - Helpers

/// Path form used to compare sources: resolved symlinks where possible,
/// lowercase on case-insensitive platforms.
fn comparable(path: &Path) -> PathBuf {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if cfg!(any(target_os = "macos", windows)) {
        PathBuf::from(resolved.to_string_lossy().to_lowercase())
    } else {
        resolved
    }
}

fn default_name(root: &Path, provider: Provider) -> String {
    let folder = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string());
    match (provider, folder.as_str()) {
        (Provider::ICloud, "com~apple~CloudDocs" | "iCloudDrive") => "iCloud Drive".into(),
        _ => folder,
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_providers() {
        let cases = [
            (
                "/Users/a/Library/Mobile Documents/com~apple~CloudDocs/Scans",
                Provider::ICloud,
            ),
            (
                "/Users/a/Library/CloudStorage/OneDrive-Personal/Docs",
                Provider::OneDrive,
            ),
            (
                "/Users/a/Library/CloudStorage/GoogleDrive-a@b.com/My Drive",
                Provider::GoogleDrive,
            ),
            ("/Users/a/Dropbox/Taxes", Provider::Dropbox),
            ("/Users/a/Documents", Provider::Local),
        ];
        for (path, expected) in cases {
            assert_eq!(Provider::from_path(Path::new(path)), expected, "{path}");
        }
    }

    #[test]
    fn add_update_remove_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("Docs");
        fs::create_dir(&docs).unwrap();
        let file = dir.path().join("config/sources.json");

        let mut store = SourceStore::load(&file).unwrap();
        assert!(store.sources().is_empty());
        let added = store.add(&docs, None).unwrap();
        assert_eq!(added.name, "Docs");
        assert!(!added.show_all_types);

        store
            .update(&added.id, Some("Paper".into()), Some(true), Some(true))
            .unwrap();
        let reloaded = SourceStore::load(&file).unwrap();
        assert_eq!(reloaded.sources().len(), 1);
        assert_eq!(reloaded.sources()[0].name, "Paper");
        assert!(reloaded.sources()[0].show_all_types);
        assert!(reloaded.sources()[0].download_online_only);

        store.remove(&added.id).unwrap();
        assert!(SourceStore::load(&file).unwrap().sources().is_empty());
    }

    #[test]
    fn rejects_duplicates_nesting_and_non_folders() {
        let dir = tempfile::tempdir().unwrap();
        let docs = dir.path().join("Docs");
        let inner = docs.join("Inner");
        fs::create_dir_all(&inner).unwrap();
        let mut store = SourceStore::load(dir.path().join("sources.json")).unwrap();

        store.add(&docs, None).unwrap();
        assert!(matches!(
            store.add(&docs, None),
            Err(Error::DuplicateSource)
        ));
        assert!(matches!(
            store.add(&inner, None),
            Err(Error::OverlappingSource(_))
        ));
        assert!(matches!(
            store.add(dir.path(), None),
            Err(Error::OverlappingSource(_))
        ));
        assert!(matches!(
            store.add(dir.path().join("missing"), None),
            Err(Error::NotADirectory(_))
        ));
        assert!(matches!(
            store.add("relative/path", None),
            Err(Error::InvalidPath(_))
        ));
    }

    #[test]
    fn sibling_with_common_prefix_is_not_nested() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Tax")).unwrap();
        fs::create_dir(dir.path().join("Taxes")).unwrap();
        let mut store = SourceStore::load(dir.path().join("sources.json")).unwrap();
        store.add(dir.path().join("Tax"), None).unwrap();
        store.add(dir.path().join("Taxes"), None).unwrap();
    }

    /// macOS: a source without a bookmark gets one at launch; a moved
    /// folder is followed; a new pick is remembered.
    #[cfg(target_os = "macos")]
    #[test]
    fn bookmarks_follow_folders() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sources.json");
        let boat = dir.path().join("Boat");
        fs::create_dir(&boat).unwrap();
        let id = {
            let mut store = SourceStore::load(&file).unwrap();
            let s = store.add(&boat, None).unwrap();
            assert!(s.bookmark.is_none());
            assert!(store.open_bookmarks().is_empty());
            assert!(store.get(&s.id).unwrap().bookmark.is_some(), "made quietly");
            s.id
        };

        let moved = dir.path().join("Sailing");
        fs::rename(&boat, &moved).unwrap();
        let mut store = SourceStore::load(&file).unwrap();
        assert!(store.open_bookmarks().is_empty());
        assert_eq!(
            store.get(&id).unwrap().root.canonicalize().unwrap(),
            moved.canonicalize().unwrap()
        );

        let other = dir.path().join("Elsewhere");
        fs::create_dir(&other).unwrap();
        let picked = store.remember_pick(&id, &other).unwrap();
        assert_eq!(picked.root, other);
        assert!(picked.bookmark.is_some());
        let reloaded = SourceStore::load(&file).unwrap();
        assert_eq!(reloaded.get(&id).unwrap().root, other);
    }
}
