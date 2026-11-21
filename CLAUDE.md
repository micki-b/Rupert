# CLAUDE.md

Rupert is its own git repo (github.com/micki-b/Rupert, private). The parent `~/Code/CLAUDE.md` describes a different project; ignore it here.

Read `docs/DESIGN.md` first. It records the product decisions and the metadata layering, and those are settled.

## Rules that shape the code

- **Disk is the source of truth.** Rupert never owns files. The SQLite index and the `.rupert/catalog/` cache must always be rebuildable from the files.
- **Metadata lives in the file** (PDF: XMP + Info dict) or, for formats that can't hold it, in a sidecar under the directory's hidden `.rupert/` folder. Don't add metadata that exists only in the database.
- **Never read the contents of online-only files** (iCloud/OneDrive placeholders). Any read starts a download. The one exception is a source with "download online-only files to index them" turned on (`indexer` download queue + `cloud.rs`), which downloads, indexes and then evicts.
- **AI is local by default** ("Local AI": an OpenAI-compatible server such as LM Studio or Ollama, on this computer or the local network, where `llm::reach` decides what counts; or Apple Intelligence on the Mac, `apple_ai`). OCR and inbox suggestions never use a cloud service. Name it "Local AI" in the app and docs, not a particular server. The one exception is the chat assistant, which may use a cloud model (Claude, or an OpenAI-compatible service) only when the user chooses it in Settings ▸ Assistant; the UI must say so whenever one is active. API keys go in the system keychain, never in settings files. The assistant's tools stay read-only plus "show this in the window".
- Logic goes in `crates/rupert-core` (testable, no Tauri). `src-tauri` commands stay thin.
- PDFium is loaded once per process and lives on one service thread: create one `TextExtractor` and share clones; never call `TextExtractor::new` with a library path twice.
- Never point a test at the user's live app data or inbox. Use copies, and for UI checks a separate instance (`tauri dev --config '{"identifier":"io.github.micki-b.rupert-test", …}'`).
- Prefer permissive licenses. No AGPL (MuPDF, Ghostscript).
- Use `// MARK: -` section dividers in Rust files.
- SvelteKit 3: its config is in `vite.config.js` (there is no `svelte.config.js`), and `src/lib` is imported as `#lib/…` with the file's extension (`#lib/api.ts`, `#lib/i18n/index.ts`); `$lib` no longer exists.
- The desktop binary is `Rupert` (capital R, so the Dock says "Rupert" under `tauri dev`); bundles rename it to `rupert` (`mainBinaryName`). A `tauri build` on macOS renames `target/debug/Rupert` too (same file on a case-insensitive disk).
- Two macOS editions from one code base: the DMG (default) and the Mac App Store edition (`--features app-store`, `src-tauri/tauri.appstore.conf.json`, sandboxed). Anything that reads outside folders the user picked, runs another program, or updates the app must work in (or be gated off for) the sandbox. Folders are kept with security-scoped bookmarks (`rupert_core::bookmark`, `Source::bookmark`). Lint both: `cargo clippy -p rupert --features app-store`.
- The user help (Help ▸ Rupert Help, F1) is one page per language in `src/lib/help/` (`En.svelte`, `Es.svelte`, `De.svelte`), framed by `src/routes/help/+page.svelte`. When a feature, label or setting changes, update every language's help too.
- Text people read goes in the catalogs, never directly in code: screens in `src/lib/i18n/en.ts` plus every other language's catalog (`npm run check` fails on a missing key), Rust messages through `rupert_core::i18n::tr`/`trf` with an entry in each `i18n/<lang>.rs` (`cargo test` fails without one). Spanish is Rioplatense ("vos"); German is formal ("Sie"), with ß.

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

- The phone app is read-only toward the library (`indexer::Mode::Viewer`), with exactly two exceptions: saving scans into the inbox, and starring folders (writing only that folder's `.rupert/.folder.json`). Keep it that way: no catalog writes, no document metadata edits, Quick Look markup off.
- `tauri ios init` installs and upgrades Homebrew packages. Run it with `HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_UPGRADE=1`, and don't rerun it casually: it regenerates `mobile/gen/apple`, whose build script was edited to set `TAURI_APP_PATH`.
