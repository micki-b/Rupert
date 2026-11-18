//! Watching sources for changes made outside Rupert (Finder, Explorer, sync
//! clients) and reporting which folders changed.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use notify_debouncer_mini::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use serde::Serialize;

use crate::error::Result;
use crate::file_kind::is_ignored_name;
use crate::listing::relative;
use crate::sidecar;
use crate::source::Source;

const DEBOUNCE: Duration = Duration::from_millis(400);

// MARK: - DirChange

/// A folder whose contents changed.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirChange {
    pub source_id: String,
    /// `/`-separated, relative to the source root; `""` is the root.
    pub rel_dir: String,
}

// MARK: - LibraryWatcher

#[derive(Debug, Clone)]
struct WatchedRoot {
    source_id: String,
    /// The path as the user chose it.
    root: PathBuf,
    /// The fully resolved path. Some backends (FSEvents) report resolved
    /// paths, e.g. `/private/var/...` for `/var/...`.
    canonical: PathBuf,
}

/// Watches every source root recursively and calls `on_change` with the
/// folders whose contents changed, batched and de-duplicated.
pub struct LibraryWatcher {
    debouncer: Debouncer<RecommendedWatcher>,
    roots: Arc<RwLock<Vec<WatchedRoot>>>,
}

impl LibraryWatcher {
    pub fn new(on_change: impl Fn(Vec<DirChange>) + Send + 'static) -> Result<Self> {
        let roots: Arc<RwLock<Vec<WatchedRoot>>> = Arc::default();
        let roots_for_events = Arc::clone(&roots);
        let debouncer = new_debouncer(DEBOUNCE, move |result: DebounceEventResult| {
            let Ok(events) = result else { return };
            let roots = roots_for_events.read().unwrap();
            let changes: BTreeSet<DirChange> = events
                .iter()
                .filter_map(|e| changed_dir(&roots, &e.path))
                .collect();
            if !changes.is_empty() {
                on_change(changes.into_iter().collect());
            }
        })?;
        Ok(Self { debouncer, roots })
    }

    /// Makes the watched set match `sources`. Unavailable roots are skipped;
    /// call again when they come back.
    pub fn set_sources(&mut self, sources: &[Source]) -> Result<()> {
        let mut roots = self.roots.write().unwrap();
        for old in roots.drain(..) {
            let _ = self.debouncer.watcher().unwatch(&old.root);
        }
        for source in sources.iter().filter(|s| s.is_available()) {
            self.debouncer
                .watcher()
                .watch(&source.root, RecursiveMode::Recursive)?;
            roots.push(WatchedRoot {
                source_id: source.id.clone(),
                root: source.root.clone(),
                canonical: std::fs::canonicalize(&source.root)
                    .unwrap_or_else(|_| source.root.clone()),
            });
        }
        Ok(())
    }
}

/// Maps a changed path to the folder that contains it, or `None` if the
/// change is irrelevant (outside every source, or inside ignored files such
/// as `.rupert/catalog/` or `.DS_Store`). A changed sidecar
/// (`Taxes/.rupert/W-2.pdf.json`, e.g. synced from another machine) counts
/// as a change to the document's folder.
fn changed_dir(roots: &[WatchedRoot], path: &Path) -> Option<DirChange> {
    let root = roots
        .iter()
        .find(|r| path.starts_with(&r.canonical) || path.starts_with(&r.root))?;
    let base = if path.starts_with(&root.canonical) {
        &root.canonical
    } else {
        &root.root
    };
    let rel = relative(base, path)?;
    if rel.is_empty() {
        return None; // the root itself
    }
    let rel = sidecar_owner(&rel).unwrap_or(rel);
    if rel.split('/').any(is_ignored_name) {
        return None;
    }
    let rel_dir = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    Some(DirChange {
        source_id: root.source_id.clone(),
        rel_dir: rel_dir.to_string(),
    })
}

/// `Taxes/.rupert/W-2.pdf.json` → `Taxes/W-2.pdf`.
fn sidecar_owner(rel: &str) -> Option<String> {
    let (parent, name) = rel.rsplit_once('/').unwrap_or(("", rel));
    let dir = parent.strip_suffix(sidecar::DIR_NAME)?;
    if !(dir.is_empty() || dir.ends_with('/')) {
        return None;
    }
    let file = name.strip_suffix(".json")?;
    Some(format!("{dir}{file}"))
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    fn root(id: &str, path: &str) -> WatchedRoot {
        WatchedRoot {
            source_id: id.into(),
            root: path.into(),
            canonical: path.into(),
        }
    }

    #[test]
    fn maps_paths_to_parent_folders() {
        let roots = [root("a", "/lib/a"), root("b", "/lib/b")];
        let change = |p: &str| changed_dir(&roots, Path::new(p));

        assert_eq!(
            change("/lib/a/Taxes/W-2.pdf"),
            Some(DirChange {
                source_id: "a".into(),
                rel_dir: "Taxes".into()
            })
        );
        assert_eq!(change("/lib/b/scan.pdf").unwrap().rel_dir, "");
        assert_eq!(change("/lib/b/scan.pdf").unwrap().source_id, "b");
        assert_eq!(change("/lib/a"), None);
        assert_eq!(change("/elsewhere/x.pdf"), None);
        assert_eq!(change("/lib/a/.DS_Store"), None);
        assert_eq!(
            change("/lib/a/Taxes/.rupert/W-2.pdf.json").unwrap().rel_dir,
            "Taxes"
        );
        assert_eq!(change("/lib/a/.rupert/scan.pdf.json").unwrap().rel_dir, "");
        // A folder's own details (favorite) re-sync that folder.
        assert_eq!(
            change("/lib/a/Taxes/.rupert/.folder.json").unwrap().rel_dir,
            "Taxes"
        );
        assert_eq!(change("/lib/a/.rupert/.folder.json").unwrap().rel_dir, "");
        assert_eq!(change("/lib/a/Taxes/.rupert/W-2.pdf.json.tmp"), None);
        assert_eq!(change("/lib/a/.rupert/catalog/ab/ab12.json.gz"), None);
        assert_eq!(change("/lib/a/.rupert/catalog/ab.json"), None);
        assert_eq!(
            change("/lib/a/x.rupert/a.json").unwrap().rel_dir,
            "x.rupert"
        );
    }

    #[test]
    fn reports_new_file_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Inbox")).unwrap();
        let mut store = crate::source::SourceStore::load(dir.path().join("sources.json")).unwrap();
        let source = store.add(dir.path().join("Inbox"), None).unwrap();

        let (tx, rx) = mpsc::channel();
        let mut watcher = LibraryWatcher::new(move |changes| {
            let _ = tx.send(changes);
        })
        .unwrap();
        watcher.set_sources(store.sources()).unwrap();

        // Give the backend a moment to start (FSEvents can miss very early writes).
        std::thread::sleep(Duration::from_millis(500));
        std::fs::write(dir.path().join("Inbox/scan.pdf"), b"x").unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            let Ok(changes) = rx.recv_timeout(left) else {
                break;
            };
            if changes
                .iter()
                .any(|c| c.source_id == source.id && c.rel_dir.is_empty())
            {
                return;
            }
        }
        panic!("no change reported for the new file");
    }
}
