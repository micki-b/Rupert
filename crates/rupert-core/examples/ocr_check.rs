//! Dev tool: OCR PDFs in place (use copies!) and print what was recognized.
//! Usage: cargo run --release -p rupert-core --example ocr_check -- <pdfium-dir> <pdf>...

use std::path::PathBuf;
use std::time::Instant;

use rupert_core::ocr::{self, Engine};
use rupert_core::text::TextExtractor;

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = PathBuf::from(args.next().expect("pdfium dir"));
    let pdf = TextExtractor::new(Some(&lib));
    let engine = Engine::detect().expect("no OCR engine");
    println!("engine: {}", engine.name());
    for file in args {
        let path = PathBuf::from(&file);
        let t = Instant::now();
        match ocr::ocr_pdf(&path, &pdf, &engine, |_, _| {}) {
            Ok(r) => {
                if let Some(e) = &r.layer_error {
                    println!("  text layer not written: {e}");
                }
                let text = pdf.extract(&path).unwrap().unwrap_or_default();
                println!(
                    "{}: {} pages, {} recognized, {} words in {:.1}s\n  {}",
                    path.file_name().unwrap().to_string_lossy(),
                    r.pages,
                    r.pages_recognized,
                    r.words,
                    t.elapsed().as_secs_f64(),
                    text.replace('\n', " ")
                        .chars()
                        .take(140)
                        .collect::<String>()
                );
            }
            Err(e) => println!("{}: ERROR {e}", path.display()),
        }
    }
}
