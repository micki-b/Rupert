//! Dev tool: index a folder into a throwaway database (read-only: the source
//! is treated as local, so no catalog is written) and run sample searches.
//! Usage: cargo run --release -p rupert-core --example index_check -- <pdfium-dir> <folder> <db> [queries...]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rupert_core::{Index, Job, Provider, SearchQuery, Source};

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = std::path::PathBuf::from(args.next().expect("pdfium dir"));
    let folder = std::path::PathBuf::from(args.next().expect("folder"));
    let db = std::path::PathBuf::from(args.next().expect("db path"));
    let queries: Vec<String> = args.collect();

    // Idle = an inactive report after at least one active one.
    let idle = Arc::new(Mutex::new(false));
    let busy = Arc::new(Mutex::new(false));
    let last = Arc::new(Mutex::new(Instant::now()));
    let (i, b, l) = (Arc::clone(&idle), Arc::clone(&busy), Arc::clone(&last));
    let start = Instant::now();
    let handle = rupert_core::indexer::spawn(
        db.clone(),
        rupert_core::text::TextExtractor::new(Some(&lib)),
        move |p| {
            if p.active {
                *b.lock().unwrap() = true;
                let mut last = l.lock().unwrap();
                if last.elapsed() > Duration::from_secs(5) {
                    eprintln!("  {}/{} …", p.done, p.total);
                    *last = Instant::now();
                }
            } else if *b.lock().unwrap() {
                *i.lock().unwrap() = true;
            }
        },
        || {},
    )
    .unwrap();
    handle.send(Job::SyncAll(vec![Source {
        id: "test".into(),
        name: "test".into(),
        root: folder,
        provider: Provider::Local,
        show_all_types: false,
        download_online_only: false,
    }]));
    while !*idle.lock().unwrap() {
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("indexed in {:.1}s", start.elapsed().as_secs_f64());

    let index = Index::open(&db).unwrap();
    println!("{:?}", index.stats().unwrap());
    for q in queries {
        let t = Instant::now();
        let hits = index
            .search(&SearchQuery {
                text: q.clone(),
                limit: 5,
                ..Default::default()
            })
            .unwrap();
        println!(
            "\n\"{q}\" → {} shown ({} µs)",
            hits.len(),
            t.elapsed().as_micros()
        );
        for h in hits {
            let snip = h
                .snippet
                .unwrap_or_default()
                .replace(['\u{E000}', '\u{E001}'], "*")
                .replace('\n', " ");
            println!(
                "  {}  | {}",
                h.rel_path,
                snip.chars().take(90).collect::<String>()
            );
        }
    }
}
