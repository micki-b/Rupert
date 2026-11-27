//! Import from ScanSnap Home (the ScanSnap scanner software on macOS).
//!
//! ScanSnap Home keeps its own catalog in a Core Data SQLite database
//! (`ScanSnapHome.sqlite`) plus a full-text database
//! (`ScanSnapHomeSearch_V2.db`) holding the OCR text of scans whose PDFs
//! have no text layer. Both are read from a temporary copy, never in place.
//!
//! What's imported, per item (only what isn't already known):
//! - the **document date**, when it differs from the date the file was made
//!   (otherwise it's just the scan date and adds nothing);
//! - the **favorite** flag;
//! - **receipt** details (vendor, amount, tax), as a "Receipt" tag, a line
//!   in the notes, and From and Amount where the document has none (#108);
//! - the **OCR text**, into the file's sidecar, unless the file's own text
//!   is already indexed.
//!
//! Items are found on disk by their folder path and filename. Files renamed
//! since (e.g. in Finder, which ScanSnap Home doesn't notice) are matched by
//! size and type within the same folder, then anywhere in Rupert's index.
//!
//! Local files get the details written into them (or their sidecar);
//! online-only files are never downloaded: their details go into the
//! sidecar as pending (see [`crate::sidecar`]).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::i18n::trf;
use crate::index::{Index, TextState};
use crate::listing::{CloudState, Entry, cloud_state, list_dir, relative, resolve};
use crate::meta::{self, DocMeta, Storage};
use crate::sidecar;
use crate::source::Source;

const MAIN_DB: &str = "ScanSnapHome.sqlite";
const SEARCH_DB: &str = "ScanSnapHomeSearch_V2.db";
/// Core Data timestamps count seconds from 2001-01-01 UTC.
const CORE_DATA_EPOCH: i64 = 978_307_200;
pub const TEXT_SOURCE: &str = "ScanSnap Home";
const RECEIPT_TAG: &str = "Receipt";

// MARK: - Plan types

/// How an item was found on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Match {
    /// At the path ScanSnap Home recorded.
    Exact,
    /// Renamed: the only file of the same size and type in the same folder.
    Renamed,
    /// Moved: the only indexed document of the same size and type.
    Moved,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub vendor: Option<String>,
    pub amount: Option<f64>,
    pub tax: Option<f64>,
    pub currency: Option<String>,
    /// YYYY-MM-DD. Not imported separately: ScanSnap Home also sets it as
    /// the document date.
    pub date: Option<String>,
}

impl Receipt {
    /// The line added to the notes, e.g. "Receipt: CVS, 20.50 USD (tax 0.92)".
    pub fn note(&self) -> String {
        let mut s = String::from("Receipt");
        if let Some(v) = self
            .vendor
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            s.push_str(": ");
            s.push_str(v);
        }
        if let Some(a) = self.amount {
            s.push_str(&format!(", {a:.2}"));
            if let Some(c) = self.currency.as_deref().filter(|c| !c.is_empty()) {
                s.push(' ');
                s.push_str(c);
            }
        }
        if let Some(t) = self.tax.filter(|t| *t > 0.0) {
            s.push_str(&format!(" (tax {t:.2})"));
        }
        s
    }
}

/// One ScanSnap Home item matched to a file in a Rupert source.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportItem {
    pub source_id: String,
    pub rel_path: String,
    pub matched: Match,
    pub cloud: CloudState,
    /// Where ScanSnap Home thinks the file is.
    pub scansnap_path: PathBuf,
    pub document_date: Option<String>,
    pub favorite: bool,
    pub receipt: Option<Receipt>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCount {
    pub path: String,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamedFile {
    pub from: String,
    pub source_id: String,
    pub rel_path: String,
}

/// What an import would do, for review before anything is written.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub library: String,
    /// Items in ScanSnap Home (not counting its trash).
    pub items: usize,
    pub in_trash: usize,
    /// Items with something worth importing.
    pub with_details: usize,
    /// Items matched to files in Rupert's sources: what will be imported.
    pub ready: usize,
    pub dates: usize,
    pub favorites: usize,
    pub receipts: usize,
    pub texts: usize,
    /// Ready items that are online-only (details go into sidecars).
    pub online_only: usize,
    /// Ready items whose only detail is OCR text, and whose own text is
    /// already indexed.
    pub already_searchable: usize,
    /// Matched by size after being renamed or moved outside ScanSnap Home.
    pub renamed: Vec<RenamedFile>,
    /// Items in folders that aren't (inside) a Rupert source, by the
    /// top-level ScanSnap Home folder.
    pub outside_sources: Vec<FolderCount>,
    /// Items whose file couldn't be found.
    pub not_found: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ImportPlan {
    pub summary: ImportSummary,
    pub items: Vec<ImportItem>,
}

// MARK: - Finding the library

/// ScanSnap Home's data folder, if it exists in the usual place (macOS).
pub fn default_library() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join("Library/Application Support/PFU/ScanSnap Home/Managed");
    dir.join(MAIN_DB).is_file().then_some(dir)
}

/// A temporary copy of the databases, deleted on drop.
struct Snapshot {
    dir: PathBuf,
}

impl Snapshot {
    fn take(library: &Path) -> Result<Self> {
        if !library.join(MAIN_DB).is_file() {
            return Err(Error::InvalidValue(trf(
                "no ScanSnap Home library in {0} ({1} is missing)",
                &[&library.display(), &MAIN_DB],
            )));
        }
        let dir = std::env::temp_dir().join(format!("rupert-scansnap-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir)?;
        let snap = Snapshot { dir };
        for db in [MAIN_DB, SEARCH_DB] {
            for suffix in ["", "-wal", "-shm"] {
                let name = format!("{db}{suffix}");
                let from = library.join(&name);
                if from.is_file() {
                    fs::copy(&from, snap.dir.join(&name))?;
                }
            }
        }
        Ok(snap)
    }

    fn open(&self, name: &str) -> Result<Option<Connection>> {
        let path = self.dir.join(name);
        if !path.is_file() {
            return Ok(None);
        }
        // Read-write so SQLite can replay the copied WAL; it's our copy.
        let conn = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(not_scansnap)?;
        Ok(Some(conn))
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn not_scansnap(e: rusqlite::Error) -> Error {
    Error::Unsupported(trf(
        "this doesn't look like a ScanSnap Home library this version of Rupert can read ({0})",
        &[&e],
    ))
}

// MARK: - Reading ScanSnap Home

struct Folder {
    path: PathBuf,
    parent: Option<i64>,
}

struct RawItem {
    folder: Option<i64>,
    file_name: String,
    in_trash: bool,
    favorite: bool,
    is_receipt: bool,
    content_id: Option<String>,
    size: Option<u64>,
    document_date: Option<String>,
    file_date: Option<String>,
    receipt: Receipt,
}

fn read_folders(db: &Connection) -> Result<HashMap<i64, Folder>> {
    let mut stmt = db
        .prepare("SELECT Z_PK, ZINDEXPATH, ZPARENTFOLDER FROM ZFOLDER WHERE ZINDEXPATH IS NOT NULL")
        .map_err(not_scansnap)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })
        .map_err(not_scansnap)?;
    let mut out = HashMap::new();
    for row in rows {
        let (pk, url, parent) = row.map_err(not_scansnap)?;
        if let Some(path) = file_url_to_path(&url) {
            out.insert(pk, Folder { path, parent });
        }
    }
    Ok(out)
}

fn read_items(db: &Connection) -> Result<Vec<RawItem>> {
    let receipt_entity: Option<i64> = db
        .query_row(
            "SELECT Z_ENT FROM Z_PRIMARYKEY WHERE Z_NAME = 'Receipt'",
            [],
            |r| r.get(0),
        )
        .optional()
        .map_err(not_scansnap)?;
    // Dates are stored as local midnight (or the scan time); 'localtime'
    // turns them back into the calendar day the user saw.
    let sql = format!(
        "SELECT c.ZCONTENTFOLDER, c.ZFILENAME, c.ZISINTRASH, c.ZISFAVORITE, c.Z_ENT,
                c.ZCONTENTID, c.ZFILESIZE,
                date(c.ZDOCUMENTDATE + {e}, 'unixepoch', 'localtime'),
                date(c.ZFILECREATEDATE + {e}, 'unixepoch', 'localtime'),
                c.ZAMOUNT, c.ZTAX,
                date(c.ZRECEIPTDATE + {e}, 'unixepoch', 'localtime'),
                v.ZVALUE, cs.ZVALUE
         FROM ZCONTENT c
         LEFT JOIN ZVENDOR v ON v.Z_PK = c.ZVENDOR
         LEFT JOIN ZCURRENCYSIGN cs ON cs.Z_PK = c.ZCURRENCYSIGN
         WHERE c.ZFILENAME IS NOT NULL",
        e = CORE_DATA_EPOCH
    );
    let mut stmt = db.prepare(&sql).map_err(not_scansnap)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(RawItem {
                folder: r.get(0)?,
                file_name: r.get(1)?,
                in_trash: r.get::<_, Option<i64>>(2)?.unwrap_or(0) != 0,
                favorite: r.get::<_, Option<i64>>(3)?.unwrap_or(0) != 0,
                is_receipt: receipt_entity.is_some()
                    && r.get::<_, Option<i64>>(4)? == receipt_entity,
                content_id: r.get(5)?,
                size: r.get::<_, Option<i64>>(6)?.map(|s| s as u64),
                document_date: r.get(7)?,
                file_date: r.get(8)?,
                receipt: Receipt {
                    amount: r.get(9)?,
                    tax: r.get(10)?,
                    date: r.get(11)?,
                    vendor: r.get(12)?,
                    currency: r.get(13)?,
                },
            })
        })
        .map_err(not_scansnap)?;
    rows.collect::<std::result::Result<_, _>>()
        .map_err(not_scansnap)
}

/// OCR text by ScanSnap content ID.
fn read_texts(db: &Connection) -> Result<HashMap<String, String>> {
    let mut stmt = db
        .prepare("SELECT contentId, text FROM TextTbl")
        .map_err(not_scansnap)?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, Option<String>>(0)?,
                r.get::<_, Option<String>>(1)?,
            ))
        })
        .map_err(not_scansnap)?;
    let mut out = HashMap::new();
    for row in rows {
        if let (Some(id), Some(text)) = row.map_err(not_scansnap)? {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if !text.is_empty() {
                out.insert(id, text);
            }
        }
    }
    Ok(out)
}

/// `file:///Users/me/A%20B/` → `/Users/me/A B`; `file:///C:/x/` → `C:/x`.
fn file_url_to_path(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let mut bytes = Vec::with_capacity(rest.len());
    let raw = rest.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut i = 0;
    while i < raw.len() {
        let decoded = (raw[i] == b'%' && i + 2 < raw.len())
            .then(|| Some(hex(raw[i + 1])? << 4 | hex(raw[i + 2])?))
            .flatten();
        match decoded {
            Some(b) => {
                bytes.push(b);
                i += 3;
            }
            None => {
                bytes.push(raw[i]);
                i += 1;
            }
        }
    }
    let mut s = String::from_utf8(bytes).ok()?;
    // Windows drive letters: "/C:/..." → "C:/..."
    if s.len() > 2 && s.as_bytes()[0] == b'/' && s.as_bytes()[2] == b':' {
        s.remove(0);
    }
    while s.len() > 1 && s.ends_with('/') {
        s.pop();
    }
    Some(PathBuf::from(s))
}

// MARK: - Planning

/// A source root, resolved once.
struct Root<'a> {
    source: &'a Source,
    canonical: PathBuf,
}

/// A ScanSnap folder as found on disk.
struct DirInfo {
    /// Index into `roots` and the folder's path relative to that root.
    place: Option<(usize, String)>,
    /// The folder's files (all types), if it exists.
    files: Vec<Entry>,
}

/// Reads ScanSnap Home's library and works out what to import into the
/// given sources. Writes nothing. `index` (when given) finds files moved to
/// other folders and skips OCR text for files whose own text is indexed.
pub fn plan(library: &Path, sources: &[Source], index: Option<&Index>) -> Result<ImportPlan> {
    let snapshot = Snapshot::take(library)?;
    let main = snapshot
        .open(MAIN_DB)?
        .ok_or_else(|| Error::InvalidValue(trf("{0} is missing", &[&MAIN_DB])))?;
    let folders = read_folders(&main)?;
    let raw = read_items(&main)?;
    let texts = match snapshot.open(SEARCH_DB)? {
        Some(db) => read_texts(&db).unwrap_or_default(),
        None => HashMap::new(),
    };
    drop(main);

    let roots: Vec<Root> = sources
        .iter()
        .filter(|s| s.is_available())
        .map(|s| Root {
            source: s,
            canonical: fs::canonicalize(&s.root).unwrap_or_else(|_| s.root.clone()),
        })
        .collect();

    // Every file ScanSnap Home knows (outside its trash), so a renamed item
    // is never matched to a file that is another item.
    let mut known: HashSet<PathBuf> = HashSet::new();
    for item in raw.iter().filter(|i| !i.in_trash) {
        if let Some(f) = item.folder.and_then(|f| folders.get(&f)) {
            known.insert(f.path.join(&item.file_name));
        }
    }

    let mut summary = ImportSummary {
        library: library.display().to_string(),
        ..Default::default()
    };
    let mut items = Vec::new();
    let mut dirs: HashMap<i64, DirInfo> = HashMap::new();
    let mut claimed: HashSet<(usize, String)> = HashSet::new();
    let mut outside: BTreeMap<String, usize> = BTreeMap::new();

    for item in raw {
        if item.in_trash {
            summary.in_trash += 1;
            continue;
        }
        summary.items += 1;

        let date = item
            .document_date
            .clone()
            .filter(|d| meta::is_iso_date(d) && Some(d) != item.file_date.as_ref());
        let receipt = item.is_receipt.then(|| item.receipt.clone());
        let text = item
            .content_id
            .as_ref()
            .and_then(|id| texts.get(id))
            .cloned();
        if date.is_none() && !item.favorite && receipt.is_none() && text.is_none() {
            continue;
        }
        summary.with_details += 1;

        let Some(folder_pk) = item.folder.filter(|f| folders.contains_key(f)) else {
            summary.not_found.push(item.file_name.clone());
            continue;
        };
        let folder = &folders[&folder_pk];
        let scansnap_path = folder.path.join(&item.file_name);
        let dir = dirs
            .entry(folder_pk)
            .or_insert_with(|| dir_info(&folder.path, &roots));
        let Some((root_ix, rel_dir)) = dir.place.clone() else {
            let top = top_folder(&folders, folder_pk);
            *outside.entry(top.display().to_string()).or_default() += 1;
            continue;
        };

        // Find the file: where it was, renamed in its folder, or moved.
        let ext = extension(&item.file_name);
        let mut found: Option<(usize, String, CloudState, Match)> = None;
        if let Some(e) = dir.files.iter().find(|e| e.name == item.file_name) {
            found = Some((root_ix, join(&rel_dir, &e.name), e.cloud, Match::Exact));
        } else if scansnap_path.exists() {
            // Same file under a differently normalized name.
            let rel = join(&rel_dir, &item.file_name);
            let cloud = cloud_state(&scansnap_path).unwrap_or(CloudState::Local);
            found = Some((root_ix, rel, cloud, Match::Exact));
        } else if let Some(size) = item.size {
            let candidates: Vec<&Entry> = dir
                .files
                .iter()
                .filter(|e| {
                    e.size == size
                        && extension(&e.name) == ext
                        && !known.contains(&folder.path.join(&e.name))
                })
                .collect();
            if let [e] = candidates[..] {
                found = Some((root_ix, join(&rel_dir, &e.name), e.cloud, Match::Renamed));
            } else if candidates.is_empty()
                && let Some(index) = index
            {
                found = find_moved(index, &roots, size, &ext, &known);
            }
        }

        let Some((root_ix, rel_path, cloud, matched)) = found else {
            summary.not_found.push(scansnap_path.display().to_string());
            continue;
        };
        if matched != Match::Exact && !claimed.insert((root_ix, rel_path.clone())) {
            // Two items claim the same file: too ambiguous to guess.
            summary.not_found.push(scansnap_path.display().to_string());
            continue;
        }
        let source = roots[root_ix].source;

        // OCR text adds nothing if the file's own text is already indexed.
        let text = text.filter(|_| {
            index.and_then(|i| i.text_state(&source.id, &rel_path).ok().flatten())
                != Some(TextState::Extracted)
        });
        if date.is_none() && !item.favorite && receipt.is_none() && text.is_none() {
            summary.already_searchable += 1;
            continue;
        }

        summary.ready += 1;
        summary.dates += date.is_some() as usize;
        summary.favorites += item.favorite as usize;
        summary.receipts += receipt.is_some() as usize;
        summary.texts += text.is_some() as usize;
        summary.online_only += (cloud == CloudState::CloudOnly) as usize;
        if matched != Match::Exact {
            summary.renamed.push(RenamedFile {
                from: scansnap_path.display().to_string(),
                source_id: source.id.clone(),
                rel_path: rel_path.clone(),
            });
        }
        items.push(ImportItem {
            source_id: source.id.clone(),
            rel_path,
            matched,
            cloud,
            scansnap_path,
            document_date: date,
            favorite: item.favorite,
            receipt,
            text,
        });
    }

    let mut outside: Vec<FolderCount> = outside
        .into_iter()
        .map(|(path, count)| FolderCount { path, count })
        .collect();
    outside.sort_by(|a, b| b.count.cmp(&a.count).then(a.path.cmp(&b.path)));
    summary.outside_sources = outside;
    summary.not_found.sort();
    Ok(ImportPlan { summary, items })
}

fn dir_info(path: &Path, roots: &[Root]) -> DirInfo {
    let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let place = roots.iter().enumerate().find_map(|(i, r)| {
        relative(&r.canonical, &canonical)
            .or_else(|| relative(&r.source.root, path))
            .map(|rel| (i, rel))
    });
    let files = match &place {
        Some((i, rel)) => list_dir(&roots[*i].source.root, rel, true)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !e.is_dir)
            .collect(),
        None => Vec::new(),
    };
    DirInfo { place, files }
}

/// The only indexed document of this size and type that ScanSnap Home
/// doesn't know under another name.
fn find_moved(
    index: &Index,
    roots: &[Root],
    size: u64,
    ext: &str,
    known: &HashSet<PathBuf>,
) -> Option<(usize, String, CloudState, Match)> {
    let hits: Vec<(usize, String)> = index
        .find_by_size(size)
        .ok()?
        .into_iter()
        .filter(|(_, rel)| extension(rel) == ext)
        .filter_map(|(sid, rel)| {
            let ix = roots.iter().position(|r| r.source.id == sid)?;
            let path = resolve(&roots[ix].source.root, &rel).ok()?;
            (!known.contains(&path) && path.exists()).then_some((ix, rel))
        })
        .collect();
    let [(ix, rel)] = &hits[..] else {
        return None;
    };
    let path = resolve(&roots[*ix].source.root, rel).ok()?;
    let cloud = cloud_state(&path).ok()?;
    Some((*ix, rel.clone(), cloud, Match::Moved))
}

fn top_folder(folders: &HashMap<i64, Folder>, mut pk: i64) -> PathBuf {
    let mut seen = 0;
    while let Some(parent) = folders[&pk].parent.filter(|p| folders.contains_key(p)) {
        pk = parent;
        seen += 1;
        if seen > 256 {
            break; // cycle guard
        }
    }
    folders[&pk].path.clone()
}

fn extension(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

// MARK: - Applying

/// What applying one item did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    /// Details written into the file.
    Embedded,
    /// Details and/or text written to the sidecar.
    Sidecar,
    /// Everything was already there.
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub source_id: String,
    pub rel_path: String,
    pub error: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub embedded: usize,
    pub sidecar: usize,
    pub unchanged: usize,
    pub failed: Vec<Failure>,
}

/// Applies every item in the plan, reporting progress as (done, total).
/// Returns the report and the folders touched, as (source id, rel dir),
/// so the caller can have them re-indexed.
pub fn apply(
    plan: &ImportPlan,
    sources: &[Source],
    mut on_progress: impl FnMut(usize, usize),
) -> (ImportReport, Vec<(String, String)>) {
    let mut report = ImportReport::default();
    let mut touched: HashSet<(String, String)> = HashSet::new();
    let total = plan.items.len();
    for (i, item) in plan.items.iter().enumerate() {
        let result = match sources.iter().find(|s| s.id == item.source_id) {
            Some(source) => {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| apply_item(source, item)))
                    .unwrap_or_else(|_| Err(Error::Pdf("crashed while writing".into())))
            }
            None => Err(Error::UnknownSource),
        };
        match result {
            Ok(Outcome::Unchanged) => report.unchanged += 1,
            Ok(outcome) => {
                if outcome == Outcome::Embedded {
                    report.embedded += 1;
                } else {
                    report.sidecar += 1;
                }
                let dir = item.rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
                touched.insert((item.source_id.clone(), dir.to_string()));
            }
            Err(e) => report.failed.push(Failure {
                source_id: item.source_id.clone(),
                rel_path: item.rel_path.clone(),
                error: e.to_string(),
            }),
        }
        on_progress(i + 1, total);
    }
    let mut touched: Vec<_> = touched.into_iter().collect();
    touched.sort();
    (report, touched)
}

/// Writes one item's details. Online-only files are never read: their
/// details go into the sidecar.
pub fn apply_item(source: &Source, item: &ImportItem) -> Result<Outcome> {
    let path = resolve(&source.root, &item.rel_path)?;
    let local = cloud_state(&path)? == CloudState::Local;

    let existing = if local {
        meta::read_meta(&path)?.meta
    } else {
        meta::read_meta_beside(&path)?
            .map(|(m, _)| m)
            .unwrap_or_default()
    };
    let merged = merge(existing.clone(), item);

    let mut outcome = Outcome::Unchanged;
    if merged != existing {
        outcome = if local {
            match meta::write_meta(&path, merged, true)?.storage {
                Storage::Embedded => Outcome::Embedded,
                _ => Outcome::Sidecar,
            }
        } else {
            meta::write_meta_beside(&path, merged)?;
            Outcome::Sidecar
        };
    }
    if let Some(text) = &item.text
        && sidecar::write_text(&path, text, TEXT_SOURCE, local)?
        && outcome == Outcome::Unchanged
    {
        outcome = Outcome::Sidecar;
    }
    Ok(outcome)
}

/// Adds the item's details to what the document already has, never
/// overwriting anything the user set.
fn merge(mut meta: DocMeta, item: &ImportItem) -> DocMeta {
    if meta.document_date.is_none() {
        meta.document_date = item.document_date.clone();
    }
    if item.favorite {
        meta.favorite = true;
    }
    if let Some(receipt) = &item.receipt {
        if !meta
            .tags
            .iter()
            .any(|t| t.eq_ignore_ascii_case(RECEIPT_TAG))
        {
            meta.tags.push(RECEIPT_TAG.into());
        }
        // From and amount as details of their own (#108), where none are
        // set; the note keeps the tax, which has no field.
        if meta.sender.is_none() {
            meta.sender = receipt
                .vendor
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(String::from);
        }
        if meta.amount.is_none()
            && let Some(a) = receipt.amount.filter(|a| a.is_finite())
        {
            meta.amount = Some(format!("{a:.2}"));
            meta.currency = receipt
                .currency
                .as_deref()
                .and_then(crate::meta::parse_currency);
        }
        let line = receipt.note();
        meta.notes = match meta.notes.take() {
            Some(n) if n.contains(&line) => Some(n),
            Some(n) => Some(format!("{n}\n{line}")),
            None => Some(line),
        };
    }
    meta
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::DocRow;
    use crate::source::Provider;

    const MAIN_SCHEMA: &str = "
        CREATE TABLE Z_PRIMARYKEY (Z_ENT INTEGER PRIMARY KEY, Z_NAME VARCHAR);
        INSERT INTO Z_PRIMARYKEY VALUES (6, 'Document'), (8, 'Receipt');
        CREATE TABLE ZFOLDER (Z_PK INTEGER PRIMARY KEY, ZINDEXPATH VARCHAR, ZPARENTFOLDER INTEGER);
        CREATE TABLE ZVENDOR (Z_PK INTEGER PRIMARY KEY, ZVALUE VARCHAR);
        CREATE TABLE ZCURRENCYSIGN (Z_PK INTEGER PRIMARY KEY, ZVALUE VARCHAR);
        CREATE TABLE ZCONTENT (Z_PK INTEGER PRIMARY KEY, Z_ENT INTEGER, ZCONTENTFOLDER INTEGER,
            ZFILENAME VARCHAR, ZISINTRASH INTEGER, ZISFAVORITE INTEGER, ZCONTENTID VARCHAR,
            ZFILESIZE INTEGER, ZDOCUMENTDATE TIMESTAMP, ZFILECREATEDATE TIMESTAMP,
            ZAMOUNT FLOAT, ZTAX FLOAT, ZRECEIPTDATE TIMESTAMP, ZVENDOR INTEGER,
            ZCURRENCYSIGN INTEGER);
    ";

    /// Core Data timestamp for noon local time on a date, so the local
    /// calendar day is the same in every time zone the tests run in.
    fn ts(conn: &Connection, date: &str) -> f64 {
        conn.query_row(
            "SELECT CAST(strftime('%s', ?1 || ' 12:00:00', 'utc') AS INTEGER) - ?2",
            rusqlite::params![date, CORE_DATA_EPOCH],
            |r| r.get::<_, i64>(0),
        )
        .unwrap() as f64
    }

    fn url(p: &Path) -> String {
        format!("file://{}/", p.display().to_string().replace(' ', "%20"))
    }

    struct Fixture {
        _tmp: tempfile::TempDir,
        library: PathBuf,
        docs: PathBuf,
        source: Source,
    }

    /// A library with a source folder "My Docs" holding:
    /// - bill.txt: a document date that differs from the file date
    /// - same.txt: a document date equal to the file date (nothing to import)
    /// - receipt.txt: a favorite receipt
    /// - Renamed Scan.txt: OCR text; ScanSnap still calls it scan.txt
    /// - gone.txt: favorite, but deleted from disk
    /// - trashed.txt: in ScanSnap's trash
    ///
    /// plus far.txt in a folder outside every source.
    fn fixture() -> Fixture {
        let tmp = tempfile::tempdir().unwrap();
        let docs = tmp.path().join("My Docs");
        let elsewhere = tmp.path().join("Elsewhere");
        fs::create_dir_all(&docs).unwrap();
        fs::create_dir_all(&elsewhere).unwrap();
        fs::write(docs.join("bill.txt"), "electric bill").unwrap();
        fs::write(docs.join("same.txt"), "same").unwrap();
        fs::write(docs.join("receipt.txt"), "cvs").unwrap();
        fs::write(docs.join("Renamed Scan.txt"), "12345").unwrap(); // 5 bytes
        fs::write(docs.join("other.txt"), "1234567").unwrap();
        fs::write(elsewhere.join("far.txt"), "far").unwrap();

        let library = tmp.path().join("Managed");
        fs::create_dir_all(&library).unwrap();
        let db = Connection::open(library.join(MAIN_DB)).unwrap();
        db.execute_batch(MAIN_SCHEMA).unwrap();
        db.execute("INSERT INTO ZFOLDER VALUES (1, ?1, NULL)", [url(&docs)])
            .unwrap();
        db.execute(
            "INSERT INTO ZFOLDER VALUES (2, ?1, NULL)",
            [url(&elsewhere)],
        )
        .unwrap();
        db.execute_batch(
            "INSERT INTO ZVENDOR VALUES (1, 'CVS'); INSERT INTO ZCURRENCYSIGN VALUES (1, 'USD');",
        )
        .unwrap();
        let (d1, d2, d3) = (
            ts(&db, "2024-03-05"),
            ts(&db, "2026-11-01"),
            ts(&db, "2026-09-12"),
        );
        type Row<'a> = (i64, i64, &'a str, i64, i64, &'a str, i64, f64, f64);
        let rows: Vec<Row> = vec![
            (6, 1, "bill.txt", 0, 0, "c1", 13, d1, d2),
            (6, 1, "same.txt", 0, 0, "c2", 4, d2, d2),
            (8, 1, "receipt.txt", 0, 1, "c3", 3, d3, d3),
            (6, 1, "scan.txt", 0, 0, "c4", 5, d2, d2),
            (6, 1, "gone.txt", 0, 1, "c5", 9, d2, d2),
            (6, 1, "trashed.txt", 1, 1, "c6", 9, d2, d2),
            (6, 2, "far.txt", 0, 1, "c7", 3, d2, d2),
        ];
        for (ent, folder, name, trash, fav, cid, size, doc, file) in rows {
            db.execute(
                "INSERT INTO ZCONTENT (Z_ENT, ZCONTENTFOLDER, ZFILENAME, ZISINTRASH, ZISFAVORITE,
                    ZCONTENTID, ZFILESIZE, ZDOCUMENTDATE, ZFILECREATEDATE)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![ent, folder, name, trash, fav, cid, size, doc, file],
            )
            .unwrap();
        }
        db.execute(
            "UPDATE ZCONTENT SET ZAMOUNT = 20.5, ZTAX = 0.92, ZVENDOR = 1, ZCURRENCYSIGN = 1,
                ZRECEIPTDATE = ZDOCUMENTDATE WHERE ZFILENAME = 'receipt.txt'",
            [],
        )
        .unwrap();
        drop(db);

        let search = Connection::open(library.join(SEARCH_DB)).unwrap();
        search
            .execute_batch(
                "CREATE VIRTUAL TABLE TextTbl USING fts5(contentId, text, words, languageCode);
                 INSERT INTO TextTbl VALUES ('c4', 'harbor  master
                    invoice', '', 'en');",
            )
            .unwrap();
        drop(search);

        let source = Source {
            id: "docs".into(),
            name: "My Docs".into(),
            root: docs.clone(),
            provider: Provider::Local,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        };
        Fixture {
            _tmp: tmp,
            library,
            docs,
            source,
        }
    }

    #[test]
    fn decodes_file_urls() {
        assert_eq!(
            file_url_to_path("file:///Users/me/Utilities%20&%20Telco/"),
            Some(PathBuf::from("/Users/me/Utilities & Telco"))
        );
        assert_eq!(
            file_url_to_path("file:///Users/me/Caf%C3%A9/"),
            Some(PathBuf::from("/Users/me/Café"))
        );
        assert_eq!(
            file_url_to_path("file:///C:/Scans/"),
            Some(PathBuf::from("C:/Scans"))
        );
        assert_eq!(file_url_to_path("https://x"), None);
    }

    #[test]
    fn plans_and_applies() {
        let f = fixture();
        let sources = [f.source.clone()];
        let plan = plan(&f.library, &sources, None).unwrap();
        let s = &plan.summary;
        assert_eq!(s.items, 6);
        assert_eq!(s.in_trash, 1);
        assert_eq!(s.with_details, 5, "{s:?}"); // bill, receipt, scan, gone, far
        assert_eq!(s.ready, 3, "{s:?}");
        assert_eq!((s.dates, s.favorites, s.receipts, s.texts), (1, 1, 1, 1));
        assert_eq!(s.renamed.len(), 1);
        assert_eq!(s.renamed[0].rel_path, "Renamed Scan.txt");
        assert_eq!(s.outside_sources.len(), 1);
        assert_eq!(s.outside_sources[0].count, 1);
        assert!(s.outside_sources[0].path.ends_with("Elsewhere"));
        assert_eq!(s.not_found.len(), 1);
        assert!(s.not_found[0].ends_with("gone.txt"));

        let (report, touched) = apply(&plan, &sources, |_, _| {});
        assert_eq!(report.failed, []);
        assert_eq!(report.sidecar, 3, "{report:?}"); // text files can't hold metadata
        assert_eq!(touched, [("docs".to_string(), String::new())]);

        let bill = meta::read_meta(&f.docs.join("bill.txt")).unwrap().meta;
        assert_eq!(bill.document_date.as_deref(), Some("2024-03-05"));
        assert!(bill.id.is_some());

        let receipt = meta::read_meta(&f.docs.join("receipt.txt")).unwrap().meta;
        assert!(receipt.favorite);
        assert_eq!(receipt.tags, ["Receipt"]);
        assert_eq!(
            receipt.notes.as_deref(),
            Some("Receipt: CVS, 20.50 USD (tax 0.92)")
        );
        assert_eq!(receipt.document_date.as_deref(), None, "same as file date");

        let scan = sidecar::read(&f.docs.join("Renamed Scan.txt"))
            .unwrap()
            .unwrap();
        assert_eq!(scan.meta, None);
        assert_eq!(scan.text.as_deref(), Some("harbor master invoice"));
        assert_eq!(scan.text_source.as_deref(), Some(TEXT_SOURCE));

        // Importing again changes nothing.
        let again = plan_and_apply(&f.library, &sources);
        assert_eq!(again.unchanged, 3, "{again:?}");
    }

    fn plan_and_apply(library: &Path, sources: &[Source]) -> ImportReport {
        let p = plan(library, sources, None).unwrap();
        apply(&p, sources, |_, _| {}).0
    }

    #[test]
    fn keeps_what_the_user_set() {
        let item = ImportItem {
            source_id: "s".into(),
            rel_path: "a.pdf".into(),
            matched: Match::Exact,
            cloud: CloudState::Local,
            scansnap_path: "/x/a.pdf".into(),
            document_date: Some("2020-01-01".into()),
            favorite: false,
            receipt: Some(Receipt {
                vendor: Some("CVS".into()),
                amount: Some(3.0),
                tax: None,
                currency: None,
                date: None,
            }),
            text: None,
        };
        let merged = merge(
            DocMeta {
                document_date: Some("2021-02-02".into()),
                tags: vec!["receipt".into()],
                notes: Some("Paid cash".into()),
                favorite: true,
                ..Default::default()
            },
            &item,
        );
        assert_eq!(merged.document_date.as_deref(), Some("2021-02-02"));
        assert_eq!(merged.tags, ["receipt"]);
        assert_eq!(
            merged.notes.as_deref(),
            Some("Paid cash\nReceipt: CVS, 3.00")
        );
        assert!(merged.favorite);
        // The receipt's vendor and amount become From and Amount (#108),
        // unless the document has its own.
        assert_eq!(merged.sender.as_deref(), Some("CVS"));
        assert_eq!(merged.amount.as_deref(), Some("3.00"));
        let kept = merge(
            DocMeta {
                sender: Some("CVS Pharmacy".into()),
                amount: Some("3.10".into()),
                ..Default::default()
            },
            &item,
        );
        assert_eq!(
            (kept.sender.as_deref(), kept.amount.as_deref()),
            (Some("CVS Pharmacy"), Some("3.10"))
        );
    }

    #[test]
    fn finds_moved_files_through_the_index() {
        let f = fixture();
        // scan.txt moved to a subfolder (and renamed); its old folder has
        // no file of that size any more.
        fs::create_dir(f.docs.join("Boat")).unwrap();
        fs::rename(
            f.docs.join("Renamed Scan.txt"),
            f.docs.join("Boat/harbor.txt"),
        )
        .unwrap();
        let index = Index::open_in_memory().unwrap();
        index
            .upsert(
                &DocRow {
                    source_id: "docs".into(),
                    rel_path: "Boat/harbor.txt".into(),
                    name: "harbor.txt".into(),
                    kind: crate::FileKind::Text,
                    size: 5,
                    modified_ms: None,
                    cloud: CloudState::Local,
                    meta: DocMeta::default(),
                    storage: Storage::None,
                    text_state: TextState::Empty,
                    sidecar_ms: None,
                },
                None,
            )
            .unwrap();
        let plan = plan(&f.library, std::slice::from_ref(&f.source), Some(&index)).unwrap();
        let moved = plan
            .items
            .iter()
            .find(|i| i.matched == Match::Moved)
            .expect("moved file found");
        assert_eq!(moved.rel_path, "Boat/harbor.txt");
        assert!(moved.text.is_some());
    }

    #[test]
    fn online_only_files_get_pending_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("scan.pdf");
        crate::pdf_meta::tests::make_pdf(&pdf, None);
        let before = fs::read(&pdf).unwrap();
        let source = Source {
            id: "s".into(),
            name: "s".into(),
            root: dir.path().to_path_buf(),
            provider: Provider::OneDrive,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        };
        let item = ImportItem {
            source_id: "s".into(),
            rel_path: "scan.pdf".into(),
            matched: Match::Exact,
            cloud: CloudState::CloudOnly,
            scansnap_path: pdf.clone(),
            document_date: Some("2019-06-30".into()),
            favorite: true,
            receipt: None,
            text: Some("boat title".into()),
        };
        // The fixture file is really local, so write as an online-only
        // file would be written.
        let merged = merge(DocMeta::default(), &item);
        meta::write_meta_beside(&pdf, merged).unwrap();
        sidecar::write_text(&pdf, "boat title", TEXT_SOURCE, false).unwrap();
        assert_eq!(fs::read(&pdf).unwrap(), before, "PDF untouched");

        // Once local, reading merges the pending details over the PDF's own.
        let rec = meta::read_meta(&pdf).unwrap();
        assert_eq!(rec.storage, Storage::Sidecar);
        assert_eq!(rec.meta.document_date.as_deref(), Some("2019-06-30"));
        assert!(rec.note.is_some());

        // Saving in Rupert moves the details into the PDF and keeps the text.
        let saved = meta::write_meta(&pdf, rec.meta, true).unwrap();
        assert_eq!(saved.storage, Storage::Embedded);
        assert!(saved.meta.favorite);
        let side = sidecar::read(&pdf).unwrap().unwrap();
        assert_eq!(side.meta, None);
        assert_eq!(side.text.as_deref(), Some("boat title"));

        // apply_item on the now-local PDF finds nothing left to do.
        assert_eq!(apply_item(&source, &item).unwrap(), Outcome::Unchanged);
    }
}
