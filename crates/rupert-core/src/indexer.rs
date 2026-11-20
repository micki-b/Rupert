//! Background indexing: keeps the search index in step with the sources.
//!
//! Runs on its own thread and processes [`Job`]s: a full sync at startup
//! (only changed files are re-read), folder syncs when the watcher reports
//! changes, and single files after Rupert saves metadata.
//!
//! Online-only files are never read, unless the source has "download
//! online-only files to index them" turned on. Their entry comes from the
//! shared catalog when another machine has indexed them, from the file's
//! sidecar (metadata, imported text), and otherwise from the name.
//!
//! With downloading on, once the regular work is done the worker fetches
//! online-only documents whose text is unknown a few at a time, indexes
//! them, records them in the catalog (even when they turn out to have no
//! text, so no machine fetches them again) and frees the space again.
//!
//! In [`Mode::Viewer`] (the phone) the indexer only reads: it uses the
//! catalog and sidecars of every source but never writes, prunes or
//! downloads anything.

use std::collections::{HashMap, HashSet, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::catalog::{self, CatalogEntry};
use crate::cloud;
use crate::error::Result;
use crate::file_kind::FileKind;
use crate::folder_info::{self, FolderInfo};
use crate::index::{DocRow, Index, Stamp, TextState};
use crate::listing::{CloudState, Entry, cloud_state, list_dir};
use crate::meta::{self, DocMeta, PartialMeta, Storage};
use crate::sidecar::{self, Sidecar};
use crate::source::Source;
use crate::text::TextExtractor;

/// Online-only files fetched at once (downloads are slow but not
/// bandwidth-bound, so a few in parallel help a lot).
const DOWNLOAD_BATCH: usize = 6;
/// Larger files aren't downloaded just to read their text.
const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;

// MARK: - Options

/// What the indexer may change besides its own index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Full Rupert: keeps the catalog of cloud sources current and
    /// downloads online-only files for sources that ask for it.
    #[default]
    Library,
    /// A viewer such as the phone app: reads the catalog and sidecars of
    /// every source (its sync folders don't reveal the provider) and never
    /// writes, prunes or downloads.
    Viewer,
}

/// Called with a hidden `.rupert` folder before it is read, so a viewer can
/// have the sync service fetch it first. Only those small folders are
/// passed; documents are never.
pub type PrepareHidden = Box<dyn Fn(&Path) + Send>;

#[derive(Default)]
pub struct Options {
    pub mode: Mode,
    pub prepare_hidden: Option<PrepareHidden>,
}

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
    /// Online-only documents being downloaded to read their text.
    pub download_done: usize,
    pub download_total: usize,
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
    extractor: TextExtractor,
    on_progress: impl Fn(Progress) + Send + 'static,
    on_changed: impl Fn() + Send + 'static,
) -> Result<IndexerHandle> {
    spawn_with(
        index_path,
        extractor,
        Options::default(),
        on_progress,
        on_changed,
    )
}

/// [`spawn`] with [`Options`], e.g. read-only for a viewer.
pub fn spawn_with(
    index_path: PathBuf,
    extractor: TextExtractor,
    options: Options,
    on_progress: impl Fn(Progress) + Send + 'static,
    on_changed: impl Fn() + Send + 'static,
) -> Result<IndexerHandle> {
    let index = Index::open(&index_path)?;
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new()
        .name("rupert-indexer".into())
        .spawn(move || {
            let mut worker = Worker {
                index,
                extractor,
                on_progress: Box::new(on_progress),
                on_changed: Box::new(on_changed),
                progress: Progress::default(),
                last_report: Instant::now(),
                dirty: false,
                downloads: Downloads::default(),
                mode: options.mode,
                prepare_hidden: options.prepare_hidden,
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
    downloads: Downloads,
    mode: Mode,
    prepare_hidden: Option<PrepareHidden>,
}

/// The queue of online-only documents to download and index.
#[derive(Default)]
struct Downloads {
    queue: VecDeque<(String, String)>,
    queued: HashSet<(String, String)>,
    /// Tried this session (whatever the outcome); never queued again, so a
    /// file that can't be read or evicted can't loop.
    attempted: HashSet<(String, String)>,
    /// Latest settings of every source seen, by id.
    sources: HashMap<String, Source>,
}

/// A document file found on disk.
struct Found {
    rel_path: String,
    entry: Entry,
    /// Modification time of its sidecar, if it has one.
    sidecar_ms: Option<i64>,
}

impl Found {
    fn new(entry: Entry, sidecars: &HashMap<String, i64>) -> Self {
        Found {
            rel_path: entry.rel_path.clone(),
            sidecar_ms: sidecars.get(&entry.name).copied(),
            entry,
        }
    }
}

impl Worker {
    fn run(&mut self, rx: Receiver<Job>) {
        self.progress.pdf_text = self.extractor.pdf_supported();
        self.report(true);
        loop {
            // Jobs come first; downloads fill the idle time between them.
            let first = if self.downloads.queue.is_empty() {
                match rx.recv() {
                    Ok(job) => job,
                    Err(_) => return,
                }
            } else {
                match rx.try_recv() {
                    Ok(job) => job,
                    Err(TryRecvError::Empty) => {
                        self.download_batch();
                        continue;
                    }
                    Err(TryRecvError::Disconnected) => return,
                }
            };
            // Coalesce whatever queued up meanwhile; a full sync covers the rest.
            let mut jobs = vec![first];
            jobs.extend(rx.try_iter());
            let jobs = coalesce(jobs);

            self.progress.active = true;
            self.report(true);
            for job in jobs {
                if let Err(e) = self.handle(job) {
                    eprintln!("rupert: indexing error: {e}");
                }
            }
            self.progress = Progress {
                pdf_text: self.extractor.pdf_supported(),
                download_done: self.progress.download_done,
                download_total: self.progress.download_total,
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
                self.downloads
                    .sources
                    .retain(|id, _| ids.contains(&id.as_str()));
                for source in &sources {
                    self.sync(source, "", true)?;
                    self.queue_downloads(source)?;
                }
            }
            Job::SyncSource(source) => {
                self.sync(&source, "", true)?;
                self.queue_downloads(&source)?;
            }
            Job::SyncDir { source, rel_dir } => {
                self.sync(&source, &rel_dir, false)?;
                self.queue_downloads(&source)?;
            }
            Job::File { source, rel_path } => {
                let parent = rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                let found = list_dir(&source.root, parent, source.show_all_types)?
                    .into_iter()
                    .find(|e| !e.is_dir && e.rel_path == rel_path);
                match found {
                    Some(entry) => {
                        let sidecars = self.sidecar_mtimes(&source.root.join(parent));
                        self.index_file(&source, &Found::new(entry, &sidecars))?
                    }
                    None => self.remove_file(&source, &rel_path)?,
                }
            }
            Job::RemoveSource(id) => {
                self.index.remove_source(&id)?;
                self.downloads.sources.remove(&id);
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
        if rel_dir.is_empty() && recursive {
            self.prepare(&catalog::dir(&source.root));
        }
        let mut found = Vec::new();
        let mut subdirs = Vec::new();
        if recursive {
            let mut dirs = Vec::new();
            self.walk(source, rel_dir, &mut found, &mut dirs);
            // Folder details (favorites) of every folder walked.
            let infos: Vec<(String, FolderInfo)> = dirs
                .into_iter()
                .filter_map(|d| folder_info::read(&source.root.join(&d)).map(|i| (d, i)))
                .collect();
            self.dirty |= self
                .index
                .replace_folders_under(&source.id, rel_dir, &infos)?;
        } else {
            let sidecars = self.sidecar_mtimes(&source.root.join(rel_dir));
            for entry in list_dir(&source.root, rel_dir, source.show_all_types)? {
                if entry.is_dir {
                    subdirs.push(entry.rel_path.clone());
                } else {
                    found.push(Found::new(entry, &sidecars));
                }
            }
            // This folder's own details, and forget vanished subfolders'.
            let info = folder_info::read(&source.root.join(rel_dir));
            self.index.set_folder(&source.id, rel_dir, info.as_ref())?;
            for known in self.index.folders_under(&source.id, rel_dir)? {
                let top = known
                    .strip_prefix(rel_dir)
                    .unwrap_or(&known)
                    .trim_start_matches('/')
                    .split('/')
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let sub = if rel_dir.is_empty() {
                    top
                } else {
                    format!("{rel_dir}/{top}")
                };
                if known != rel_dir && !subdirs.contains(&sub) {
                    self.index.set_folder(&source.id, &known, None)?;
                }
            }
            self.dirty = true;
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
            .filter(|f| needs_indexing(known.get(&f.rel_path), f, self.mode))
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
        if recursive && rel_dir.is_empty() && self.writes_catalog(source) {
            let live: HashSet<String> = found.into_iter().map(|f| f.rel_path).collect();
            let _ = catalog::prune(&source.root, &live);
        }
        Ok(())
    }

    fn index_file(&mut self, source: &Source, f: &Found) -> Result<()> {
        self.index_file_with(source, f, false)
    }

    /// `always_catalog`: record the file in the catalog even if it has no
    /// text or details (after downloading it, so it isn't fetched again).
    fn index_file_with(&mut self, source: &Source, f: &Found, always_catalog: bool) -> Result<()> {
        let path = source.root.join(&f.rel_path);
        let e = &f.entry;
        let kind = e.kind.unwrap_or(FileKind::Other);
        // Sidecars are separate small files, safe to read even when the
        // document itself is online-only.
        let side = if f.sidecar_ms.is_some() {
            sidecar::read(&path).ok().flatten()
        } else {
            None
        };
        let side_text = side
            .as_ref()
            .and_then(|s| s.text.clone())
            .filter(|t| !t.trim().is_empty());

        let (meta, storage, text_state, body) = if e.cloud == CloudState::CloudOnly {
            // Never read online-only files. Use the catalog if it's current.
            let entry = catalog::read(&source.root, &f.rel_path)?
                .filter(|c| c.matches(e.size, e.modified_ms));
            online_only_details(entry, side.as_ref())
        } else {
            let record = meta::read_meta(&path)?;
            let (mut state, mut body) = match self.extractor.extract(&path) {
                Ok(Some(t)) if !t.is_empty() => (TextState::Extracted, Some(t)),
                _ if side_text.is_some() => (TextState::Sidecar, side_text),
                Ok(Some(_)) => (TextState::Empty, None),
                Ok(None) => (TextState::Unsupported, None),
                Err(_) => (TextState::Failed, None),
            };
            // This machine can't read the text (no PDFium, e.g. the phone),
            // but another one may have: use its catalog entry, and keep it.
            let mut from_catalog = false;
            if matches!(state, TextState::Unsupported | TextState::Failed)
                && self.reads_catalog(source)
                && let Some(text) = catalog::read(&source.root, &f.rel_path)?
                    .filter(|c| c.matches(e.size, e.modified_ms))
                    .and_then(|c| c.text)
                    .filter(|t| !t.is_empty())
            {
                (state, body, from_catalog) = (TextState::Catalog, Some(text), true);
            }
            if self.writes_catalog(source) && !from_catalog {
                if always_catalog || body.is_some() || !record.meta.is_empty() {
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
                sidecar_ms: f.sidecar_ms,
            },
            body.as_deref(),
        )?;
        self.dirty = true;
        Ok(())
    }

    // MARK: Downloading online-only files

    /// Queues the source's online-only documents whose text is unknown, if
    /// the source has downloading turned on.
    fn queue_downloads(&mut self, source: &Source) -> Result<()> {
        if self.mode == Mode::Viewer {
            return Ok(());
        }
        self.downloads
            .sources
            .insert(source.id.clone(), source.clone());
        if !source.download_online_only || !source.is_available() {
            return Ok(());
        }
        for rel in self
            .index
            .download_candidates(&source.id, MAX_DOWNLOAD_BYTES)?
        {
            let key = (source.id.clone(), rel);
            if self.downloads.attempted.contains(&key) || !self.downloads.queued.insert(key.clone())
            {
                continue;
            }
            self.downloads.queue.push_back(key);
            self.progress.download_total += 1;
        }
        Ok(())
    }

    /// Downloads a few queued files in parallel, then indexes each and
    /// frees its space again.
    fn download_batch(&mut self) {
        let mut batch: Vec<(Source, String)> = Vec::new();
        while batch.len() < DOWNLOAD_BATCH
            && let Some(key) = self.downloads.queue.pop_front()
        {
            self.downloads.queued.remove(&key);
            self.downloads.attempted.insert(key.clone());
            let (id, rel) = key;
            match self.downloads.sources.get(&id) {
                Some(s) if s.download_online_only && s.is_available() => {
                    batch.push((s.clone(), rel))
                }
                _ => self.progress.download_done += 1, // turned off or removed
            }
        }

        let fetched: Vec<bool> = std::thread::scope(|scope| {
            let handles: Vec<_> = batch
                .iter()
                .map(|(source, rel)| {
                    let path = source.root.join(rel);
                    scope.spawn(move || {
                        // Skip files downloaded meanwhile; regular indexing has them.
                        cloud_state(&path).ok() == Some(CloudState::CloudOnly)
                            && cloud::download(&path).is_ok()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().unwrap_or(false))
                .collect()
        });

        for ((source, rel), ok) in batch.into_iter().zip(fetched) {
            self.progress.current = Some(rel.clone());
            if ok {
                let result =
                    catch_unwind(AssertUnwindSafe(|| self.index_downloaded(&source, &rel)));
                match result {
                    Ok(Err(e)) => eprintln!("rupert: couldn't index downloaded {rel}: {e}"),
                    Err(_) => eprintln!("rupert: indexing downloaded {rel} crashed; skipped"),
                    Ok(Ok(())) => {}
                }
            }
            self.progress.download_done += 1;
            self.report(false);
        }

        if self.downloads.queue.is_empty() {
            self.progress.download_done = 0;
            self.progress.download_total = 0;
            self.progress.current = None;
            self.flush_changed();
            self.report(true);
        }
    }

    /// Indexes a just-downloaded file, records it in the catalog, and (for
    /// cloud sources, where the catalog keeps the text) frees its space.
    fn index_downloaded(&mut self, source: &Source, rel: &str) -> Result<()> {
        let parent = rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let Some(entry) = list_dir(&source.root, parent, source.show_all_types)?
            .into_iter()
            .find(|e| !e.is_dir && e.rel_path == rel)
        else {
            return Ok(());
        };
        let sidecars = self.sidecar_mtimes(&source.root.join(parent));
        let found = Found::new(entry, &sidecars);
        self.index_file_with(source, &found, true)?;

        // If the space can't be freed (e.g. the file is open elsewhere) it
        // simply stays downloaded.
        let evict = || {
            cloud::evict(&source.root.join(rel))
                .inspect_err(|e| eprintln!("rupert: {rel}: {e}"))
                .unwrap_or(false)
        };
        if shares_catalog(source) && found.entry.cloud == CloudState::Local && evict() {
            let evicted = Found {
                entry: Entry {
                    cloud: CloudState::CloudOnly,
                    ..found.entry.clone()
                },
                rel_path: found.rel_path.clone(),
                sidecar_ms: found.sidecar_ms,
            };
            self.index_file(source, &evicted)?;
        }
        Ok(())
    }

    fn remove_file(&mut self, source: &Source, rel_path: &str) -> Result<()> {
        self.index.remove(&source.id, rel_path)?;
        if self.writes_catalog(source) {
            let _ = catalog::remove(&source.root, rel_path);
        }
        self.dirty = true;
        Ok(())
    }

    // MARK: Shared files

    fn writes_catalog(&self, source: &Source) -> bool {
        self.mode == Mode::Library && shares_catalog(source)
    }

    fn reads_catalog(&self, source: &Source) -> bool {
        self.mode == Mode::Viewer || shares_catalog(source)
    }

    /// Lets the viewer fetch a hidden folder before it's read.
    fn prepare(&self, hidden_dir: &Path) {
        if let Some(prepare) = &self.prepare_hidden
            && hidden_dir.is_dir()
        {
            prepare(hidden_dir);
        }
    }

    fn sidecar_mtimes(&self, dir: &Path) -> HashMap<String, i64> {
        let hidden = dir.join(sidecar::DIR_NAME);
        self.prepare(&hidden);
        // Created elsewhere (e.g. by the phone): make sure Windows hides it
        // too, where a leading dot doesn't.
        if self.mode == Mode::Library && cfg!(windows) && hidden.is_dir() {
            sidecar::hide(&hidden);
        }
        sidecar::mtimes(dir)
    }

    /// Every document file at or below `rel_dir`, using the same filtering
    /// as the folder view (no file contents are read).
    fn walk(&self, source: &Source, rel_dir: &str, out: &mut Vec<Found>, dirs: &mut Vec<String>) {
        let Ok(entries) = list_dir(&source.root, rel_dir, source.show_all_types) else {
            return;
        };
        dirs.push(rel_dir.to_string());
        let sidecars = self.sidecar_mtimes(&source.root.join(rel_dir));
        for entry in entries {
            if entry.is_dir {
                self.walk(source, &entry.rel_path, out, dirs);
            } else {
                out.push(Found::new(entry, &sidecars));
            }
        }
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

/// What is known about a document without reading it: its current catalog
/// entry (an entry without text means a machine read the file and found
/// none) and its sidecar, whose details and text win.
pub(crate) fn online_only_details(
    entry: Option<CatalogEntry>,
    side: Option<&Sidecar>,
) -> (DocMeta, Storage, TextState, Option<String>) {
    let (mut meta, mut storage, mut state, mut body) = match entry {
        Some(c) => {
            let state = if c.text.as_deref().is_some_and(|t| !t.is_empty()) {
                TextState::Catalog
            } else {
                TextState::Empty
            };
            (c.meta, c.storage.into(), state, c.text)
        }
        None => (
            DocMeta::default(),
            Storage::None,
            TextState::CloudOnly,
            None,
        ),
    };
    if let Some(s) = side
        && let Some(m) = &s.meta
    {
        meta = if s.pending {
            PartialMeta::merge(m.clone(), meta)
        } else {
            m.clone()
        };
        storage = Storage::Sidecar;
    }
    let side_text = side
        .and_then(|s| s.text.clone())
        .filter(|t| !t.trim().is_empty());
    if state != TextState::Catalog && side_text.is_some() {
        (state, body) = (TextState::Sidecar, side_text);
    }
    (meta, storage, state, body)
}

/// Cloud sources share a catalog so other machines can search online-only
/// files; a purely local folder has no one to share with.
fn shares_catalog(source: &Source) -> bool {
    source.shares_catalog()
}

fn needs_indexing(known: Option<&Stamp>, f: &Found, mode: Mode) -> bool {
    let e = &f.entry;
    match known {
        None => true,
        Some(k) => {
            k.size != e.size
                || k.modified_ms != e.modified_ms
                || k.cloud != e.cloud
                || k.sidecar_ms != f.sidecar_ms
                // Downloaded since we last saw it, or text failed before.
                // (A viewer can't read text itself, so retrying won't help.)
                || (mode == Mode::Library
                    && e.cloud == CloudState::Local
                    && matches!(
                        k.text_state,
                        TextState::CloudOnly | TextState::Catalog | TextState::Sidecar
                    ))
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
pub(crate) mod tests {
    use super::*;
    use crate::index::SearchQuery;
    use crate::meta::DocMeta;
    use crate::source::Provider;
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
            download_online_only: false,
            bookmark: None,
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
            downloads: Downloads::default(),
            mode: Mode::Library,
            prepare_hidden: None,
        }
    }

    /// Indexes `root` as a computer would for a cloud source, writing its
    /// catalog (for tests of readers elsewhere).
    pub(crate) fn index_as_cloud(root: &Path) {
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(&source(root, Provider::OneDrive), "", true).unwrap();
    }

    /// Indexes a source into a fresh in-memory index (no PDF text).
    pub(crate) fn index_as_local(_root: &Path, source: &Source) -> Index {
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(source, "", true).unwrap();
        w.index
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
    fn favorite_folders_are_indexed_and_follow_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Boat/Engine")).unwrap();
        fs::create_dir_all(root.join("Taxes")).unwrap();
        fs::write(root.join("Boat/Engine/impeller.md"), "Impeller").unwrap();
        let fav = FolderInfo { favorite: true };
        folder_info::write(&root.join("Boat/Engine"), &fav).unwrap();
        folder_info::write(&root.join("Taxes"), &fav).unwrap();

        let src = source(root, Provider::Local);
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(&src, "", true).unwrap();
        let favs = |w: &Worker| -> Vec<String> {
            w.index
                .favorite_folders()
                .unwrap()
                .into_iter()
                .map(|(_, p)| p)
                .collect()
        };
        assert_eq!(favs(&w), ["Boat/Engine", "Taxes"]);
        // The favorite file isn't mistaken for a document's sidecar.
        assert_eq!(w.index.stats().unwrap().documents, 1);

        // Renamed elsewhere: the favorite moves with the folder.
        fs::rename(root.join("Boat"), root.join("Boating")).unwrap();
        w.sync(&src, "", false).unwrap();
        assert_eq!(favs(&w), ["Boating/Engine", "Taxes"]);

        // Unfavorited on another machine: a folder pass notices.
        folder_info::write(&root.join("Taxes"), &FolderInfo::default()).unwrap();
        w.sync(&src, "Taxes", false).unwrap();
        assert_eq!(favs(&w), ["Boating/Engine"]);
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
            sidecar_ms: None,
        };
        let mut b = worker(Index::open_in_memory().unwrap());
        b.index_file(&src, &found).unwrap();
        assert_eq!(search(&b, "tenant"), ["lease.md"]);
    }

    /// Writes a catalog entry with text for a PDF, as a machine with PDFium would.
    fn catalog_text_for(root: &Path, rel: &str, text: &str) {
        let listed = list_dir(root, "", false)
            .unwrap()
            .into_iter()
            .find(|e| e.rel_path == rel)
            .unwrap();
        let entry = CatalogEntry::new(
            rel,
            listed.size,
            listed.modified_ms,
            DocMeta::default(),
            Storage::None,
            Some(text.into()),
        );
        catalog::write(root, &entry).unwrap();
    }

    #[test]
    fn machines_without_pdf_text_keep_the_catalog_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        crate::pdf_meta::tests::make_pdf(&root.join("deed.pdf"), None);
        catalog_text_for(root, "deed.pdf", "Grant deed for the slip");

        // No PDFium here: the other machine's text is used, not erased.
        let src = source(root, Provider::OneDrive);
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(&src, "", true).unwrap();
        assert_eq!(search(&w, "slip"), ["deed.pdf"]);
        let kept = catalog::read(root, "deed.pdf").unwrap().unwrap();
        assert_eq!(kept.text.as_deref(), Some("Grant deed for the slip"));
    }

    #[test]
    fn viewer_reads_but_never_writes_shared_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Boat")).unwrap();
        fs::write(root.join("Boat/survey.md"), "Hull survey").unwrap();
        crate::pdf_meta::tests::make_pdf(&root.join("deed.pdf"), None);
        catalog_text_for(root, "deed.pdf", "Grant deed for the slip");
        // An entry for a file this machine can't see (e.g. not synced yet).
        let ghost = CatalogEntry::new(
            "Boat/ghost.pdf",
            1,
            Some(0),
            DocMeta::default(),
            Storage::None,
            Some("ghost".into()),
        );
        catalog::write(root, &ghost).unwrap();
        fs::create_dir_all(root.join("Boat/.rupert")).unwrap();
        let before = catalog_files(root);

        // Even a source that looks local is read like a cloud one.
        let src = source(root, Provider::Local);
        let prepared = Arc::new(Mutex::new(Vec::new()));
        let seen = prepared.clone();
        let top = root.to_path_buf();
        let mut w = worker(Index::open_in_memory().unwrap());
        w.mode = Mode::Viewer;
        w.prepare_hidden = Some(Box::new(move |p: &Path| {
            seen.lock()
                .unwrap()
                .push(p.strip_prefix(&top).unwrap().to_path_buf())
        }));
        w.sync(&src, "", true).unwrap();
        assert_eq!(search(&w, "slip"), ["deed.pdf"]);
        assert_eq!(search(&w, "hull"), ["Boat/survey.md"]);
        assert_eq!(catalog_files(root), before, "nothing written or pruned");

        // Removing a file doesn't touch the catalog either.
        fs::remove_file(root.join("deed.pdf")).unwrap();
        w.sync(&src, "", true).unwrap();
        assert!(search(&w, "slip").is_empty());
        assert_eq!(catalog_files(root), before);

        let prepared = prepared.lock().unwrap();
        assert!(prepared.iter().any(|p| p.ends_with(".rupert/catalog")));
        assert!(prepared.iter().any(|p| p.ends_with("Boat/.rupert")));
        assert!(
            prepared
                .iter()
                .all(|p| p.to_string_lossy().contains(".rupert"))
        );
    }

    fn catalog_files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = Vec::new();
        let mut stack = vec![catalog::dir(root)];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                if e.path().is_dir() {
                    stack.push(e.path());
                } else {
                    out.push((e.path(), fs::read(e.path()).unwrap()));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn sidecar_text_and_pending_details_are_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("scan.md"), "").unwrap(); // no text of its own
        let src = source(root, Provider::Local);
        let mut w = worker(Index::open_in_memory().unwrap());
        w.sync(&src, "", true).unwrap();
        assert!(search(&w, "harbor").is_empty());

        // Imported OCR text in the sidecar is noticed by the next pass.
        let file = root.join("scan.md");
        sidecar::write_text(&file, "harbor master invoice", "ScanSnap Home", true).unwrap();
        w.sync(&src, "", true).unwrap();
        assert_eq!(search(&w, "harbor"), ["scan.md"]);

        // Online-only: never read, but the sidecar's text and details count.
        meta::write_meta_beside(
            &file,
            DocMeta {
                title: Some("Harbor invoice".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let listed = list_dir(root, "", false).unwrap().remove(0);
        let sidecars = sidecar::mtimes(root);
        let found = Found::new(
            Entry {
                cloud: CloudState::CloudOnly,
                ..listed
            },
            &sidecars,
        );
        assert!(found.sidecar_ms.is_some());
        w.index_file(&src, &found).unwrap();
        let hit = &w
            .index
            .search(&SearchQuery {
                text: "master".into(),
                ..Default::default()
            })
            .unwrap()[0];
        assert_eq!(hit.title.as_deref(), Some("Harbor invoice"));
        assert_eq!(
            w.index.text_state("src", "scan.md").unwrap(),
            Some(TextState::Sidecar)
        );
    }

    #[test]
    fn downloaded_files_are_recorded_even_without_text() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("blank.md"), "").unwrap();
        fs::write(root.join("memo.md"), "anchor chain").unwrap();
        let src = Source {
            download_online_only: true,
            bookmark: None,
            ..source(root, Provider::OneDrive)
        };
        let mut w = worker(Index::open_in_memory().unwrap());

        // Both seen as online-only first: candidates for downloading.
        for name in ["blank.md", "memo.md"] {
            let listed = list_dir(root, "", false)
                .unwrap()
                .into_iter()
                .find(|e| e.name == name)
                .unwrap();
            let found = Found::new(
                Entry {
                    cloud: CloudState::CloudOnly,
                    ..listed
                },
                &HashMap::new(),
            );
            w.index_file(&src, &found).unwrap();
        }
        assert_eq!(
            w.index
                .download_candidates("src", MAX_DOWNLOAD_BYTES)
                .unwrap(),
            ["blank.md", "memo.md"]
        );
        w.queue_downloads(&src).unwrap();
        assert_eq!(w.progress.download_total, 2);
        w.queue_downloads(&src).unwrap();
        assert_eq!(w.progress.download_total, 2, "not queued twice");

        // "Downloaded" (these files are local already): indexed, and both
        // recorded in the catalog, the blank one too.
        w.index_downloaded(&src, "blank.md").unwrap();
        w.index_downloaded(&src, "memo.md").unwrap();
        assert!(catalog::read(root, "blank.md").unwrap().is_some());
        assert_eq!(search(&w, "anchor"), ["memo.md"]);

        // Online-only again (evicted, or on another machine): the catalog
        // answers, and nothing is a candidate any more.
        for name in ["blank.md", "memo.md"] {
            let listed = list_dir(root, "", false)
                .unwrap()
                .into_iter()
                .find(|e| e.name == name)
                .unwrap();
            let found = Found::new(
                Entry {
                    cloud: CloudState::CloudOnly,
                    ..listed
                },
                &HashMap::new(),
            );
            w.index_file(&src, &found).unwrap();
        }
        assert_eq!(
            w.index.text_state("src", "blank.md").unwrap(),
            Some(TextState::Empty)
        );
        assert_eq!(
            w.index.text_state("src", "memo.md").unwrap(),
            Some(TextState::Catalog)
        );
        assert!(
            w.index
                .download_candidates("src", MAX_DOWNLOAD_BYTES)
                .unwrap()
                .is_empty()
        );

        // Turning the setting off drops what's still queued.
        let off = Source {
            download_online_only: false,
            bookmark: None,
            ..src.clone()
        };
        w.queue_downloads(&off).unwrap();
        w.download_batch();
        assert_eq!(w.progress.download_total, 0, "queue drained");
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
            TextExtractor::new(None),
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
            download_online_only: false,
            bookmark: None,
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
