//! Document metadata: what Rupert lets you record about a document, and
//! where it is stored.
//!
//! - PDFs: inside the file (XMP + Info dictionary), see [`crate::pdf_meta`].
//! - Everything else, and PDFs that can't be safely changed (encrypted or
//!   signed): a hidden sidecar file, see [`crate::sidecar`].
//! - Details recorded for an online-only PDF without downloading it wait in
//!   the sidecar ("pending") and move into the PDF on its next save.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::file_kind::FileKind;
use crate::i18n::{tr, trf};
use crate::{pdf_meta, sidecar};

// MARK: - DocMeta

/// The fields Rupert manages for a document.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocMeta {
    /// Stable identity, assigned the first time Rupert saves metadata.
    /// Lets Rupert recognize the document after it is moved or renamed.
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// The date printed on the document (not the file date), `YYYY-MM-DD`.
    #[serde(default)]
    pub document_date: Option<String>,
    #[serde(default)]
    pub favorite: bool,
}

impl DocMeta {
    /// Trims text, drops empty values, de-duplicates tags (ignoring case,
    /// keeping the first spelling and the user's order) and validates the date.
    pub fn normalized(self) -> Result<Self> {
        let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let mut tags: Vec<String> = Vec::new();
        for tag in self.tags {
            let tag = tag.split_whitespace().collect::<Vec<_>>().join(" ");
            if !tag.is_empty() && !tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
                tags.push(tag);
            }
        }
        let document_date = clean(self.document_date);
        if let Some(date) = &document_date
            && !is_iso_date(date)
        {
            return Err(Error::InvalidValue(trf(
                "document date must be YYYY-MM-DD, got \"{0}\"",
                &[date],
            )));
        }
        Ok(Self {
            id: clean(self.id),
            title: clean(self.title),
            tags,
            notes: clean(self.notes),
            document_date,
            favorite: self.favorite,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.tags.is_empty()
            && self.notes.is_none()
            && self.document_date.is_none()
            && !self.favorite
    }
}

/// Field-by-field partial metadata, used while combining sources
/// (XMP first, then the PDF Info dictionary).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PartialMeta {
    pub id: Option<String>,
    pub title: Option<String>,
    pub tags: Option<Vec<String>>,
    pub notes: Option<String>,
    pub document_date: Option<String>,
    pub favorite: Option<bool>,
}

impl PartialMeta {
    /// Fills fields that are missing here from `other`.
    pub fn or(self, other: PartialMeta) -> PartialMeta {
        PartialMeta {
            id: self.id.or(other.id),
            title: self.title.or(other.title),
            tags: self.tags.filter(|t| !t.is_empty()).or(other.tags),
            notes: self.notes.or(other.notes),
            document_date: self.document_date.or(other.document_date),
            favorite: self.favorite.or(other.favorite),
        }
    }

    /// Combines pending sidecar details with what the file holds: fields
    /// set in `pending` win, the rest come from `base`.
    pub fn merge(pending: DocMeta, base: DocMeta) -> DocMeta {
        PartialMeta::from(pending)
            .or(PartialMeta::from(base))
            .into_meta()
    }

    pub fn into_meta(self) -> DocMeta {
        DocMeta {
            id: self.id,
            title: self.title,
            tags: self.tags.unwrap_or_default(),
            notes: self.notes,
            document_date: self.document_date.filter(|d| is_iso_date(d)),
            favorite: self.favorite.unwrap_or(false),
        }
    }
}

impl From<DocMeta> for PartialMeta {
    /// Empty values (no tags, not a favorite) count as "not set".
    fn from(m: DocMeta) -> Self {
        PartialMeta {
            id: m.id,
            title: m.title,
            tags: Some(m.tags).filter(|t| !t.is_empty()),
            notes: m.notes,
            document_date: m.document_date,
            favorite: m.favorite.then_some(true),
        }
    }
}

pub fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
        && (1..=12).contains(&s[5..7].parse::<u8>().unwrap_or(0))
        && (1..=31).contains(&s[8..10].parse::<u8>().unwrap_or(0))
}

/// Splits a keywords string (`"a, b; c"`) into tags.
pub fn split_keywords(s: &str) -> Vec<String> {
    s.split([',', ';'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

// MARK: - Storage

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Storage {
    /// Inside the file itself.
    Embedded,
    /// In `.rupert/<file>.json` next to the file.
    Sidecar,
    /// Nothing recorded yet.
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaRecord {
    pub meta: DocMeta,
    pub storage: Storage,
    /// Why metadata for this file goes to a sidecar even though it's a PDF
    /// (e.g. "the PDF is encrypted"), or why the PDF couldn't be read.
    pub note: Option<String>,
}

// MARK: - Read / write

/// Reads a document's metadata from wherever it is stored. Reads the file,
/// so for online-only files this downloads it; callers decide whether that
/// is acceptable.
pub fn read_meta(path: &Path) -> Result<MetaRecord> {
    let side = sidecar::read(path)?.and_then(|s| s.meta.map(|m| (m, s.pending)));

    if FileKind::from_path(path) != FileKind::Pdf {
        return Ok(match side {
            Some((meta, _)) => record(meta, Storage::Sidecar, None),
            None => record(DocMeta::default(), Storage::None, None),
        });
    }

    match pdf_meta::read(path) {
        Ok(pdf) => {
            let note = pdf.unwritable_reason;
            match side {
                Some((meta, false)) => Ok(record(meta, Storage::Sidecar, note)),
                Some((meta, true)) => {
                    let merged = PartialMeta::merge(meta, pdf.meta.into_meta());
                    let note = note.or_else(|| {
                        Some(
                            tr("Some details are kept beside the file until it's next saved in Rupert")
                                .into(),
                        )
                    });
                    Ok(record(merged, Storage::Sidecar, note))
                }
                None => {
                    let meta = pdf.meta.into_meta();
                    let storage = if meta.is_empty() && meta.id.is_none() {
                        Storage::None
                    } else {
                        Storage::Embedded
                    };
                    Ok(record(meta, storage, note))
                }
            }
        }
        Err(e) => {
            let note = Some(trf(
                "couldn't read the PDF ({0}); details are kept in a sidecar",
                &[&e],
            ));
            Ok(match side {
                Some((meta, _)) => record(meta, Storage::Sidecar, note),
                None => record(DocMeta::default(), Storage::None, note),
            })
        }
    }
}

/// Reads only what can be known without touching the file itself: the
/// sidecar's metadata, if any (and whether it is pending for a PDF).
pub fn read_meta_beside(path: &Path) -> Result<Option<(DocMeta, bool)>> {
    Ok(sidecar::read(path)?.and_then(|s| s.meta.map(|m| (m, s.pending))))
}

/// Saves `meta` for the file at `path`: inside the PDF when possible,
/// otherwise in a sidecar. Assigns an `id` on first save. `file_is_local`
/// controls whether the sidecar records a content hash (hashing an
/// online-only file would download it).
pub fn write_meta(path: &Path, meta: DocMeta, file_is_local: bool) -> Result<MetaRecord> {
    let meta = prepare(meta)?;

    if FileKind::from_path(path) == FileKind::Pdf {
        match pdf_meta::write(path, &meta) {
            Ok(()) => {
                // Drop fallback or pending details that would now shadow the
                // PDF (text in the sidecar stays).
                sidecar::write_meta(path, None, false, file_is_local)?;
                return read_meta(path);
            }
            Err(Error::Unsupported(_) | Error::Pdf(_)) => {
                // Can't change this PDF safely; fall through to a sidecar.
            }
            Err(e) => return Err(e),
        }
    }

    sidecar::write_meta(path, Some(meta), false, file_is_local)?;
    read_meta(path)
}

/// Saves `meta` without reading or changing the file itself (for
/// online-only files). For PDFs the details are marked pending and move
/// into the PDF on its next save; other formats keep them in the sidecar
/// anyway.
pub fn write_meta_beside(path: &Path, meta: DocMeta) -> Result<DocMeta> {
    let meta = prepare(meta)?;
    let pending = FileKind::from_path(path) == FileKind::Pdf;
    sidecar::write_meta(path, Some(meta.clone()), pending, false)?;
    Ok(meta)
}

/// Stars or unstars a document (the star on it in the list), keeping its
/// other details. `indexed` is what the index knows about it.
///
/// Starring an online-only PDF doesn't download it: the star waits beside the
/// file (pending, starting from the indexed details) until its next save.
/// Unstarring one reads the file, which downloads it, because a pending "not a
/// favorite" can't override a star inside the PDF. Other formats keep their
/// details in the sidecar, so they're never downloaded.
pub fn set_favorite(
    path: &Path,
    favorite: bool,
    file_is_local: bool,
    indexed: Option<DocMeta>,
) -> Result<DocMeta> {
    let pdf = FileKind::from_path(path) == FileKind::Pdf;
    if pdf
        && !file_is_local
        && favorite
        && let Some(current) = indexed
    {
        if current.favorite {
            return Ok(current);
        }
        return write_meta_beside(
            path,
            DocMeta {
                favorite,
                ..current
            },
        );
    }
    let current = read_meta(path)?.meta;
    if current.favorite == favorite {
        return Ok(current);
    }
    Ok(write_meta(
        path,
        DocMeta {
            favorite,
            ..current
        },
        file_is_local,
    )?
    .meta)
}

fn prepare(meta: DocMeta) -> Result<DocMeta> {
    let mut meta = meta.normalized()?;
    if meta.id.is_none() {
        meta.id = Some(uuid::Uuid::new_v4().to_string());
    }
    Ok(meta)
}

fn record(meta: DocMeta, storage: Storage, note: Option<String>) -> MetaRecord {
    MetaRecord {
        meta,
        storage,
        note,
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_fields() {
        let meta = DocMeta {
            id: Some("  ".into()),
            title: Some("  PG&E bill ".into()),
            tags: vec![
                " Utilities ".into(),
                "utilities".into(),
                "".into(),
                "Home   Office".into(),
            ],
            notes: Some("\n".into()),
            document_date: Some("2026-11-03".into()),
            favorite: true,
        }
        .normalized()
        .unwrap();
        assert_eq!(meta.id, None);
        assert_eq!(meta.title.as_deref(), Some("PG&E bill"));
        assert_eq!(meta.tags, ["Utilities", "Home Office"]);
        assert_eq!(meta.notes, None);
        assert!(meta.favorite);
    }

    #[test]
    fn rejects_bad_dates() {
        for bad in ["2026-13-01", "2026-1-01", "11/03/2026", "2026-11-00"] {
            let meta = DocMeta {
                document_date: Some(bad.into()),
                ..Default::default()
            };
            assert!(meta.normalized().is_err(), "{bad}");
        }
    }

    #[test]
    fn partial_merge_prefers_first() {
        let xmp = PartialMeta {
            title: Some("From XMP".into()),
            tags: Some(vec![]),
            ..Default::default()
        };
        let info = PartialMeta {
            title: Some("From Info".into()),
            tags: Some(vec!["a".into()]),
            notes: Some("n".into()),
            ..Default::default()
        };
        let merged = xmp.or(info).into_meta();
        assert_eq!(merged.title.as_deref(), Some("From XMP"));
        assert_eq!(merged.tags, ["a"]);
        assert_eq!(merged.notes.as_deref(), Some("n"));
    }

    #[test]
    fn splits_keywords() {
        assert_eq!(split_keywords("a, b;c ,, "), ["a", "b", "c"]);
    }

    #[test]
    fn non_pdf_round_trip_uses_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("lease.docx");
        std::fs::write(&file, b"doc").unwrap();

        assert_eq!(read_meta(&file).unwrap().storage, Storage::None);
        let saved = write_meta(
            &file,
            DocMeta {
                title: Some("Lease".into()),
                tags: vec!["Home".into()],
                ..Default::default()
            },
            true,
        )
        .unwrap();
        assert_eq!(saved.storage, Storage::Sidecar);
        assert_eq!(saved.meta.title.as_deref(), Some("Lease"));
        assert!(saved.meta.id.is_some());
        assert_eq!(std::fs::read(&file).unwrap(), b"doc", "file untouched");
    }

    #[test]
    fn star_goes_into_a_local_pdf_and_keeps_its_details() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("deed.pdf");
        crate::pdf_meta::tests::make_pdf(&pdf, None);
        let titled = DocMeta {
            title: Some("Deed".into()),
            ..Default::default()
        };
        let id = write_meta(&pdf, titled, true).unwrap().meta.id;

        let starred = set_favorite(&pdf, true, true, None).unwrap();
        assert!(starred.favorite);
        let record = read_meta(&pdf).unwrap();
        assert_eq!(record.storage, Storage::Embedded);
        assert!(record.meta.favorite);
        assert_eq!(record.meta.title.as_deref(), Some("Deed"));
        assert_eq!(record.meta.id, id);

        assert!(!set_favorite(&pdf, false, true, None).unwrap().favorite);
        assert!(!read_meta(&pdf).unwrap().meta.favorite);
    }

    #[test]
    fn starring_an_online_only_pdf_waits_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("deed.pdf");
        crate::pdf_meta::tests::make_pdf(&pdf, None);
        let before = std::fs::read(&pdf).unwrap();
        let indexed = DocMeta {
            id: Some("doc-1".into()),
            title: Some("Deed".into()),
            ..Default::default()
        };

        set_favorite(&pdf, true, false, Some(indexed)).unwrap();
        assert_eq!(std::fs::read(&pdf).unwrap(), before, "PDF untouched");
        let (side, pending) = read_meta_beside(&pdf).unwrap().unwrap();
        assert!(pending);
        assert!(side.favorite);
        assert_eq!(side.id.as_deref(), Some("doc-1"));
        assert_eq!(side.title.as_deref(), Some("Deed"));
        // Once it's local, the star is part of what it says.
        assert!(read_meta(&pdf).unwrap().meta.favorite);
    }

    #[test]
    fn stars_other_formats_in_the_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("lease.docx");
        std::fs::write(&file, b"doc").unwrap();
        assert!(set_favorite(&file, true, false, None).unwrap().favorite);
        assert!(read_meta(&file).unwrap().meta.favorite);
        assert!(!set_favorite(&file, false, false, None).unwrap().favorite);
        assert!(!read_meta(&file).unwrap().meta.favorite);
        assert_eq!(std::fs::read(&file).unwrap(), b"doc", "file untouched");
    }
}
