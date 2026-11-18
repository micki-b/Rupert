//! The inbox: where new scans arrive, get their text recognized and a
//! filing suggestion, and wait for the user to review and file them.
//!
//! A background thread watches over the inbox folder (non-recursive):
//! 1. **OCR:** PDFs with pages that have no text get an invisible text
//!    layer ([`crate::ocr`]). Scanned images get their text recognized into
//!    their sidecar, so they can be searched too.
//! 2. **Suggestion:** LM Studio proposes a title, document date, tags and
//!    one of the folders where similar documents already live
//!    ([`crate::llm`]).
//!
//! Results are cached by file size and modification time (in memory and in
//! the app's data folder), so nothing is redone after a restart. Filing
//! ([`file_document`]) writes the reviewed details into the document and
//! moves it into place.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::file_kind::FileKind;
use crate::fileops;
use crate::index::Index;
use crate::listing::{CloudState, cloud_state, list_dir, relative, resolve};
use crate::llm::{self, LmStudio, Suggestion};
use crate::meta::{self, DocMeta};
use crate::ocr::{self, Engine};
use crate::settings::Settings;
use crate::sidecar;
use crate::source::Source;
use crate::text::TextExtractor;

/// Where image OCR text is credited in the sidecar.
const OCR_TEXT_SOURCE: &str = "Rupert OCR";
/// Folders offered to the model (and in the review) per document.
const CANDIDATE_FOLDERS: usize = 12;

// MARK: - Items

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    Waiting,
    Recognizing,
    Suggesting,
    Ready,
}

/// A folder the document could be filed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderChoice {
    pub source_id: String,
    pub dir: String,
    /// "Source/dir", as shown to the user and the model.
    pub label: String,
}

/// A document in the inbox and what's known about it so far.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IntakeItem {
    pub source_id: String,
    pub rel_path: String,
    pub name: String,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub cloud: CloudState,
    pub stage: Stage,
    /// OCR progress: pages done, pages to do.
    pub progress: Option<(usize, usize)>,
    /// What OCR did, e.g. "Text recognized on 2 pages (Apple Vision)".
    pub ocr: Option<String>,
    pub suggestion: Option<Suggestion>,
    /// The folder the suggestion points to, if any.
    pub suggested_folder: Option<FolderChoice>,
    /// Folders where similar documents are filed, best first.
    pub folders: Vec<FolderChoice>,
    /// Problems worth showing (e.g. LM Studio not running).
    pub note: Option<String>,
}

/// Cached results for one file version.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Done {
    size: u64,
    modified_ms: Option<i64>,
    ocr: Option<String>,
    suggestion: Option<Suggestion>,
    folders: Vec<FolderChoice>,
    note: Option<String>,
}

// MARK: - Handle

enum Msg {
    Configure(Settings, Vec<Source>),
    Refresh,
    /// Forget cached results for this document and process it again.
    Redo(String),
}

#[derive(Default)]
struct Shared {
    items: Vec<IntakeItem>,
}

#[derive(Clone)]
pub struct IntakeHandle {
    tx: Sender<Msg>,
    shared: Arc<Mutex<Shared>>,
}

impl IntakeHandle {
    pub fn items(&self) -> Vec<IntakeItem> {
        self.shared.lock().unwrap().items.clone()
    }

    /// New settings or sources (also starts the first scan).
    pub fn configure(&self, settings: Settings, sources: Vec<Source>) {
        let _ = self.tx.send(Msg::Configure(settings, sources));
    }

    /// The inbox folder changed on disk.
    pub fn refresh(&self) {
        let _ = self.tx.send(Msg::Refresh);
    }

    pub fn redo(&self, rel_path: String) {
        let _ = self.tx.send(Msg::Redo(rel_path));
    }
}

/// Starts the intake thread. `on_change` is called whenever the list or an
/// item's state changes.
pub fn spawn(
    index_path: PathBuf,
    cache_path: PathBuf,
    extractor: TextExtractor,
    on_change: impl Fn() + Send + 'static,
) -> Result<IntakeHandle> {
    let (tx, rx) = mpsc::channel();
    let shared = Arc::new(Mutex::new(Shared::default()));
    let worker_shared = Arc::clone(&shared);
    std::thread::Builder::new()
        .name("rupert-intake".into())
        .spawn(move || {
            let cache = fs::read(&cache_path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
                .unwrap_or_default();
            let mut worker = Worker {
                index_path,
                cache_path,
                cache,
                extractor,
                engine: Engine::detect(),
                settings: Settings::default(),
                configured: false,
                sources: Vec::new(),
                shared: worker_shared,
                on_change: Box::new(on_change),
            };
            worker.run(rx);
        })?;
    Ok(IntakeHandle { tx, shared })
}

// MARK: - Worker

struct Worker {
    index_path: PathBuf,
    cache_path: PathBuf,
    /// By relative path in the inbox's source.
    cache: HashMap<String, Done>,
    extractor: TextExtractor,
    engine: Option<Engine>,
    settings: Settings,
    configured: bool,
    sources: Vec<Source>,
    shared: Arc<Mutex<Shared>>,
    on_change: Box<dyn Fn() + Send>,
}

impl Worker {
    fn run(&mut self, rx: Receiver<Msg>) {
        while let Ok(msg) = rx.recv() {
            self.apply(msg);
            for msg in rx.try_iter() {
                self.apply(msg);
            }
            // Process one item at a time, checking for news in between.
            loop {
                self.scan();
                let Some(index) = self.next_todo() else { break };
                self.process(index);
                match rx.try_recv() {
                    Ok(msg) => self.apply(msg),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => return,
                }
            }
        }
    }

    fn apply(&mut self, msg: Msg) {
        match msg {
            Msg::Configure(settings, sources) => {
                // A different inbox folder: its results don't apply. (The
                // first configuration after startup keeps the saved cache.)
                if self.configured && settings.inbox != self.settings.inbox {
                    self.cache.clear();
                }
                self.configured = true;
                self.settings = settings;
                self.sources = sources;
            }
            Msg::Refresh => {}
            Msg::Redo(rel) => {
                self.cache.remove(&rel);
            }
        }
    }

    fn inbox(&self) -> Option<(&Source, &str)> {
        let inbox = self.settings.inbox.as_ref()?;
        let source = self.sources.iter().find(|s| s.id == inbox.source_id)?;
        Some((source, inbox.rel_path.as_str()))
    }

    /// Re-lists the inbox, keeping what's known about unchanged files.
    fn scan(&mut self) {
        let listed = match self.inbox() {
            Some((source, rel)) if source.is_available() => {
                list_dir(&source.root, rel, false).unwrap_or_default()
            }
            _ => Vec::new(),
        };
        let source_id = self
            .settings
            .inbox
            .as_ref()
            .map(|i| i.source_id.clone())
            .unwrap_or_default();
        let old = std::mem::take(&mut self.shared.lock().unwrap().items);
        let mut items = Vec::new();
        for e in listed.into_iter().filter(|e| !e.is_dir) {
            let done = self
                .cache
                .get(&e.rel_path)
                .filter(|d| d.size == e.size && d.modified_ms == e.modified_ms);
            let mut item = IntakeItem {
                source_id: source_id.clone(),
                rel_path: e.rel_path.clone(),
                name: e.name.clone(),
                size: e.size,
                modified_ms: e.modified_ms,
                cloud: e.cloud,
                stage: Stage::Waiting,
                progress: None,
                ocr: None,
                suggestion: None,
                suggested_folder: None,
                folders: Vec::new(),
                note: None,
            };
            if let Some(d) = done {
                item.stage = Stage::Ready;
                item.ocr = d.ocr.clone();
                item.suggestion = d.suggestion.clone();
                item.folders = d.folders.clone();
                item.note = d.note.clone();
                item.suggested_folder = suggested_folder(&item);
            } else if e.cloud == CloudState::CloudOnly {
                item.stage = Stage::Ready;
                item.note = Some(
                    "Online-only: open it once to download it, then Rupert will read it.".into(),
                );
            }
            items.push(item);
        }
        let changed = items != old;
        self.shared.lock().unwrap().items = items;
        // Forget files that left the inbox.
        let present: Vec<String> = self
            .items_snapshot()
            .into_iter()
            .map(|i| i.rel_path)
            .collect();
        self.cache.retain(|k, _| present.contains(k));
        if changed {
            (self.on_change)();
        }
    }

    fn items_snapshot(&self) -> Vec<IntakeItem> {
        self.shared.lock().unwrap().items.clone()
    }

    fn next_todo(&self) -> Option<usize> {
        self.shared
            .lock()
            .unwrap()
            .items
            .iter()
            .position(|i| i.stage == Stage::Waiting)
    }

    fn update(&self, index: usize, f: impl FnOnce(&mut IntakeItem)) {
        if let Some(item) = self.shared.lock().unwrap().items.get_mut(index) {
            f(item);
        }
        (self.on_change)();
    }

    fn process(&mut self, index: usize) {
        let Some(item) = self.items_snapshot().into_iter().nth(index) else {
            return;
        };
        let Some((source, _)) = self.inbox() else {
            return;
        };
        let source = source.clone();
        let Ok(path) = resolve(&source.root, &item.rel_path) else {
            return;
        };
        let kind = FileKind::from_path(&path);
        let mut notes: Vec<String> = Vec::new();

        // 1. OCR.
        let mut ocr_summary = None;
        let mut image_text = None;
        if self.settings.ocr
            && let Some(engine) = self.engine.clone()
        {
            match kind {
                FileKind::Pdf if self.extractor.pdf_supported() => {
                    self.update(index, |i| i.stage = Stage::Recognizing);
                    let result = ocr::ocr_pdf(&path, &self.extractor, &engine, |done, total| {
                        self.update(index, |i| i.progress = Some((done, total)));
                    });
                    match result {
                        Ok(r) if r.pages_recognized > 0 => {
                            ocr_summary = Some(format!(
                                "Text recognized on {} of {} pages ({})",
                                r.pages_recognized,
                                r.pages,
                                engine.name()
                            ))
                        }
                        Ok(_) => {}
                        Err(e) => notes.push(format!("Text recognition failed: {e}")),
                    }
                }
                FileKind::Image => {
                    self.update(index, |i| i.stage = Stage::Recognizing);
                    match fs::read(&path)
                        .map_err(Error::from)
                        .and_then(|bytes| engine.recognize(&bytes, 1, 1))
                    {
                        Ok(words) if !words.is_empty() => {
                            let text = words
                                .iter()
                                .map(|w| w.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            if let Err(e) = sidecar::write_text(&path, &text, OCR_TEXT_SOURCE, true)
                            {
                                notes.push(format!("Couldn't save the recognized text: {e}"));
                            }
                            ocr_summary = Some(format!("Text recognized ({})", engine.name()));
                            image_text = Some(text);
                        }
                        Ok(_) => {}
                        Err(e) => notes.push(format!("Text recognition failed: {e}")),
                    }
                }
                _ => {}
            }
        }

        // 2. Suggestion.
        let text = image_text.unwrap_or_else(|| {
            self.extractor
                .extract(&path)
                .ok()
                .flatten()
                .or_else(|| sidecar::read(&path).ok().flatten().and_then(|s| s.text))
                .unwrap_or_default()
        });
        let mut folders = Vec::new();
        let mut suggestion = None;
        if self.settings.suggestions && !text.trim().is_empty() {
            self.update(index, |i| {
                i.stage = Stage::Suggesting;
                i.progress = None;
            });
            folders = self.candidate_folders(&text);
            let labels: Vec<String> = folders.iter().map(|f| f.label.clone()).collect();
            let tags = Index::open(&self.index_path)
                .and_then(|i| i.top_tags(150))
                .unwrap_or_default();
            let lm = LmStudio {
                url: self.settings.lm_studio_url.clone(),
                model: self.settings.lm_studio_model.clone(),
            };
            let year = crate::pdf_meta::utc_today()[..4].to_string();
            let req = llm::Request {
                text: &text,
                file_name: &item.name,
                year: &year,
                folders: &labels,
                tags: &tags,
            };
            match lm.suggest(&req) {
                Ok(s) => suggestion = Some(s),
                Err(e) => notes.push(e.to_string()),
            }
        } else if self.settings.suggestions {
            notes.push("No text to suggest from.".into());
        }

        // Cache against the file as it is now (OCR changed it).
        let entry = list_dir(&source.root, parent_of(&item.rel_path), false)
            .ok()
            .and_then(|l| l.into_iter().find(|e| e.rel_path == item.rel_path));
        let (size, modified_ms) = entry
            .map(|e| (e.size, e.modified_ms))
            .unwrap_or((item.size, item.modified_ms));
        let done = Done {
            size,
            modified_ms,
            ocr: ocr_summary,
            suggestion,
            folders,
            note: (!notes.is_empty()).then(|| notes.join(" ")),
        };
        self.cache.insert(item.rel_path.clone(), done.clone());
        self.save_cache();
        self.update(index, |i| {
            i.size = size;
            i.modified_ms = modified_ms;
            i.stage = Stage::Ready;
            i.progress = None;
            i.ocr = done.ocr;
            i.suggestion = done.suggestion;
            i.folders = done.folders;
            i.note = done.note;
            i.suggested_folder = suggested_folder(i);
        });
    }

    /// Folders where documents like this one live (not the inbox itself).
    fn candidate_folders(&self, text: &str) -> Vec<FolderChoice> {
        let Ok(index) = Index::open(&self.index_path) else {
            return Vec::new();
        };
        let exclude = self
            .settings
            .inbox
            .as_ref()
            .map(|i| (i.source_id.as_str(), i.rel_path.as_str()));
        index
            .similar_folders(text, exclude, CANDIDATE_FOLDERS)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(source_id, dir, _)| {
                let source = self.sources.iter().find(|s| s.id == source_id)?;
                Some(folder_choice(source, &dir))
            })
            .collect()
    }

    fn save_cache(&self) {
        if let Ok(bytes) = serde_json::to_vec(&self.cache) {
            let tmp = self.cache_path.with_extension("json.tmp");
            if fs::write(&tmp, bytes).is_ok() {
                let _ = fs::rename(&tmp, &self.cache_path);
            }
        }
    }
}

fn suggested_folder(item: &IntakeItem) -> Option<FolderChoice> {
    let label = item.suggestion.as_ref()?.folder.as_ref()?;
    item.folders.iter().find(|f| &f.label == label).cloned()
}

pub fn folder_choice(source: &Source, dir: &str) -> FolderChoice {
    FolderChoice {
        source_id: source.id.clone(),
        dir: dir.to_string(),
        label: if dir.is_empty() {
            source.name.clone()
        } else {
            format!("{}/{dir}", source.name)
        },
    }
}

fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map(|(d, _)| d).unwrap_or("")
}

// MARK: - Filing

/// What the user decided in the review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filing {
    pub title: Option<String>,
    pub document_date: Option<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub favorite: bool,
    pub dest_source_id: String,
    pub dest_dir: String,
    /// Without extension.
    pub file_name: String,
}

/// Where a filed document ended up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Filed {
    pub source_id: String,
    pub rel_path: String,
}

/// Writes the reviewed details into the document, then renames and moves
/// it. If writing the details fails, the file stays where it is.
pub fn file_document(
    sources: &[Source],
    source_id: &str,
    rel_path: &str,
    filing: &Filing,
) -> Result<Filed> {
    let find = |id: &str| {
        sources
            .iter()
            .find(|s| s.id == id)
            .ok_or(Error::UnknownSource)
    };
    let from_source = find(source_id)?;
    let dest_source = find(&filing.dest_source_id)?;
    let path = resolve(&from_source.root, rel_path)?;
    let dest_dir = resolve(&dest_source.root, &filing.dest_dir)?;
    let local = cloud_state(&path)? == CloudState::Local;

    let existing = if local {
        meta::read_meta(&path)?.meta
    } else {
        meta::read_meta_beside(&path)?
            .map(|(m, _)| m)
            .unwrap_or_default()
    };
    let meta = DocMeta {
        title: filing.title.clone(),
        document_date: filing.document_date.clone(),
        tags: filing.tags.clone(),
        notes: filing.notes.clone(),
        favorite: filing.favorite,
        ..existing
    };
    if local {
        meta::write_meta(&path, meta, true)?;
    } else {
        meta::write_meta_beside(&path, meta)?;
    }

    let name = if filing.file_name.trim().is_empty() {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        filing.file_name.clone()
    };
    let new_path = fileops::move_document(&path, &dest_dir, &name)?;
    let rel = relative(&dest_source.root, &new_path)
        .ok_or_else(|| Error::InvalidPath(new_path.display().to_string()))?;
    Ok(Filed {
        source_id: dest_source.id.clone(),
        rel_path: rel,
    })
}

/// A small page image for the review screen: PNG of the first page of a
/// PDF, or the image itself. `None` for online-only files (showing them
/// would download them) and other formats.
pub fn preview(
    extractor: &TextExtractor,
    path: &Path,
    page: usize,
) -> Result<Option<(Vec<u8>, &'static str)>> {
    if cloud_state(path)? == CloudState::CloudOnly {
        return Ok(None);
    }
    match FileKind::from_path(path) {
        FileKind::Pdf => {
            let rendered = extractor.render_page(path, page, 1400, 2.0, true)?;
            Ok(Some((rendered.to_png()?, "image/png")))
        }
        FileKind::Image => {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let mime = match ext.as_str() {
                "jpg" | "jpeg" => "image/jpeg",
                "png" => "image/png",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "heic" => "image/heic",
                "tif" | "tiff" => "image/tiff",
                _ => return Ok(None),
            };
            Ok(Some((fs::read(path)?, mime)))
        }
        _ => Ok(None),
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::Provider;

    fn source(root: &Path) -> Source {
        Source {
            id: "s".into(),
            name: "Docs".into(),
            root: root.to_path_buf(),
            provider: Provider::Local,
            show_all_types: false,
            download_online_only: false,
        }
    }

    #[test]
    fn files_a_document() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Scans")).unwrap();
        fs::create_dir_all(dir.path().join("Insurance/SafeCo")).unwrap();
        fs::write(
            dir.path().join("Scans/scan 001.txt"),
            "auto policy cancelled",
        )
        .unwrap();
        let sources = [source(dir.path())];
        let filed = file_document(
            &sources,
            "s",
            "Scans/scan 001.txt",
            &Filing {
                title: Some("Safeco auto policy cancellation".into()),
                document_date: Some("2026-04-29".into()),
                tags: vec!["Insurance".into()],
                dest_source_id: "s".into(),
                dest_dir: "Insurance/SafeCo".into(),
                file_name: "2026-04-29 Safeco auto policy cancellation".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            filed.rel_path,
            "Insurance/SafeCo/2026-04-29 Safeco auto policy cancellation.txt"
        );
        let path = dir.path().join(&filed.rel_path);
        let meta = meta::read_meta(&path).unwrap().meta;
        assert_eq!(meta.document_date.as_deref(), Some("2026-04-29"));
        assert_eq!(meta.tags, ["Insurance"]);
        assert!(!dir.path().join("Scans/scan 001.txt").exists());
    }

    #[test]
    fn inbox_lists_and_processes_without_lm_studio() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Scans")).unwrap();
        fs::write(dir.path().join("Scans/note.txt"), "Harbor invoice").unwrap();
        let changes = Arc::new(Mutex::new(0));
        let c = Arc::clone(&changes);
        let handle = spawn(
            dir.path().join("index.sqlite"),
            dir.path().join("intake.json"),
            TextExtractor::new(None),
            move || *c.lock().unwrap() += 1,
        )
        .unwrap();
        let settings = Settings {
            inbox: Some(crate::settings::InboxLocation {
                source_id: "s".into(),
                rel_path: "Scans".into(),
            }),
            lm_studio_url: "http://127.0.0.1:9".into(), // nothing listens here
            ..Default::default()
        };
        handle.configure(settings.clone(), vec![source(dir.path())]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let items = handle.items();
            if items.first().is_some_and(|i| i.stage == Stage::Ready) {
                let item = &items[0];
                assert_eq!(item.rel_path, "Scans/note.txt");
                assert!(
                    item.note.as_deref().unwrap_or("").contains("LM Studio"),
                    "{item:?}"
                );
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "not processed: {items:?}"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(dir.path().join("intake.json").exists(), "results cached");

        // After a restart the cached results are used, not redone.
        drop(handle);
        let again = spawn(
            dir.path().join("index.sqlite"),
            dir.path().join("intake.json"),
            TextExtractor::new(None),
            || {},
        )
        .unwrap();
        again.configure(settings, vec![source(dir.path())]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while again.items().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(
            again.items()[0].stage,
            Stage::Ready,
            "ready at once from the cache"
        );
    }
}
