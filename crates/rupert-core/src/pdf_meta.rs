//! Reading and writing Rupert's metadata inside PDFs.
//!
//! Writes are **incremental updates** (ISO 32000 §7.5.6): the original bytes
//! are never rewritten; a new XMP stream, Info dictionary, cross-reference
//! section and trailer are appended. The file is changed in place (same
//! file, same extended attributes), and the append is undone if anything
//! goes wrong.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use lopdf::{Dictionary, Document, IncrementalDocument, Object, ObjectId, Stream, text_string};

use crate::error::{Error, Result};
use crate::i18n::tr;
use crate::meta::{DocMeta, PartialMeta, split_keywords};
use crate::xmp;

// MARK: - Clean copies

/// Rewrites the PDF at `path` whole, keeping only the objects in use, so
/// nothing of its earlier revisions is left (incremental updates keep the
/// old details, e.g. notes since removed). For copies that are sent away;
/// never for the user's own files.
pub fn rewrite_whole(path: &Path) -> Result<()> {
    let mut doc = Document::load(path).map_err(pdf_err)?;
    if let Some(why) = unwritable(&doc) {
        return Err(Error::Unsupported(why.sidecar_reason()));
    }
    doc.prune_objects();
    doc.save(path).map_err(pdf_err)?;
    Ok(())
}

// MARK: - Reading

#[derive(Debug)]
pub struct PdfRead {
    /// XMP values, with gaps filled from the Info dictionary.
    pub meta: PartialMeta,
    /// `Some(reason)` if Rupert won't write into this PDF.
    pub unwritable_reason: Option<String>,
}

pub fn read(path: &Path) -> Result<PdfRead> {
    let doc = Document::load(path).map_err(pdf_err)?;
    Ok(read_doc(&doc))
}

fn read_doc(doc: &Document) -> PdfRead {
    let from_xmp = existing_xmp(doc)
        .map(|(_, xml)| xmp::read(&xml))
        .unwrap_or_default();
    let from_info = info_dict(doc).map(read_info).unwrap_or_default();
    PdfRead {
        meta: from_xmp.or(from_info),
        unwritable_reason: unwritable(doc).map(Unwritable::sidecar_reason),
    }
}

fn read_info(info: &Dictionary) -> PartialMeta {
    let text = |key: &[u8]| {
        info.get(key)
            .ok()
            .and_then(|o| lopdf::decode_text_string(o).ok())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    };
    PartialMeta {
        title: text(b"Title"),
        tags: text(b"Keywords").map(|k| split_keywords(&k)),
        notes: text(b"Subject"),
        ..Default::default()
    }
}

fn info_dict(doc: &Document) -> Option<&Dictionary> {
    match doc.trailer.get(b"Info").ok()? {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(d) => Some(d),
        _ => None,
    }
}

/// The catalog's XMP stream: its object id (if indirect) and its text.
fn existing_xmp(doc: &Document) -> Option<(Option<ObjectId>, String)> {
    let catalog = doc.catalog().ok()?;
    let (id, stream) = match catalog.get(b"Metadata").ok()? {
        Object::Reference(id) => (Some(*id), doc.get_object(*id).ok()?.as_stream().ok()?),
        Object::Stream(s) => (None, s),
        _ => return None,
    };
    let bytes = stream
        .decompressed_content()
        .unwrap_or_else(|_| stream.content.clone());
    Some((id, String::from_utf8_lossy(&bytes).into_owned()))
}

/// Why Rupert won't change a PDF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unwritable {
    Encrypted,
    SignaturePermissions,
    Signed,
}

impl Unwritable {
    /// Why its details are kept in a sidecar.
    pub(crate) fn sidecar_reason(self) -> String {
        match self {
            Unwritable::Encrypted => {
                tr("the PDF is encrypted, so details are kept in a sidecar file")
            }
            Unwritable::SignaturePermissions => {
                tr("the PDF has signature permissions, so details are kept in a sidecar file")
            }
            Unwritable::Signed => {
                tr("the PDF is digitally signed, so details are kept in a sidecar file")
            }
        }
        .into()
    }

    /// Why it gets no text layer.
    pub(crate) fn text_layer_reason(self) -> String {
        match self {
            Unwritable::Encrypted => {
                tr("the PDF is encrypted, so Rupert won't add a text layer to it")
            }
            Unwritable::SignaturePermissions => {
                tr("the PDF has signature permissions, so Rupert won't add a text layer to it")
            }
            Unwritable::Signed => {
                tr("the PDF is digitally signed, so Rupert won't add a text layer to it")
            }
        }
        .into()
    }
}

/// Encrypted or signed PDFs are left untouched; their metadata goes to a
/// sidecar instead.
pub(crate) fn unwritable(doc: &Document) -> Option<Unwritable> {
    if doc.trailer.has(b"Encrypt") || doc.encryption_state.is_some() {
        return Some(Unwritable::Encrypted);
    }
    let catalog = doc.catalog().ok()?;
    if catalog.has(b"Perms") {
        return Some(Unwritable::SignaturePermissions);
    }
    let acroform = match catalog.get(b"AcroForm").ok()? {
        Object::Reference(id) => doc.get_dictionary(*id).ok(),
        Object::Dictionary(d) => Some(d),
        _ => None,
    };
    let sig_flags = acroform
        .and_then(|f| f.get(b"SigFlags").ok())
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);
    (sig_flags != 0).then_some(Unwritable::Signed)
}

// MARK: - Writing

/// Appends an incremental update carrying `meta` to the PDF at `path`.
///
/// Returns [`Error::Unsupported`] for PDFs Rupert won't modify (encrypted,
/// signed) and [`Error::Pdf`] for PDFs it can't parse; the file is unchanged
/// in both cases.
pub fn write(path: &Path, meta: &DocMeta) -> Result<()> {
    let before = fs::metadata(path)?;
    let mut inc = IncrementalDocument::load(path).map_err(pdf_err)?;
    let prev = inc.get_prev_documents();
    if let Some(why) = unwritable(prev) {
        return Err(Error::Unsupported(why.sidecar_reason()));
    }

    let catalog_id = prev
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .map_err(pdf_err)?;
    let (xmp_id, old_xmp) = match existing_xmp(prev) {
        Some((id, xml)) => (id, Some(xml)),
        None => (None, None),
    };
    let info_id = prev
        .trailer
        .get(b"Info")
        .and_then(Object::as_reference)
        .ok();
    let mut info = info_dict(prev).cloned().unwrap_or_default();

    // XMP: replace the existing stream object, or add one and point the catalog at it.
    let xml = xmp::merge(old_xmp.as_deref(), meta, &utc_now());
    let mut stream = Stream::new(
        Dictionary::from_iter([
            ("Type", Object::Name(b"Metadata".to_vec())),
            ("Subtype", Object::Name(b"XML".to_vec())),
        ]),
        xml.into_bytes(),
    );
    stream.allows_compression = false; // XMP must stay readable by scanners
    match xmp_id {
        Some(id) => inc.new_document.set_object(id, stream),
        None => {
            let id = inc.new_document.add_object(stream);
            inc.opt_clone_object_to_new_document(catalog_id)
                .map_err(pdf_err)?;
            inc.new_document
                .get_object_mut(catalog_id)
                .and_then(Object::as_dict_mut)
                .map_err(pdf_err)?
                .set("Metadata", Object::Reference(id));
        }
    }

    // Info dictionary: the same fields, for software that ignores XMP.
    set_or_remove(&mut info, "Title", meta.title.as_deref());
    let keywords = (!meta.tags.is_empty()).then(|| meta.tags.join(", "));
    set_or_remove(&mut info, "Keywords", keywords.as_deref());
    set_or_remove(&mut info, "Subject", meta.notes.as_deref());
    let info_id = match info_id {
        Some(id) => {
            inc.new_document.set_object(id, info);
            id
        }
        None => inc.new_document.add_object(info),
    };

    // Trailer for the update: only the keys that belong in it.
    let prev_trailer = inc.get_prev_documents().trailer.clone();
    let mut trailer = Dictionary::new();
    for key in [&b"Root"[..], b"ID", b"Prev"] {
        if let Ok(value) = inc
            .new_document
            .trailer
            .get(key)
            .or_else(|_| prev_trailer.get(key))
        {
            trailer.set(key.to_vec(), value.clone());
        }
    }
    trailer.set("Info", Object::Reference(info_id));
    inc.new_document.trailer = trailer;

    // Build the whole updated file in memory and check it before touching disk.
    let mut updated = Vec::new();
    inc.save_to(&mut updated).map_err(pdf_err)?;
    let prev_len = inc.get_prev_documents_bytes().len();
    verify(&updated, meta)?;

    // Don't append if someone (e.g. a sync client) changed the file meanwhile.
    let now = fs::metadata(path)?;
    if now.len() != before.len() || now.modified().ok() != before.modified().ok() {
        return Err(Error::InvalidValue(
            tr("the file changed while saving; please try again").into(),
        ));
    }

    append(path, prev_len as u64, &updated[prev_len..])
}

fn set_or_remove(dict: &mut Dictionary, key: &str, value: Option<&str>) {
    match value {
        Some(v) => dict.set(key, text_string(v)),
        None => {
            dict.remove(key.as_bytes());
        }
    }
}

/// Parses the updated file and confirms it reads back as `meta`.
fn verify(updated: &[u8], meta: &DocMeta) -> Result<()> {
    let doc = Document::load_mem(updated)
        .map_err(|e| Error::Pdf(format!("updated PDF didn't parse: {e}")))?;
    let read_back = read_doc(&doc).meta.into_meta();
    if &read_back != meta {
        return Err(Error::Pdf(format!(
            "updated PDF read back differently: {read_back:?}"
        )));
    }
    Ok(())
}

/// Appends `tail` to the file, restoring the original length on failure.
pub(crate) fn append(path: &Path, original_len: u64, tail: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().append(true).open(path)?;
    let result = file.write_all(tail).and_then(|_| file.sync_all());
    if let Err(e) = result {
        let _ = file.set_len(original_len);
        return Err(e.into());
    }
    Ok(())
}

fn pdf_err(e: impl std::fmt::Display) -> Error {
    Error::Pdf(e.to_string())
}

// MARK: - Time

/// Current UTC time as `YYYY-MM-DDTHH:MM:SSZ` (XMP date format).
fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs)
}

/// Today's date (UTC) as `YYYY-MM-DD`.
pub fn utc_today() -> String {
    utc_now()[..10].to_string()
}

pub(crate) fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

// MARK: - Tests

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::dictionary;

    /// A minimal one-page PDF, optionally with an Info dictionary.
    pub(crate) fn make_pdf(path: &Path, info: Option<Dictionary>) {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = Content {
            operations: vec![Operation::new("BT", vec![]), Operation::new("ET", vec![])],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        if let Some(info) = info {
            let info_id = doc.add_object(info);
            doc.trailer.set("Info", info_id);
        }
        doc.save(path).unwrap();
    }

    fn sample() -> DocMeta {
        DocMeta {
            id: Some("id-1".into()),
            title: Some("Électricité – facture".into()),
            tags: vec!["Utilities".into(), "Home".into()],
            notes: Some("Paid".into()),
            document_date: Some("2026-10-31".into()),
            favorite: true,
            sender: Some("Électricité de France".into()),
            amount: Some("84.20".into()),
            currency: Some("EUR".into()),
            due_date: Some("2026-11-15".into()),
            remind: Some("1w".into()),
            done: None,
            snoozed: None,
        }
    }

    #[test]
    fn reads_info_dictionary_written_by_other_apps() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.pdf");
        make_pdf(
            &path,
            Some(
                dictionary! { "Title" => text_string("Lease"), "Keywords" => text_string("home; legal") },
            ),
        );
        let meta = read(&path).unwrap().meta.into_meta();
        assert_eq!(meta.title.as_deref(), Some("Lease"));
        assert_eq!(meta.tags, ["home", "legal"]);
    }

    #[test]
    fn incremental_write_round_trips_and_keeps_original_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("scan.pdf");
        make_pdf(
            &path,
            Some(dictionary! { "Producer" => text_string("ScanSnap") }),
        );
        let original = fs::read(&path).unwrap();

        write(&path, &sample()).unwrap();
        let updated = fs::read(&path).unwrap();
        assert!(updated.starts_with(&original), "original bytes preserved");
        assert!(updated.len() > original.len());

        let doc = Document::load(&path).unwrap();
        assert_eq!(read_doc(&doc).meta.into_meta(), sample());
        let info = info_dict(&doc).unwrap();
        assert_eq!(
            lopdf::decode_text_string(info.get(b"Producer").unwrap()).unwrap(),
            "ScanSnap",
            "other Info entries kept"
        );

        // A second edit appends another update and reads back the new values.
        let mut edited = sample();
        edited.title = None;
        edited.tags = vec!["Taxes".into()];
        write(&path, &edited).unwrap();
        assert_eq!(read(&path).unwrap().meta.into_meta(), edited);
    }

    #[test]
    fn writes_pdf_without_info_dictionary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bare.pdf");
        make_pdf(&path, None);
        write(&path, &sample()).unwrap();
        assert_eq!(read(&path).unwrap().meta.into_meta(), sample());
    }

    #[test]
    fn refuses_signed_pdfs() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("signed.pdf");
        make_pdf(&path, None);
        // Mark the catalog as having signatures.
        let mut doc = Document::load(&path).unwrap();
        let root = doc.trailer.get(b"Root").unwrap().as_reference().unwrap();
        doc.get_object_mut(root)
            .unwrap()
            .as_dict_mut()
            .unwrap()
            .set(
                "AcroForm",
                dictionary! { "SigFlags" => 3, "Fields" => vec![] },
            );
        doc.save(&path).unwrap();
        let before = fs::read(&path).unwrap();

        assert!(matches!(
            write(&path, &sample()),
            Err(Error::Unsupported(_))
        ));
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(read(&path).unwrap().unwritable_reason.is_some());
    }

    #[test]
    fn formats_utc_dates() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_795_000_000), "2026-11-18T11:06:40Z");
        assert_eq!(format_utc(951_782_400), "2000-02-29T00:00:00Z");
    }
}
