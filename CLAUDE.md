# CLAUDE.md

Rupert is its own git repo (github.com/micki-b/Rupert, private). The parent `~/Code/CLAUDE.md` describes a different project; ignore it here.

Read `docs/DESIGN.md` first. It records the product decisions and the metadata layering, and those are settled.

## Rules that shape the code

- **Disk is the source of truth.** Rupert never owns files. The SQLite index and the `.rupert/catalog/` cache must always be rebuildable from the files.
- **Metadata lives in the file** (PDF: XMP + Info dict) or, for formats that can't hold it, in a sidecar under the directory's hidden `.rupert/` folder. Don't add metadata that exists only in the database.
- **Never read the contents of online-only files** (iCloud/OneDrive placeholders). Any read starts a download.
- **AI = local LM Studio only.** No cloud services for document content.
- Logic goes in `crates/rupert-core` (testable, no Tauri). `src-tauri` commands stay thin.
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
```
