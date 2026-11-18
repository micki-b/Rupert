//! File operations Rupert performs on the user's behalf: renaming and
//! moving documents. They change the real files right away, take the
//! document's sidecar along, and never overwrite anything.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::sidecar;

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

    let with_ext = |s: &str| {
        if ext.is_empty() {
            s.to_string()
        } else {
            format!("{s}.{ext}")
        }
    };
    let mut target = dest_dir.join(with_ext(&stem));
    if same_file(from, &target) {
        return Ok(from.to_path_buf()); // nothing to do
    }
    let mut n = 2;
    while target.exists() && !same_file(from, &target) {
        target = dest_dir.join(with_ext(&format!("{stem} ({n})")));
        n += 1;
    }

    move_file(from, &target)?;

    // The sidecar follows its document.
    let old_side = sidecar::path_for(from)?;
    if old_side.is_file() {
        let new_side = sidecar::path_for(&target)?;
        let side_dir = new_side.parent().expect("sidecar has a parent");
        if !side_dir.exists() {
            fs::create_dir(side_dir)?;
            sidecar::hide(side_dir);
        }
        move_file(&old_side, &new_side)?;
        if let Some(dir) = old_side.parent() {
            let _ = fs::remove_dir(dir); // only if now empty
        }
    }
    Ok(target)
}

/// Rename, or copy and delete across volumes.
fn move_file(from: &Path, to: &Path) -> Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
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

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::DocMeta;

    #[test]
    fn cleans_names() {
        assert_eq!(
            clean_name("2026-04-29 Safeco: auto / policy?", "pdf"),
            "2026-04-29 Safeco auto policy"
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
                title: Some("Safeco".into()),
                ..Default::default()
            }),
            false,
            false,
        )
        .unwrap();
        fs::write(dest.join("Safeco notice.jpg"), b"other").unwrap();

        let moved = move_document(&scan, &dest, "Safeco notice").unwrap();
        assert_eq!(moved, dest.join("Safeco notice (2).jpg"));
        assert_eq!(fs::read(&moved).unwrap(), b"jpeg");
        assert_eq!(
            fs::read(dest.join("Safeco notice.jpg")).unwrap(),
            b"other",
            "not overwritten"
        );
        assert!(!scan.exists());
        let side = sidecar::read(&moved).unwrap().unwrap();
        assert_eq!(side.meta.unwrap().title.as_deref(), Some("Safeco"));
        assert!(
            !inbox.join(".rupert").exists(),
            "empty sidecar folder removed"
        );

        // Renaming in place to its own name is a no-op.
        assert_eq!(
            move_document(&moved, &dest, "Safeco notice (2)").unwrap(),
            moved
        );
    }
}
