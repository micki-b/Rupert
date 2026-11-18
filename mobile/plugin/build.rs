// The commands are only called from Rust (the phone app), never from the
// web view, so none are exposed to JavaScript.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).ios_path("ios").build();
}
