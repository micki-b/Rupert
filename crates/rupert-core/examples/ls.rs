//! Dev tool: list a folder the way Rupert sees it (never reads file contents).
//! Usage: cargo run -p rupert-core --example ls -- <folder> [--all]

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args.next().expect("usage: ls <folder> [--all]");
    let all = args.next().as_deref() == Some("--all");
    let entries = rupert_core::list_dir(std::path::Path::new(&dir), "", all).expect("list failed");
    for e in &entries {
        let cloud = match e.cloud {
            rupert_core::CloudState::Local => "      ",
            rupert_core::CloudState::CloudOnly => "cloud ",
        };
        let kind = e
            .kind
            .map(|k| format!("{k:?}"))
            .unwrap_or_else(|| "folder".into());
        println!("{cloud}{kind:<7} {:>10}  {}", e.size, e.name);
    }
    let cloud = entries
        .iter()
        .filter(|e| e.cloud == rupert_core::CloudState::CloudOnly)
        .count();
    eprintln!("{} entries, {cloud} online-only", entries.len());
}
