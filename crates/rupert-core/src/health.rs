//! Library health (#113) and duplicates (#111): which documents could be
//! better (the counts are `Index::health_counts`), and which are copies of
//! one another, with clearing them up.
//!
//! Duplicates come in three kinds:
//! - **The same file**: same size and the same bytes (SHA-256), only for
//!   files on this computer (hashing an online-only file would download it).
//! - **The same text**: the same recognized text (at least a paragraph's
//!   worth), which also catches a PDF Rupert wrote details into, or the
//!   same paper scanned twice when OCR read it alike.
//! - **Probably the same**: an online-only file with the same size and name
//!   as another. Shown, never trashed on that alone.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;
use crate::index::{Hit, Index};
use crate::listing::CloudState;
use crate::meta::{self, DocMeta};
use crate::source::Source;

// MARK: - Scanner-style names

/// Words scanners, cameras and "untitled" put before a number.
const MACHINE_WORDS: &[&str] = &[
    "scan",
    "scanned",
    "scansnap",
    "img",
    "image",
    "dsc",
    "dscn",
    "pxl",
    "photo",
    "document",
    "doc",
    "untitled",
    "escaneo",
    "escaneado",
    "documento",
    "imagen",
    "sin título",
    "dokument",
    "unbenannt",
    "bild",
];

/// Whether a file name looks like one a scanner or camera gave it: only
/// digits and separators ("20251016_134520.pdf"), or a machine word and
/// then only those ("Scan 2026-11-12 08.31.pdf", "IMG_2034.jpg").
pub fn scanner_name(name: &str) -> bool {
    let stem = name
        .rsplit_once('.')
        .map_or(name, |(s, _)| s)
        .trim()
        .to_lowercase();
    let rest = MACHINE_WORDS
        .iter()
        .filter(|w| stem.starts_with(*w))
        .max_by_key(|w| w.len())
        .map_or(stem.as_str(), |w| &stem[w.len()..]);
    let digits = rest.chars().filter(char::is_ascii_digit).count();
    let only_numbers = rest
        .chars()
        .all(|c| c.is_ascii_digit() || " _-.,()#".contains(c));
    let had_word = rest.len() != stem.len();
    only_numbers
        && (digits >= 6 || (had_word && digits >= 1) || (had_word && rest.trim().is_empty()))
}

// MARK: - Duplicates

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DupKind {
    /// The same bytes.
    SameFile,
    /// The same recognized text.
    SameText,
    /// Online-only, same size and name: likely, not certain.
    Probably,
}

/// Documents that are copies of one another, and the one Rupert would keep.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupGroup {
    pub kind: DupKind,
    pub docs: Vec<Hit>,
    /// Index into `docs` of the copy to keep.
    pub keep: usize,
    /// What makes them the same, whatever their names and folders: the
    /// content hash ("file:…"), the text's ("text:…"), or size and name
    /// ("probably:…"). Keep all remembers this.
    pub key: String,
    /// Kept as they are (Keep all), with no copy added since.
    pub kept: bool,
}

/// Text shorter than this (after tidying) doesn't make two documents the
/// same: a one-line receipt can match another by chance.
const MIN_SAME_TEXT: usize = 300;

/// What the duplicate search needs from the index, read in one go so the
/// index isn't held while files are hashed: documents grouped by size, and
/// a fingerprint of each document's text (long enough to count).
pub struct Candidates {
    pub by_size: Vec<Vec<Hit>>,
    pub by_text: Vec<(Hit, [u8; 32])>,
}

pub fn candidates(index: &Index, source_id: Option<&str>) -> Result<Candidates> {
    let mut by_text = Vec::new();
    index.for_each_text(source_id, |h, body| {
        let tidy: String = body
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if tidy.chars().count() < MIN_SAME_TEXT {
            return;
        }
        use sha2::Digest;
        by_text.push((h, sha2::Sha256::digest(tidy.as_bytes()).into()));
    })?;
    Ok(Candidates {
        by_size: index.same_size_groups(source_id)?,
        by_text,
    })
}

/// Finds duplicates among `c` (see [`candidates`]). `progress(done, total)`
/// follows the hashing; `stop` is checked between files.
pub fn find_duplicates(
    c: Candidates,
    sources: &[Source],
    mut progress: impl FnMut(usize, usize),
    stop: impl Fn() -> bool,
) -> Result<Vec<DupGroup>> {
    let root = |id: &str| sources.iter().find(|s| s.id == id).map(|s| s.root.clone());
    let mut groups: Vec<DupGroup> = Vec::new();
    // Documents already in a group of the same bytes, by place.
    let mut in_same: HashMap<(String, String), usize> = HashMap::new();

    // 1. The same bytes, among files of the same size.
    let sized = c.by_size;
    let total: usize = sized.iter().map(Vec::len).sum();
    let mut done = 0;
    for group in sized {
        let mut by_hash: HashMap<String, Vec<Hit>> = HashMap::new();
        let mut online: Vec<Hit> = Vec::new();
        for h in group {
            if stop() {
                return Ok(groups);
            }
            done += 1;
            progress(done, total);
            if h.cloud == CloudState::CloudOnly {
                online.push(h);
                continue;
            }
            let Some(path) = root(&h.source_id).and_then(|r| crate::resolve(&r, &h.rel_path).ok())
            else {
                continue;
            };
            if let Ok(hash) = crate::sidecar::sha256(&path) {
                by_hash.entry(hash).or_default().push(h);
            }
        }
        let local: Vec<Hit> = by_hash.values().flatten().cloned().collect();
        for (hash, docs) in by_hash.into_iter().filter(|(_, d)| d.len() > 1) {
            let n = groups.len();
            for d in &docs {
                in_same.insert((d.source_id.clone(), d.rel_path.clone()), n);
            }
            groups.push(group_of(DupKind::SameFile, docs, format!("file:{hash}")));
        }
        // Online-only ones with a same-sized, same-named partner.
        for o in online {
            let partners: Vec<Hit> = local
                .iter()
                .chain(std::iter::once(&o))
                .filter(|h| h.name.to_lowercase() == o.name.to_lowercase())
                .cloned()
                .collect();
            let already = groups
                .iter()
                .any(|g| g.kind == DupKind::Probably && g.docs.iter().any(|d| same_place(d, &o)));
            if partners.len() > 1 && !already {
                groups.push(group_of(DupKind::Probably, partners, String::new()));
            } else if partners.len() == 1 {
                // Two online-only files of this size and name.
                if let Some(g) = groups.iter_mut().rev().find(|g| {
                    g.kind == DupKind::Probably
                        && g.docs[0].size == o.size
                        && g.docs[0].name.to_lowercase() == o.name.to_lowercase()
                }) {
                    if !g.docs.iter().any(|d| same_place(d, &o)) {
                        g.docs.push(o);
                    }
                } else {
                    groups.push(DupGroup {
                        kind: DupKind::Probably,
                        docs: vec![o],
                        keep: 0,
                        key: String::new(),
                        kept: false,
                    });
                }
            }
        }
    }
    groups.retain(|g| g.docs.len() > 1);
    for g in groups.iter_mut().filter(|g| g.kind == DupKind::Probably) {
        g.keep = keep_of(&g.docs);
        g.key = format!(
            "probably:{}:{}",
            g.docs[0].size,
            g.docs[0].name.to_lowercase()
        );
    }

    // 2. The same text, where the bytes differ.
    let mut by_text: HashMap<[u8; 32], Vec<Hit>> = HashMap::new();
    for (h, key) in c.by_text {
        by_text.entry(key).or_default().push(h);
    }
    for (text_key, docs) in by_text.into_iter().filter(|(_, d)| d.len() > 1) {
        // Already one group of the same bytes: nothing new.
        let firsts: Vec<Option<&usize>> = docs
            .iter()
            .map(|d| in_same.get(&(d.source_id.clone(), d.rel_path.clone())))
            .collect();
        if firsts.iter().all(|f| f.is_some() && *f == firsts[0]) {
            continue;
        }
        let hex: String = text_key.iter().map(|b| format!("{b:02x}")).collect();
        groups.push(group_of(DupKind::SameText, docs, format!("text:{hex}")));
    }
    // Groups kept as they are, in any of the sources they're in.
    let kept: Vec<Kept> = sources.iter().flat_map(|s| kept_list(&s.root)).collect();
    for g in &mut groups {
        g.kept = kept
            .iter()
            .any(|k| k.key == g.key && g.docs.len() <= k.copies);
    }
    groups.sort_by_key(|g| (g.kind as u8, g.docs[0].name.to_lowercase()));
    Ok(groups)
}

fn same_place(a: &Hit, b: &Hit) -> bool {
    a.source_id == b.source_id && a.rel_path == b.rel_path
}

fn group_of(kind: DupKind, mut docs: Vec<Hit>, key: String) -> DupGroup {
    docs.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    let keep = keep_of(&docs);
    DupGroup {
        kind,
        docs,
        keep,
        key,
        kept: false,
    }
}

// MARK: - Duplicates kept as they are

/// A group the user chose to keep as it is ("Keep all"): its key, and how
/// many copies it had then (a further copy brings it back).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize)]
pub struct Kept {
    pub key: String,
    pub copies: usize,
}

#[derive(Debug, Default, Serialize, serde::Deserialize)]
struct KeptFile {
    v: u32,
    #[serde(default)]
    kept: Vec<Kept>,
}

/// Where a source keeps them: beside its catalog in the hidden `.rupert`
/// folder at its root, so it syncs with the library (every computer
/// honors it) and no document changes.
fn kept_path(root: &Path) -> std::path::PathBuf {
    root.join(".rupert").join("duplicates.json")
}

/// The groups kept as they are in a source (none if it has no list).
pub fn kept_list(root: &Path) -> Vec<Kept> {
    std::fs::read(kept_path(root))
        .ok()
        .and_then(|b| serde_json::from_slice::<KeptFile>(&b).ok())
        .map(|f| f.kept)
        .unwrap_or_default()
}

/// Keeps a group as it is (`keep`), or lets it be reviewed again.
pub fn set_kept(root: &Path, key: &str, copies: usize, keep: bool) -> Result<()> {
    let mut list = kept_list(root);
    list.retain(|k| k.key != key);
    if keep {
        list.push(Kept {
            key: key.to_string(),
            copies,
        });
    }
    write_kept(root, list)
}

/// Forgets every group kept in a source (Settings ▸ Library ▸ Reset).
pub fn reset_kept(root: &Path) -> Result<()> {
    let path = kept_path(root);
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn write_kept(root: &Path, kept: Vec<Kept>) -> Result<()> {
    let path = kept_path(root);
    if kept.is_empty() {
        return reset_kept(root);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(&KeptFile { v: 1, kept })?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

/// The copy to keep: the one with the most details, then in the deeper
/// (more specific) folder, then with the cleaner name (not a scanner's,
/// no " (2)" or "copy"), then on this computer, then the oldest.
pub fn keep_of(docs: &[Hit]) -> usize {
    let score = |h: &Hit| {
        let details = h.title.is_some() as i32
            + h.document_date.is_some() as i32
            + (!h.tags.is_empty()) as i32
            + h.sender.is_some() as i32
            + h.favorite as i32;
        let depth = h.rel_path.matches('/').count() as i32;
        let lower = h.name.to_lowercase();
        let messy = scanner_name(&h.name) as i32
            + (lower.contains(" (")
                || lower.contains("copy")
                || lower.contains("kopie")
                || lower.contains("copia")) as i32;
        let local = (h.cloud != CloudState::CloudOnly) as i32;
        (
            details,
            depth,
            -messy,
            local,
            -(h.modified_ms.unwrap_or(i64::MAX) / 1000),
        )
    };
    (0..docs.len())
        .max_by_key(|&i| score(&docs[i]))
        .unwrap_or(0)
}

// MARK: - Clearing up

/// What the copies about to go had that the one kept didn't: their tags,
/// and notes it doesn't already have. `None` when nothing is new.
pub fn merged(keep: &DocMeta, others: &[DocMeta]) -> Option<DocMeta> {
    let mut out = keep.clone();
    for o in others {
        out.tags = meta::unique_tags(out.tags.into_iter().chain(o.tags.iter().cloned()));
        if let Some(n) = o.notes.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
            match &out.notes {
                Some(mine) if mine.contains(n) => {}
                Some(mine) => out.notes = Some(format!("{mine}\n{n}")),
                None => out.notes = Some(n.to_string()),
            }
        }
        if !out.favorite && o.favorite {
            out.favorite = true;
        }
    }
    (out != *keep).then_some(out)
}

/// Gives the copy kept what the others had (`merged`), where it keeps its
/// details: inside a local file, or waiting in the sidecar of an online-only
/// one (nothing is downloaded).
pub fn keep_details(path: &Path, others: &[DocMeta], known: &DocMeta) -> Result<()> {
    let local =
        crate::cloud_state(path)? == CloudState::Local || crate::cloud::streams_on_demand(path);
    if local {
        let mine = meta::read_meta(path)?.meta;
        if let Some(m) = merged(&mine, others) {
            meta::write_meta(path, m, true)?;
        }
    } else if let Some(m) = merged(known, others) {
        // Only what's new goes in the waiting details.
        let waiting = meta::read_meta_beside(path)?
            .map(|(m, _)| m)
            .unwrap_or_default();
        meta::write_meta_beside(
            path,
            DocMeta {
                tags: m.tags,
                notes: m.notes,
                favorite: m.favorite,
                ..waiting
            },
        )?;
    }
    Ok(())
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_kind::FileKind;
    use crate::index::{DocRow, TextState};
    use crate::meta::Storage;

    #[test]
    fn scanner_style_names() {
        for yes in [
            "Scan 2026-11-12 08.31.pdf",
            "IMG_2034.jpg",
            "20251016_134520.pdf",
            "scan.pdf",
            "Document (3).pdf",
            "Escaneo 12.pdf",
            "PXL_20240101_123456789.jpg",
        ] {
            assert!(scanner_name(yes), "{yes}");
        }
        for no in [
            "2024-04-26 Invoice.pdf",
            "Lease 2024.pdf",
            "Scanner manual.pdf",
            "Images of the boat.pdf",
            "2024.pdf",
        ] {
            assert!(!scanner_name(no), "{no}");
        }
    }

    fn row(rel: &str, size: u64, tags: &[&str], cloud: CloudState) -> DocRow {
        DocRow {
            source_id: "s".into(),
            rel_path: rel.into(),
            name: rel.rsplit('/').next().unwrap().into(),
            kind: FileKind::Pdf,
            size,
            modified_ms: Some(1),
            cloud,
            meta: DocMeta {
                tags: tags.iter().map(|t| t.to_string()).collect(),
                ..Default::default()
            },
            storage: Storage::Embedded,
            text_state: TextState::Extracted,
            sidecar_ms: None,
        }
    }

    #[test]
    fn finds_the_same_file_the_same_text_and_probable_copies() {
        let dir = tempfile::tempdir().unwrap();
        let w = |rel: &str, bytes: &[u8]| {
            let p = dir.path().join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, bytes).unwrap();
        };
        w("Taxes/2025/W-2.pdf", b"same bytes here");
        w("Inbox/scan 1.pdf", b"same bytes here");
        w("Other/size15.pdf", b"other bytes 015");
        let idx = Index::open_in_memory().unwrap();
        let text = "Pacific Gas and Electric statement ".repeat(12);
        idx.upsert(
            &row("Taxes/2025/W-2.pdf", 15, &["Taxes"], CloudState::Local),
            Some("w2"),
        )
        .unwrap();
        idx.upsert(
            &row("Inbox/scan 1.pdf", 15, &[], CloudState::Local),
            Some("w2"),
        )
        .unwrap();
        idx.upsert(
            &row("Other/size15.pdf", 15, &[], CloudState::Local),
            Some("x"),
        )
        .unwrap();
        // The same text, different bytes (details written into one).
        idx.upsert(
            &row("Home/bill.pdf", 100, &[], CloudState::Local),
            Some(&text),
        )
        .unwrap();
        idx.upsert(
            &row("Home/bill (2).pdf", 120, &[], CloudState::Local),
            Some(&text),
        )
        .unwrap();
        // Online-only with a same-named, same-sized partner.
        idx.upsert(&row("A/lease.pdf", 500, &[], CloudState::CloudOnly), None)
            .unwrap();
        idx.upsert(&row("B/lease.pdf", 500, &[], CloudState::CloudOnly), None)
            .unwrap();
        let sources = vec![Source {
            id: "s".into(),
            name: "Docs".into(),
            root: dir.path().to_path_buf(),
            provider: crate::source::Provider::Local,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        }];
        let c = candidates(&idx, None).unwrap();
        let groups = find_duplicates(c, &sources, |_, _| {}, || false).unwrap();
        let kinds: Vec<DupKind> = groups.iter().map(|g| g.kind).collect();
        assert_eq!(
            kinds,
            [DupKind::SameFile, DupKind::SameText, DupKind::Probably]
        );
        // Keep the one with tags in the deeper folder.
        let same = &groups[0];
        assert_eq!(same.docs[same.keep].rel_path, "Taxes/2025/W-2.pdf");
        // Keep the cleaner name.
        let text_group = &groups[1];
        assert_eq!(text_group.docs[text_group.keep].rel_path, "Home/bill.pdf");
        assert_eq!(groups[2].docs.len(), 2);
        assert!(groups.iter().all(|g| !g.kept && !g.key.is_empty()));

        // Keep all: the group is marked kept; a further copy brings it back.
        set_kept(dir.path(), &groups[0].key, 2, true).unwrap();
        let again = find_duplicates(
            candidates(&idx, None).unwrap(),
            &sources,
            |_, _| {},
            || false,
        )
        .unwrap();
        assert!(again[0].kept && !again[1].kept);
        set_kept(dir.path(), &groups[0].key, 1, true).unwrap();
        let more = find_duplicates(
            candidates(&idx, None).unwrap(),
            &sources,
            |_, _| {},
            || false,
        )
        .unwrap();
        assert!(!more[0].kept, "two copies now, one kept: back for review");
        reset_kept(dir.path()).unwrap();
        assert!(kept_list(dir.path()).is_empty());
    }

    #[test]
    fn the_copy_kept_gets_the_others_tags_and_notes() {
        let keep = DocMeta {
            tags: vec!["Taxes".into()],
            notes: Some("Paid".into()),
            ..Default::default()
        };
        let other = DocMeta {
            tags: vec!["taxes".into(), "2025".into()],
            notes: Some("Sent to the accountant".into()),
            favorite: true,
            ..Default::default()
        };
        let m = merged(&keep, &[other]).unwrap();
        assert_eq!(m.tags, ["Taxes", "2025"]);
        assert_eq!(m.notes.as_deref(), Some("Paid\nSent to the accountant"));
        assert!(m.favorite);
        assert!(merged(&keep, std::slice::from_ref(&keep)).is_none());
    }
}
