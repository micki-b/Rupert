# CLAUDE.md

Rupert is its own git repo (github.com/micki-b/Rupert, private). The parent `~/Code/CLAUDE.md` describes a different project; ignore it here.

Read `docs/DESIGN.md` first. It records the product decisions and the metadata layering, and those are settled.

## Rules that shape the code

- **Disk is the source of truth.** Rupert never owns files. The SQLite index and the `.rupert/catalog/` cache must always be rebuildable from the files.
- **Metadata lives in the file** (PDF: XMP + Info dict) or, for formats that can't hold it, in a sidecar under the directory's hidden `.rupert/` folder. Don't add metadata that exists only in the database.
- **Never read the contents of online-only files** (iCloud/OneDrive placeholders). Any read starts a download. The one exception is a source with "download online-only files to index them" turned on (`indexer` download queue + `cloud.rs`), which downloads, indexes and then evicts.
- **AI = local LM Studio only.** No cloud services for document content.
- Logic goes in `crates/rupert-core` (testable, no Tauri). `src-tauri` commands stay thin.
- PDFium is loaded once per process and lives on one service thread: create one `TextExtractor` and share clones; never call `TextExtractor::new` with a library path twice.
- Never point a test at the user's live app data or inbox. Use copies, and for UI checks a separate instance (`tauri dev --config '{"identifier":"io.github.micki-b.rupert-test", …}'`).
- Prefer permissive licenses. No AGPL (MuPDF, Ghostscript).
- Use `// MARK: -` section dividers in Rust files.

## Commands

```sh
export PATH="/opt/homebrew/opt/rustup/bin:$PATH"   # Homebrew rustup on this Mac
./scripts/fetch-pdfium.sh   # needed before any build (bundled resource)
cargo test --workspace
npm run check
npm run tauri dev
npm run tauri build
npm run phone -- ios build --debug --target aarch64-sim   # iPhone app (mobile/), simulator
```

- The phone app is read-only toward the library (`indexer::Mode::Viewer`). Keep it that way: no catalog writes, no metadata edits, Quick Look markup off.
- `tauri ios init` installs and upgrades Homebrew packages. Run it with `HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_UPGRADE=1`, and don't rerun it casually: it regenerates `mobile/gen/apple`, whose build script was edited to set `TAURI_APP_PATH`.
