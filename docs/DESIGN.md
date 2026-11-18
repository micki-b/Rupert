# Rupert — Design

Rupert is a librarian for scanned documents (mainly PDFs) on macOS, Windows and Linux.

## Principles

1. **The folders on disk are the library.** Rupert never owns files. Anything done in Finder, Explorer, a shell or a sync client shows up in Rupert, and anything done in Rupert is a normal file operation.
2. **Metadata travels with the document.** Titles, tags, dates and notes live inside the file wherever the format allows, so another machine, another copy of Rupert, or another program can read them.
3. **Rupert's database is disposable.** The local index is a cache that can be deleted and rebuilt from disk at any time.
4. **Nothing private leaves the machine.** OCR and AI classification run locally (AI through LM Studio).

## Decisions so far

| Area | Decision |
|---|---|
| Platforms | macOS, Windows, Linux. Phone viewer later. |
| Stack | Tauri 2 (Rust core, web UI). Tauri 2 also targets iOS/Android. |
| Sources | Folders already synced to disk (OneDrive, iCloud Drive, local). No cloud APIs. |
| Visibility | User picks folders; subfolders included. Each source is its own tree. |
| File types | Document types only by default (PDF, images, Office, text); per-source setting to show everything. |
| Online-only files | Indexed without downloading (name, path, size, dates at minimum). |
| Metadata | Embedded in the file by default; hidden sidecar for formats that can't hold it. |
| Organization | Folders + tags (one document, many tags). |
| Search | Full-text search across all documents. |
| Auto-naming / filing | Yes, using only a local model via LM Studio. |
| Page editing | Out of scope (no rotate/reorder/merge/split). |
| File operations | Rename/move change the real file right away; delete goes to the system Trash. |
| Migration | Import from ScanSnap Home. |
| License | Personal first, likely open source, so prefer permissive dependencies. |

## Metadata storage

### Layers

| Layer | Where | Role | If it disagrees |
|---|---|---|---|
| 1. In-file metadata | XMP + PDF Info dict inside the file | **Master copy** for formats that support it | Wins |
| 2. Sidecar | `.rupert/<file>.json` in the same directory | **Master copy** for formats that don't | Wins |
| 3. Finder tags (macOS) | `com.apple.metadata:_kMDItemUserTags` xattr | Kept in sync both ways (see below) | Changes made in Finder flow into layer 1/2 |
| 4. Shared catalog | `.rupert/catalog/` at the source root, synced | Cache: metadata + extracted text, so other machines can search online-only files | Rebuilt from 1/2 |
| 5. Local index | SQLite in app data, per machine | Cache for fast browsing and search | Rebuilt from 1/2/4 |

Layers 1 and 2 are the truth. Layers 4 and 5 can be deleted at any time and will be rebuilt.

### Finder tags (macOS only)

- When Rupert writes tags, it also writes Finder tags. When a Finder tag changes (FSEvents reports xattr changes), Rupert copies the change into the file's metadata, so Windows and Linux see it too.
- Finder tag colors are kept in the Finder only. Rupert tags have no color.
- **To verify:** that setting xattrs on online-only (dataless) files doesn't trigger a download, and whether OneDrive preserves Finder tags. (iCloud Drive syncs them between Macs.) If the sync can't be made reliable, Rupert leaves Finder tags alone; consistent metadata across platforms comes first.

### Embedded (PDF, and later JPEG/PNG/TIFF/HEIC)

Rupert writes **XMP** (the standard metadata format that Acrobat, Preview, exiftool, DEVONthink and others read). For older readers it also mirrors the core fields into the PDF **Info dictionary**.

| Field | XMP | PDF Info dict |
|---|---|---|
| Title | `dc:title` | `/Title` |
| Tags | `dc:subject` (bag) | `/Keywords` (comma-separated) |
| Notes | `dc:description` | `/Subject` |
| Document date (date on the paper) | `rupert:documentDate` (+ `dc:date`) | — |
| Rupert document ID (stable UUID) | `rupert:id` | — |
| Favorite, source, scan info | `rupert:*` | — |

- Prefer **incremental updates** (appending to the PDF rather than rewriting it). This keeps the original bytes intact and doesn't break digital signatures.
- The embedded `rupert:id` lets Rupert recognize a document after it has been moved or renamed outside the app.
- Writing metadata changes the file, so the sync client re-uploads it. To write to an online-only file, Rupert has to download it first.

### Sidecars (formats that can't hold metadata)

- One hidden `.rupert/` folder per directory, holding one JSON sidecar per file. This is one hidden folder per directory instead of a dotfile next to every document, which matters on Windows, where dotfiles are visible. Rupert also sets the Windows hidden attribute.
- Each sidecar stores the file's content hash. If someone moves the file without its sidecar, Rupert reattaches the sidecar when the file turns up elsewhere.
- **To verify:** that iCloud Drive and OneDrive reliably sync dot-folders on all three platforms.

## Index, watching, online-only files

- The local index is a SQLite database (with FTS5 full-text search) in the app-data folder. It is keyed by `rupert:id` (or content hash) plus path.
- Changes on disk are picked up by watching the folders (the `notify` crate: FSEvents, ReadDirectoryChangesW, inotify), with a full rescan on startup to catch anything missed.
- **Online-only files:** Rupert must not read their contents, because any read triggers a download. Name, size and dates come from the filesystem. The download state comes from macOS File Provider / iCloud keys and Windows Cloud Files attributes (`RECALL_ON_DATA_ACCESS` / `OFFLINE`).
- **The catch:** embedded metadata and full text can't be read without downloading. Fix (approved): a shared catalog in a hidden `.rupert/catalog/` folder at each source root, holding one small file per document with its metadata and extracted text. It syncs with the documents, so a second machine can show and search online-only files without downloading them. It's a cache (layer 4), not the source of truth, and the one-file-per-document layout avoids sync conflicts between machines.

## Scanning and intake

### Pipeline

Inbox folder → OCR if the PDF has no text layer → LM Studio suggests title, document date, tags and destination folder → user reviews and accepts → file is renamed, metadata written, file moved.

- **OCR** adds an invisible text layer to the PDF, so other programs can search it too. Engine: Tesseract (Apache-2.0), bundled. ScanSnap output already includes OCR text.
- **LM Studio** offers an OpenAI-compatible server on `localhost:1234`. If it isn't running, suggestions are turned off and nothing else breaks.

### Scanners

- **Phase 1: watched inbox.** Any scanner or app that saves to a folder works, e.g. a ScanSnap Home "save to folder" profile or HP Smart.
- **Phase 2: direct scanning.**
  - **HP OfficeJet 250:** eSCL/AirScan over the network if the device supports it (to verify). eSCL is plain HTTP+XML, so one Rust implementation would cover all three OSes. Fallbacks: Windows WIA, macOS ImageCaptureCore, Linux SANE/HPLIP.
  - **ScanSnap iX1600** (USB, scanned from the MacBook Pro): ScanSnap models don't support TWAIN or ISIS.
    - Phase 1: a ScanSnap Home profile saves to Rupert's inbox folder, or uses ScanSnap Home's "send to application" option to hand the file to Rupert. ScanSnap Home stays installed only as the scanner driver.
    - Phase 2: the ScanSnap SDK (Windows/macOS, needs ScanSnap Home 3.3+, iX1600 listed as supported), through a small native macOS bridge.

## Migrating from ScanSnap Home

Source: `~/Library/Application Support/PFU/ScanSnap Home/Managed/ScanSnapHome.sqlite` (Core Data SQLite; read from a copy). Snapshot taken 2026-11-17:

- 10,761 items in 1,242 folders. About 5,200 are PDFs; the rest are mixed (HTML, JPEG, ZIP, EXE/DLL, Office…).
- **Titles:** almost always the filename (only 134 differ, mostly cut off at 64 characters). Nothing to migrate.
- **Document date:** set on 10,753 items; 723 PDFs have a document date more than a day away from the file date. **This must migrate.**
- **Labels / memos / comments:** unused (0).
- **Favorites:** 11. **Receipt fields** (vendor, category, amount): 8 items.
- **Trash:** 17 items.

The importer matches items by folder path + filename and writes document date, favorite flag and receipt fields into each file's metadata.

## Still to verify

- iCloud Drive and OneDrive sync `.rupert/` dot-folders on all platforms.
- Finder-tag writes on online-only files don't trigger downloads; whether OneDrive keeps Finder tags.
- HP OfficeJet 250 supports eSCL/AirScan.
- ScanSnap Home can save to a folder (or send to an app) without also cataloging the file.

## Milestones

0. ✅ Tauri skeleton, CI builds for all three OSes.
1. ✅ Sources: add folders, browse trees, watch for changes, show online-only status. Folders are listed lazily (one level at a time), so there is no full index yet; that arrives with search in milestone 3.
2. Metadata: read/write XMP + Info dict, sidecars, tags, `rupert:id` identity.
3. Search: text extraction (pdfium), FTS5, shared catalog.
4. Intake: inbox, OCR, LM Studio suggestions, review screen.
5. ScanSnap Home import.
6. Direct scanning (eSCL first, then ScanSnap).
7. Phone viewer.

## Dependencies (licenses)

`notify` (CC0/Artistic), `rusqlite` + SQLite FTS5 (MIT/public domain), `lopdf` (MIT), `pdfium-render` + PDFium (MIT/Apache, BSD), pdf.js viewer (Apache-2.0), Tesseract (Apache-2.0). Avoid MuPDF and Ghostscript (AGPL).
