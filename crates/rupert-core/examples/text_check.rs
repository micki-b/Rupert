//! Dev tool: extract text from every PDF in a folder and report size, time
//! and a sample. Reads files only.
//! Usage: cargo run --release -p rupert-core --example text_check -- <pdfium-dir> <folder>

use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = std::path::PathBuf::from(args.next().expect("pdfium dir"));
    let dir = std::path::PathBuf::from(args.next().expect("folder"));
    let extractor = rupert_core::text::TextExtractor::new(Some(&lib));
    assert!(extractor.pdf_supported(), "PDFium failed to load");

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .collect();
    files.sort();
    for f in files {
        let t = Instant::now();
        let result = extractor.extract(&f);
        let ms = t.elapsed().as_millis();
        let name = f.file_name().unwrap().to_string_lossy();
        match result {
            Ok(Some(text)) => {
                let sample: String = text
                    .chars()
                    .take(70)
                    .collect::<String>()
                    .replace('\n', " ⏎ ");
                println!(
                    "{:<34} {:>6} ms {:>8} chars  {sample}",
                    &name[..name.len().min(34)],
                    ms,
                    text.len()
                );
            }
            Ok(None) => println!("{name}: unsupported"),
            Err(e) => println!("{name}: ERROR {e}"),
        }
    }
}
