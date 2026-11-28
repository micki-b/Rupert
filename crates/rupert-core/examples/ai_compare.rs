//! Dev tool: compare filing suggestions from different models on documents
//! that are already filed, scoring folder, date and tags against where and
//! how they really are. Reads files only (indexes into a throwaway database).
//!
//! Usage: cargo run --release -p rupert-core --example ai_compare -- \
//!     <pdfium-dir> <library-folder> <db> <max-docs> <backend>...
//!
//! A backend is `apple` (Apple Intelligence) or `<url>|<model>` for Local AI,
//! e.g. `http://localhost:11434|qwen3:4b`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rupert_core::llm::{self, AppleAi, LocalAi};
use rupert_core::{Index, Job, Provider, SearchQuery, Source};

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = PathBuf::from(args.next().expect("pdfium dir"));
    let folder = PathBuf::from(args.next().expect("library folder"));
    let db = PathBuf::from(args.next().expect("db path"));
    let max: usize = args.next().expect("max docs").parse().unwrap();
    let backends: Vec<String> = args.collect();
    let name = folder.file_name().unwrap().to_string_lossy().to_string();

    index(&lib, &folder, &db, &name);
    let index = Index::open(&db).unwrap();
    let mut docs = index
        .search(&SearchQuery {
            limit: 1000,
            ..Default::default()
        })
        .unwrap();
    docs.retain(|d| d.rel_path.contains('/') && !d.rel_path.starts_with("Inbox/"));
    docs.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    // An even spread across folders.
    let step = (docs.len() / max.max(1)).max(1);
    let docs: Vec<_> = docs.into_iter().step_by(step).take(max).collect();
    let tags = index.top_tags(150).unwrap();
    println!("{} documents, backends: {backends:?}\n", docs.len());

    let mut score: BTreeMap<String, [f64; 6]> = BTreeMap::new(); // folder, date, tags, secs, errors, tag precision
    for d in &docs {
        let text = index
            .document_text(&d.source_id, &d.rel_path)
            .unwrap()
            .unwrap_or_default();
        let folders: Vec<String> = index
            .similar_folders(&text, Some((&d.source_id, &d.rel_path)), 12)
            .unwrap()
            .into_iter()
            .map(|(_, dir, _)| {
                if dir.is_empty() {
                    name.clone()
                } else {
                    format!("{name}/{dir}")
                }
            })
            .collect();
        let actual_folder = format!(
            "{name}/{}",
            d.rel_path.rsplit_once('/').map(|(p, _)| p).unwrap_or("")
        );
        let req = llm::Request {
            text: &text,
            file_name: "Scan 2026-11-20 09.41.pdf",
            year: "2026",
            folders: &folders,
            tags: &tags,
        };
        println!(
            "■ {}  [{}; {}; {}]",
            d.rel_path,
            d.title.as_deref().unwrap_or("-"),
            d.document_date.as_deref().unwrap_or("-"),
            d.tags.join(", ")
        );
        for b in &backends {
            let t = Instant::now();
            let result = if b == "apple" {
                AppleAi.suggest(&req)
            } else {
                let (url, model) = b.split_once('|').expect("url|model");
                LocalAi {
                    url: url.into(),
                    model: Some(model.into()),
                }
                .suggest(&req)
            };
            let secs = t.elapsed().as_secs_f64();
            let s = score.entry(b.clone()).or_default();
            s[3] += secs;
            match result {
                Ok(sg) => {
                    let folder_ok = sg.folder.as_deref() == Some(actual_folder.as_str());
                    let date_ok = d.document_date.is_some() && sg.document_date == d.document_date;
                    let tag_ok = sg
                        .tags
                        .iter()
                        .any(|t| d.tags.iter().any(|a| a.eq_ignore_ascii_case(t)));
                    s[0] += f64::from(u8::from(folder_ok));
                    s[1] += f64::from(u8::from(date_ok));
                    s[2] += f64::from(u8::from(tag_ok));
                    let right = sg
                        .tags
                        .iter()
                        .filter(|t| d.tags.iter().any(|a| a.eq_ignore_ascii_case(t)))
                        .count();
                    s[5] += if sg.tags.is_empty() {
                        f64::from(u8::from(d.tags.is_empty()))
                    } else {
                        right as f64 / sg.tags.len() as f64
                    };
                    println!(
                        "  {:<28} {:>5.1}s {}{}{} {} | {} | {} | {}",
                        short(b),
                        secs,
                        mark(folder_ok),
                        mark(date_ok),
                        mark(tag_ok),
                        sg.title.unwrap_or_default(),
                        sg.document_date.unwrap_or_default(),
                        sg.tags.join(", "),
                        sg.folder.unwrap_or_default(),
                    );
                }
                Err(e) => {
                    s[4] += 1.0;
                    println!("  {:<28} {:>5.1}s error: {e}", short(b), secs);
                }
            }
        }
    }
    let n = docs.len() as f64;
    println!("\nbackend                       folder  date  tags  tag precision  avg s  errors");
    for (b, s) in &score {
        println!(
            "{:<28} {:>5.0}% {:>4.0}% {:>4.0}% {:>13.0}% {:>6.1} {:>6}",
            short(b),
            100.0 * s[0] / n,
            100.0 * s[1] / n,
            100.0 * s[2] / n,
            100.0 * s[5] / n,
            s[3] / n,
            s[4]
        );
    }
}

fn short(b: &str) -> &str {
    b.split_once('|').map_or(b, |(_, m)| m)
}

fn mark(ok: bool) -> char {
    if ok { '✓' } else { '·' }
}

/// Indexes `folder` into `db` and waits until it's done.
fn index(lib: &Path, folder: &Path, db: &Path, name: &str) {
    let idle = Arc::new(Mutex::new(false));
    let busy = Arc::new(Mutex::new(false));
    let (i, b) = (Arc::clone(&idle), Arc::clone(&busy));
    let handle = rupert_core::indexer::spawn(
        db.to_path_buf(),
        rupert_core::text::TextExtractor::new(Some(lib)),
        move |p| {
            if p.active {
                *b.lock().unwrap() = true;
            } else if *b.lock().unwrap() {
                *i.lock().unwrap() = true;
            }
        },
        || {},
    )
    .unwrap();
    handle.send(Job::SyncAll(vec![Source {
        id: "test".into(),
        name: name.into(),
        root: folder.to_path_buf(),
        provider: Provider::Local,
        show_all_types: false,
        download_online_only: false,
        offline: false,
        bookmark: None,
    }]));
    let start = Instant::now();
    while !*idle.lock().unwrap() && start.elapsed() < Duration::from_secs(120) {
        std::thread::sleep(Duration::from_millis(100));
    }
}
