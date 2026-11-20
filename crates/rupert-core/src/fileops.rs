//! File operations Rupert performs on the user's behalf: renaming, moving
//! and trashing documents. They change the real files right away, take the
//! document's sidecar along, and never overwrite anything.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::catalog;
use crate::error::{Error, Result};
use crate::listing::{CloudState, cloud_state, list_dir};
use crate::sidecar;
use crate::source::{Provider, Source};

/// Longest file name Rupert creates (bytes, before the extension); well
/// under every platform's limit, and short enough for synced paths.
const MAX_NAME_BYTES: usize = 150;

/// Moves and/or renames the document at `from` to `dest_dir/name`, keeping
/// its extension. If that name is taken, " (2)", " (3)", … is added. The
/// sidecar (if any) moves too. Returns the new path.
pub fn move_document(from: &Path, dest_dir: &Path, name: &str) -> Result<PathBuf> {
    if !from.is_file() {
        return Err(Error::InvalidPath(from.display().to_string()));
    }
    if !dest_dir.is_dir() {
        return Err(Error::NotADirectory(dest_dir.to_path_buf()));
    }
    let ext = from
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    let stem = clean_name(name, &ext);
    if stem.is_empty() {
        return Err(Error::InvalidValue("the file name is empty".into()));
    }

    let target = free_name(from, dest_dir, &stem, &ext);
    if target == from {
        return Ok(target); // nothing to do
    }
    move_with_sidecar(from, &target, true)?;
    Ok(target)
}

/// `dest_dir/stem.ext`, or with " (2)", " (3)", … added if that's taken by
/// something other than `from` itself. `from` if it already is that file.
fn free_name(from: &Path, dest_dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let with_ext = |s: &str| {
        if ext.is_empty() {
            s.to_string()
        } else {
            format!("{s}.{ext}")
        }
    };
    let mut target = dest_dir.join(with_ext(stem));
    let mut n = 2;
    while fs::symlink_metadata(&target).is_ok() {
        if same_file(from, &target) {
            return from.to_path_buf();
        }
        target = dest_dir.join(with_ext(&format!("{stem} ({n})")));
        n += 1;
    }
    target
}

/// Moves a document to `target` and its sidecar (if any) along with it.
/// `copy_across_disks`: copy and delete when a rename can't cross volumes
/// (never for online-only files: copying reads them).
/// If the sidecar can't follow, the document is moved back, so the two are
/// never separated.
fn move_with_sidecar(from: &Path, target: &Path, copy_across_disks: bool) -> Result<()> {
    let old_side = sidecar::path_for(from)?;
    let new_side = sidecar::path_for(target)?;
    move_file(from, target, copy_across_disks)?;

    // The sidecar follows its document.
    if old_side.is_file() {
        let side_dir = new_side.parent().expect("sidecar has a parent");
        let made_dir = !side_dir.exists();
        let moved = (|| {
            if made_dir {
                fs::create_dir(side_dir)?;
                sidecar::hide(side_dir);
            }
            move_file(&old_side, &new_side, true)
        })();
        if let Err(e) = moved {
            if made_dir {
                let _ = fs::remove_dir(side_dir);
            }
            return match move_file(target, from, copy_across_disks) {
                Ok(()) => Err(Error::InvalidValue(format!(
                    "its details file couldn't be moved ({e}), so it was left where it was"
                ))),
                Err(back) => Err(Error::InvalidValue(format!(
                    "its details file couldn't be moved ({e}), and moving the document back \
                     failed too ({back}): it is at {}, its details file still in {}",
                    target.display(),
                    old_side
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ))),
            };
        }
        if let Some(dir) = old_side.parent() {
            let _ = fs::remove_dir(dir); // only if now empty
        }
    }
    Ok(())
}

/// Rename, or (if allowed) copy and delete across volumes.
fn move_file(from: &Path, to: &Path, copy_across_disks: bool) -> Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::CrossesDevices && !copy_across_disks => {
            Err(Error::InvalidValue(ONLINE_ONLY_ELSEWHERE.into()))
        }
        Err(e) if e.kind() == ErrorKind::CrossesDevices => {
            let len = fs::copy(from, to)?;
            if len != fs::metadata(from)?.len() {
                let _ = fs::remove_file(to);
                return Err(Error::InvalidValue("copying the file failed".into()));
            }
            fs::remove_file(from)?;
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Whether two paths are the same file (e.g. a case-only rename on a
/// case-insensitive disk).
fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

/// A file name that's safe on macOS, Windows and Linux: no path separators
/// or reserved characters, no trailing dots or spaces, no doubled
/// extension, sensible length.
pub fn clean_name(name: &str, ext: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if !ext.is_empty() {
        let suffix = format!(".{}", ext.to_lowercase());
        if s.to_lowercase().ends_with(&suffix) {
            s.truncate(s.len() - suffix.len());
        }
    }
    let mut s = s
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_string();
    if s.len() > MAX_NAME_BYTES {
        let mut cut = MAX_NAME_BYTES;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        s = s.trim_end().to_string();
    }
    s
}

// MARK: - Moving to another folder

/// Why an online-only file can't go where it was asked to.
const ONLINE_ONLY_ELSEWHERE: &str = "it's online-only, and moving it out of its cloud folder would \
     download it. Download it first (open it once), then move it";

/// Where a document or folder went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    /// Its path in the destination source.
    pub rel_path: String,
    pub is_dir: bool,
}

/// Moves the document (with its sidecar) or folder at `rel_path` in `from`
/// into the folder `dest_dir` of `to`, which may be another source. Its name
/// is kept, with " (2)" etc. added if taken. Its catalog entries move too,
/// so online-only documents keep their details and text in the index.
///
/// Nothing is read or downloaded. An online-only document (or a folder
/// holding one) only moves within its source, or between two sources of the
/// same cloud service; anything else would download it. Folders move only
/// within one disk (a rename), never by copying.
pub fn move_entry(from: &Source, rel_path: &str, to: &Source, dest_dir: &str) -> Result<Moved> {
    let rel_path = rel_path.trim_matches('/');
    let dest_dir = dest_dir.trim_matches('/');
    if rel_path.is_empty() {
        return Err(Error::InvalidPath("a source itself can't be moved".into()));
    }
    if dest_dir.split('/').any(|p| p == sidecar::DIR_NAME) {
        return Err(Error::InvalidPath(dest_dir.into()));
    }
    let path = crate::listing::resolve(&from.root, rel_path)?;
    let dest = crate::listing::resolve(&to.root, dest_dir)?;
    // `symlink_metadata` only asks the file system; it doesn't open the file.
    let is_dir = fs::symlink_metadata(&path)?.is_dir();
    if !dest.is_dir() {
        return Err(Error::NotADirectory(dest));
    }
    let name = path
        .file_name()
        .ok_or_else(|| Error::InvalidPath(rel_path.into()))?
        .to_string_lossy()
        .into_owned();
    // Already there.
    if path.parent().is_some_and(|p| same_file(p, &dest)) {
        return Ok(Moved {
            rel_path: rel_path.into(),
            is_dir,
        });
    }
    if is_dir
        && let (Ok(p), Ok(d)) = (fs::canonicalize(&path), fs::canonicalize(&dest))
        && d.starts_with(&p)
    {
        return Err(Error::InvalidValue(format!(
            "“{name}” can't be moved into itself"
        )));
    }

    // Every document being moved (a folder: all of them, from its listing).
    let docs = if is_dir {
        let mut docs = Vec::new();
        walk_files(&from.root, rel_path, &mut docs)?;
        docs
    } else {
        vec![(rel_path.to_string(), cloud_state(&path)?)]
    };
    let stays_in_cloud =
        from.id == to.id || (from.provider == to.provider && from.provider != Provider::Local);
    if !stays_in_cloud
        && let Some((doc, _)) = docs.iter().find(|(_, c)| *c == CloudState::CloudOnly)
    {
        let doc_name = doc.rsplit('/').next().unwrap_or(doc);
        return Err(Error::InvalidValue(format!(
            "Can't move “{doc_name}”: {ONLINE_ONLY_ELSEWHERE}."
        )));
    }

    let target = if is_dir {
        free_name(&path, &dest, &name, "")
    } else {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| name.clone());
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        free_name(&path, &dest, &stem, &ext)
    };
    let new_rel = crate::listing::relative(&to.root, &target)
        .ok_or_else(|| Error::InvalidPath(target.display().to_string()))?;
    let renamed = |old: &str| format!("{new_rel}{}", &old[rel_path.len()..]);

    // Catalog entries are written at the new paths before the move, so the
    // indexer finds them as soon as it sees the moved documents (and drops
    // the old ones when it sees them gone).
    let mut written = Vec::new();
    if from.shares_catalog() && to.shares_catalog() {
        for (doc, _) in &docs {
            if let Some(mut entry) = catalog::read(&from.root, doc)? {
                entry.rel_path = renamed(doc);
                catalog::write(&to.root, &entry)?;
                written.push(entry.rel_path);
            }
        }
    }

    let result = if is_dir {
        fs::rename(&path, &target).map_err(|e| {
            if e.kind() == ErrorKind::CrossesDevices {
                Error::InvalidValue(format!(
                    "“{name}” is a folder on another disk. Folders can be moved only within the \
                     same disk; move it in the Finder or File Explorer instead"
                ))
            } else {
                e.into()
            }
        })
    } else {
        let local = docs[0].1 == CloudState::Local;
        move_with_sidecar(&path, &target, local)
    };
    if let Err(e) = result {
        for rel in &written {
            let _ = catalog::remove(&to.root, rel);
        }
        return Err(e);
    }
    for (doc, _) in &docs {
        let _ = catalog::remove(&from.root, doc);
    }
    Ok(Moved {
        rel_path: new_rel,
        is_dir,
    })
}

/// Whether `rel_path` is the folder `dir` or inside it.
pub fn is_within(rel_path: &str, dir: &str) -> bool {
    let (rel_path, dir) = (rel_path.trim_matches('/'), dir.trim_matches('/'));
    dir.is_empty() || rel_path == dir || rel_path.starts_with(&format!("{dir}/"))
}

/// Every file under `rel_dir` with whether it's online-only, from directory
/// listings only (every type, hidden `.rupert` folders aside).
fn walk_files(root: &Path, rel_dir: &str, out: &mut Vec<(String, CloudState)>) -> Result<()> {
    for e in list_dir(root, rel_dir, true)? {
        if e.is_dir {
            walk_files(root, &e.rel_path, out)?;
        } else {
            out.push((e.rel_path, e.cloud));
        }
    }
    Ok(())
}

// MARK: - Trash

/// Moves the document at `rel_path` (with its sidecar), or a whole folder,
/// to the system Trash / Recycle Bin. A source's own root can't be trashed
/// (it's removed from Rupert instead). Nothing is read or downloaded: an
/// online-only file is moved as it is, and its cloud service keeps it with
/// its deleted items. Returns whether it was a folder.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn trash(root: &Path, rel_path: &str) -> Result<bool> {
    trash_with(root, rel_path, &|path| {
        trash_context()
            .delete(path)
            .map_err(|e| Error::InvalidValue(format!("couldn't move it to the trash: {e}")))
    })
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
fn trash_context() -> trash::TrashContext {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    // NSFileManager rather than asking the Finder: no automation prompt.
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx
}

/// [`trash`] with the deleting done by `delete` (tests use a stand-in).
#[cfg_attr(any(target_os = "ios", target_os = "android"), allow(dead_code))]
fn trash_with(root: &Path, rel_path: &str, delete: &dyn Fn(&Path) -> Result<()>) -> Result<bool> {
    if rel_path.trim_matches('/').is_empty() {
        return Err(Error::InvalidPath(
            "a source itself can't be moved to the trash".into(),
        ));
    }
    let path = crate::listing::resolve(root, rel_path)?;
    // `symlink_metadata` only asks the file system; it doesn't open the file.
    let is_dir = fs::symlink_metadata(&path)?.is_dir();
    delete(&path)?;
    if !is_dir {
        // The sidecar follows its document. The document is already gone,
        // so a sidecar that won't go is left (it reattaches if the file
        // comes back) rather than reported.
        let side = sidecar::path_for(&path)?;
        if side.is_file()
            && delete(&side).is_ok()
            && let Some(dir) = side.parent()
        {
            let _ = fs::remove_dir(dir); // only if now empty
        }
    }
    Ok(is_dir)
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::DocMeta;

    #[test]
    fn cleans_names() {
        assert_eq!(
            clean_name("2026-04-29 Acme: auto / policy?", "pdf"),
            "2026-04-29 Acme auto policy"
        );
        assert_eq!(clean_name("Bill.PDF", "pdf"), "Bill");
        assert_eq!(clean_name("  ..Notes.  ", "txt"), "Notes");
        assert_eq!(clean_name(&"é".repeat(200), "pdf").len(), 150);
    }

    #[test]
    fn moves_with_sidecar_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let inbox = dir.path().join("Inbox");
        let dest = dir.path().join("Insurance");
        fs::create_dir_all(&inbox).unwrap();
        fs::create_dir_all(&dest).unwrap();
        let scan = inbox.join("scan001.jpg");
        fs::write(&scan, b"jpeg").unwrap();
        sidecar::write_meta(
            &scan,
            Some(DocMeta {
                title: Some("Acme".into()),
                ..Default::default()
            }),
            false,
            false,
        )
        .unwrap();
        fs::write(dest.join("Acme notice.jpg"), b"other").unwrap();

        let moved = move_document(&scan, &dest, "Acme notice").unwrap();
        assert_eq!(moved, dest.join("Acme notice (2).jpg"));
        assert_eq!(fs::read(&moved).unwrap(), b"jpeg");
        assert_eq!(
            fs::read(dest.join("Acme notice.jpg")).unwrap(),
            b"other",
            "not overwritten"
        );
        assert!(!scan.exists());
        let side = sidecar::read(&moved).unwrap().unwrap();
        assert_eq!(side.meta.unwrap().title.as_deref(), Some("Acme"));
        assert!(
            !inbox.join(".rupert").exists(),
            "empty sidecar folder removed"
        );

        // Renaming in place to its own name is a no-op.
        assert_eq!(
            move_document(&moved, &dest, "Acme notice (2)").unwrap(),
            moved
        );
    }

    fn source(id: &str, root: &Path, provider: Provider) -> Source {
        Source {
            id: id.into(),
            name: id.into(),
            root: root.to_path_buf(),
            provider,
            show_all_types: false,
            download_online_only: false,
            bookmark: None,
        }
    }

    fn titled(title: &str) -> DocMeta {
        DocMeta {
            title: Some(title.into()),
            ..Default::default()
        }
    }

    fn catalog_entry(rel: &str, title: &str) -> catalog::CatalogEntry {
        catalog::CatalogEntry::new(
            rel,
            4,
            Some(1_000),
            titled(title),
            crate::meta::Storage::Sidecar,
            Some("text".into()),
        )
    }

    #[test]
    fn move_takes_details_and_catalog_to_another_source() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let from = source("a", a.path(), Provider::OneDrive);
        let to = source("b", b.path(), Provider::OneDrive);
        fs::create_dir_all(a.path().join("Boat")).unwrap();
        fs::create_dir_all(b.path().join("Archive")).unwrap();
        let doc = a.path().join("Boat/Quote.docx");
        fs::write(&doc, b"word").unwrap();
        sidecar::write_meta(&doc, Some(titled("Quote")), false, false).unwrap();
        catalog::write(a.path(), &catalog_entry("Boat/Quote.docx", "Quote")).unwrap();
        fs::write(b.path().join("Archive/Quote.docx"), b"other").unwrap();

        let moved = move_entry(&from, "Boat/Quote.docx", &to, "Archive").unwrap();
        assert_eq!(
            moved,
            Moved {
                rel_path: "Archive/Quote (2).docx".into(),
                is_dir: false
            }
        );
        let new = b.path().join("Archive/Quote (2).docx");
        assert_eq!(fs::read(&new).unwrap(), b"word");
        assert_eq!(
            fs::read(b.path().join("Archive/Quote.docx")).unwrap(),
            b"other",
            "not overwritten"
        );
        // The details file followed; the emptied `.rupert` went away.
        let side = sidecar::read(&new).unwrap().unwrap();
        assert_eq!(side.meta.unwrap().title.as_deref(), Some("Quote"));
        assert!(!doc.exists());
        assert!(!a.path().join("Boat/.rupert").exists());
        // So did the catalog entry, under its new path.
        let entry = catalog::read(b.path(), "Archive/Quote (2).docx")
            .unwrap()
            .unwrap();
        assert_eq!(entry.meta.title.as_deref(), Some("Quote"));
        assert_eq!(entry.text.as_deref(), Some("text"));
        assert!(
            catalog::read(a.path(), "Boat/Quote.docx")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn move_into_a_local_source_leaves_no_catalog() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let from = source("a", a.path(), Provider::ICloud);
        let to = source("b", b.path(), Provider::Local);
        fs::write(a.path().join("bill.pdf"), b"%PDF").unwrap();
        catalog::write(a.path(), &catalog_entry("bill.pdf", "Bill")).unwrap();

        move_entry(&from, "bill.pdf", &to, "").unwrap();
        assert!(b.path().join("bill.pdf").is_file());
        assert!(!catalog::dir(b.path()).exists());
        assert!(catalog::read(a.path(), "bill.pdf").unwrap().is_none());
    }

    #[test]
    fn move_folders_with_everything_inside() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let src = source("a", root, Provider::OneDrive);
        fs::create_dir_all(root.join("Boat/Engine")).unwrap();
        fs::create_dir_all(root.join("Old")).unwrap();
        let doc = root.join("Boat/Engine/manual.pdf");
        fs::write(&doc, b"%PDF").unwrap();
        sidecar::write_meta(&doc, Some(titled("Manual")), false, false).unwrap();
        catalog::write(root, &catalog_entry("Boat/Engine/manual.pdf", "Manual")).unwrap();

        let moved = move_entry(&src, "Boat", &src, "Old").unwrap();
        assert_eq!(moved.rel_path, "Old/Boat");
        assert!(moved.is_dir);
        let new = root.join("Old/Boat/Engine/manual.pdf");
        assert!(new.is_file());
        assert!(sidecar::read(&new).unwrap().is_some(), "details came along");
        assert!(!root.join("Boat").exists());
        let entry = catalog::read(root, "Old/Boat/Engine/manual.pdf").unwrap();
        assert_eq!(entry.unwrap().meta.title.as_deref(), Some("Manual"));
        assert!(
            catalog::read(root, "Boat/Engine/manual.pdf")
                .unwrap()
                .is_none()
        );

        // Already in that folder: nothing happens.
        assert_eq!(
            move_entry(&src, "Old/Boat", &src, "Old").unwrap().rel_path,
            "Old/Boat"
        );
        // Not into itself, not the source itself, not into `.rupert`.
        assert!(move_entry(&src, "Old", &src, "Old/Boat/Engine").is_err());
        assert!(move_entry(&src, "Old", &src, "Old").is_err());
        assert!(move_entry(&src, "", &src, "Old").is_err());
        assert!(move_entry(&src, "Old/Boat", &src, ".rupert/catalog").is_err());
        assert!(move_entry(&src, "missing.pdf", &src, "Old").is_err());
        assert!(move_entry(&src, "Old/Boat", &src, "Old/Boat/Engine/manual.pdf").is_err());
    }

    #[test]
    fn a_document_stays_put_if_its_details_cant_follow() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let src = source("a", root, Provider::Local);
        fs::create_dir_all(root.join("In")).unwrap();
        fs::create_dir_all(root.join("Out")).unwrap();
        let doc = root.join("In/scan.pdf");
        fs::write(&doc, b"%PDF").unwrap();
        sidecar::write_meta(&doc, Some(titled("Scan")), false, false).unwrap();
        // A file where the destination's `.rupert` folder would go.
        fs::write(root.join("Out/.rupert"), b"in the way").unwrap();

        assert!(move_entry(&src, "In/scan.pdf", &src, "Out").is_err());
        assert!(doc.is_file(), "moved back");
        assert!(!root.join("Out/scan.pdf").exists());
        assert!(sidecar::read(&doc).unwrap().is_some());
    }

    #[test]
    fn within() {
        assert!(is_within("Boat/Inbox", "Boat"));
        assert!(is_within("Boat", "Boat"));
        assert!(is_within("Boat", ""));
        assert!(!is_within("Boats", "Boat"));
        assert!(!is_within("Boat", "Boat/Inbox"));
    }

    /// Moves into `bin` instead of the real trash.
    fn fake_trash(bin: &Path) -> impl Fn(&Path) -> Result<()> + '_ {
        move |p: &Path| {
            let to = bin.join(p.file_name().unwrap());
            fs::rename(p, to)?;
            Ok(())
        }
    }

    #[test]
    fn trash_takes_the_sidecar_along() {
        let dir = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir(root.join("Boat")).unwrap();
        fs::write(root.join("Boat/Quote.docx"), b"x").unwrap();
        fs::write(root.join("Boat/Keep.docx"), b"y").unwrap();
        let meta = DocMeta {
            title: Some("Quote".into()),
            ..Default::default()
        };
        for name in ["Quote.docx", "Keep.docx"] {
            sidecar::write_meta(
                &root.join("Boat").join(name),
                Some(meta.clone()),
                false,
                false,
            )
            .unwrap();
        }
        let delete = fake_trash(bin.path());

        assert!(!trash_with(root, "Boat/Quote.docx", &delete).unwrap());
        assert!(!root.join("Boat/Quote.docx").exists());
        assert!(bin.path().join("Quote.docx").is_file());
        assert!(bin.path().join("Quote.docx.json").is_file());
        // The other document's sidecar stays, and so does `.rupert/`.
        assert!(root.join("Boat/.rupert/Keep.docx.json").is_file());

        assert!(!trash_with(root, "Boat/Keep.docx", &delete).unwrap());
        assert!(!root.join("Boat/.rupert").exists());
    }

    #[test]
    fn trash_folders_but_not_the_source() {
        let dir = tempfile::tempdir().unwrap();
        let bin = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Old/Inner")).unwrap();
        fs::write(root.join("Old/Inner/a.pdf"), b"%PDF").unwrap();
        fs::write(root.join("plain.txt"), b"t").unwrap();
        let delete = fake_trash(bin.path());

        assert!(trash_with(root, "Old", &delete).unwrap());
        assert!(bin.path().join("Old/Inner/a.pdf").is_file());
        assert!(!trash_with(root, "plain.txt", &delete).unwrap());
        assert!(trash_with(root, "", &delete).is_err());
        assert!(trash_with(root, "/", &delete).is_err());
        assert!(trash_with(root, "../x", &delete).is_err());
        assert!(trash_with(root, "missing.pdf", &delete).is_err());
    }
}
