//! Background indexing: keeps the search index in step with the sources.
//!
//! Runs on its own thread and processes [`Job`]s: a full sync at startup
//! (only changed files are re-read), folder syncs when the watcher reports
//! changes, and single files after Rupert saves metadata.
//!
//! Online-only files are never read. Their entry comes from the shared
//! catalog when another machine has indexed them, otherwise from the name.

use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::catalog::{self, CatalogEntry};
use crate::error::Result;
use crate::file_kind::FileKind;
use crate::index::{DocRow, Index, Stamp, TextState};
use crate::listing::{CloudState, Entry, list_dir};
use crate::meta::{self, Storage};
use crate::sidecar;
use crate::source::{Provider, Source};
use crate::text::TextExtractor;

// MARK: - Jobs and progress

#[derive(Debug, Clone)]
pub enum Job {
    /// Bring every source up to date and drop sources not listed.
    SyncAll(Vec<Source>),
    /// Bring one source up to date (e.g. just added, or its filter changed).
    SyncSource(Source),
    /// One folder changed (non-recursive, plus vanished/new subfolders).
    SyncDir {
        source: Source,
        rel_dir: String,
    },
    /// One file changed (e.g. Rupert just saved its metadata).
    File {
        source: Source,
        rel_path: String,
    },
    RemoveSource(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub active: bool,
    pub done: usize,
    pub total: usize,
    pub current: Option<String>,
    /// False when PDFium couldn't be loaded: PDFs get no body text.
    pub pdf_text: bool,
}

#[derive(Clone)]
pub struct IndexerHandle {
    tx: Sender<Job>,
}

impl IndexerHandle {
    pub fn send(&self, job: Job) {
        let _ = self.tx.send(job);
    }
}

/// Starts the indexer thread. `on_progress` is called while work is in
/// progress (throttled) and once when idle; `on_changed` after the index
/// changed, so the UI can refresh searches and tag lists.
pub fn spawn(
    index_path: PathBuf,
    pdfium_dir: Option<PathBuf>,
    on_progress: impl Fn(Progress) + Send + 'static,
    on_changed: impl Fn() + Send + 'static,
) -> Result<IndexerHandle> {
    let index = Index::open(&index_path)?;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("rupert-indexer".into())
        .spawn(move || {
            let extractor = TextExtractor::new(pdfium_dir.as_deref());
            let mut worker = Worker {
                index,
                extractor,
                on_progress: Box::new(on_progress),
                on_changed: Box::new(on_changed),
                progress: Progress::default(),
                last_report: Instant::now(),
                dirty: false,
            };
            worker.run(rx);
        })?;
    Ok(IndexerHandle { tx })
}

// MARK: - Worker

struct Worker {
    index: Index,
    extractor: TextExtractor,
    on_progress: Box<dyn Fn(Progress) + Send>,
    on_changed: Box<dyn Fn() + Send>,
    progress: Progress,
    last_report: Instant,
    dirty: bool,
}

/// A document file found on disk.
struct Found {
    rel_path: String,
    entry: Entry,
}

impl Worker {
    fn run(&mut self, rx: Receiver<Job>) {
        self.progress.pdf_text = self.extractor.pdf_supported();
        self.report(true);
        while let Ok(first) = rx.recv() {
            // Coalesce whatever queued up meanwhile; a full sync covers the rest.
            let mut jobs = vec![first];
            jobs.extend(rx.try_iter());
            let jobs = coalesce(jobs);

            self.progress.active = true;
            for job in jobs {
                if let Err(e) = self.handle(job) {
                    eprintln!("rupert: indexing error: {e}");
                }
            }
            self.progress = Progress {
                pdf_text: self.extractor.pdf_supported(),
                ..Default::default()
            };
            self.flush_changed();
            self.report(true);
        }
    }

    fn handle(&mut self, job: Job) -> Result<()> {
        match job {
            Job::SyncAll(sources) => {
                let ids: Vec<&str> = sources.iter().map(|s| s.id.as_str()).collect();
                self.index.retain_sources(&ids)?;
                self.dirty = true;
                for source in &sources {
                    self.sync(source, "", true)?;
                }
            }
            Job::SyncSource(source) => self.sync(&source, "", true)?,
            Job::SyncDir { source, rel_dir } => self.sync(&source, &rel_dir, false)?,
            Job::File { source, rel_path } => {
                let parent = rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                let found = list_dir(&source.root, parent, source.show_all_types)?
                    .into_iter()
                    .find(|e| !e.is_dir && e.rel_path == rel_path);
                match found {
                    Some(entry) => self.index_file(&source, &Found { rel_path, entry })?,
                    None => self.remove_file(&source, &rel_path)?,
                }
            }
            Job::RemoveSource(id) => {
                self.index.remove_source(&id)?;
                self.dirty = true;
            }
        }
        Ok(())
    }

    /// Brings the index in line with disk at `rel_dir`: everything below it
    /// if `recursive`, otherwise the folder's own files plus subfolders that
    /// appeared or disappeared.
    fn sync(&mut self, source: &Source, rel_dir: &str, recursive: bool) -> Result<()> {
        if !source.is_available() {
            return Ok(()); // keep what we know until it comes back
        }
        let rel_dir = rel_dir.trim_matches('/');
        let mut found = Vec::new();
        let mut subdirs = Vec::new();
        if recursive {
            walk(source, rel_dir, &mut found);
        } else {
            for entry in list_dir(&source.root, rel_dir, source.show_all_types)? {
                if entry.is_dir {
                    subdirs.push(entry.rel_path.clone());
                } else {
                    found.push(Found {
                        rel_path: entry.rel_path.clone(),
                        entry,
                    });
                }
            }
        }

        let known = self.index.stamps_under(&source.id, rel_dir)?;
        let in_dir = |p: &str| p.rsplit_once('/').map(|(d, _)| d).unwrap_or("") == rel_dir;

        // Vanished files (and, non-recursively, whole vanished subfolders).
        let present: HashSet<&str> = found.iter().map(|f| f.rel_path.as_str()).collect();
        let subdir_set: HashSet<&str> = subdirs.iter().map(String::as_str).collect();
        let gone: Vec<String> = known
            .keys()
            .filter(|p| {
                if recursive || in_dir(p) {
                    !present.contains(p.as_str())
                } else {
                    !subdir_set.iter().any(|d| p.starts_with(&format!("{d}/")))
                }
            })
            .cloned()
            .collect();
        for rel in &gone {
            self.remove_file(source, rel)?;
        }

        // New or changed files.
        let todo: Vec<&Found> = found
            .iter()
            .filter(|f| needs_indexing(known.get(&f.rel_path), &f.entry))
            .collect();
        self.progress.total += todo.len();
        for f in todo {
            self.progress.current = Some(f.rel_path.clone());
            let result = catch_unwind(AssertUnwindSafe(|| self.index_file(source, f)));
            match result {
                Ok(Err(e)) => eprintln!("rupert: couldn't index {}: {e}", f.rel_path),
                Err(_) => eprintln!("rupert: indexing {} crashed; skipped", f.rel_path),
                Ok(Ok(())) => {}
            }
            self.progress.done += 1;
            self.report(false);
        }

        // Subfolders we have nothing for may be newly moved in.
        if !recursive {
            for d in &subdirs {
                let prefix = format!("{d}/");
                if !known.keys().any(|p| p.starts_with(&prefix)) {
                    self.sync(source, d, true)?;
                }
            }
        }

        // After a complete pass over a cloud source, drop stale catalog entries.
        if recursive && rel_dir.is_empty() && shares_catalog(source) {
            let live: HashSet<String> = found.into_iter().map(|f| f.rel_path).collect();
            let _ = catalog::prune(&source.root, &live);
        }
        Ok(())
    }

    fn index_file(&mut self, source: &Source, f: &Found) -> Result<()> {
        let path = source.root.join(&f.rel_path);
        let e = &f.entry;
        let kind = e.kind.unwrap_or(FileKind::Other);

        let (meta, storage, text_state, body) = if e.cloud == CloudState::CloudOnly {
            // Never read online-only files. Use the catalog if it's current.
            match catalog::read(&source.root, &f.rel_path)?
                .filter(|c| c.matches(e.size, e.modified_ms))
            {
                Some(c) => {
                    let state = if c.text.as_deref().is_some_and(|t| !t.is_empty()) {
                        TextState::Catalog
                    } else {
                        TextState::CloudOnly
                    };
                    (c.meta, c.storage.into(), state, c.text)
                }
                None => {
                    // Sidecars are separate small files, safe to read.
                    let meta = if kind == FileKind::Pdf {
                        None
                    } else {
                        sidecar::read(&path).ok().flatten()
                    };
                    let storage = if meta.is_some() {
                        Storage::Sidecar
                    } else {
                        Storage::None
                    };
                    (
                        meta.unwrap_or_default(),
                        storage,
                        TextState::CloudOnly,
                        None,
                    )
                }
            }
        } else {
            let record = meta::read_meta(&path)?;
            let (state, body) = match self.extractor.extract(&path) {
                Ok(Some(t)) if !t.is_empty() => (TextState::Extracted, Some(t)),
                Ok(Some(_)) => (TextState::Empty, None),
                Ok(None) => (TextState::Unsupported, None),
                Err(_) => (TextState::Failed, None),
            };
            if shares_catalog(source) {
                if body.is_some() || !record.meta.is_empty() {
                    let entry = CatalogEntry::new(
                        &f.rel_path,
                        e.size,
                        e.modified_ms,
                        record.meta.clone(),
                        record.storage,
                        body.clone(),
                    );
                    if let Err(err) = catalog::write(&source.root, &entry) {
                        eprintln!("rupert: couldn't write catalog for {}: {err}", f.rel_path);
                    }
                } else {
                    let _ = catalog::remove(&source.root, &f.rel_path);
                }
            }
            (record.meta, record.storage, state, body)
        };

        self.index.upsert(
            &DocRow {
                source_id: source.id.clone(),
                rel_path: f.rel_path.clone(),
                name: e.name.clone(),
                kind,
                size: e.size,
                modified_ms: e.modified_ms,
                cloud: e.cloud,
                meta,
                storage,
                text_state,
            },
            body.as_deref(),
        )?;
        self.dirty = true;
        Ok(())
    }

    fn remove_file(&mut self, source: &Source, rel_path: &str) -> Result<()> {
        self.index.remove(&source.id, rel_path)?;
        if shares_catalog(source) {
            let _ = catalog::remove(&source.root, rel_path);
        }
        self.dirty = true;
        Ok(())
    }

    /// Reports progress at most ~4×/s (or immediately if `force`), and lets
    /// the UI refresh results about once a second while indexing.
    fn report(&mut self, force: bool) {
        if force || self.last_report.elapsed() >= Duration::from_millis(250) {
            (self.on_progress)(self.progress.clone());
            if self.last_report.elapsed() >= Duration::from_secs(1) {
                self.flush_changed();
            }
            self.last_report = Instant::now();
        }
    }

    fn flush_changed(&mut self) {
        if self.dirty {
            self.dirty = false;
            (self.on_changed)();
        }
    }
}

// MARK: - Helpers

/// Cloud sources share a catalog so other machines can search online-only
/// files; a purely local folder has no one to share with.
fn shares_catalog(source: &Source) -> bool {
    source.provider != Provider::Local
}

fn needs_indexing(known: Option<&Stamp>, e: &Entry) -> bool {
    match known {
        None => true,
        Some(k) => {
            k.size != e.size
                || k.modified_ms != e.modified_ms
                || k.cloud != e.cloud
                // Downloaded since we last saw it, or text failed before.
                || (e.cloud == CloudState::Local
                    && matches!(k.text_state, TextState::CloudOnly | TextState::Catalog))
        }
    }
}

/// Every document file at or below `rel_dir`, using the same filtering as
/// the folder view (no file contents are read).
fn walk(source: &Source, rel_dir: &str, out: &mut Vec<Found>) {
    let Ok(entries) = list_dir(&source.root, rel_dir, source.show_all_types) else {
        return;
    };
    for entry in entries {
        if entry.is_dir {
            walk(source, &entry.rel_path, out);
        } else {
            out.push(Found {
                rel_path: entry.rel_path.clone(),
                entry,
            });
        }
    }
}

/// Collapses a burst of jobs: a full sync makes earlier folder syncs
/// redundant, and repeated folder or file jobs are de-duplicated.
fn coalesce(jobs: Vec<Job>) -> Vec<Job> {
    let mut out: Vec<Job> = Vec::new();
    let mut seen_dirs: HashSet<(String, String)> = HashSet::new();
    let mut seen_files: HashSet<(String, String)> = HashSet::new();
    let last_full = jobs.iter().rposition(|j| matches!(j, Job::SyncAll(_)));
    for (i, job) in jobs.into_iter().enumerate() {
        let skip = match &job {
            Job::SyncAll(_) => Some(i) != last_full,
            Job::SyncDir { source, rel_dir } => {
                last_full.is_some_and(|f| f > i)
                    || !seen_dirs.insert((source.id.clone(), rel_dir.clone()))
            }
            Job::File { source, rel_path } => {
                !seen_files.insert((source.id.clone(), rel_path.clone()))
            }
            Job::SyncSource(_) => last_full.is_some_and(|f| f > i),
            Job::RemoveSource(_) => false,
        };
        if !skip {
            out.push(job);
        }
    }
    out
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::SearchQuery;
    use crate::meta::DocMeta;
    use std::fs;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    fn source(root: &Path, provider: Provider) -> Source {
        Source {
            id: "src".into(),
            name: "Test".into(),
            root: root.to_path_buf(),
            provider,
            show_all_types: false,
        }
    }

    fn worker(index: Index) -> Worker {
        Worker {
            index,
            extractor: TextExtractor::new(None),
            on_progress: Box::new(|_| {}),
            on_changed: Box::new(|| {}),
            progress: Progress::default(),
            last_report: Instant::now(),
            dirty: false,
        }
    }

    fn search(w: &Worker, text: &str) -> Vec<String> {
        w.index
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
    fn full_and_folder_sync() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Boat/Engine")).unwrap();
        fs::write(
            root.join("Boat/Engine/impeller.md"),
            "Replace the raw water impeller",
        )
        .unwrap();
        fs::write(root.join("notes.txt"), "Marina fees due").unwrap();
        fs::write(root.join("setup.exe"), "binary").unwrap();

        let src = source(root, Provider::Local);
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(&src, "", true).unwrap();
        assert_eq!(search(&w, "impeller"), ["Boat/Engine/impeller.md"]);
        assert_eq!(search(&w, "marina"), ["notes.txt"]);
        assert!(search(&w, "binary").is_empty(), "non-documents not indexed");
        assert_eq!(w.progress.total, 2);

        // Unchanged files aren't re-read.
        w.progress = Progress::default();
        w.sync(&src, "", true).unwrap();
        assert_eq!(w.progress.total, 0);

        // A folder sync picks up new files and removed subfolders.
        fs::write(root.join("fuel.txt"), "Diesel receipt").unwrap();
        fs::remove_dir_all(root.join("Boat")).unwrap();
        w.sync(&src, "", false).unwrap();
        assert_eq!(search(&w, "diesel"), ["fuel.txt"]);
        assert!(search(&w, "impeller").is_empty());

        // A folder moved in is indexed recursively.
        fs::create_dir_all(root.join("Taxes/2025")).unwrap();
        fs::write(root.join("Taxes/2025/w2.txt"), "Wages and tips").unwrap();
        w.sync(&src, "", false).unwrap();
        assert_eq!(search(&w, "wages"), ["Taxes/2025/w2.txt"]);
    }

    #[test]
    fn cloud_sources_write_and_use_the_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("lease.md"), "The tenant agrees to pay rent").unwrap();
        let src = source(root, Provider::OneDrive);

        // Machine A indexes the local file and writes the catalog.
        let mut a = worker(Index::open_in_memory().unwrap());
        a.sync(&src, "", true).unwrap();
        let entry = catalog::read(root, "lease.md")
            .unwrap()
            .expect("catalog written");
        assert!(entry.text.unwrap().contains("tenant"));

        // Machine B sees the same file as online-only: it must not read the
        // file, and finds the text through the catalog instead.
        let listed = list_dir(root, "", false).unwrap().remove(0);
        let found = Found {
            rel_path: "lease.md".into(),
            entry: Entry {
                cloud: CloudState::CloudOnly,
                ..listed
            },
        };
        let mut b = worker(Index::open_in_memory().unwrap());
        b.index_file(&src, &found).unwrap();
        assert_eq!(search(&b, "tenant"), ["lease.md"]);
    }

    #[test]
    fn local_sources_write_no_catalog() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(&source(dir.path(), Provider::Local), "", true)
            .unwrap();
        assert!(!catalog::dir(dir.path()).exists());
    }

    #[test]
    fn spawned_indexer_reports_progress() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "hello").unwrap();
        let reports = Arc::new(Mutex::new(Vec::new()));
        let changed = Arc::new(Mutex::new(0));
        let (r, c) = (Arc::clone(&reports), Arc::clone(&changed));
        let handle = spawn(
            dir.path().join("index.sqlite"),
            None,
            move |p| r.lock().unwrap().push(p),
            move || *c.lock().unwrap() += 1,
        )
        .unwrap();
        handle.send(Job::SyncAll(vec![source(dir.path(), Provider::Local)]));

        let deadline = Instant::now() + Duration::from_secs(10);
        while *changed.lock().unwrap() == 0 && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(*changed.lock().unwrap() > 0, "on_changed called");
        let index = Index::open(&dir.path().join("index.sqlite")).unwrap();
        assert_eq!(index.stats().unwrap().documents, 1);
    }

    #[test]
    fn coalesces_bursts() {
        let s = Source {
            id: "a".into(),
            name: "a".into(),
            root: "/x".into(),
            provider: Provider::Local,
            show_all_types: false,
        };
        let dir = |d: &str| Job::SyncDir {
            source: s.clone(),
            rel_dir: d.into(),
        };
        let jobs = coalesce(vec![
            dir("x"),
            dir("x"),
            dir("y"),
            Job::SyncAll(vec![]),
            dir("z"),
        ]);
        assert_eq!(jobs.len(), 2, "{jobs:?}"); // SyncAll, then z
        assert!(matches!(jobs[0], Job::SyncAll(_)));
    }

    #[test]
    fn unchanged_meta_kept_for_catalog_entry_type() {
        // DocMeta round-trips through the catalog unchanged.
        let m = DocMeta {
            title: Some("t".into()),
            ..Default::default()
        };
        let e = CatalogEntry::new("a", 1, Some(1), m.clone(), Storage::Embedded, None);
        assert_eq!(e.meta, m);
    }
}
