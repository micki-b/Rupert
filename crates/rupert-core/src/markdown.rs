//! Markdown documents, for Rupert's reader and gallery: telling them apart
//! and reading their text. Rendering happens in the window.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::error::Result;
use crate::listing::{CloudState, cloud_state};

/// Longest file the reader shows in full; bigger ones are cut off.
pub const MAX_BYTES: usize = 2 * 1024 * 1024;

/// Whether the file is Markdown, by its extension.
pub fn is_markdown(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "md" | "markdown"))
}

/// The text of the Markdown file at `path`, at most `limit` bytes (cut at a
/// character boundary). `None` for an online-only file: reading it would
/// download it, so the caller offers to open it instead.
pub fn read(path: &Path, limit: usize) -> Result<Option<String>> {
    if cloud_state(path)? == CloudState::CloudOnly {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    File::open(path)?
        .take(limit.min(MAX_BYTES) as u64)
        .read_to_end(&mut bytes)?;
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    // A cut in the middle of a character leaves a replacement mark at the end.
    if text.ends_with('\u{FFFD}') && bytes.len() >= limit.min(MAX_BYTES) {
        text.pop();
    }
    Ok(Some(text))
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_markdown() {
        assert!(is_markdown("notes/Boat.md"));
        assert!(is_markdown("README.MARKDOWN"));
        assert!(!is_markdown("todo.txt"));
        assert!(!is_markdown("md"));
    }

    #[test]
    fn reads_and_cuts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("n.md");
        std::fs::write(&path, "# Café\n\nWinter list").unwrap();
        assert_eq!(
            read(&path, MAX_BYTES).unwrap().unwrap(),
            "# Café\n\nWinter list"
        );
        // 5 bytes ends inside "é": the half character is dropped.
        assert_eq!(read(&path, 5).unwrap().unwrap(), "# Caf");
    }
}
