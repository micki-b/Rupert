//! Dev tool: ask Local AI (`RUPERT_LOCAL_AI`, LM Studio by default;
//! `RUPERT_LOCAL_MODEL`) for filing suggestions for PDFs, using a copy of an
//! index for folders and tags.
//! Usage: cargo run --release -p rupert-core --example llm_check -- <pdfium-dir> <index-copy> <sources.json> <pdf>...

use std::path::PathBuf;
use std::time::Instant;

use rupert_core::llm::{self, LocalAi};
use rupert_core::text::TextExtractor;
use rupert_core::{Index, SourceStore};

fn main() {
    let mut args = std::env::args().skip(1);
    let pdf = TextExtractor::new(Some(&PathBuf::from(args.next().unwrap())));
    let index = Index::open(&PathBuf::from(args.next().unwrap())).unwrap();
    let store = SourceStore::load(PathBuf::from(args.next().unwrap())).unwrap();
    let name = |id: &str| store.get(id).map(|s| s.name.clone()).unwrap_or_default();
    let lm = LocalAi {
        url: std::env::var("RUPERT_LOCAL_AI").unwrap_or_else(|_| llm::DEFAULT_URL.into()),
        model: std::env::var("RUPERT_LOCAL_MODEL").ok(),
    };
    println!("models: {:?}", lm.models());
    let tags = index.top_tags(150).unwrap();
    for file in args {
        let path = PathBuf::from(&file);
        let text = pdf.extract(&path).unwrap().unwrap_or_default();
        let folders: Vec<String> = index
            .similar_folders(&text, None, 12)
            .unwrap()
            .into_iter()
            .map(|(s, d, _)| {
                if d.is_empty() {
                    name(&s)
                } else {
                    format!("{}/{d}", name(&s))
                }
            })
            .collect();
        let t = Instant::now();
        let req = llm::Request {
            text: &text,
            file_name: path.file_name().unwrap().to_str().unwrap(),
            year: "2026",
            folders: &folders,
            tags: &tags,
            examples: &[],
        };
        match lm.suggest(&req) {
            Ok(s) => println!(
                "\n{}\n  {:.1}s  title={:?} date={:?} tags={:?}\n  folder={:?}\n  reason={:?}\n  candidates={:?}",
                path.file_name().unwrap().to_string_lossy(),
                t.elapsed().as_secs_f64(),
                s.title,
                s.document_date,
                s.tags,
                s.folder,
                s.reason,
                &folders[..folders.len().min(4)]
            ),
            Err(e) => println!("{file}: ERROR {e}"),
        }
    }
}
