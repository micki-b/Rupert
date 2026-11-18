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

- **Status: not built yet.** Planned after milestone 3: a correct two-way sync has to tell whether the Finder side or the file changed last, which needs the index.
- When Rupert writes tags, it also writes Finder tags. When a Finder tag changes (FSEvents reports xattr changes), Rupert copies the change into the file's metadata, so Windows and Linux see it too.
- Finder tag colors are kept in the Finder only. Rupert tags have no color.
- **To verify:** that setting xattrs on online-only (dataless) files doesn't trigger a download, and whether OneDrive preserves Finder tags. (iCloud Drive syncs them between Macs.) If the sync can't be made reliable, Rupert leaves Finder tags alone; consistent metadata across platforms comes first.

### Embedded (PDF, and later JPEG/PNG/TIFF/HEIC)

Rupert writes **XMP** (the standard metadata format that Acrobat, Preview, exiftool, DEVONthink and others read). For older readers it also mirrors the core fields into the PDF **Info dictionary**.

| Field | XMP | PDF Info dict |
|---|---|---|
| Title | `dc:title` | `/Title` |
| Tags | `dc:subject` (bag) + `pdf:Keywords` | `/Keywords` (comma-separated) |
| Notes | `dc:description` | `/Subject` |
| Document date (date on the paper) | `rupert:documentDate` (`YYYY-MM-DD`) | — |
| Rupert document ID (stable UUID, assigned on first save) | `rupert:id` | — |
| Favorite | `rupert:favorite` | — |
| (bookkeeping) | `xmp:MetadataDate` updated on every save | — |
| Favorite, source, scan info | `rupert:*` | — |

- Prefer **incremental updates** (appending to the PDF rather than rewriting it). This keeps the original bytes intact and doesn't break digital signatures.
- The embedded `rupert:id` lets Rupert recognize a document after it has been moved or renamed outside the app.
- Rupert's XMP namespace is `https://github.com/micki-b/Rupert/ns/1.0/` (prefix `rupert`).
- Saving removes every property Rupert manages from the existing XMP (wherever it appears) and appends one `rdf:Description` with the new values; everything else other software wrote is kept. Clearing a field in Rupert removes it from the file.
- Before anything is appended, the complete updated file is built in memory, parsed again and checked to read back the same values. If the file changed on disk in the meantime (e.g. a sync), the save is refused. A failed append is truncated back to the original length.
- Writing an online-only file downloads it first (the OS does this when Rupert reads it). Reading details of an online-only PDF only happens when the user asks.
- Writing metadata changes the file, so the sync client re-uploads it. To write to an online-only file, Rupert has to download it first.

### Sidecars (formats that can't hold metadata)

- A sidecar holds up to three things:
  - **Metadata** for formats that can't hold it (and PDFs that can't be changed safely). This copy is the master.
  - **Pending metadata** for a PDF that was online-only when the details were recorded (e.g. by the ScanSnap Home import), so nothing had to be downloaded. Fields set there take precedence over the PDF's own; they move into the PDF the next time it's saved in Rupert.
  - **Text** from elsewhere (ScanSnap Home's OCR) for files with no text layer, so they can be searched even while online-only.
- One hidden `.rupert/` folder per directory, holding one JSON sidecar per file. This is one hidden folder per directory instead of a dotfile next to every document, which matters on Windows, where dotfiles are visible. Rupert also sets the Windows hidden attribute.
- Each sidecar stores the file's content hash. If someone moves the file without its sidecar, Rupert reattaches the sidecar when the file turns up elsewhere.
- The indexer notices sidecar changes (including ones synced from another machine): the watcher maps `.rupert/<file>.json` to its document, and the index stamp includes the sidecar's modification time.
- **To verify:** that iCloud Drive and OneDrive reliably sync dot-folders on all three platforms.

## Index, watching, online-only files

- **Providers** are detected from the path and, on macOS, from the folder's File Provider domain (`com.apple.file-provider-domain-id` xattr on it or a parent). That catches `~/Documents` and `~/Desktop` when iCloud's "Desktop & Documents Folders" is on. Saved sources are re-checked at startup.

- The local index is a SQLite database (`index.sqlite`, FTS5 full-text search) in the app-data folder, keyed by source + relative path. A schema change simply discards and rebuilds it.
- A background thread keeps it current: a full pass at startup (only files whose size, modification time or download state changed are re-read), a folder pass whenever the watcher reports a change, and a single-file pass after Rupert saves metadata. Each file is processed in isolation (`catch_unwind`; the release build unwinds instead of aborting), so one malformed PDF can't crash the app.
- Text comes from PDFium for PDFs (bundled per platform by `scripts/fetch-pdfium.sh`; without it PDFs are indexed by name and details only) and directly from plain-text files. Capped at 1,500 pages / 1 MB of text per document.
- Searching matches every word typed (the last one as a prefix), ignores accents, and ranks title > name > tags > notes > body text.
- Changes on disk are picked up by watching the folders (the `notify` crate: FSEvents, ReadDirectoryChangesW, inotify), with a full rescan on startup to catch anything missed.
- **Online-only files:** Rupert must not read their contents, because any read triggers a download. Name, size and dates come from the filesystem. The download state comes from macOS File Provider / iCloud keys and Windows Cloud Files attributes (`RECALL_ON_DATA_ACCESS` / `OFFLINE`).
- **The catch:** embedded metadata and full text can't be read without downloading. Fix (approved): a shared catalog in a hidden `.rupert/catalog/` folder at each source root, holding one small file per document with its metadata and extracted text. It syncs with the documents, so a second machine can show and search online-only files without downloading them. It's a cache (layer 4), not the source of truth, and the one-file-per-document layout avoids sync conflicts between machines.
  - Entries are gzip'd JSON named by a hash of the document's relative path (`.rupert/catalog/ab/ab12….json.gz`) and are trusted only if size and modification time (±2 s) still match.
  - Written only for cloud sources (OneDrive, iCloud, Google Drive, Dropbox), only for documents with text or details, and only when the content changed — so two machines that agree never re-upload.
  - After a full pass, entries for deleted documents and sync-conflict copies are removed.
- **Finding (2026-11-18):** on the MacBook Pro, 18,463 of 19,421 OneDrive documents are online-only. The catalog only helps if *some* machine has the files downloaded, so on this machine most documents are searchable by name and details only. **Built:** an opt-in per source, "Download online-only files to index them" (in the source's ⋯ menu, cloud sources only):
  - After regular indexing is done, the indexer downloads online-only PDFs and text files whose text is unknown, 6 at a time (a download is ~2–5 s of mostly waiting, so parallel fetches help), skipping files over 100 MB.
  - Each file is indexed and recorded in the catalog, even when it has no text, so no machine downloads it again. Then its space is freed again: macOS `NSFileManager.evictUbiquitousItem` (tested on iCloud Drive and OneDrive; the modification time is unchanged, so the catalog entry stays valid), Windows by setting `FILE_ATTRIBUTE_UNPINNED`, which OneDrive honors. Files in sources without a catalog stay downloaded.
  - Regular jobs always come first; each file is tried at most once per session; the status bar shows the progress.

## Scanning and intake

### Pipeline

Inbox folder → OCR if the PDF has no text layer → LM Studio suggests title, document date, tags and destination folder → user reviews and accepts → file is renamed, metadata written, file moved. **Built in milestone 4.**

- **Inbox:** a folder inside one of the sources (Settings; e.g. the folder a ScanSnap Home profile saves to). A background thread (`intake.rs`) processes new files one at a time and caches the results by size and modification time in the app's data folder, so nothing is redone after a restart.
- **OCR** (`ocr.rs`, `text_layer.rs`) adds an invisible text layer to every PDF page without text, so other programs can search it too. Scanned images get their recognized text in their sidecar instead.
  - Engines, all local: **Apple Vision** on macOS (built in: nothing to bundle, better than Tesseract on scans, detects the language per text block), and the **`tesseract` command** on Windows and Linux when installed. This replaces the earlier plan of bundling Tesseract, whose native libraries are awkward to ship on all three platforms.
  - Pages are rendered by PDFium at up to 300 dpi; word boxes are mapped back to PDF coordinates with PDFium's own pixel-to-page transform, so rotated and cropped pages work.
  - The layer uses a "glyphless" font as Tesseract does: Type 0, Identity-H (UTF-16 code units), an embedded 600-byte TrueType program whose glyphs are empty, fixed widths and a ToUnicode map. A Type 3 font was tried first; PDFium and Poppler read it but **PDFKit (Preview, Spotlight) ignores Type 3 text**, so it was replaced.
  - Written as an incremental update like metadata, checked before writing (the updated file must parse and PDFium must read back ≥ 90 % of the words). Tested on real ScanSnap scans: original bytes intact, pages render identically, text found by PDFKit, PDFium and Poppler, selection boxes line up with the printed words.
  - PDFium can be loaded once per process and is then tied to its thread, so it lives on one service thread (`text.rs`); the indexer and the intake share a handle.
- **LM Studio** (`llm.rs`) via its OpenAI-compatible server on `localhost:1234`, with a JSON schema for the answer. If it isn't running, the review simply has no suggestion.
  - The model sees the start of the text, the file name, the current year (not today's date, which models copy as the document date), the tags in use, and up to 12 **folders where similar documents live** — found by a full-text OR query of the document's most telling words against the index — so the prompt stays small even with 1,200+ folders. It may only pick one of those folders.
  - Some reasoning models return the JSON in `reasoning_content` instead of `content`; both are read.
  - With qwen3.6-35b-a3b: 1–5 s per document; titles, dates and folders were right on 7 of 8 real scans.
- **Review and filing:** the inbox view shows the list, a page preview and a form prefilled from the suggestion (title, date, tags, folder, file name from a pattern, default `{date} {title}`). **File** writes the details into the file, then renames and moves it (`fileops.rs`: sidecar moves along, never overwrites — " (2)" is added — copy + delete across volumes).

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

Every ScanSnap Home folder records its full disk path (`ZINDEXPATH`, a `file://` URL), and document dates are stored as local midnight (read back with SQLite's `localtime`). The OCR text lives in `ScanSnapHomeSearch_V2.db` (`TextTbl`, joined on `contentId = ZCONTENTID`).

**The importer (built, `rupert_core::scansnap`)**, from a temporary copy of both databases:

- Imports only what isn't already in the files: the document date when it differs from the file's creation date, the favorite flag, receipts (tag "Receipt" plus a notes line such as "Receipt: CVS, 20.50 USD (tax 0.92)"), and OCR text unless the file's own text is already indexed. Fields the user has set are never overwritten, and running it again changes nothing.
- Finds each file by folder path + name. Renamed files are matched by size + type within the same folder, moved files by size + type across Rupert's index. A match must be unique, and never a file that ScanSnap Home knows under another name.
- Local PDFs get the details embedded; other formats use their sidecar. Online-only files are never read: details go into the sidecar as pending, text into the sidecar.
- A review screen comes first (counts, renamed matches, files not found, folders outside Rupert's sources with an "Add as source" button). It is reached from File ▸ Import from ScanSnap Home… on macOS, or the sidebar.
- **Run on the real library 2026-11-18:** 1,498 documents in Documents and OneDrive/Boating; 223 PDFs updated (all still readable with Poppler); none of the online-only files were downloaded. 3 items couldn't be found. Content-searchable documents went from 1,211 to 3,122.

## Still to verify

- iCloud Drive and OneDrive sync `.rupert/` dot-folders on all platforms.
- Finder-tag writes on online-only files don't trigger downloads; whether OneDrive keeps Finder tags.
- HP OfficeJet 250 supports eSCL/AirScan.
- ScanSnap Home can save to a folder (or send to an app) without also cataloging the file.

## Milestones

0. ✅ Tauri skeleton, CI builds for all three OSes.
1. ✅ Sources: add folders, browse trees, watch for changes, show online-only status. Folders are listed lazily (one level at a time), so there is no full index yet; that arrives with search in milestone 3.
2. ✅ Metadata: read/write XMP + Info dict, sidecars, tags, `rupert:id` identity. Tested on 26 real PDFs (Poppler, PDFKit and Spotlight read the results; original bytes and rendering unchanged). Finder-tag sync deferred until after milestone 3.
3. ✅ Search: text extraction (PDFium), FTS5 index, shared catalog, search bar, Favorites and Tags in the sidebar. Whole OneDrive (19,421 documents) walked in 32 s; searches take 4–30 ms.
4. ✅ Intake: inbox, OCR (Apple Vision / Tesseract) with an invisible text layer, LM Studio suggestions, review screen, renaming and moving.
5. ✅ ScanSnap Home import (done before 4): document dates, favorites, receipts and OCR text, with a review screen; sidecars can hold pending details and imported text.
6. Direct scanning (eSCL first, then ScanSnap).
7. Phone viewer.

## Dependencies (licenses)

`notify` (CC0/Artistic), `rusqlite` + SQLite FTS5 (MIT/public domain), `lopdf` (MIT), `pdfium-render` + PDFium (MIT/Apache, BSD), pdf.js viewer (Apache-2.0), `ureq` (MIT/Apache, LM Studio client), `png` (MIT/Apache), `objc2-*` (MIT; Apple Vision, file eviction), Tesseract (Apache-2.0, optional, not bundled). Avoid MuPDF and Ghostscript (AGPL).
