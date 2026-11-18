//! Dev tool: copy every PDF in <in-dir> to <out-dir>, write test metadata
//! into each copy, and report timings and results. Never touches <in-dir>.
//! Usage: cargo run --release -p rupert-core --example meta_check -- <in-dir> <out-dir>

use std::time::Instant;

use rupert_core::{DocMeta, read_meta, write_meta};

fn main() {
    let mut args = std::env::args().skip(1);
    let input = std::path::PathBuf::from(args.next().expect("in-dir"));
    let output = std::path::PathBuf::from(args.next().expect("out-dir"));
    std::fs::create_dir_all(&output).unwrap();

    let mut files: Vec<_> = std::fs::read_dir(&input)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")))
        .collect();
    files.sort();

    let meta = DocMeta {
        title: Some("Rupert test – Élan & <ok>".into()),
        tags: vec!["Boating".into(), "Rupert-Test".into()],
        notes: Some("Written by meta_check".into()),
        document_date: Some("2026-11-18".into()),
        favorite: true,
        ..Default::default()
    };

    println!("file\tsize_mb\tread_ms\twrite_ms\tstorage\tresult");
    for src in files {
        let name = src.file_name().unwrap();
        let dst = output.join(name);
        std::fs::copy(&src, &dst).unwrap();
        let size = std::fs::metadata(&dst).unwrap().len() as f64 / 1e6;

        let t = Instant::now();
        let before = read_meta(&dst);
        let read_ms = t.elapsed().as_millis();

        let t = Instant::now();
        let result = write_meta(&dst, meta.clone(), true);
        let write_ms = t.elapsed().as_millis();

        let (storage, outcome) = match (&before, &result) {
            (_, Ok(rec)) => {
                let mut expect = meta.clone();
                expect.id = rec.meta.id.clone();
                let ok = rec.meta == expect;
                (
                    format!("{:?}", rec.storage),
                    if ok {
                        format!(
                            "ok{}",
                            rec.note
                                .as_ref()
                                .map(|n| format!(" ({n})"))
                                .unwrap_or_default()
                        )
                    } else {
                        format!("MISMATCH {:?}", rec.meta)
                    },
                )
            }
            (_, Err(e)) => ("-".into(), format!("ERROR {e}")),
        };
        println!(
            "{}\t{size:.1}\t{read_ms}\t{write_ms}\t{storage}\t{outcome}",
            name.to_string_lossy()
        );
    }
}
