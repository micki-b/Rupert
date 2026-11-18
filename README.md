# Rupert

Rupert, the librarian: scanned-document management for macOS, Windows and Linux. It works directly on top of your own folders, including OneDrive and iCloud Drive.

- Your folders stay the library. Changes made in Finder or Explorer appear in Rupert, and Rupert's changes are ordinary file operations.
- Titles, tags and dates are stored inside the PDFs (XMP), so they go wherever the file goes.
- OCR and AI filing suggestions run locally (AI via LM Studio).

Status: **milestone 3**. You can add folders as sources, browse them with live updates and online-only status, and edit a document's title, tags, document date, notes and favorite flag. Details are saved inside PDFs, or in a hidden sidecar file for other formats. Full-text search across all sources, plus Favorites and Tags. See [docs/DESIGN.md](docs/DESIGN.md) for the design and roadmap.

## Layout

| Path | What |
|---|---|
| `crates/rupert-core/` | Library engine (sources, indexing, metadata). Plain Rust, no Tauri. |
| `src-tauri/` | Desktop shell: Tauri commands that call `rupert-core`. |
| `src/` | UI (SvelteKit + TypeScript, static SPA). |

## Development

Requirements: Rust (stable), Node 22+, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS.

```sh
./scripts/fetch-pdfium.sh   # PDF text extraction library (once per machine)
npm install
npm run tauri dev        # run the app with hot reload
cargo test --workspace   # Rust tests
npm run check            # Svelte/TypeScript checks
npm run tauri build      # release bundle(s) in target/release/bundle/
```

On macOS with Homebrew's `rustup`, its shims aren't on `PATH` by default:
`export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
