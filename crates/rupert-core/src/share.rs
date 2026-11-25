//! Sending documents (#69): the file the system's share menu or a new
//! email gets. Usually the document itself; a copy where it has to differ:
//! - an inbox scan goes under its new name ("2026-11-18 Ridgeview Tire &
//!   Auto receipt.pdf", not "Scan 2026-11-18 08.31.pdf");
//! - a PDF goes without the notes Rupert keeps inside it, unless the user
//!   includes them (Settings ▸ Library). The copy is rewritten whole, since
//!   earlier revisions of an updated PDF still hold the old details.
//!
//! Copies wait in the app's cache (`sending/`) and are removed a day later.
//! Making one reads the file, so an online-only file is downloaded, as
//! when it's opened: the user asked to send it.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{Error, Result};
use crate::file_kind::FileKind;
use crate::fileops;
use crate::i18n::tr;
use crate::listing::{CloudState, cloud_state};
use crate::pdf_meta;
use crate::rename::split_name;

/// The folder for copies, in the app's cache.
const DIR: &str = "sending";
/// How long copies are kept (the app that got them may still read them).
const KEEP: Duration = Duration::from_secs(24 * 60 * 60);

// MARK: - Preparing

/// The file to send for the document at `path`. `name`: the name (without
/// extension) to send it under, if not its own. `include_notes`: send a
/// PDF's notes along. `cache`: the app's cache folder.
pub fn prepare(
    path: &Path,
    name: Option<&str>,
    include_notes: bool,
    cache: &Path,
) -> Result<PathBuf> {
    let dir = cache.join(DIR);
    clean_old(&dir, KEEP);
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::InvalidPath(path.display().to_string()))?;
    let (_, ext) = split_name(file_name);
    let send_as = match name.map(|n| fileops::clean_name(n, ext.trim_start_matches('.'))) {
        Some(n) if !n.is_empty() => format!("{n}{ext}"),
        _ => file_name.to_string(),
    };
    let pdf = FileKind::from_path(path) == FileKind::Pdf;
    let local = cloud_state(path)? == CloudState::Local;
    // A local file that can go as it is (an online-only PDF is only known
    // once it's here).
    let clean_needed = pdf && !include_notes && (!local || needs_cleaning(path));
    if send_as == file_name && !clean_needed {
        return Ok(path.to_path_buf());
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let folder = dir.join(format!("{nanos:x}"));
    fs::create_dir_all(&folder)?;
    let copy = folder.join(&send_as);
    fs::copy(path, &copy)?;
    if clean_needed && needs_cleaning(&copy) {
        leave_out_notes(&copy)?;
    }
    Ok(copy)
}

/// Whether a PDF holds notes, or earlier revisions that might.
fn needs_cleaning(path: &Path) -> bool {
    let notes = pdf_meta::read(path)
        .map(|r| r.meta.notes.is_some())
        .unwrap_or(false);
    notes || revisions(path).unwrap_or(1) > 1
}

/// How many times the PDF was saved: each incremental update ends with
/// another `%%EOF`.
fn revisions(path: &Path) -> Result<usize> {
    let bytes = fs::read(path)?;
    Ok(bytes.windows(5).filter(|w| w == b"%%EOF").count())
}

/// Takes the notes out of a PDF copy and rewrites it whole. When that can't
/// be done (an encrypted or signed PDF) and it does hold notes, it isn't
/// sent: the user didn't want the notes to go along.
fn leave_out_notes(copy: &Path) -> Result<()> {
    let read = pdf_meta::read(copy)?;
    let had_notes = read.meta.notes.is_some();
    let result = (|| {
        if had_notes {
            let mut meta = read.meta.into_meta();
            meta.notes = None;
            pdf_meta::write(copy, &meta)?;
        }
        pdf_meta::rewrite_whole(copy)
    })();
    match result {
        Ok(()) => Ok(()),
        Err(_) if !had_notes => Ok(()),
        Err(_) => Err(Error::Unsupported(
            tr("Rupert can't take the notes out of this PDF. To send it anyway, include notes in Settings ▸ Library.").into(),
        )),
    }
}

/// Removes copies older than `keep`.
fn clean_old(dir: &Path, keep: Duration) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let now = SystemTime::now();
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .is_ok_and(|t| now.duration_since(t).unwrap_or_default() > keep);
        if old {
            let _ = fs::remove_dir_all(e.path());
        }
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::DocMeta;

    fn noted() -> DocMeta {
        DocMeta {
            title: Some("Ridgeview Auto invoice".into()),
            tags: vec!["Car".into()],
            notes: Some("Paid by card, warranty until 2027".into()),
            document_date: Some("2026-03-03".into()),
            ..Default::default()
        }
    }

    #[test]
    fn sends_the_document_itself_when_nothing_differs() {
        let dir = tempfile::tempdir().unwrap();
        let (cache, doc) = (dir.path().join("cache"), dir.path().join("Plain.pdf"));
        pdf_meta::tests::make_pdf(&doc, None);
        assert_eq!(prepare(&doc, None, false, &cache).unwrap(), doc);
        // Other kinds: never copied for notes (theirs stay in the sidecar).
        let txt = dir.path().join("note.txt");
        fs::write(&txt, "hello").unwrap();
        assert_eq!(prepare(&txt, None, false, &cache).unwrap(), txt);
    }

    #[test]
    fn leaves_the_notes_out_of_a_copy_even_from_earlier_revisions() {
        let dir = tempfile::tempdir().unwrap();
        let (cache, doc) = (dir.path().join("cache"), dir.path().join("Invoice.pdf"));
        pdf_meta::tests::make_pdf(&doc, None);
        pdf_meta::write(&doc, &noted()).unwrap();
        let sent = prepare(&doc, None, false, &cache).unwrap();
        assert_ne!(sent, doc);
        assert_eq!(sent.file_name().unwrap(), "Invoice.pdf");
        let meta = pdf_meta::read(&sent).unwrap().meta;
        assert_eq!(meta.notes, None);
        assert_eq!(meta.title.as_deref(), Some("Ridgeview Auto invoice"));
        // Not anywhere in the bytes either (no older revision left).
        let bytes = fs::read(&sent).unwrap();
        assert!(
            !bytes.windows(7).any(|w| w == b"Paid by"),
            "old notes left in the copy"
        );
        assert_eq!(revisions(&sent).unwrap(), 1);
        // The original keeps its notes.
        assert!(pdf_meta::read(&doc).unwrap().meta.notes.is_some());

        // Notes removed earlier still sit in an older revision: cleaned too.
        let mut no_notes = noted();
        no_notes.notes = None;
        pdf_meta::write(&doc, &no_notes).unwrap();
        let sent = prepare(&doc, None, false, &cache).unwrap();
        assert_ne!(sent, doc);
        assert!(!fs::read(&sent).unwrap().windows(7).any(|w| w == b"Paid by"));

        // Included on request: the document itself.
        assert_eq!(prepare(&doc, None, true, &cache).unwrap(), doc);
    }

    #[test]
    fn sends_an_inbox_scan_under_its_new_name() {
        let dir = tempfile::tempdir().unwrap();
        let (cache, doc) = (
            dir.path().join("cache"),
            dir.path().join("Scan 2026-11-18 08.31.pdf"),
        );
        pdf_meta::tests::make_pdf(&doc, None);
        let sent = prepare(
            &doc,
            Some("2026-11-18 Ridgeview Tire: receipt"),
            true,
            &cache,
        )
        .unwrap();
        assert_eq!(
            sent.file_name().unwrap(),
            "2026-11-18 Ridgeview Tire receipt.pdf"
        );
        assert!(doc.exists());
    }

    #[test]
    fn removes_old_copies() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("abc");
        fs::create_dir_all(&old).unwrap();
        clean_old(dir.path(), Duration::from_secs(3600));
        assert!(old.exists());
        clean_old(dir.path(), Duration::ZERO);
        std::thread::sleep(Duration::from_millis(10));
        clean_old(dir.path(), Duration::ZERO);
        assert!(!old.exists());
    }
}
