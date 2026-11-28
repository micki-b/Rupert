//! Dev tool: run the inbox pipeline (OCR → Local AI) on a test folder and
//! file the first document. Use copies!
//! Usage: cargo run --release -p rupert-core --example intake_check -- <pdfium-dir> <test-root> <inbox-rel> <index-copy> <sources.json>

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rupert_core::intake::{self, Filing, Stage};
use rupert_core::settings::{InboxLocation, Settings};
use rupert_core::text::TextExtractor;
use rupert_core::{Provider, Source, SourceStore};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let extractor = TextExtractor::new(Some(&PathBuf::from(&a[0])));
    let test = Source {
        id: "test".into(),
        name: "Test".into(),
        root: PathBuf::from(&a[1]),
        provider: Provider::Local,
        show_all_types: false,
        download_online_only: false,
        offline: false,
        bookmark: None,
    };
    // Real sources only lend their names to folder suggestions; nothing is
    // written to them.
    let mut sources = SourceStore::load(PathBuf::from(&a[4]))
        .unwrap()
        .sources()
        .to_vec();
    sources.push(test.clone());
    let handle = intake::spawn(
        PathBuf::from(&a[3]),
        std::env::temp_dir().join("rupert-intake-check.json"),
        extractor,
        || {},
    )
    .unwrap();
    let start = Instant::now();
    handle.configure(
        Settings {
            inbox: Some(InboxLocation {
                source_id: "test".into(),
                rel_path: a[2].clone(),
            }),
            ..Default::default()
        },
        sources.clone(),
    );
    loop {
        std::thread::sleep(Duration::from_millis(500));
        let items = handle.items();
        if !items.is_empty() && items.iter().all(|i| i.stage == Stage::Ready) {
            println!("all ready in {:.1}s", start.elapsed().as_secs_f64());
            for i in &items {
                let s = i.suggestion.clone().unwrap_or_default();
                println!(
                    "\n{}\n  ocr: {:?}\n  title: {:?}  date: {:?}  tags: {:?}\n  folder: {:?}\n  note: {:?}",
                    i.name,
                    i.ocr,
                    s.title,
                    s.document_date,
                    s.tags,
                    i.suggested_folder.as_ref().map(|f| &f.label),
                    i.note
                );
            }
            let first = &items[0];
            let s = first.suggestion.clone().unwrap_or_default();
            let filed = intake::file_document(
                &sources,
                "test",
                &first.rel_path,
                &Filing {
                    title: s.title.clone(),
                    document_date: s.document_date.clone(),
                    tags: s.tags.clone(),
                    dest_source_id: "test".into(),
                    dest_dir: "Filed".into(),
                    file_name: Settings::default().file_name(
                        s.document_date.as_deref(),
                        s.title.as_deref().unwrap_or("Untitled"),
                    ),
                    ..Default::default()
                },
            )
            .unwrap();
            println!("\nfiled: {}", filed.rel_path);
            break;
        }
        if start.elapsed() > Duration::from_secs(600) {
            println!(
                "timeout: {:?}",
                handle
                    .items()
                    .iter()
                    .map(|i| (&i.name, i.stage))
                    .collect::<Vec<_>>()
            );
            break;
        }
    }
}
