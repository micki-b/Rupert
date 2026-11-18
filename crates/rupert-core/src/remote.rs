//! Sources read through a web API instead of the file system: on the
//! iPhone, OneDrive folders can't be opened through the Files app, so the
//! phone lists them through Microsoft Graph (see `onedrive`).
//!
//! The service supplies the listing. Rupert's own catalog entries and
//! sidecars are fetched as the small files they are; documents are never
//! fetched while indexing. Read-only: nothing is written back.

use std::collections::{HashMap, HashSet};

use crate::catalog;
use crate::error::Result;
use crate::file_kind::{FileKind, is_ignored_name};
use crate::index::{DocRow, Index, TextState};
use crate::indexer::online_only_details;
use crate::listing::CloudState;
use crate::sidecar;
use crate::source::Source;

/// Files fetched at once (small, latency-bound requests).
const PARALLEL_FETCHES: usize = 6;
/// Documents processed between progress reports and index writes.
const BATCH: usize = 48;

// MARK: - Types

/// A file below a remote source, as the service lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Relative to the source folder, `/`-separated.
    pub rel_path: String,
    pub size: u64,
    /// The file's own modification time (as the computer that wrote it
    /// saw it), which is what catalog entries are checked against.
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SyncReport {
    pub indexed: usize,
    pub removed: usize,
}

/// Fetches a file below the source by relative path.
pub type Fetch<'a> = dyn Fn(&str) -> Result<Vec<u8>> + Sync + 'a;

// MARK: - Sync

/// Brings the index in line with a complete listing of a remote source.
/// `on_progress(done, total)` is called as documents are processed.
pub fn sync(
    index: &Index,
    source: &Source,
    files: &[RemoteFile],
    fetch: &Fetch,
    on_progress: &mut dyn FnMut(usize, usize),
) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let mut hidden: HashMap<&str, &RemoteFile> = HashMap::new();
    let mut docs: Vec<&RemoteFile> = Vec::new();
    for f in files {
        let parts: Vec<&str> = f.rel_path.split('/').collect();
        if parts.contains(&sidecar::DIR_NAME) {
            hidden.insert(&f.rel_path, f);
        } else if is_shown(&parts, source.show_all_types) {
            docs.push(f);
        }
    }

    // Vanished documents.
    let known = index.stamps_under(&source.id, "")?;
    let present: HashSet<&str> = docs.iter().map(|f| f.rel_path.as_str()).collect();
    for rel in known.keys().filter(|p| !present.contains(p.as_str())) {
        index.remove(&source.id, rel)?;
        report.removed += 1;
    }

    // New or changed documents (or a catalog entry has appeared for one
    // that had none).
    let sidecar_ms = |f: &RemoteFile| {
        hidden
            .get(sidecar::rel_path_for(&f.rel_path).as_str())
            .and_then(|s| s.modified_ms)
    };
    let todo: Vec<&RemoteFile> = docs
        .into_iter()
        .filter(|f| match known.get(&f.rel_path) {
            None => true,
            Some(k) => {
                k.size != f.size
                    || k.modified_ms != f.modified_ms
                    || k.cloud != CloudState::CloudOnly
                    || k.sidecar_ms != sidecar_ms(f)
                    || (k.text_state == TextState::CloudOnly
                        && hidden.contains_key(catalog::entry_rel_path(&f.rel_path).as_str()))
            }
        })
        .collect();

    let total = todo.len();
    on_progress(0, total);
    for (n, batch) in todo.chunks(BATCH).enumerate() {
        let details = fetch_details(batch, &hidden, fetch);
        for (f, (entry, side)) in batch.iter().zip(details) {
            let entry = entry.filter(|c| c.matches(f.size, f.modified_ms));
            let (meta, storage, text_state, body) = online_only_details(entry, side.as_ref());
            let name = f.rel_path.rsplit('/').next().unwrap_or(&f.rel_path);
            index.upsert(
                &DocRow {
                    source_id: source.id.clone(),
                    rel_path: f.rel_path.clone(),
                    name: name.to_string(),
                    kind: FileKind::from_path(name),
                    size: f.size,
                    modified_ms: f.modified_ms,
                    cloud: CloudState::CloudOnly,
                    meta,
                    storage,
                    text_state,
                    sidecar_ms: sidecar_ms(f),
                },
                body.as_deref(),
            )?;
            report.indexed += 1;
        }
        on_progress(((n + 1) * BATCH).min(total), total);
    }
    Ok(report)
}

/// Same filtering as the folder view: no hidden or ignored names, and only
/// documents unless the source shows everything.
fn is_shown(parts: &[&str], show_all_types: bool) -> bool {
    let name = parts.last().copied().unwrap_or_default();
    !parts
        .iter()
        .any(|p| p.starts_with('.') || is_ignored_name(p))
        && (show_all_types || FileKind::from_path(name).is_document())
}

type Details = (Option<catalog::CatalogEntry>, Option<sidecar::Sidecar>);

/// Fetches the catalog entry and sidecar of each document that has them,
/// a few at a time. Failures count as missing.
fn fetch_details(
    batch: &[&RemoteFile],
    hidden: &HashMap<&str, &RemoteFile>,
    fetch: &Fetch,
) -> Vec<Details> {
    let one = |f: &RemoteFile| -> Details {
        let entry_path = catalog::entry_rel_path(&f.rel_path);
        let entry = hidden
            .contains_key(entry_path.as_str())
            .then(|| fetch(&entry_path).ok())
            .flatten()
            .and_then(|bytes| catalog::parse(&bytes, &f.rel_path));
        let side_path = sidecar::rel_path_for(&f.rel_path);
        let side = hidden
            .contains_key(side_path.as_str())
            .then(|| fetch(&side_path).ok())
            .flatten()
            .and_then(|bytes| sidecar::parse(&bytes).ok());
        (entry, side)
    };
    let mut out: Vec<Option<Details>> = vec![None; batch.len()];
    std::thread::scope(|scope| {
        for (chunk_out, chunk) in out
            .chunks_mut(batch.len().div_ceil(PARALLEL_FETCHES).max(1))
            .zip(batch.chunks(batch.len().div_ceil(PARALLEL_FETCHES).max(1)))
        {
            let one = &one;
            scope.spawn(move || {
                for (slot, f) in chunk_out.iter_mut().zip(chunk) {
                    *slot = Some(one(f));
                }
            });
        }
    });
    out.into_iter().map(Option::unwrap_or_default).collect()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SearchQuery;
    use crate::source::Provider;
    use std::fs;
    use std::path::Path;
    use std::sync::Mutex;

    fn source(root: &Path, provider: Provider) -> Source {
        Source {
            id: "remote".into(),
            name: "Boating".into(),
            root: root.to_path_buf(),
            provider,
            show_all_types: false,
            download_online_only: false,
        }
    }

    /// Everything below `root`, as a service would list it.
    fn listing(root: &Path) -> Vec<RemoteFile> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for e in fs::read_dir(dir).unwrap().flatten() {
                let meta = e.metadata().unwrap();
                if meta.is_dir() {
                    stack.push(e.path());
                } else {
                    out.push(RemoteFile {
                        rel_path: crate::listing::relative(root, &e.path()).unwrap(),
                        size: meta.len(),
                        modified_ms: meta
                            .modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as i64),
                    });
                }
            }
        }
        out
    }

    fn search(index: &Index, text: &str) -> Vec<String> {
        index
            .search(&SearchQuery {
                text: text.into(),
                ..Default::default()
            })
            .unwrap()
            .into_iter()
            .map(|h| h.rel_path)
            .collect()
    }

    #[test]
    fn indexes_from_catalog_and_sidecars_without_fetching_documents() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Engine")).unwrap();
        fs::write(
            root.join("Engine/impeller.md"),
            "Replace the raw water impeller",
        )
        .unwrap();
        fs::write(root.join("insurance.pdf"), "not really a pdf").unwrap();
        fs::write(root.join("setup.exe"), "binary").unwrap();
        // The computer indexes the folder as a cloud source: catalog written.
        crate::indexer::tests::index_as_cloud(root);
        // Details for the PDF in a sidecar (e.g. from the ScanSnap import).
        let mut meta = crate::meta::DocMeta {
            title: Some("Boat insurance".into()),
            tags: vec!["Boat".into()],
            ..Default::default()
        };
        meta.favorite = true;
        sidecar::write_meta(&root.join("insurance.pdf"), Some(meta), false, false).unwrap();

        let files = listing(root);
        let fetched = Mutex::new(Vec::<String>::new());
        let fetch = |rel: &str| -> Result<Vec<u8>> {
            fetched.lock().unwrap().push(rel.to_string());
            Ok(fs::read(root.join(rel))?)
        };
        let index = Index::open_in_memory().unwrap();
        let src = source(Path::new("/nowhere"), Provider::OneDrive);
        let mut progress = Vec::new();
        let report = sync(&index, &src, &files, &fetch, &mut |d, t| {
            progress.push((d, t))
        })
        .unwrap();
        assert_eq!(report.indexed, 2);
        assert_eq!(progress.last(), Some(&(2, 2)));
        assert_eq!(search(&index, "impeller"), ["Engine/impeller.md"]);
        assert_eq!(search(&index, "insurance boat"), ["insurance.pdf"]);
        let doc = index.document("remote", "insurance.pdf").unwrap().unwrap();
        assert!(doc.hit.favorite);
        assert_eq!(doc.hit.cloud, CloudState::CloudOnly);
        assert!(
            fetched
                .lock()
                .unwrap()
                .iter()
                .all(|p| p.contains(".rupert/")),
            "only Rupert's own files are fetched: {:?}",
            fetched.lock().unwrap()
        );

        // Nothing changed: nothing fetched or re-indexed.
        fetched.lock().unwrap().clear();
        let again = sync(&index, &src, &files, &fetch, &mut |_, _| {}).unwrap();
        assert_eq!(again, SyncReport::default());
        assert!(fetched.lock().unwrap().is_empty());

        // A document gone from the listing leaves the index.
        let fewer: Vec<RemoteFile> = files
            .into_iter()
            .filter(|f| f.rel_path != "Engine/impeller.md")
            .collect();
        let gone = sync(&index, &src, &fewer, &fetch, &mut |_, _| {}).unwrap();
        assert_eq!(gone.removed, 1);
        assert!(search(&index, "impeller").is_empty());
    }

    #[test]
    fn stale_catalog_entries_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("lease.md"), "Slip lease for berth C-14").unwrap();
        crate::indexer::tests::index_as_cloud(root);
        let mut files = listing(root);
        for f in files.iter_mut().filter(|f| f.rel_path == "lease.md") {
            f.size += 10; // edited elsewhere since the computer indexed it
        }
        let fetch = |rel: &str| -> Result<Vec<u8>> { Ok(fs::read(root.join(rel))?) };
        let index = Index::open_in_memory().unwrap();
        let src = source(Path::new("/nowhere"), Provider::OneDrive);
        sync(&index, &src, &files, &fetch, &mut |_, _| {}).unwrap();
        assert!(search(&index, "berth").is_empty());
        assert_eq!(search(&index, "lease"), ["lease.md"], "still found by name");
    }
}
