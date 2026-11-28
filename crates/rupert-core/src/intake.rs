//! The inbox: where new scans arrive, get their text recognized and a
//! filing suggestion, and wait for the user to review and file them.
//!
//! A background thread watches over the inbox folder (non-recursive):
//! 1. **OCR:** PDFs with pages that have no text get an invisible text
//!    layer ([`crate::ocr`]). Scanned images get their text recognized into
//!    their sidecar, so they can be searched too.
//! 2. **Suggestion:** Local AI proposes a title, document date, tags and
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
use crate::file_kind::{FileKind, image_mime};
use crate::fileops;
use crate::i18n::{tr, trf};
use crate::index::Index;
use crate::listing::{CloudState, cloud_state, list_dir, relative, resolve};
use crate::llm::{self, AppleAi, LocalAi, Reach, Suggestion};
use crate::meta::{self, DocMeta};
use crate::ocr::{self, Engine};
use crate::settings::{LocalEngine, Settings};
use crate::sidecar;
use crate::source::Source;
use crate::text::TextExtractor;

/// Where image OCR text is credited in the sidecar.
const OCR_TEXT_SOURCE: &str = "Rupert OCR";
/// Folders offered to the model (and in the review) per document.
const CANDIDATE_FOLDERS: usize = 12;
/// How many similar filed documents a suggestion is weighed against (#112).
const SIMILAR_DOCS: usize = 20;

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
    /// Problems worth showing (e.g. Local AI not running).
    pub note: Option<String>,
    /// Whether the suggestion is sure (#112), and why.
    pub confidence: Option<Confidence>,
}

/// Whether a suggestion agrees with how similar documents were filed
/// (#112): sure ones can be filed all at once.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Confidence {
    pub sure: bool,
    /// Why, in Rupert's language: "Like 6 others from Allianz in Car/Insurance".
    pub reason: String,
    /// The filed documents the suggestion learned from.
    #[serde(default)]
    pub learned_from: Vec<Learned>,
}

/// A filed document a suggestion learned from: how it was titled, filed and
/// tagged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Learned {
    pub title: String,
    pub folder: String,
    pub tags: Vec<String>,
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
    #[serde(default)]
    confidence: Option<Confidence>,
}

// MARK: - Handle

enum Msg {
    Configure(Box<Settings>, Vec<Source>),
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
        let _ = self.tx.send(Msg::Configure(Box::new(settings), sources));
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
                tried: HashMap::new(),
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
    /// Online-only scans already downloaded once (or tried), and why one
    /// couldn't be (#144). A scan seen on this computer again is forgotten
    /// here, so it's downloaded again if the system frees it later.
    tried: HashMap<String, Option<String>>,
}

impl Worker {
    fn run(&mut self, rx: Receiver<Msg>) {
        while let Ok(msg) = rx.recv() {
            self.apply(msg);
            for msg in rx.try_iter() {
                self.apply(msg);
            }
            // Process one item at a time, checking for news in between.
            // Scans the system made online-only are downloaded first (#144):
            // the inbox works with the files present.
            loop {
                self.scan();
                if let Some(index) = self.next_download() {
                    self.download(index);
                    continue;
                }
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
                    self.tried.clear();
                }
                self.configured = true;
                self.settings = *settings;
                self.sources = sources;
            }
            Msg::Refresh => {}
            Msg::Redo(rel) => {
                self.tried.remove(&rel);
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
        // On a drive that doesn't say which files are local, scans in the
        // inbox are read anyway (reading them is the point).
        let streamed = self
            .inbox()
            .and_then(|(source, rel)| resolve(&source.root, rel).ok())
            .is_some_and(|dir| readable_anyway(&dir));
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
                // Read anyway on such a drive, so shown and previewed as a
                // file on this computer ("Available offline" or not).
                cloud: if streamed { CloudState::Local } else { e.cloud },
                stage: Stage::Waiting,
                progress: None,
                ocr: None,
                suggestion: None,
                suggested_folder: None,
                folders: Vec::new(),
                note: None,
                confidence: None,
            };
            if let Some(d) = done {
                item.stage = Stage::Ready;
                item.ocr = d.ocr.clone();
                item.suggestion = d.suggestion.clone();
                item.folders = d.folders.clone();
                item.note = d.note.clone();
                item.suggested_folder = suggested_folder(&item);
                item.confidence = d.confidence.clone();
            } else if e.cloud == CloudState::CloudOnly && !streamed {
                // Waiting for its download (below), or it couldn't be had.
                item.stage = Stage::Ready;
                item.note = Some(match self.tried.get(&e.rel_path) {
                    Some(Some(why)) => {
                        trf("Online-only, and it couldn't be downloaded: {0}", &[why])
                    }
                    Some(None) => {
                        tr("Online-only: open it once to download it, then Rupert will read it.")
                            .into()
                    }
                    None => tr("Online-only: downloading it…").into(),
                });
            }
            if e.cloud != CloudState::CloudOnly {
                self.tried.remove(&e.rel_path);
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

    /// An online-only scan to download (#144): one not tried yet. On a
    /// drive that reads files on demand, processing reads it anyway.
    fn next_download(&self) -> Option<usize> {
        let streamed = self
            .inbox()
            .and_then(|(source, rel)| resolve(&source.root, rel).ok())
            .is_some_and(|dir| readable_anyway(&dir));
        if streamed {
            return None;
        }
        self.shared
            .lock()
            .unwrap()
            .items
            .iter()
            .position(|i| i.cloud == CloudState::CloudOnly && !self.tried.contains_key(&i.rel_path))
    }

    /// Downloads one online-only scan by reading it through, which the
    /// system answers by fetching it. Tried once per time it's online-only.
    fn download(&mut self, index: usize) {
        let Some(item) = self.items_snapshot().into_iter().nth(index) else {
            return;
        };
        let Some(path) = self
            .inbox()
            .and_then(|(source, _)| resolve(&source.root, &item.rel_path).ok())
        else {
            return;
        };
        self.update(index, |i| i.note = Some(tr("Downloading…").into()));
        let result = crate::cloud::download(&path);
        self.tried
            .insert(item.rel_path, result.err().map(|e| e.to_string()));
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
        let mut ocr_text = None;
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
                            ocr_summary = Some(trf(
                                "Text recognized on {0} of {1} pages ({2})",
                                &[&r.pages_recognized, &r.pages, &engine.name()],
                            ));
                            // The PDF couldn't take a text layer: keep the
                            // text beside it (searchable) and suggest from it.
                            if let Some(e) = r.layer_error {
                                match sidecar::write_text(&path, &r.text, OCR_TEXT_SOURCE, true) {
                                    Ok(_) => notes.push(trf(
                                        "The text couldn't be added to the PDF itself ({0}); \
                                         Rupert keeps it beside the file, so it can still be searched.",
                                        &[&e],
                                    )),
                                    Err(e2) => notes.push(trf(
                                        "Text recognition worked, but the text couldn't be saved: {0}; {1}",
                                        &[&e, &e2],
                                    )),
                                }
                                ocr_text = Some(r.text);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => notes.push(trf("Text recognition failed: {0}", &[&e])),
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
                                notes.push(trf("Couldn't save the recognized text: {0}", &[&e]));
                            }
                            ocr_summary = Some(trf("Text recognized ({0})", &[&engine.name()]));
                            ocr_text = Some(text);
                        }
                        Ok(_) => {}
                        Err(e) => notes.push(trf("Text recognition failed: {0}", &[&e])),
                    }
                }
                _ => {}
            }
        }

        // 2. Suggestion.
        let text = ocr_text.unwrap_or_else(|| {
            self.extractor
                .extract(&path)
                .ok()
                .flatten()
                .or_else(|| sidecar::read(&path).ok().flatten().and_then(|s| s.text))
                .unwrap_or_default()
        });
        let mut folders = Vec::new();
        let mut suggestion = None;
        let mut similar = Vec::new();
        let mut learned = Vec::new();
        let apple = self.settings.local_engine == LocalEngine::Apple;
        let elsewhere = !apple && llm::reach(&self.settings.local_url) == Reach::Elsewhere;
        if self.settings.suggestions && elsewhere {
            // Inbox suggestions never use a cloud service.
            notes.push(
                tr("Suggestions only use a Local AI server on this computer or your local network.")
                    .into(),
            );
        } else if self.settings.suggestions && !text.trim().is_empty() {
            self.update(index, |i| {
                i.stage = Stage::Suggesting;
                i.progress = None;
            });
            folders = self.candidate_folders(&text);
            let labels: Vec<String> = folders.iter().map(|f| f.label.clone()).collect();
            // #112: how the person filed documents like this one. Apple
            // Intelligence's context is short: two examples; a server model
            // takes three.
            similar = self.similar_docs(&text);
            learned = self.learned_from(&similar, if apple { 2 } else { 3 });
            let examples: Vec<String> = learned
                .iter()
                .map(|l| {
                    let mut e = format!("{} → {}", l.title, l.folder);
                    if !l.tags.is_empty() {
                        e.push_str(&format!(" · {}", l.tags.join(", ")));
                    }
                    e
                })
                .collect();
            let tags = Index::open(&self.index_path)
                .and_then(|i| i.top_tags(150))
                .unwrap_or_default();
            let year = crate::pdf_meta::utc_today()[..4].to_string();
            let req = llm::Request {
                text: &text,
                file_name: &item.name,
                year: &year,
                folders: &labels,
                tags: &tags,
                examples: &examples,
            };
            let result = if apple {
                AppleAi.suggest(&req)
            } else {
                LocalAi {
                    url: self.settings.local_url.clone(),
                    model: self.settings.local_model.clone(),
                }
                .suggest(&req)
            };
            match result {
                Ok(s) => suggestion = Some(s),
                Err(e) => notes.push(e.to_string()),
            }
        } else if self.settings.suggestions {
            notes.push(tr("No text to suggest from.").into());
        }

        // Cache against the file as it is now (OCR changed it).
        let entry = list_dir(&source.root, parent_of(&item.rel_path), false)
            .ok()
            .and_then(|l| l.into_iter().find(|e| e.rel_path == item.rel_path));
        let (size, modified_ms) = entry
            .map(|e| (e.size, e.modified_ms))
            .unwrap_or((item.size, item.modified_ms));
        let confidence = suggestion.as_ref().map(|s: &Suggestion| {
            let chosen = s
                .folder
                .as_ref()
                .and_then(|label| folders.iter().find(|f| &f.label == label));
            let mut c = assess(&similar, s.sender.as_deref(), chosen, |source, dir| {
                self.sources
                    .iter()
                    .find(|x| x.id == source)
                    .map(|x| folder_choice(x, dir).label)
                    .unwrap_or_else(|| dir.to_string())
            });
            c.learned_from = learned.clone();
            c
        });
        let done = Done {
            size,
            modified_ms,
            ocr: ocr_summary,
            suggestion,
            folders,
            note: (!notes.is_empty()).then(|| notes.join(" ")),
            confidence,
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
            i.confidence = done.confidence;
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

    /// Filed documents most like this one (#112), not in the inbox.
    fn similar_docs(&self, text: &str) -> Vec<crate::index::SimilarDoc> {
        let Ok(index) = Index::open(&self.index_path) else {
            return Vec::new();
        };
        let exclude = self
            .settings
            .inbox
            .as_ref()
            .map(|i| (i.source_id.as_str(), i.rel_path.as_str()));
        index
            .similar_docs(text, exclude, SIMILAR_DOCS)
            .unwrap_or_default()
            .into_iter()
            .filter(|d| self.sources.iter().any(|s| s.id == d.source_id))
            .collect()
    }

    /// The closest filed documents with a title, as examples for Local AI.
    fn learned_from(&self, similar: &[crate::index::SimilarDoc], n: usize) -> Vec<Learned> {
        similar
            .iter()
            .filter_map(|d| {
                let title = d.title.clone().filter(|t| !t.trim().is_empty())?;
                let source = self.sources.iter().find(|s| s.id == d.source_id)?;
                Some(Learned {
                    title,
                    folder: folder_choice(source, &d.dir).label,
                    tags: d.tags.clone(),
                })
            })
            .take(n)
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

// MARK: - Sure or not (#112)

/// Whether a suggestion is sure: the folder it suggests is where most of
/// the closest filed documents are (at least two, and more than in any
/// other), counting only those from the same sender when some are. `label`
/// names a folder ("Documents/Car/Insurance"). The reason is in Rupert's
/// language; `learned_from` is left for the caller.
pub fn assess(
    similar: &[crate::index::SimilarDoc],
    sender: Option<&str>,
    chosen: Option<&FolderChoice>,
    label: impl Fn(&str, &str) -> String,
) -> Confidence {
    let sender = sender.map(str::trim).filter(|s| !s.is_empty());
    let same_sender: Vec<_> = sender
        .map(|from| {
            similar
                .iter()
                .filter(|d| {
                    d.sender
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case(from))
                })
                .collect()
        })
        .unwrap_or_default();
    let (pool, from): (Vec<_>, Option<&str>) = if same_sender.is_empty() {
        (similar.iter().take(10).collect(), None)
    } else {
        (same_sender, sender)
    };
    let not_sure = |reason: String| Confidence {
        sure: false,
        reason,
        learned_from: Vec::new(),
    };
    if pool.is_empty() {
        return not_sure(tr("Nothing like it filed yet.").into());
    }
    // Folders by how many of the pool are in each, in order of similarity.
    let mut counts: Vec<((&str, &str), usize)> = Vec::new();
    for d in &pool {
        let key = (d.source_id.as_str(), d.dir.as_str());
        match counts.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => *n += 1,
            None => counts.push((key, 1)),
        }
    }
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    let ((best_source, best_dir), best) = counts[0];
    let second = counts.get(1).map(|(_, n)| *n).unwrap_or(0);
    let best_label = label(best_source, best_dir);
    let matches = chosen.is_some_and(|c| c.source_id == best_source && c.dir == best_dir);
    let others = best.to_string();
    if matches && best >= 2 && second < best {
        return Confidence {
            sure: true,
            reason: match from {
                Some(from) => trf(
                    "Like {0} others from {1} in {2}.",
                    &[&others, &from, &best_label],
                ),
                None => trf("Like {0} others in {1}.", &[&others, &best_label]),
            },
            learned_from: Vec::new(),
        };
    }
    if second > 0 {
        let ((s2, d2), n2) = counts[1];
        return not_sure(trf(
            "Similar documents are in two places: {0} in {1} and {2} in {3}.",
            &[
                &best.to_string(),
                &best_label,
                &n2.to_string(),
                &label(s2, d2),
            ],
        ));
    }
    if !matches {
        return not_sure(trf("Similar documents are in {0}.", &[&best_label]));
    }
    not_sure(tr("Only one document like it filed so far.").into())
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

/// Whether a file in the inbox may be read although it counts as
/// online-only: on a drive that streams files without saying which are
/// local (Google Drive on Windows), Rupert can't tell, and a new scan in the
/// inbox is meant to be read.
pub fn readable_anyway(path: &Path) -> bool {
    crate::cloud::streams_on_demand(path)
}

/// What the user decided in the review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Filing {
    pub title: Option<String>,
    pub document_date: Option<String>,
    pub tags: Vec<String>,
    pub notes: Option<String>,
    pub favorite: bool,
    /// From, amount, currency and due date (#108), as reviewed.
    #[serde(default)]
    pub sender: Option<String>,
    #[serde(default)]
    pub amount: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
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
    let local = cloud_state(&path)? == CloudState::Local || readable_anyway(&path);

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
        sender: filing.sender.clone(),
        amount: filing.amount.clone(),
        currency: filing.currency.clone(),
        due_date: filing.due_date.clone(),
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
    if cloud_state(path)? == CloudState::CloudOnly && !readable_anyway(path) {
        return Ok(None);
    }
    match FileKind::from_path(path) {
        FileKind::Pdf => {
            let rendered = extractor.render_page(path, page, 1400, 2.0, true)?;
            Ok(Some((rendered.to_png()?, "image/png")))
        }
        FileKind::Image => match image_mime(path) {
            Some(mime) => Ok(Some((fs::read(path)?, mime))),
            None => Ok(None),
        },
        _ => Ok(None),
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn similar(dir: &str, sender: Option<&str>) -> crate::index::SimilarDoc {
        crate::index::SimilarDoc {
            source_id: "s".into(),
            dir: dir.into(),
            name: "x.pdf".into(),
            title: Some("x".into()),
            tags: Vec::new(),
            sender: sender.map(Into::into),
        }
    }

    fn choice(dir: &str) -> FolderChoice {
        FolderChoice {
            source_id: "s".into(),
            dir: dir.into(),
            label: format!("Docs/{dir}"),
        }
    }

    #[test]
    fn sure_when_the_closest_filed_documents_agree() {
        let label = |_: &str, dir: &str| format!("Docs/{dir}");
        let docs = vec![
            similar("Car/Insurance", Some("Allianz")),
            similar("Home", Some("City Power")),
            similar("Car/Insurance", Some("ALLIANZ")),
            similar("Car/Insurance", Some("Allianz")),
            similar("Home", None),
        ];
        let sure = assess(
            &docs,
            Some("Allianz"),
            Some(&choice("Car/Insurance")),
            label,
        );
        assert!(sure.sure);
        assert_eq!(
            sure.reason,
            "Like 3 others from Allianz in Docs/Car/Insurance."
        );
        // Another folder than where they are: not sure.
        let other = assess(&docs, Some("Allianz"), Some(&choice("Home")), label);
        assert!(!other.sure);
        assert_eq!(other.reason, "Similar documents are in Docs/Car/Insurance.");
        // No sender known: all the closest count, and they're split.
        let split = assess(&docs[..2], None, Some(&choice("Car/Insurance")), label);
        assert!(!split.sure);
        assert!(
            split
                .reason
                .starts_with("Similar documents are in two places")
        );
        // Only one like it, or none.
        assert!(!assess(&docs[..1], None, Some(&choice("Car/Insurance")), label).sure);
        assert_eq!(
            assess(&[], None, None, label).reason,
            "Nothing like it filed yet."
        );
    }
    use crate::source::Provider;

    fn source(root: &Path) -> Source {
        Source {
            id: "s".into(),
            name: "Docs".into(),
            root: root.to_path_buf(),
            provider: Provider::Local,
            show_all_types: false,
            download_online_only: false,
            offline: false,
            bookmark: None,
        }
    }

    #[test]
    fn files_a_document() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Scans")).unwrap();
        fs::create_dir_all(dir.path().join("Insurance/Acme")).unwrap();
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
                title: Some("Acme auto policy cancellation".into()),
                document_date: Some("2026-04-29".into()),
                tags: vec!["Insurance".into()],
                dest_source_id: "s".into(),
                dest_dir: "Insurance/Acme".into(),
                file_name: "2026-04-29 Acme auto policy cancellation".into(),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            filed.rel_path,
            "Insurance/Acme/2026-04-29 Acme auto policy cancellation.txt"
        );
        let path = dir.path().join(&filed.rel_path);
        let meta = meta::read_meta(&path).unwrap().meta;
        assert_eq!(meta.document_date.as_deref(), Some("2026-04-29"));
        assert_eq!(meta.tags, ["Insurance"]);
        assert!(!dir.path().join("Scans/scan 001.txt").exists());
    }

    #[test]
    fn inbox_lists_and_processes_without_local_ai() {
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
            local_url: "http://127.0.0.1:9".into(), // nothing listens here
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
                    item.note.as_deref().unwrap_or("").contains("Local AI"),
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
