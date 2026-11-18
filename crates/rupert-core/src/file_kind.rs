//! Classifying files by name: which ones count as documents, and which are
//! system clutter Rupert never shows.

use std::path::Path;

use serde::{Deserialize, Serialize};

// MARK: - FileKind

/// The broad kind of a file, decided by its extension.
///
/// Sources show only documents (`is_document() == true`) by default; a
/// per-source setting shows everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Pdf,
    Image,
    Office,
    Text,
    Email,
    Other,
}

impl FileKind {
    /// Classifies a path by its extension (case-insensitive).
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        let Some(ext) = path.as_ref().extension().and_then(|e| e.to_str()) else {
            return Self::Other;
        };
        match ext.to_ascii_lowercase().as_str() {
            "pdf" => Self::Pdf,
            "jpg" | "jpeg" | "png" | "tif" | "tiff" | "heic" | "heif" | "gif" | "webp" | "bmp" => {
                Self::Image
            }
            "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" | "odt" | "ods" | "odp" | "rtf"
            | "pages" | "numbers" | "key" => Self::Office,
            "txt" | "md" | "markdown" | "csv" => Self::Text,
            "eml" | "emlx" | "msg" => Self::Email,
            _ => Self::Other,
        }
    }

    /// Whether this kind is shown by the default "documents only" filter.
    pub fn is_document(self) -> bool {
        self != Self::Other
    }
}

// MARK: - Ignored names

/// Names Rupert never shows or indexes, regardless of filters: its own
/// metadata folder, OS clutter, and editor lock files.
pub fn is_ignored_name(name: &str) -> bool {
    matches!(
        name,
        ".rupert" | ".DS_Store" | ".localized" | "Thumbs.db" | "desktop.ini" | "Icon\r"
    ) || name.starts_with("~$") // Microsoft Office lock files
        || name.starts_with(".~lock.") // LibreOffice lock files
        || name.starts_with("._") // AppleDouble files on non-Mac volumes
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_by_extension_case_insensitively() {
        assert_eq!(FileKind::from_path("Taxes/2025 W-2.PDF"), FileKind::Pdf);
        assert_eq!(FileKind::from_path("receipt.HEIC"), FileKind::Image);
        assert_eq!(FileKind::from_path("lease.docx"), FileKind::Office);
        assert_eq!(FileKind::from_path("notes.md"), FileKind::Text);
        assert_eq!(FileKind::from_path("order.eml"), FileKind::Email);
    }

    #[test]
    fn non_documents_are_other() {
        for name in [
            "setup.exe",
            "lib.dll",
            "archive.zip",
            "page.html",
            "Makefile",
            "noext",
        ] {
            let kind = FileKind::from_path(name);
            assert_eq!(kind, FileKind::Other, "{name}");
            assert!(!kind.is_document(), "{name}");
        }
    }

    #[test]
    fn dotfile_without_extension_is_other() {
        // `Path::extension` returns None for ".pdf" — a hidden file named "pdf".
        assert_eq!(FileKind::from_path(".pdf"), FileKind::Other);
    }

    #[test]
    fn ignores_clutter_and_own_folder() {
        for name in [
            ".rupert",
            ".DS_Store",
            "Thumbs.db",
            "desktop.ini",
            "~$Budget.xlsx",
            "._scan.pdf",
        ] {
            assert!(is_ignored_name(name), "{name}");
        }
        assert!(!is_ignored_name("scan.pdf"));
        assert!(!is_ignored_name(".hidden-but-real.pdf"));
    }
}
