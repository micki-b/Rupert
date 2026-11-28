//! Saved searches (#110): a search with its filters, kept under a name and
//! listed in the sidebar; choosing one runs it again, so it's always current.
//!
//! Each is kept with the source it searches, in the source's hidden
//! `.rupert/searches.json` (synced with the documents, so other computers
//! and the phone see it); a search over every source is kept in the first.
//! Like `.rupert/duplicates.json`, the file is a decision, not a cache.
//!
//! What a search is made of follows the window's search bar: words, tags
//! with any/all, favorites, and the date, kind, sender, due date and
//! folder filters. Dates stay relative ("this year" moves with the year).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::Result;
use crate::index::{SearchQuery, TagMode};
use crate::reminders::Day;

// MARK: - The search

/// A saved search, as kept in `searches.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSearch {
    /// Stable, so renaming and updating keep it in place.
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Words to find.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tag_mode: TagMode,
    #[serde(default)]
    pub favorites: bool,
    /// The date filter as the window has it: `{"kind": "thisYear"}`,
    /// `{"kind": "lastYear"}` or `{"kind": "range", "from", "to"}`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date: Option<Value>,
    /// A kind of file ("pdf", "image", …).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Documents from this sender.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// "upcoming" (due from today on) or "past".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub due: Option<String>,
    /// Only this source ("source") or a folder in it and its subfolders
    /// ("folder", see `folder`); none: every source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
    /// Newest first instead of best match first.
    #[serde(default)]
    pub newest: bool,
    /// Whatever a newer Rupert wrote, kept as it is.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl SavedSearch {
    /// Whether it looks in one source (and so belongs with it) rather than
    /// in every source.
    pub fn in_one_source(&self) -> bool {
        matches!(self.scope.as_deref(), Some("source") | Some("folder"))
    }

    /// The search to run, as the window runs it: `source_id` is the source
    /// it's kept with, and `today` places "this year" and "due from today
    /// on". The phone runs saved searches with this (#162).
    pub fn query(&self, source_id: &str, today: Day, limit: usize) -> SearchQuery {
        let (year, ..) = today.ymd();
        let field = |key: &str| {
            self.date
                .as_ref()
                .and_then(|d| d.get(key))
                .and_then(Value::as_str)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };
        let (date_from, date_to) = match field("kind").as_deref() {
            Some("thisYear") => (Some(format!("{year}-01-01")), Some(format!("{year}-12-31"))),
            Some("lastYear") => (
                Some(format!("{}-01-01", year - 1)),
                Some(format!("{}-12-31", year - 1)),
            ),
            Some("range") => (field("from"), field("to")),
            _ => (None, None),
        };
        let (due_from, due_to) = match self.due.as_deref() {
            Some("upcoming") => (Some(today.iso()), None),
            Some("past") => (None, Some(today.plus_days(-1).iso())),
            _ => (None, None),
        };
        SearchQuery {
            text: self.text.clone(),
            source_id: self.in_one_source().then(|| source_id.to_string()),
            tags: self.tags.clone(),
            tag_mode: self.tag_mode,
            favorites_only: self.favorites,
            limit,
            date_from,
            date_to,
            kinds: self.kind.iter().cloned().collect(),
            folder: (self.scope.as_deref() == Some("folder"))
                .then(|| self.folder.clone().unwrap_or_default()),
            by_date: self.newest,
            senders: self.from.iter().cloned().collect(),
            due_from,
            due_to,
            skip_excerpts: false,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SearchesFile {
    v: u32,
    #[serde(default)]
    searches: Vec<SavedSearch>,
}

// MARK: - Reading and writing

/// Where the file is, relative to the source folder (for sources read
/// through a web API, which fetch it by this path).
pub const REL_PATH: &str = ".rupert/searches.json";

fn path(root: &Path) -> PathBuf {
    root.join(crate::sidecar::DIR_NAME).join("searches.json")
}

/// The searches kept with a source, in the order saved (none if the file is
/// missing or can't be read).
pub fn list(root: &Path) -> Vec<SavedSearch> {
    std::fs::read(path(root))
        .map(|bytes| parse(&bytes))
        .unwrap_or_default()
}

/// The searches in a `searches.json` (none if it can't be read).
pub fn parse(bytes: &[u8]) -> Vec<SavedSearch> {
    serde_json::from_slice::<SearchesFile>(bytes)
        .map(|f| f.searches)
        .unwrap_or_default()
}

fn write(root: &Path, searches: Vec<SavedSearch>) -> Result<()> {
    let path = path(root);
    if searches.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(&SearchesFile { v: 1, searches })?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// Saves a search with a source: replaces the one with its id, or adds it
/// (with a new id if it has none). Returns it as saved.
pub fn save(root: &Path, mut search: SavedSearch) -> Result<SavedSearch> {
    search.name = search.name.trim().to_string();
    if search.name.is_empty() {
        return Err(crate::Error::InvalidValue(
            crate::i18n::tr("A saved search needs a name.").into(),
        ));
    }
    if search.id.is_empty() {
        search.id = uuid::Uuid::new_v4().to_string();
    }
    let mut all = list(root);
    match all.iter_mut().find(|s| s.id == search.id) {
        Some(s) => *s = search.clone(),
        None => all.push(search.clone()),
    }
    write(root, all)?;
    Ok(search)
}

/// Forgets a search (never any documents). `false` if it wasn't there.
pub fn remove(root: &Path, id: &str) -> Result<bool> {
    let mut all = list(root);
    let before = all.len();
    all.retain(|s| s.id != id);
    if all.len() == before {
        return Ok(false);
    }
    write(root, all)?;
    Ok(true)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    fn search(name: &str) -> SavedSearch {
        serde_json::from_value(
            serde_json::json!({"name": name, "tags": ["Taxes", "2025"], "tagMode": "all"}),
        )
        .unwrap()
    }

    #[test]
    fn save_rename_update_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(dir.path()).is_empty());
        let tax = save(dir.path(), search("Tax 2025")).unwrap();
        assert!(!tax.id.is_empty());
        let mut boat = search("  Boat ");
        boat.text = "boat".into();
        boat.tags.clear();
        boat.date = Some(serde_json::json!({"kind": "thisYear"}));
        let boat = save(dir.path(), boat).unwrap();
        assert_eq!(boat.name, "Boat");
        // Renaming keeps it in its place.
        let renamed = save(
            dir.path(),
            SavedSearch {
                name: "Taxes".into(),
                ..tax.clone()
            },
        )
        .unwrap();
        let names: Vec<_> = list(dir.path()).into_iter().map(|s| s.name).collect();
        assert_eq!(names, ["Taxes", "Boat"]);
        assert_eq!(list(dir.path())[0].tag_mode, TagMode::All);
        assert_eq!(list(dir.path())[1].date, boat.date);
        assert!(save(dir.path(), search("  ")).is_err());
        // Removing the last one removes the file.
        assert!(remove(dir.path(), &renamed.id).unwrap());
        assert!(!remove(dir.path(), &renamed.id).unwrap());
        assert!(remove(dir.path(), &boat.id).unwrap());
        assert!(!path(dir.path()).exists());
    }

    #[test]
    fn runs_as_the_window_does() {
        let today = Day::from_ymd(2026, 11, 28);
        let s: SavedSearch = serde_json::from_value(serde_json::json!({
            "name": "Unpaid bills", "text": "edison", "date": {"kind": "thisYear"},
            "kind": "pdf", "from": "Edison", "due": "upcoming",
            "scope": "folder", "folder": "Bills/Utilities", "newest": true
        }))
        .unwrap();
        let q = s.query("src1", today, 200);
        assert_eq!(q.text, "edison");
        assert_eq!(q.source_id.as_deref(), Some("src1"));
        assert_eq!(q.folder.as_deref(), Some("Bills/Utilities"));
        assert_eq!(
            (q.date_from.as_deref(), q.date_to.as_deref()),
            (Some("2026-01-01"), Some("2026-12-31"))
        );
        assert_eq!(q.kinds, ["pdf"]);
        assert_eq!(q.senders, ["Edison"]);
        assert_eq!(
            (q.due_from.as_deref(), q.due_to),
            (Some("2026-11-28"), None)
        );
        assert!(q.by_date);
        assert_eq!(q.limit, 200);

        // Every source; last year; overdue; a range with one end.
        let mut s = search("Old");
        s.date = Some(serde_json::json!({"kind": "lastYear"}));
        s.due = Some("past".into());
        let q = s.query("src1", today, 50);
        assert_eq!((q.source_id, q.folder), (None, None));
        assert_eq!(q.date_from.as_deref(), Some("2025-01-01"));
        assert_eq!(
            (q.due_from, q.due_to.as_deref()),
            (None, Some("2026-11-27"))
        );
        assert_eq!(q.tags, ["Taxes", "2025"]);
        assert_eq!(q.tag_mode, TagMode::All);
        s.date = Some(serde_json::json!({"kind": "range", "from": "2024-03-01", "to": ""}));
        s.scope = Some("source".into());
        let q = s.query("src1", today, 50);
        assert_eq!(
            (q.date_from.as_deref(), q.date_to),
            (Some("2024-03-01"), None)
        );
        assert_eq!((q.source_id.as_deref(), q.folder), (Some("src1"), None));
    }

    #[test]
    fn parses_what_is_fetched() {
        assert!(parse(b"not json").is_empty());
        let got = parse(br#"{"v": 1, "searches": [{"id": "a", "name": "Bills"}]}"#);
        assert_eq!(got[0].name, "Bills");
    }

    #[test]
    fn keeps_what_a_newer_rupert_wrote() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".rupert")).unwrap();
        std::fs::write(
            path(dir.path()),
            r#"{"v": 1, "searches": [{"id": "a", "name": "Bills", "color": "red"}]}"#,
        )
        .unwrap();
        let mut s = list(dir.path()).remove(0);
        assert_eq!(s.other.get("color"), Some(&serde_json::json!("red")));
        s.name = "Unpaid bills".into();
        save(dir.path(), s).unwrap();
        let again = list(dir.path()).remove(0);
        assert_eq!(again.name, "Unpaid bills");
        assert_eq!(again.other.get("color"), Some(&serde_json::json!("red")));
    }
}
