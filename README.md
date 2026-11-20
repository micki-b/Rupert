# Rupert

Rupert, the librarian: scanned-document management for macOS, Windows and Linux, with an iPhone app. It works directly on top of your own folders, including iCloud Drive, OneDrive and Google Drive.

- Your folders stay the library. Changes made in Finder or Explorer appear in Rupert, and Rupert's changes are ordinary file operations.
- Titles, tags and dates are stored inside the PDFs (XMP), so they go wherever the file goes.
- OCR and AI filing suggestions run locally (AI via LM Studio).
- An "Ask Rupert" chat finds and opens documents for you, with the local model by default, or with Claude or another cloud model if you choose.

Status: **milestones 1–7** (6 for network scanners). Sources with live updates and online-only status; titles, tags, dates, notes and favorites saved inside PDFs (or a hidden sidecar); full-text search; moving documents and folders (drag onto a folder, or Move to…) with their details; an inbox that recognizes the text in new scans (Apple Vision on macOS, Tesseract elsewhere), asks a local LM Studio model how to file them, and renames and moves them once you agree; import from ScanSnap Home; scanning from network (eSCL/AirScan) scanners straight into the inbox; an iPhone app that browses and searches the same folders and scans with the camera into the inbox. See [docs/DESIGN.md](docs/DESIGN.md) for the design and roadmap.

## Install

Download the installer for your system from the [latest release](https://github.com/micki-b/Rupert/releases/latest): a `.dmg` for macOS (Apple silicon and Intel), the `-setup.exe` for Windows, or an `.AppImage` / `.deb` / `.rpm` for Linux. Rupert updates itself from there. Optional: [LM Studio](https://lmstudio.ai) for filing suggestions and the chat assistant, and on Windows and Linux [Tesseract](https://github.com/tesseract-ocr/tesseract) for text recognition.

Rupert is free and open source ([MIT](LICENSE)). It collects nothing about you; see the [privacy policy](PRIVACY.md). The open-source components it includes are listed in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

## Layout

| Path | What |
|---|---|
| `crates/rupert-core/` | Library engine (sources, indexing, metadata). Plain Rust, no Tauri. |
| `src-tauri/` | Desktop shell: Tauri commands that call `rupert-core`. |
| `mobile/` | iPhone app: Tauri commands (`src/`), Swift for iOS services (`plugin/ios/`), Xcode project (`gen/apple/`). |
| `src/` | UI (SvelteKit + TypeScript, static SPA). The phone UI is the `/phone` route. |

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

### iPhone app

Needs Xcode, CocoaPods and the iOS Rust targets (`rustup target add aarch64-apple-ios aarch64-apple-ios-sim`).

```sh
npm run phone -- ios dev                                   # simulator or a connected iPhone
npm run phone -- ios build --debug --target aarch64-sim    # simulator build
npm run phone -- ios build                                 # signed build for an iPhone
```

`TAURI_APP_PATH` (set by `npm run phone`) picks the phone app; plain `npm run tauri` is the desktop app.

On macOS with Homebrew's `rustup`, its shims aren't on `PATH` by default:
`export PATH="/opt/homebrew/opt/rustup/bin:$PATH"`.
