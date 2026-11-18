//! Full-text extraction for search.
//!
//! PDFs go through PDFium (loaded at runtime from the app bundle; see
//! `scripts/fetch-pdfium.sh`). Plain-text formats are read directly. Other
//! formats are indexed by name and metadata only for now.

use std::io::Read;
use std::path::Path;

use pdfium_render::prelude::*;

use crate::error::{Error, Result};
use crate::file_kind::FileKind;

/// Stop after this many pages or characters; enough to find any document,
/// and keeps scanned books from bloating the index.
const MAX_PAGES: i32 = 1500;
const MAX_CHARS: usize = 1_000_000;

// MARK: - TextExtractor

pub struct TextExtractor {
    pdfium: Option<Pdfium>,
}

impl TextExtractor {
    /// Loads PDFium from `lib_dir`. Without it, PDFs get no body text but
    /// everything else still works.
    pub fn new(lib_dir: Option<&Path>) -> Self {
        let pdfium = lib_dir.and_then(|dir| {
            Pdfium::bind_to_library(Pdfium::pdfium_platform_library_name_at_path(dir))
                .inspect_err(|e| eprintln!("rupert: PDFium unavailable in {}: {e}", dir.display()))
                .ok()
                .map(Pdfium::new)
        });
        Self { pdfium }
    }

    pub fn pdf_supported(&self) -> bool {
        self.pdfium.is_some()
    }

    /// Body text of a local file, or `None` if its format isn't supported.
    pub fn extract(&self, path: &Path) -> Result<Option<String>> {
        match FileKind::from_path(path) {
            FileKind::Pdf => match &self.pdfium {
                Some(pdfium) => extract_pdf(pdfium, path).map(Some),
                None => Ok(None),
            },
            FileKind::Text => extract_plain(path).map(Some),
            _ => Ok(None),
        }
    }
}

fn extract_pdf(pdfium: &Pdfium, path: &Path) -> Result<String> {
    let doc = pdfium
        .load_pdf_from_file(path, None)
        .map_err(|e| Error::Pdf(e.to_string()))?;
    let mut out = String::new();
    for (i, page) in doc.pages().iter().enumerate() {
        if i as i32 >= MAX_PAGES || out.len() >= MAX_CHARS {
            break;
        }
        if let Ok(text) = page.text() {
            push_normalized(&mut out, &text.all());
            out.push('\n');
        }
    }
    truncate(&mut out);
    Ok(out.trim().to_string())
}

fn extract_plain(path: &Path) -> Result<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_CHARS as u64)
        .read_to_end(&mut bytes)?;
    let mut out = String::new();
    push_normalized(&mut out, &String::from_utf8_lossy(&bytes));
    Ok(out.trim().to_string())
}

/// Appends `text` with runs of spaces collapsed and blank lines removed, so
/// search snippets read cleanly.
fn push_normalized(out: &mut String, text: &str) {
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if !words.is_empty() {
            out.push_str(&words.join(" "));
            out.push('\n');
        }
    }
}

fn truncate(s: &mut String) {
    if s.len() > MAX_CHARS {
        let mut cut = MAX_CHARS;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_whitespace() {
        let mut out = String::new();
        push_normalized(&mut out, "  Invoice   4179 \n\n\t Yanmar  injectors\r\n");
        assert_eq!(out, "Invoice 4179\nYanmar injectors\n");
    }

    #[test]
    fn plain_text_files() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.md");
        std::fs::write(&file, "# Boat\n\nReplace   impeller").unwrap();
        let text = TextExtractor::new(None).extract(&file).unwrap();
        assert_eq!(text.as_deref(), Some("# Boat\nReplace impeller"));
    }

    #[test]
    fn pdfs_without_pdfium_have_no_text() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.pdf");
        std::fs::write(&file, b"%PDF-1.4").unwrap();
        assert_eq!(TextExtractor::new(None).extract(&file).unwrap(), None);
    }
}
