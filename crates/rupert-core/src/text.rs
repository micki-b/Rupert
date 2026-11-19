//! Full-text extraction for search, and page rendering for OCR and
//! previews.
//!
//! PDFs go through PDFium (loaded at runtime from the app bundle; see
//! `scripts/fetch-pdfium.sh`). PDFium can be loaded once per process and is
//! then tied to the thread that loaded it, so it lives on one service thread
//! and [`TextExtractor`] is a cloneable handle that sends it requests. Create
//! it once and share clones (indexer, intake). Plain-text formats are read
//! directly. Other formats are indexed by name and metadata only for now.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use pdfium_render::prelude::*;

use crate::error::{Error, Result};
use crate::file_kind::FileKind;

/// Stop after this many pages or characters; enough to find any document,
/// and keeps scanned books from bloating the index.
const MAX_PAGES: i32 = 1500;
const MAX_CHARS: usize = 1_000_000;

// MARK: - Rendered pages

/// One page rendered to pixels.
#[derive(Debug, Clone)]
pub struct RenderedPage {
    pub width: u32,
    pub height: u32,
    /// 1 (grayscale) or 4 (RGBA) bytes per pixel, rows top to bottom.
    pub channels: u8,
    pub pixels: Vec<u8>,
    /// Maps a pixel position to PDF user space (accounting for the page's
    /// rotation and crop box): `x = a·px + c·py + e`, `y = b·px + d·py + f`
    /// for `[a, b, c, d, e, f]`.
    pub to_page: [f64; 6],
}

impl RenderedPage {
    pub fn page_point(&self, px: f64, py: f64) -> (f64, f64) {
        let [a, b, c, d, e, f] = self.to_page;
        (a * px + c * py + e, b * px + d * py + f)
    }

    /// PNG encoding (fast compression: these are short-lived).
    pub fn to_png(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.width, self.height);
        enc.set_color(if self.channels == 1 {
            png::ColorType::Grayscale
        } else {
            png::ColorType::Rgba
        });
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::Fast);
        let mut writer = enc
            .write_header()
            .map_err(|e| Error::InvalidValue(e.to_string()))?;
        writer
            .write_image_data(&self.pixels)
            .map_err(|e| Error::InvalidValue(e.to_string()))?;
        writer
            .finish()
            .map_err(|e| Error::InvalidValue(e.to_string()))?;
        Ok(out)
    }
}

// MARK: - TextExtractor

enum Request {
    Extract(PathBuf, Sender<Result<String>>),
    ExtractBytes(Vec<u8>, Sender<Result<String>>),
    PageTextLengths(PathBuf, Sender<Result<Vec<usize>>>),
    PageCount(PathBuf, Sender<Result<usize>>),
    Render {
        path: PathBuf,
        page: usize,
        /// Longest side in pixels.
        max_side: u32,
        /// Pixels per point at most (e.g. 300/72 for 300 dpi).
        max_scale: f32,
        color: bool,
        reply: Sender<Result<RenderedPage>>,
    },
}

/// Handle to the PDFium service thread (see the module docs).
#[derive(Clone)]
pub struct TextExtractor {
    tx: Option<Sender<Request>>,
}

impl TextExtractor {
    /// Loads PDFium from `lib_dir` on a new service thread. Without it,
    /// PDFs get no body text but everything else still works. Call once
    /// per process and clone the handle.
    pub fn new(lib_dir: Option<&Path>) -> Self {
        let Some(dir) = lib_dir.map(Path::to_path_buf) else {
            return Self { tx: None };
        };
        let (tx, rx) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<bool>();
        let spawned = std::thread::Builder::new()
            .name("rupert-pdfium".into())
            .spawn(move || {
                let pdfium = match Pdfium::bind_to_library(
                    Pdfium::pdfium_platform_library_name_at_path(&dir),
                ) {
                    Ok(bindings) => Pdfium::new(bindings),
                    Err(e) => {
                        eprintln!("rupert: PDFium unavailable in {}: {e}", dir.display());
                        let _ = ready_tx.send(false);
                        return;
                    }
                };
                let _ = ready_tx.send(true);
                for request in rx {
                    // One bad PDF must not take the service down.
                    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        serve(&pdfium, request)
                    }));
                }
            });
        let ok = spawned.is_ok() && ready_rx.recv().unwrap_or(false);
        Self {
            tx: ok.then_some(tx),
        }
    }

    pub fn pdf_supported(&self) -> bool {
        self.tx.is_some()
    }

    /// Body text of a local file, or `None` if its format isn't supported.
    pub fn extract(&self, path: &Path) -> Result<Option<String>> {
        match FileKind::from_path(path) {
            FileKind::Pdf => match &self.tx {
                Some(_) => self
                    .ask(|reply| Request::Extract(path.to_path_buf(), reply))
                    .map(Some),
                None => Ok(None),
            },
            FileKind::Text => extract_plain(path).map(Some),
            _ => Ok(None),
        }
    }

    /// Text of a PDF held in memory (e.g. to check an update before it's
    /// written).
    pub fn extract_pdf_bytes(&self, bytes: Vec<u8>) -> Result<String> {
        self.ask(|reply| Request::ExtractBytes(bytes, reply))
    }

    /// Number of text characters on each page (0 = needs OCR).
    pub fn page_text_lengths(&self, path: &Path) -> Result<Vec<usize>> {
        self.ask(|reply| Request::PageTextLengths(path.to_path_buf(), reply))
    }

    /// How many pages a PDF has.
    pub fn page_count(&self, path: &Path) -> Result<usize> {
        self.ask(|reply| Request::PageCount(path.to_path_buf(), reply))
    }

    /// Renders one page (0-based), at most `max_scale` pixels per point and
    /// `max_side` pixels on the longest side.
    pub fn render_page(
        &self,
        path: &Path,
        page: usize,
        max_side: u32,
        max_scale: f32,
        color: bool,
    ) -> Result<RenderedPage> {
        self.ask(|reply| Request::Render {
            path: path.to_path_buf(),
            page,
            max_side,
            max_scale,
            color,
            reply,
        })
    }

    fn ask<T>(&self, make: impl FnOnce(Sender<Result<T>>) -> Request) -> Result<T> {
        let tx = self
            .tx
            .as_ref()
            .ok_or_else(|| Error::Unsupported("PDFium isn't available".into()))?;
        let (reply_tx, reply_rx) = mpsc::channel();
        tx.send(make(reply_tx))
            .map_err(|_| Error::Pdf("the PDF service stopped".into()))?;
        reply_rx
            .recv()
            .unwrap_or_else(|_| Err(Error::Pdf("PDFium failed on this file".into())))
    }
}

// MARK: - Service

fn serve(pdfium: &Pdfium, request: Request) {
    match request {
        Request::Extract(path, reply) => {
            let _ = reply.send(open(pdfium, &path).and_then(|d| document_text(&d)));
        }
        Request::ExtractBytes(bytes, reply) => {
            let result = pdfium
                .load_pdf_from_byte_slice(&bytes, None)
                .map_err(pdf_err)
                .and_then(|d| document_text(&d));
            let _ = reply.send(result);
        }
        Request::PageTextLengths(path, reply) => {
            let result = open(pdfium, &path).map(|doc| {
                doc.pages()
                    .iter()
                    .map(|p| {
                        page_text(&p)
                            .map(|t| t.chars().filter(|c| !c.is_whitespace()).count())
                            .unwrap_or(0)
                    })
                    .collect()
            });
            let _ = reply.send(result);
        }
        Request::PageCount(path, reply) => {
            let _ = reply.send(open(pdfium, &path).map(|doc| doc.pages().len() as usize));
        }
        Request::Render {
            path,
            page,
            max_side,
            max_scale,
            color,
            reply,
        } => {
            let result =
                open(pdfium, &path).and_then(|doc| render(&doc, page, max_side, max_scale, color));
            let _ = reply.send(result);
        }
    }
}

fn open<'a>(pdfium: &'a Pdfium, path: &Path) -> Result<PdfDocument<'a>> {
    pdfium.load_pdf_from_file(path, None).map_err(pdf_err)
}

fn pdf_err(e: impl std::fmt::Display) -> Error {
    Error::Pdf(e.to_string())
}

/// All of a page's text. pdfium-render's `PdfPageText::all` only takes the
/// text inside the page's *displayed* size, which on a page shown turned
/// (`/Rotate 90` or `270`) has width and height swapped, so whatever lies
/// beyond the shorter side was cut off. The media box, in the page's own
/// coordinates, holds everything that can be seen.
fn page_text(page: &PdfPage) -> Option<String> {
    let text = page.text().ok()?;
    let bounds = page
        .boundaries()
        .media()
        .map(|b| b.bounds)
        .unwrap_or_else(|_| PdfRect::new_from_values(-1.0e5, -1.0e5, 1.0e5, 1.0e5));
    Some(text.inside_rect(bounds))
}

fn document_text(doc: &PdfDocument) -> Result<String> {
    let mut out = String::new();
    for (i, page) in doc.pages().iter().enumerate() {
        if i as i32 >= MAX_PAGES || out.len() >= MAX_CHARS {
            break;
        }
        if let Some(text) = page_text(&page) {
            push_normalized(&mut out, &text);
            out.push('\n');
        }
    }
    truncate(&mut out);
    Ok(out.trim().to_string())
}

fn render(
    doc: &PdfDocument,
    index: usize,
    max_side: u32,
    max_scale: f32,
    color: bool,
) -> Result<RenderedPage> {
    let page = doc.pages().get(index as PdfPageIndex).map_err(pdf_err)?;
    let (w, h) = (page.width().value, page.height().value);
    let scale = max_scale.min(max_side as f32 / w.max(h).max(1.0));
    let config = PdfRenderConfig::new()
        .scale_page_by_factor(scale)
        .render_form_data(true)
        .use_grayscale_rendering(!color);
    let bitmap = page.render_with_config(&config).map_err(pdf_err)?;
    let (width, height) = (bitmap.width() as u32, bitmap.height() as u32);
    let rgba = bitmap.as_rgba_bytes();
    let pixels = if color {
        rgba
    } else {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .map(|p| ((p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000) as u8)
            .collect()
    };

    // Pixel → page mapping from three reference points (exact for the
    // affine transforms PDFium uses; far-apart points keep it precise).
    let pt = |x: i32, y: i32| -> Result<(f64, f64)> {
        let (px, py) = page.pixels_to_points(x, y, &config).map_err(pdf_err)?;
        Ok((px.value as f64, py.value as f64))
    };
    let (sx, sy) = (width as i32, height as i32);
    let o = pt(0, 0)?;
    let ax = pt(sx, 0)?;
    let ay = pt(0, sy)?;
    let to_page = [
        (ax.0 - o.0) / sx as f64,
        (ax.1 - o.1) / sx as f64,
        (ay.0 - o.0) / sy as f64,
        (ay.1 - o.1) / sy as f64,
        o.0,
        o.1,
    ];
    Ok(RenderedPage {
        width,
        height,
        channels: if color { 4 } else { 1 },
        pixels,
        to_page,
    })
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
pub(crate) mod tests {
    use super::*;

    /// PDFium for tests, if `scripts/fetch-pdfium.sh` has been run. One per
    /// test process (PDFium can only be loaded once).
    pub(crate) fn shared_pdfium() -> Option<&'static TextExtractor> {
        static PDFIUM: std::sync::OnceLock<Option<TextExtractor>> = std::sync::OnceLock::new();
        PDFIUM
            .get_or_init(|| {
                let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/pdfium");
                let extractor = TextExtractor::new(Some(&dir));
                extractor.pdf_supported().then_some(extractor)
            })
            .as_ref()
    }

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
