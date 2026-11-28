//! Dev tool: ask Rupert questions on Apple Intelligence over a folder,
//! printing each tool call, the answer and the UI actions. Reads files
//! only (indexes into a throwaway database).
//! Usage: cargo run --release -p rupert-core --example assistant_check -- \
//!     <pdfium-dir> <library-folder> <db> <question>...
//! `RUPERT_LANG=es` (or de) sets Rupert's language; `RUPERT_REPEAT=3` asks
//! each question that many times, each in a new conversation;
//! `RUPERT_MODEL=qwen/qwen3.6-35b-a3b` uses that model in LM Studio
//! (localhost:1234) instead of Apple Intelligence.

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
        offline: false,
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
    if let Some(lang) = std::env::var("RUPERT_LANG")
        .ok()
        .and_then(|c| rupert_core::i18n::Lang::from_code(&c))
    {
        rupert_core::i18n::set(lang);
    }
    let repeat: usize = std::env::var("RUPERT_REPEAT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let questions: Vec<String> = if repeat > 0 {
        args.flat_map(|q| std::iter::repeat_n(q, repeat)).collect()
    } else {
        args.collect()
    };
    let backend = match std::env::var("RUPERT_MODEL") {
        Ok(model) => Backend::OpenAiCompatible {
            base_url: "http://localhost:1234/v1".into(),
            model,
            api_key: None,
        },
        Err(_) => Backend::Apple,
    };
    let mut chat = Chat::new(assistant::system_prompt("2026-11-20"));
    for question in questions {
        if repeat > 0 {
            chat = Chat::new(assistant::system_prompt("2026-11-20"));
        }
        let tools = LibraryTools::new(&index, &sources);
        let t = Instant::now();
        println!("\n> {question}");
        let reply = chat.send(
            &backend,
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
