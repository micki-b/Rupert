//! Dev tool: ask Rupert questions on Apple Intelligence over a folder,
//! printing each tool call, the answer and the UI actions. Reads files
//! only (indexes into a throwaway database).
//! Usage: cargo run --release -p rupert-core --example assistant_check -- \
//!     <pdfium-dir> <library-folder> <db> <question>...

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rupert_core::assistant::{self, Backend, Cancel, Chat, LibraryTools, ToolBox, ToolSpec};
use rupert_core::{Index, Job, Provider, Source};
use serde_json::Value;

/// Prints each tool call's input and the start of its result.
struct Logged<'a>(&'a LibraryTools<'a>);

impl ToolBox for Logged<'_> {
    fn specs(&self) -> Vec<ToolSpec> {
        self.0.specs()
    }
    fn call(&self, name: &str, input: &Value) -> String {
        let out = self.0.call(name, input);
        println!(
            "    {name} {input} → {}",
            out.chars().take(160).collect::<String>()
        );
        out
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let lib = PathBuf::from(args.next().expect("pdfium dir"));
    let folder = PathBuf::from(args.next().expect("library folder"));
    let db = PathBuf::from(args.next().expect("db path"));
    let source = Source {
        id: "test".into(),
        name: folder.file_name().unwrap().to_string_lossy().into(),
        root: folder,
        provider: Provider::Local,
        show_all_types: false,
        download_online_only: false,
        bookmark: None,
    };
    let idle = Arc::new(Mutex::new(false));
    let busy = Arc::new(Mutex::new(false));
    let (i, b) = (Arc::clone(&idle), Arc::clone(&busy));
    let handle = rupert_core::indexer::spawn(
        db.clone(),
        rupert_core::text::TextExtractor::new(Some(&lib)),
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
    handle.send(Job::SyncAll(vec![source.clone()]));
    let start = Instant::now();
    while !*idle.lock().unwrap() && start.elapsed() < Duration::from_secs(120) {
        std::thread::sleep(Duration::from_millis(100));
    }
    let index = Index::open(&db).unwrap();
    let sources = [source];
    let mut chat = Chat::new(assistant::system_prompt("2026-11-20"));
    for question in args {
        let tools = LibraryTools::new(&index, &sources);
        let t = Instant::now();
        println!("\n> {question}");
        let reply = chat.send(
            &Backend::Apple,
            &Logged(&tools),
            &question,
            &Cancel::new(),
            &mut |s| {
                println!("  · {} ({})", s.summary, s.tool);
            },
        );
        match reply {
            Ok(r) => println!("  = {} [{:.1}s]", r.text, t.elapsed().as_secs_f64()),
            Err(e) => println!("  ! {e}"),
        }
        println!("  actions: {:?}", tools.take_actions());
        println!("  suggestions: {:?}", tools.take_suggestions());
    }
}
