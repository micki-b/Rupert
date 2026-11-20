//! Optical character recognition for scans without a text layer.
//!
//! Engines, all local:
//! - **macOS:** Apple's Vision framework (built in; accurate, detects the
//!   language, handles English, German, Spanish and more).
//! - **Elsewhere:** the `tesseract` command, if installed (Linux packages,
//!   or the UB Mannheim build on Windows).
//!
//! [`ocr_pdf`] renders each page that has no text with PDFium, recognizes
//! the words with their positions, and adds them to the PDF as an invisible
//! text layer (see [`crate::text_layer`]), so every PDF reader can search
//! and select the text.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::text::TextExtractor;
use crate::text_layer::{self, PageWords, PlacedWord};

/// Render scans at up to 300 dpi, and at most this many pixels on the
/// longest side.
const OCR_SCALE: f32 = 300.0 / 72.0;
const OCR_MAX_SIDE: u32 = 4200;
/// A page with fewer text characters than this is treated as a scan.
const MIN_TEXT_CHARS: usize = 20;
/// Long documents are OCR'd only this far (the rest stays as it is).
pub const MAX_OCR_PAGES: usize = 300;

// MARK: - Words

/// A recognized word and its corners in image pixels (origin top-left):
/// top-left, top-right, bottom-right, bottom-left.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrWord {
    pub text: String,
    pub quad: [(f64, f64); 4],
}

// MARK: - Engine

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Engine {
    #[cfg(target_os = "macos")]
    Vision,
    Tesseract {
        program: PathBuf,
        languages: String,
    },
}

impl Engine {
    /// The best engine available on this machine.
    pub fn detect() -> Option<Engine> {
        #[cfg(target_os = "macos")]
        {
            Some(Engine::Vision)
        }
        #[cfg(not(target_os = "macos"))]
        {
            tesseract()
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            Engine::Vision => "Apple Vision",
            Engine::Tesseract { .. } => "Tesseract",
        }
    }

    /// Recognizes the words in a PNG image of `width`×`height` pixels.
    pub fn recognize(&self, png: &[u8], width: u32, height: u32) -> Result<Vec<OcrWord>> {
        // Tesseract reports pixels itself; only Vision needs the size.
        let _ = (width, height);
        match self {
            #[cfg(target_os = "macos")]
            Engine::Vision => vision::recognize(png, width as f64, height as f64),
            Engine::Tesseract { program, languages } => {
                tesseract_recognize(program, languages, png)
            }
        }
    }
}

// MARK: - Whole PDFs

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OcrReport {
    pub pages: usize,
    /// Pages that had no text and were recognized.
    pub pages_recognized: usize,
    pub words: usize,
    /// The recognized words, a line per page.
    pub text: String,
    /// Why the text layer couldn't be added to the PDF, if it couldn't.
    /// The recognized text is still in `text`, to be kept elsewhere.
    pub layer_error: Option<String>,
}

/// Adds a text layer to every page of the PDF that has none (up to
/// [`MAX_OCR_PAGES`]). Pages that already have text are left alone, so
/// running it again does nothing. `on_page(done, total)` reports progress.
/// If the layer can't be written, the report says why and still carries
/// the recognized text.
pub fn ocr_pdf(
    path: &Path,
    pdf: &TextExtractor,
    engine: &Engine,
    mut on_page: impl FnMut(usize, usize),
) -> Result<OcrReport> {
    let lengths = pdf.page_text_lengths(path)?;
    let todo: Vec<usize> = lengths
        .iter()
        .enumerate()
        .filter(|(_, n)| **n < MIN_TEXT_CHARS)
        .map(|(i, _)| i)
        .take(MAX_OCR_PAGES)
        .collect();
    let mut report = OcrReport {
        pages: lengths.len(),
        ..Default::default()
    };
    if todo.is_empty() {
        return Ok(report);
    }

    let mut pages = Vec::new();
    for (n, &index) in todo.iter().enumerate() {
        on_page(n, todo.len());
        let page = pdf.render_page(path, index, OCR_MAX_SIDE, OCR_SCALE, false)?;
        let words = engine.recognize(&page.to_png()?, page.width, page.height)?;
        if words.is_empty() {
            continue; // a blank page, or a photo
        }
        report.pages_recognized += 1;
        report.words += words.len();
        pages.push(PageWords {
            index,
            words: words
                .into_iter()
                .map(|w| PlacedWord {
                    text: w.text,
                    quad: w.quad.map(|(x, y)| page.page_point(x, y)),
                })
                .collect(),
        });
    }
    on_page(todo.len(), todo.len());
    if pages.is_empty() {
        return Ok(report);
    }
    report.text = pages
        .iter()
        .map(|p| {
            p.words
                .iter()
                .map(|w| w.text.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n");
    if let Err(e) = text_layer::add(path, &pages, |bytes| pdf.extract_pdf_bytes(bytes)) {
        report.layer_error = Some(e.to_string());
    }
    Ok(report)
}

// MARK: - Apple Vision

#[cfg(target_os = "macos")]
mod vision {
    use objc2::AnyThread;
    use objc2::rc::autoreleasepool;
    use objc2::runtime::NSObjectProtocol;
    use objc2::sel;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSRange};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    use super::OcrWord;
    use crate::error::{Error, Result};

    pub fn recognize(png: &[u8], width: f64, height: f64) -> Result<Vec<OcrWord>> {
        autoreleasepool(|_| {
            let data = NSData::with_bytes(png);
            let handler = VNImageRequestHandler::initWithData_options(
                VNImageRequestHandler::alloc(),
                &data,
                &NSDictionary::new(),
            );
            let request = VNRecognizeTextRequest::new();
            request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
            request.setUsesLanguageCorrection(true);
            // macOS 13+: pick the language per text block (German letters,
            // Spanish forms…).
            if request.respondsToSelector(sel!(setAutomaticallyDetectsLanguage:)) {
                request.setAutomaticallyDetectsLanguage(true);
            }
            let as_request: &VNRequest = &request;
            handler
                .performRequests_error(&NSArray::from_slice(&[as_request]))
                .map_err(|e| Error::InvalidValue(crate::i18n::trf("OCR failed: {0}", &[&e])))?;

            let mut words = Vec::new();
            let Some(results) = request.results() else {
                return Ok(words);
            };
            for i in 0..results.count() {
                let observation = results.objectAtIndex(i);
                let candidates = observation.topCandidates(1);
                let Some(text) = candidates.firstObject() else {
                    continue;
                };
                let line = text.string().to_string();
                for (start, word) in split_words(&line) {
                    // Vision ranges count UTF-16 units.
                    let loc = line[..start].encode_utf16().count();
                    let len = word.encode_utf16().count();
                    // SAFETY: the range lies within the candidate's string.
                    let Ok(rect) =
                        (unsafe { text.boundingBoxForRange_error(NSRange::new(loc, len)) })
                    else {
                        continue;
                    };
                    // Normalized, origin bottom-left → pixels, origin top-left.
                    let px =
                        |p: objc2_core_foundation::CGPoint| (p.x * width, (1.0 - p.y) * height);
                    // SAFETY: plain property reads on a valid observation.
                    let quad = unsafe {
                        [
                            px(rect.topLeft()),
                            px(rect.topRight()),
                            px(rect.bottomRight()),
                            px(rect.bottomLeft()),
                        ]
                    };
                    words.push(OcrWord {
                        text: word.to_string(),
                        quad,
                    });
                }
            }
            Ok(words)
        })
    }

    /// Words with their byte offsets.
    fn split_words(line: &str) -> Vec<(usize, &str)> {
        let mut out = Vec::new();
        let mut start = None;
        for (i, c) in line.char_indices() {
            match (c.is_whitespace(), start) {
                (true, Some(s)) => {
                    out.push((s, &line[s..i]));
                    start = None;
                }
                (false, None) => start = Some(i),
                _ => {}
            }
        }
        if let Some(s) = start {
            out.push((s, &line[s..]));
        }
        out
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn splits_words_with_offsets() {
            assert_eq!(
                super::split_words(" Grüße  aus\tKöln"),
                [(1, "Grüße"), (10, "aus"), (14, "Köln")]
            );
        }
    }
}

// MARK: - Tesseract

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn tesseract() -> Option<Engine> {
    let program = find_program("tesseract")?;
    let out = std::process::Command::new(&program)
        .arg("--list-langs")
        .output()
        .ok()?;
    let listed = String::from_utf8_lossy(&out.stdout);
    let available: Vec<&str> = listed.lines().map(str::trim).collect();
    let languages: Vec<&str> = ["eng", "deu", "spa"]
        .into_iter()
        .filter(|l| available.contains(l))
        .collect();
    Some(Engine::Tesseract {
        program,
        languages: if languages.is_empty() {
            "eng".into()
        } else {
            languages.join("+")
        },
    })
}

#[cfg_attr(target_os = "macos", allow(dead_code))]
fn find_program(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    // Common install locations that GUI apps don't have on their PATH.
    dirs.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            r"C:\Program Files\Tesseract-OCR",
        ]
        .map(PathBuf::from),
    );
    dirs.into_iter().map(|d| d.join(&exe)).find(|p| p.is_file())
}

fn tesseract_recognize(program: &Path, languages: &str, png: &[u8]) -> Result<Vec<OcrWord>> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new(program)
        .args(["stdin", "stdout", "-l", languages, "tsv"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    child.stdin.take().expect("piped stdin").write_all(png)?;
    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(Error::InvalidValue(
            crate::i18n::tr("tesseract failed").into(),
        ));
    }
    Ok(parse_tsv(&String::from_utf8_lossy(&out.stdout)))
}

/// Word rows (level 5) of Tesseract's TSV output.
fn parse_tsv(tsv: &str) -> Vec<OcrWord> {
    tsv.lines()
        .skip(1)
        .filter_map(|line| {
            let cols: Vec<&str> = line.split('\t').collect();
            if cols.len() < 12 || cols[0] != "5" {
                return None;
            }
            let text = cols[11].trim();
            let num = |i: usize| cols[i].parse::<f64>().ok();
            let (l, t, w, h) = (num(6)?, num(7)?, num(8)?, num(9)?);
            (!text.is_empty()).then(|| OcrWord {
                text: text.to_string(),
                quad: [(l, t), (l + w, t), (l + w, t + h), (l, t + h)],
            })
        })
        .collect()
}

// MARK: - Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tesseract_tsv() {
        let tsv = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n\
                   4\t1\t1\t1\t1\t0\t10\t20\t300\t40\t-1\t\n\
                   5\t1\t1\t1\t1\t1\t10\t20\t120\t40\t96.5\tInvoice\n\
                   5\t1\t1\t1\t1\t2\t140\t20\t80\t40\t95.1\t4179\n\
                   5\t1\t1\t1\t1\t3\t230\t20\t10\t40\t20.0\t \n";
        let words = parse_tsv(tsv);
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Invoice");
        assert_eq!(words[1].quad[2], (220.0, 60.0));
    }
}
