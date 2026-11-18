//! The local search index: one SQLite database per machine, in the app's
//! data folder. It is a cache — delete it and it is rebuilt from the files.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::error::{Error, Result};
use crate::file_kind::FileKind;
use crate::listing::CloudState;
use crate::meta::{DocMeta, Storage};

/// Bump to throw away and rebuild every user's index on schema changes.
const SCHEMA_VERSION: i32 = 3;

/// Private-use characters marking highlighted words in search snippets, so
/// the UI can split on them without parsing HTML.
pub const HIGHLIGHT_START: char = '\u{E000}';
pub const HIGHLIGHT_END: char = '\u{E001}';

// MARK: - Types

/// What the indexer knows about one document.
#[derive(Debug, Clone, PartialEq)]
pub struct DocRow {
    pub source_id: String,
    pub rel_path: String,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub cloud: CloudState,
    pub meta: DocMeta,
    pub storage: Storage,
    pub text_state: TextState,
    /// Modification time of the file's sidecar, if it has one.
    pub sidecar_ms: Option<i64>,
}

/// Where the indexed body text came from, or why there is none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TextState {
    /// Extracted from the local file.
    Extracted,
    /// Copied from the shared catalog (the file itself is online-only).
    Catalog,
    /// From the file's sidecar (e.g. ScanSnap Home's OCR text).
    Sidecar,
    /// The file has no text (e.g. a scan without OCR).
    Empty,
    /// Online-only and not in the catalog yet.
    CloudOnly,
    /// Format without text support (yet).
    Unsupported,
    /// Extraction failed.
    Failed,
}

impl TextState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Extracted => "extracted",
            Self::Catalog => "catalog",
            Self::Sidecar => "sidecar",
            Self::Empty => "empty",
            Self::CloudOnly => "cloudOnly",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "extracted" => Self::Extracted,
            "catalog" => Self::Catalog,
            "sidecar" => Self::Sidecar,
            "empty" => Self::Empty,
            "cloudOnly" => Self::CloudOnly,
            "unsupported" => Self::Unsupported,
            _ => Self::Failed,
        }
    }
}

/// Size, modification time and download state as last indexed (plus the
/// sidecar's modification time); used to decide whether a file needs
/// re-indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub cloud: CloudState,
    pub text_state: TextState,
    pub sidecar_ms: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    pub text: String,
    pub source_id: Option<String>,
    pub tag: Option<String>,
    pub favorites_only: bool,
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub source_id: String,
    pub rel_path: String,
    pub name: String,
    pub kind: FileKind,
    pub size: u64,
    pub modified_ms: Option<i64>,
    pub cloud: CloudState,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub document_date: Option<String>,
    pub favorite: bool,
    /// Body excerpt with matches wrapped in HIGHLIGHT_START/END.
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub tag: String,
    pub count: u32,
}

/// Indexed details shown in the file list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderMeta {
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub favorite: bool,
    pub document_date: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub documents: u32,
    pub with_text: u32,
    pub without_text: u32,
    pub cloud_only_unindexed: u32,
    pub favorites: u32,
}

// MARK: - Index

pub struct Index {
    conn: Connection,
}

impl Index {
    /// Opens (creating if needed) the index at `path`. An index from an
    /// older schema is discarded and rebuilt.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(db)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(db)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(db)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(db)?;
        let version: i32 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(db)?;
        if version != SCHEMA_VERSION {
            conn.execute_batch("DROP TABLE IF EXISTS docs; DROP TABLE IF EXISTS fts;")
                .map_err(db)?;
            conn.execute_batch(SCHEMA).map_err(db)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(db)?;
        }
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(db)?;
        conn.execute_batch(SCHEMA).map_err(db)?;
        Ok(Self { conn })
    }

    // MARK: Writing

    /// Inserts or replaces a document and its searchable text.
    pub fn upsert(&self, row: &DocRow, body: Option<&str>) -> Result<()> {
        let tx = self.conn.unchecked_transaction().map_err(db)?;
        let old: Option<i64> = tx
            .query_row(
                "SELECT id FROM docs WHERE source_id = ?1 AND rel_path = ?2",
                params![row.source_id, row.rel_path],
                |r| r.get(0),
            )
            .optional()
            .map_err(db)?;
        if let Some(id) = old {
            tx.execute("DELETE FROM fts WHERE rowid = ?1", [id])
                .map_err(db)?;
            tx.execute("DELETE FROM docs WHERE id = ?1", [id])
                .map_err(db)?;
        }
        let dir = row.rel_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let tags = serde_json::to_string(&row.meta.tags)?;
        tx.execute(
            "INSERT INTO docs (source_id, rel_path, dir, name, kind, size, mtime_ms, cloud,
                doc_id, title, tags, notes, document_date, favorite, storage, text_state,
                sidecar_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                row.source_id,
                row.rel_path,
                dir,
                row.name,
                kind_str(row.kind),
                row.size as i64,
                row.modified_ms,
                (row.cloud == CloudState::CloudOnly) as i64,
                row.meta.id,
                row.meta.title,
                tags,
                row.meta.notes,
                row.meta.document_date,
                row.meta.favorite as i64,
                storage_str(row.storage),
                row.text_state.as_str(),
                row.sidecar_ms,
            ],
        )
        .map_err(db)?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "INSERT INTO fts (rowid, name, title, tags, notes, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                searchable_name(&row.name),
                row.meta.title,
                row.meta.tags.join(" "),
                row.meta.notes,
                body
            ],
        )
        .map_err(db)?;
        tx.commit().map_err(db)
    }

    pub fn remove(&self, source_id: &str, rel_path: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM fts WHERE rowid IN
                   (SELECT id FROM docs WHERE source_id = ?1 AND rel_path = ?2)",
                params![source_id, rel_path],
            )
            .map_err(db)?;
        self.conn
            .execute(
                "DELETE FROM docs WHERE source_id = ?1 AND rel_path = ?2",
                params![source_id, rel_path],
            )
            .map_err(db)?;
        Ok(())
    }

    pub fn remove_source(&self, source_id: &str) -> Result<()> {
        self.conn
            .execute(
                "DELETE FROM fts WHERE rowid IN (SELECT id FROM docs WHERE source_id = ?1)",
                [source_id],
            )
            .map_err(db)?;
        self.conn
            .execute("DELETE FROM docs WHERE source_id = ?1", [source_id])
            .map_err(db)?;
        Ok(())
    }

    /// Removes every source not in `keep` (e.g. sources deleted while the
    /// app was closed).
    pub fn retain_sources(&self, keep: &[&str]) -> Result<()> {
        let existing: Vec<String> = {
            let mut stmt = self
                .conn
                .prepare("SELECT DISTINCT source_id FROM docs")
                .map_err(db)?;
            stmt.query_map([], |r| r.get(0))
                .map_err(db)?
                .collect::<std::result::Result<_, _>>()
                .map_err(db)?
        };
        for id in existing.iter().filter(|id| !keep.contains(&id.as_str())) {
            self.remove_source(id)?;
        }
        Ok(())
    }

    // MARK: Reading

    /// Index stamps for every document at or below `rel_dir` (`""` = whole
    /// source), keyed by relative path.
    pub fn stamps_under(
        &self,
        source_id: &str,
        rel_dir: &str,
    ) -> Result<std::collections::HashMap<String, Stamp>> {
        let (lo, hi) = prefix_range(rel_dir);
        let mut stmt = self
            .conn
            .prepare(
                "SELECT rel_path, size, mtime_ms, cloud, text_state, sidecar_ms FROM docs
                 WHERE source_id = ?1 AND rel_path >= ?2 AND rel_path < ?3",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![source_id, lo, hi], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Stamp {
                        size: r.get::<_, i64>(1)? as u64,
                        modified_ms: r.get(2)?,
                        cloud: if r.get::<_, i64>(3)? != 0 {
                            CloudState::CloudOnly
                        } else {
                            CloudState::Local
                        },
                        text_state: TextState::parse(&r.get::<_, String>(4)?),
                        sidecar_ms: r.get(5)?,
                    },
                ))
            })
            .map_err(db)?;
        rows.collect::<std::result::Result<_, _>>().map_err(db)
    }

    pub fn search(&self, q: &SearchQuery) -> Result<Vec<Hit>> {
        let fts = fts_query(&q.text);
        let limit = if q.limit == 0 { 200 } else { q.limit } as i64;
        let mut sql = String::from(
            "SELECT d.source_id, d.rel_path, d.name, d.kind, d.size, d.mtime_ms, d.cloud,
                    d.title, d.tags, d.document_date, d.favorite, ",
        );
        if fts.is_some() {
            sql.push_str(&format!(
                "snippet(fts, 4, '{HIGHLIGHT_START}', '{HIGHLIGHT_END}', '…', 14)
                 FROM fts JOIN docs d ON d.id = fts.rowid WHERE fts MATCH :q"
            ));
        } else {
            sql.push_str("NULL FROM docs d WHERE 1");
        }
        if q.source_id.is_some() {
            sql.push_str(" AND d.source_id = :source");
        }
        if q.tag.is_some() {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM json_each(d.tags) WHERE value = :tag COLLATE NOCASE)",
            );
        }
        if q.favorites_only {
            sql.push_str(" AND d.favorite = 1");
        }
        if fts.is_some() {
            // Weights: name, title, tags, notes, body.
            sql.push_str(" ORDER BY bm25(fts, 8.0, 10.0, 6.0, 3.0, 1.0)");
        } else {
            sql.push_str(
                " ORDER BY d.document_date IS NULL, d.document_date DESC, d.name COLLATE NOCASE",
            );
        }
        sql.push_str(" LIMIT :limit");

        let mut stmt = self.conn.prepare(&sql).map_err(db)?;
        let mut named: Vec<(&str, &dyn rusqlite::ToSql)> = vec![(":limit", &limit)];
        if let Some(f) = &fts {
            named.push((":q", f));
        }
        if let Some(s) = &q.source_id {
            named.push((":source", s));
        }
        if let Some(t) = &q.tag {
            named.push((":tag", t));
        }
        let rows = stmt
            .query_map(named.as_slice(), |r| {
                let tags: String = r.get(8)?;
                Ok(Hit {
                    source_id: r.get(0)?,
                    rel_path: r.get(1)?,
                    name: r.get(2)?,
                    kind: parse_kind(&r.get::<_, String>(3)?),
                    size: r.get::<_, i64>(4)? as u64,
                    modified_ms: r.get(5)?,
                    cloud: if r.get::<_, i64>(6)? != 0 {
                        CloudState::CloudOnly
                    } else {
                        CloudState::Local
                    },
                    title: r.get(7)?,
                    tags: serde_json::from_str(&tags).unwrap_or_default(),
                    document_date: r.get(9)?,
                    favorite: r.get::<_, i64>(10)? != 0,
                    snippet: r.get(11)?,
                })
            })
            .map_err(db)?;
        rows.collect::<std::result::Result<_, _>>().map_err(db)
    }

    pub fn tags(&self) -> Result<Vec<TagCount>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT value, COUNT(*) FROM docs, json_each(docs.tags)
                 GROUP BY value COLLATE NOCASE ORDER BY value COLLATE NOCASE",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(TagCount {
                    tag: r.get(0)?,
                    count: r.get(1)?,
                })
            })
            .map_err(db)?;
        rows.collect::<std::result::Result<_, _>>().map_err(db)
    }

    pub fn stats(&self) -> Result<IndexStats> {
        self.conn
            .query_row(
                "SELECT COUNT(*),
                        SUM(text_state IN ('extracted', 'catalog', 'sidecar')),
                        SUM(text_state IN ('empty', 'unsupported', 'failed')),
                        SUM(text_state = 'cloudOnly'),
                        SUM(favorite)
                 FROM docs",
                [],
                |r| {
                    Ok(IndexStats {
                        documents: r.get(0)?,
                        with_text: r.get::<_, Option<u32>>(1)?.unwrap_or(0),
                        without_text: r.get::<_, Option<u32>>(2)?.unwrap_or(0),
                        cloud_only_unindexed: r.get::<_, Option<u32>>(3)?.unwrap_or(0),
                        favorites: r.get::<_, Option<u32>>(4)?.unwrap_or(0),
                    })
                },
            )
            .map_err(db)
    }

    /// Folders that hold documents similar to `text` (sharing its most
    /// telling words), best first, as (source id, folder, matching
    /// documents). Folders under `exclude` (source id, folder) are skipped,
    /// e.g. the inbox itself.
    pub fn similar_folders(
        &self,
        text: &str,
        exclude: Option<(&str, &str)>,
        limit: usize,
    ) -> Result<Vec<(String, String, usize)>> {
        let words = telling_words(text, 40);
        if words.is_empty() {
            return Ok(Vec::new());
        }
        let query = words
            .iter()
            .map(|w| format!("\"{w}\""))
            .collect::<Vec<_>>()
            .join(" OR ");
        let mut stmt = self
            .conn
            .prepare(
                "SELECT d.source_id, d.dir FROM fts JOIN docs d ON d.id = fts.rowid
                 WHERE fts MATCH ?1 ORDER BY bm25(fts, 8.0, 10.0, 6.0, 3.0, 1.0) LIMIT 60",
            )
            .map_err(db)?;
        let rows: Vec<(String, String)> = stmt
            .query_map([query], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db)?
            .collect::<std::result::Result<_, _>>()
            .map_err(db)?;
        let mut counts: Vec<(String, String, usize)> = Vec::new();
        for (source, dir) in rows {
            if let Some((xs, xd)) = exclude
                && source == xs
                && (dir == xd || dir.starts_with(&format!("{xd}/")) || xd.is_empty())
            {
                continue;
            }
            match counts
                .iter_mut()
                .find(|(s, d, _)| *s == source && *d == dir)
            {
                Some(entry) => entry.2 += 1,
                None => counts.push((source, dir, 1)),
            }
        }
        // Stable sort keeps rank order among equal counts.
        counts.sort_by_key(|a| std::cmp::Reverse(a.2));
        counts.truncate(limit);
        Ok(counts)
    }

    /// Every folder that holds documents, and the folders above them, as
    /// (source id, folder), sorted.
    pub fn folders(&self) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT DISTINCT source_id, dir FROM docs")
            .map_err(db)?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db)?
            .collect::<std::result::Result<_, _>>()
            .map_err(db)?;
        let mut all = std::collections::BTreeSet::new();
        for (source, dir) in rows {
            let mut d = dir.as_str();
            loop {
                all.insert((source.clone(), d.to_string()));
                match d.rsplit_once('/') {
                    Some((parent, _)) => d = parent,
                    None if !d.is_empty() => d = "",
                    None => break,
                }
            }
        }
        Ok(all.into_iter().collect())
    }

    /// Tags by how often they're used, most used first.
    pub fn top_tags(&self, limit: usize) -> Result<Vec<String>> {
        let mut tags = self.tags()?;
        tags.sort_by_key(|a| std::cmp::Reverse(a.count));
        Ok(tags.into_iter().take(limit).map(|t| t.tag).collect())
    }

    /// Documents of exactly `size` bytes, as (source id, relative path).
    /// Used to find files that were moved or renamed outside Rupert.
    pub fn find_by_size(&self, size: u64) -> Result<Vec<(String, String)>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT source_id, rel_path FROM docs WHERE size = ?1")
            .map_err(db)?;
        let rows = stmt
            .query_map([size as i64], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(db)?;
        rows.collect::<std::result::Result<_, _>>().map_err(db)
    }

    /// Online-only documents of a source whose text is still unknown and
    /// could be read if downloaded (PDFs and plain text, up to `max_size`).
    pub fn download_candidates(&self, source_id: &str, max_size: u64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT rel_path FROM docs
                 WHERE source_id = ?1 AND cloud = 1 AND text_state = 'cloudOnly'
                   AND kind IN ('pdf', 'text') AND size <= ?2
                 ORDER BY rel_path",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![source_id, max_size as i64], |r| r.get(0))
            .map_err(db)?;
        rows.collect::<std::result::Result<_, _>>().map_err(db)
    }

    /// Where a document's indexed text came from, if it is indexed.
    pub fn text_state(&self, source_id: &str, rel_path: &str) -> Result<Option<TextState>> {
        self.conn
            .query_row(
                "SELECT text_state FROM docs WHERE source_id = ?1 AND rel_path = ?2",
                params![source_id, rel_path],
                |r| Ok(TextState::parse(&r.get::<_, String>(0)?)),
            )
            .optional()
            .map_err(db)
    }

    /// Indexed metadata for the files directly in a folder, for showing
    /// titles and tags in the file list.
    pub fn folder_meta(
        &self,
        source_id: &str,
        rel_dir: &str,
    ) -> Result<std::collections::HashMap<String, FolderMeta>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT rel_path, title, tags, favorite, document_date FROM docs
                 WHERE source_id = ?1 AND dir = ?2",
            )
            .map_err(db)?;
        let rows = stmt
            .query_map(params![source_id, rel_dir.trim_matches('/')], |r| {
                let tags: String = r.get(2)?;
                Ok((
                    r.get::<_, String>(0)?,
                    FolderMeta {
                        title: r.get(1)?,
                        tags: serde_json::from_str(&tags).unwrap_or_default(),
                        favorite: r.get::<_, i64>(3)? != 0,
                        document_date: r.get(4)?,
                    },
                ))
            })
            .map_err(db)?;
        rows.collect::<std::result::Result<_, _>>().map_err(db)
    }
}

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS docs (
    id INTEGER PRIMARY KEY,
    source_id TEXT NOT NULL,
    rel_path TEXT NOT NULL,
    dir TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime_ms INTEGER,
    cloud INTEGER NOT NULL,
    doc_id TEXT,
    title TEXT,
    tags TEXT NOT NULL DEFAULT '[]',
    notes TEXT,
    document_date TEXT,
    favorite INTEGER NOT NULL DEFAULT 0,
    storage TEXT NOT NULL,
    text_state TEXT NOT NULL,
    sidecar_ms INTEGER,
    UNIQUE (source_id, rel_path)
);
CREATE INDEX IF NOT EXISTS docs_dir ON docs (source_id, dir);
CREATE INDEX IF NOT EXISTS docs_size ON docs (size);
CREATE VIRTUAL TABLE IF NOT EXISTS fts USING fts5 (
    name, title, tags, notes, body,
    tokenize = 'unicode61 remove_diacritics 2',
    prefix = '2 3'
);
";

// MARK: - Helpers

fn db(e: rusqlite::Error) -> Error {
    Error::Index(e.to_string())
}

/// `[lo, hi)` covering every path at or below `dir`.
fn prefix_range(dir: &str) -> (String, String) {
    let dir = dir.trim_matches('/');
    if dir.is_empty() {
        (String::new(), "\u{10FFFF}".into())
    } else {
        // '/' + 1 == '0', so "a/" ≤ x < "a0" is everything inside "a/".
        (format!("{dir}/"), format!("{dir}0"))
    }
}

/// Splits file names on separators so "2025-10-30_Invoice.pdf" matches
/// "invoice" and "2025".
fn searchable_name(name: &str) -> String {
    name.replace(['_', '-', '.', '(', ')', '[', ']'], " ")
}

/// Turns what the user typed into an FTS5 query: every word must match, the
/// last one as a prefix (search-as-you-type). `None` for an empty query.
/// The words of `text` most likely to identify what it's about: longer
/// words that aren't numbers or common filler, most frequent first.
fn telling_words(text: &str, n: usize) -> Vec<String> {
    const STOP: &[&str] = &[
        "this",
        "that",
        "with",
        "from",
        "your",
        "have",
        "will",
        "they",
        "their",
        "there",
        "which",
        "were",
        "been",
        "what",
        "when",
        "where",
        "would",
        "could",
        "should",
        "about",
        "these",
        "those",
        "other",
        "into",
        "than",
        "then",
        "them",
        "page",
        "date",
        "please",
        "thank",
        "dear",
        "sincerely",
        "regards",
        "total",
        "amount",
        "number",
        "phone",
        "email",
        "only",
        "also",
        "more",
        "such",
        "each",
        "upon",
        "must",
        "does",
        "Sehr",
        "geehrte",
        "nicht",
        "eine",
        "einen",
        "einer",
        "oder",
        "sind",
        "wird",
        "werden",
        "para",
        "como",
        "esta",
        "este",
        "pero",
        "sobre",
    ];
    let mut counts: Vec<(String, usize)> = Vec::new();
    for raw in text.split(|c: char| !c.is_alphanumeric()) {
        let w = raw.to_lowercase();
        if w.chars().count() < 4
            || w.chars().any(|c| c.is_ascii_digit())
            || STOP.iter().any(|s| s.eq_ignore_ascii_case(&w))
        {
            continue;
        }
        match counts.iter_mut().find(|(x, _)| *x == w) {
            Some(e) => e.1 += 1,
            None => counts.push((w, 1)),
        }
        if counts.len() > 2000 {
            break;
        }
    }
    counts.sort_by_key(|a| std::cmp::Reverse(a.1));
    counts.into_iter().take(n).map(|(w, _)| w).collect()
}

pub fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\""))
        .collect();
    if words.is_empty() {
        return None;
    }
    let mut q = words.join(" ");
    q.push('*');
    Some(q)
}

fn kind_str(k: FileKind) -> &'static str {
    match k {
        FileKind::Pdf => "pdf",
        FileKind::Image => "image",
        FileKind::Office => "office",
        FileKind::Text => "text",
        FileKind::Email => "email",
        FileKind::Other => "other",
    }
}

fn parse_kind(s: &str) -> FileKind {
    match s {
        "pdf" => FileKind::Pdf,
        "image" => FileKind::Image,
        "office" => FileKind::Office,
        "text" => FileKind::Text,
        "email" => FileKind::Email,
        _ => FileKind::Other,
    }
}

fn storage_str(s: Storage) -> &'static str {
    match s {
        Storage::Embedded => "embedded",
        Storage::Sidecar => "sidecar",
        Storage::None => "none",
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn row(path: &str, title: Option<&str>, tags: &[&str], favorite: bool) -> DocRow {
        DocRow {
            source_id: "s".into(),
            rel_path: path.into(),
            name: path.rsplit('/').next().unwrap().into(),
            kind: FileKind::Pdf,
            size: 10,
            modified_ms: Some(1),
            cloud: CloudState::Local,
            meta: DocMeta {
                title: title.map(Into::into),
                tags: tags.iter().map(|t| t.to_string()).collect(),
                favorite,
                ..Default::default()
            },
            storage: Storage::Embedded,
            text_state: TextState::Extracted,
            sidecar_ms: None,
        }
    }

    fn names(hits: &[Hit]) -> Vec<&str> {
        hits.iter().map(|h| h.rel_path.as_str()).collect()
    }

    fn query(text: &str) -> SearchQuery {
        SearchQuery {
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn searches_body_names_titles_and_tags() {
        let idx = Index::open_in_memory().unwrap();
        idx.upsert(
            &row("Boat/2025-02_Invoice_4179.pdf", None, &[], false),
            Some("Replace Yanmar injectors and compression test"),
        )
        .unwrap();
        idx.upsert(
            &row("Home/lease.pdf", Some("Apartment lease"), &["Legal"], true),
            Some("The tenant agrees"),
        )
        .unwrap();

        assert_eq!(
            names(&idx.search(&query("yanmar")).unwrap()),
            ["Boat/2025-02_Invoice_4179.pdf"]
        );
        assert_eq!(
            names(&idx.search(&query("injec")).unwrap()).len(),
            1,
            "prefix match"
        );
        assert_eq!(
            names(&idx.search(&query("invoice 4179")).unwrap()).len(),
            1,
            "name words"
        );
        assert_eq!(
            names(&idx.search(&query("apartment")).unwrap()),
            ["Home/lease.pdf"]
        );
        assert_eq!(
            names(&idx.search(&query("legal")).unwrap()),
            ["Home/lease.pdf"]
        );
        assert!(
            idx.search(&query("yanmar lease")).unwrap().is_empty(),
            "all words must match"
        );

        let hit = &idx.search(&query("compression")).unwrap()[0];
        let snippet = hit.snippet.as_deref().unwrap();
        assert!(
            snippet.contains(&format!("{HIGHLIGHT_START}compression{HIGHLIGHT_END}")),
            "{snippet}"
        );
    }

    #[test]
    fn ignores_accents_and_punctuation_in_queries() {
        let idx = Index::open_in_memory().unwrap();
        idx.upsert(&row("a.pdf", Some("Électricité"), &[], false), None)
            .unwrap();
        assert_eq!(idx.search(&query("electricite")).unwrap().len(), 1);
        assert_eq!(idx.search(&query("\"elec*\" (")).unwrap().len(), 1);
        // FTS syntax in user input is treated as plain words, never an error.
        for odd in ["NEAR(", "a OR", "\"", "*", "title:x", "-y"] {
            assert!(idx.search(&query(odd)).is_ok(), "{odd}");
        }
    }

    #[test]
    fn filters_and_tags() {
        let idx = Index::open_in_memory().unwrap();
        idx.upsert(&row("a.pdf", None, &["Tax", "2025"], true), None)
            .unwrap();
        idx.upsert(&row("b.pdf", None, &["tax"], false), None)
            .unwrap();
        idx.upsert(&row("c.pdf", None, &[], false), None).unwrap();

        let by_tag = idx
            .search(&SearchQuery {
                tag: Some("TAX".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(by_tag.len(), 2);
        let favs = idx
            .search(&SearchQuery {
                favorites_only: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(names(&favs), ["a.pdf"]);
        let tags = idx.tags().unwrap();
        assert_eq!(
            tags.iter()
                .map(|t| (t.tag.to_lowercase(), t.count))
                .collect::<Vec<_>>(),
            [("2025".into(), 1), ("tax".into(), 2)]
        );
    }

    #[test]
    fn upsert_replaces_and_remove_deletes() {
        let idx = Index::open_in_memory().unwrap();
        idx.upsert(&row("a.pdf", None, &[], false), Some("old words"))
            .unwrap();
        idx.upsert(&row("a.pdf", None, &[], false), Some("new words"))
            .unwrap();
        assert!(idx.search(&query("old")).unwrap().is_empty());
        assert_eq!(idx.search(&query("new")).unwrap().len(), 1);
        assert_eq!(idx.stats().unwrap().documents, 1);
        idx.remove("s", "a.pdf").unwrap();
        assert!(idx.search(&query("new")).unwrap().is_empty());
    }

    #[test]
    fn stamps_under_a_folder() {
        let idx = Index::open_in_memory().unwrap();
        for p in ["Tax/a.pdf", "Tax/2025/b.pdf", "Taxes/c.pdf", "d.pdf"] {
            idx.upsert(&row(p, None, &[], false), None).unwrap();
        }
        let mut under: Vec<_> = idx.stamps_under("s", "Tax").unwrap().into_keys().collect();
        under.sort();
        assert_eq!(under, ["Tax/2025/b.pdf", "Tax/a.pdf"]);
        assert_eq!(idx.stamps_under("s", "").unwrap().len(), 4);
    }

    #[test]
    fn fts_query_quotes_words() {
        assert_eq!(fts_query("  "), None);
        assert_eq!(
            fts_query("PG&E bill").as_deref(),
            Some("\"PG\" \"E\" \"bill\"*")
        );
    }

    #[test]
    fn reopening_with_new_schema_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("index.sqlite");
        {
            let idx = Index::open(&path).unwrap();
            idx.upsert(&row("a.pdf", None, &[], false), None).unwrap();
            idx.conn.pragma_update(None, "user_version", 0).unwrap();
        }
        let idx = Index::open(&path).unwrap();
        assert_eq!(idx.stats().unwrap().documents, 0);
    }
}
