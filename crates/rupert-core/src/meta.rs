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
    /// Who sent or issued it (#108): "Allianz", "City Electric".
    #[serde(default)]
    pub sender: Option<String>,
    /// The amount on it (#108), as a plain decimal with a dot ("1240.00",
    /// "-12.5"), whatever the document's own format was.
    #[serde(default)]
    pub amount: Option<String>,
    /// The amount's currency (#108), an ISO 4217 code ("EUR", "USD").
    #[serde(default)]
    pub currency: Option<String>,
    /// The date to pay by, or valid until (#108), `YYYY-MM-DD`.
    #[serde(default)]
    pub due_date: Option<String>,
}

impl DocMeta {
    /// Trims text, drops empty values, de-duplicates tags (ignoring case,
    /// keeping the first spelling and the user's order) and validates the date.
    pub fn normalized(self) -> Result<Self> {
        let clean = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let tags = unique_tags(self.tags);
        let document_date = clean(self.document_date);
        if let Some(date) = &document_date
            && !is_iso_date(date)
        {
            return Err(Error::InvalidValue(trf(
                "document date must be YYYY-MM-DD, got \"{0}\"",
                &[date],
            )));
        }
        let due_date = clean(self.due_date);
        if let Some(date) = &due_date
            && !is_iso_date(date)
        {
            return Err(Error::InvalidValue(trf(
                "due date must be YYYY-MM-DD, got \"{0}\"",
                &[date],
            )));
        }
        let amount = match clean(self.amount) {
            Some(a) => Some(
                parse_amount(&a)
                    .ok_or_else(|| Error::InvalidValue(trf("\"{0}\" isn't an amount", &[&a])))?,
            ),
            None => None,
        };
        let currency = match clean(self.currency) {
            Some(c) => Some(parse_currency(&c).ok_or_else(|| {
                Error::InvalidValue(trf("\"{0}\" isn't a currency code like EUR or USD", &[&c]))
            })?),
            None => None,
        };
        Ok(Self {
            id: clean(self.id),
            title: clean(self.title),
            tags,
            notes: clean(self.notes),
            document_date,
            favorite: self.favorite,
            sender: clean(self.sender).map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ")),
            amount,
            currency,
            due_date,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.title.is_none()
            && self.tags.is_empty()
            && self.notes.is_none()
            && self.document_date.is_none()
            && !self.favorite
            && self.sender.is_none()
            && self.amount.is_none()
            && self.currency.is_none()
            && self.due_date.is_none()
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
    pub sender: Option<String>,
    pub amount: Option<String>,
    pub currency: Option<String>,
    pub due_date: Option<String>,
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
            sender: self.sender.or(other.sender),
            amount: self.amount.or(other.amount),
            currency: self.currency.or(other.currency),
            due_date: self.due_date.or(other.due_date),
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
            // Files written by other apps can hold a tag twice; lists key
            // tags by name, so they're made unique here.
            tags: unique_tags(self.tags.unwrap_or_default()),
            notes: self.notes,
            document_date: self.document_date.filter(|d| is_iso_date(d)),
            favorite: self.favorite.unwrap_or(false),
            sender: self.sender,
            amount: self.amount.and_then(|a| parse_amount(&a)),
            currency: self.currency.and_then(|c| parse_currency(&c)),
            due_date: self.due_date.filter(|d| is_iso_date(d)),
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
            sender: m.sender,
            amount: m.amount,
            currency: m.currency,
            due_date: m.due_date,
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

/// An amount as Rupert keeps it: digits with a dot before the decimals
/// ("1240.00", "-12.5"), from what people and documents write ("1,240.00",
/// "1.240,00", "1 240,5", "$12.50"). A separator followed by one or two
/// digits at the end is the decimal one; the others group thousands.
pub fn parse_amount(s: &str) -> Option<String> {
    let s = s.trim();
    let negative =
        s.starts_with('-') || s.starts_with('−') || (s.starts_with('(') && s.ends_with(')'));
    // Around the number only a currency sign or code may stand ("$12",
    // "12,50 EUR"); "12 apples" isn't an amount.
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let end = s.rfind(|c: char| c.is_ascii_digit())? + 1;
    for outer in [&s[..start], &s[end..]] {
        let letters = outer.chars().filter(|c| c.is_alphabetic()).count();
        let allowed = outer
            .chars()
            .all(|c| c.is_alphabetic() || c.is_whitespace() || "€$£¥₹-−().+".contains(c));
        if !allowed || letters > 3 {
            return None;
        }
    }
    // Inside the number only separators; anything else isn't an amount.
    let body: String = s[start..end]
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '\u{202f}' && *c != '\'')
        .collect();
    if body.is_empty()
        || !body
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    {
        return None;
    }
    let (int, dec) = match body.rfind(['.', ',']) {
        Some(i) if (1..=2).contains(&(body.len() - i - 1)) => (&body[..i], &body[i + 1..]),
        _ => (body.as_str(), ""),
    };
    let int: String = int.chars().filter(char::is_ascii_digit).collect();
    let int = int.trim_start_matches('0');
    let int = if int.is_empty() { "0" } else { int };
    if int.len() > 15 {
        return None;
    }
    let sign = if negative { "-" } else { "" };
    Some(if dec.is_empty() {
        format!("{sign}{int}")
    } else {
        format!("{sign}{int}.{dec}")
    })
}

/// A currency code: three letters, upper case ("eur" → "EUR"); the common
/// signs too ("€" → "EUR", "$" → "USD", "£" → "GBP").
pub fn parse_currency(s: &str) -> Option<String> {
    let s = s.trim();
    let code = match s {
        "€" => "EUR".to_string(),
        "$" | "US$" => "USD".to_string(),
        "£" => "GBP".to_string(),
        "CHF" | "Fr." | "SFr." => "CHF".to_string(),
        _ => s.to_ascii_uppercase(),
    };
    (code.len() == 3 && code.chars().all(|c| c.is_ascii_uppercase())).then_some(code)
}

/// Tags trimmed (inner spaces collapsed), without empty ones, and each only
/// once, ignoring case: the first spelling and the order are kept.
pub fn unique_tags(tags: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.split_whitespace().collect::<Vec<_>>().join(" ");
        if !tag.is_empty() && !out.iter().any(|t| t.to_lowercase() == tag.to_lowercase()) {
            out.push(tag);
        }
    }
    out
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
    #[test]
    fn amounts_and_currencies_read_as_people_write_them() {
        for (text, want) in [
            ("1,240.00", Some("1240.00")),
            ("1.240,00", Some("1240.00")),
            ("1 240,5", Some("1240.5")),
            ("$12.50", Some("12.50")),
            ("12,50 €", Some("12.50")),
            ("-7.25", Some("-7.25")),
            ("(15.00)", Some("-15.00")),
            ("1.240", Some("1240")),
            ("0042", Some("42")),
            ("12 apples", None),
            ("", None),
            ("abc", None),
        ] {
            assert_eq!(parse_amount(text).as_deref(), want, "{text}");
        }
        assert_eq!(parse_currency("eur").as_deref(), Some("EUR"));
        assert_eq!(parse_currency("€").as_deref(), Some("EUR"));
        assert_eq!(parse_currency("Euro"), None);
    }

    #[test]
    fn tags_read_from_a_file_come_once() {
        // Another app wrote "Steuer" twice (and once in other case).
        let read = PartialMeta {
            tags: Some(vec![
                "Steuer".into(),
                " Bank ".into(),
                "Steuer".into(),
                "STEUER".into(),
                "".into(),
            ]),
            ..Default::default()
        }
        .into_meta();
        assert_eq!(read.tags, ["Steuer", "Bank"]);
        assert_eq!(
            unique_tags(["Ärzte".to_string(), "ärzte".to_string()]),
            ["Ärzte"]
        );
    }

    #[test]
    fn new_details_are_validated() {
        let m = DocMeta {
            sender: Some("  City   Electric ".into()),
            amount: Some("1.234,56".into()),
            currency: Some("eur".into()),
            due_date: Some("2026-12-01".into()),
            ..Default::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(m.sender.as_deref(), Some("City Electric"));
        assert_eq!(m.amount.as_deref(), Some("1234.56"));
        assert_eq!(m.currency.as_deref(), Some("EUR"));
        assert!(!m.is_empty());
        for bad in [
            DocMeta {
                due_date: Some("12/01/2026".into()),
                ..Default::default()
            },
            DocMeta {
                amount: Some("lots".into()),
                ..Default::default()
            },
            DocMeta {
                currency: Some("Euro".into()),
                ..Default::default()
            },
        ] {
            assert!(bad.normalized().is_err());
        }
    }

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
            ..Default::default()
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
