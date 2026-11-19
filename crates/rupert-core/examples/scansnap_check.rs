//! Dev tool: dry-run the ScanSnap Home import (reads only, writes nothing),
//! or apply it with `APPLY=1` (only ever on test copies).
//! Usage: cargo run -p rupert-core --example scansnap_check -- <library-dir|default> <source-root>...

use std::path::PathBuf;
use std::time::Instant;

use rupert_core::scansnap;
use rupert_core::{Provider, Source};

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = args.next().expect("library dir or 'default'");
    let lib = if lib == "default" {
        scansnap::default_library().expect("no ScanSnap Home library found")
    } else {
        PathBuf::from(lib)
    };
    let sources: Vec<Source> = args
        .enumerate()
        .map(|(i, root)| {
            let root = PathBuf::from(root);
            Source {
                id: format!("s{i}"),
                name: root.display().to_string(),
                provider: Provider::detect(&root),
                root,
                show_all_types: false,
                download_online_only: false,
                bookmark: None,
            }
        })
        .collect();
    let t = Instant::now();
    let plan = scansnap::plan(&lib, &sources, None).unwrap();
    let mut s = plan.summary.clone();
    println!("planned in {:.1}s", t.elapsed().as_secs_f64());
    let (renamed, nf) = (s.renamed.len(), s.not_found.len());
    for r in s.renamed.iter().take(5) {
        println!("  renamed: {} -> {}", r.from, r.rel_path);
    }
    for p in s.not_found.iter().take(5) {
        println!("  not found: {p}");
    }
    s.renamed.clear();
    s.not_found.clear();
    println!("{s:#?}\nrenamed: {renamed}, not found: {nf}");
    let text_bytes: usize = plan
        .items
        .iter()
        .filter_map(|i| i.text.as_ref())
        .map(|t| t.len())
        .sum();
    println!("text: {:.1} MB", text_bytes as f64 / 1e6);

    if std::env::var("APPLY").as_deref() == Ok("1") {
        let t = Instant::now();
        let (report, touched) = scansnap::apply(&plan, &sources, |_, _| {});
        println!(
            "applied in {:.1}s: {report:#?}\n{} folders touched",
            t.elapsed().as_secs_f64(),
            touched.len()
        );
    }
}
