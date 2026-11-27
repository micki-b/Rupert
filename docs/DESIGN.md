# Rupert — Design

Rupert is a librarian for scanned documents (mainly PDFs) on macOS, Windows and Linux.

## Principles

1. **The folders on disk are the library.** Rupert never owns files. Anything done in Finder, Explorer, a shell or a sync client shows up in Rupert, and anything done in Rupert is a normal file operation.
2. **Metadata travels with the document.** Titles, tags, dates and notes live inside the file wherever the format allows, so another machine, another copy of Rupert, or another program can read them.
3. **Rupert's database is disposable.** The local index is a cache that can be deleted and rebuilt from disk at any time.
4. **Nothing private leaves the machine.** OCR and AI classification run locally (AI through Local AI: an OpenAI-compatible server such as LM Studio or Ollama, on this computer or the local network).

## Decisions so far

| Area | Decision |
|---|---|
| Platforms | macOS, Windows, Linux. Phone viewer later. |
| Stack | Tauri 2 (Rust core, web UI). Tauri 2 also targets iOS/Android. |
| Sources | Folders already synced to disk (OneDrive, iCloud Drive, Google Drive, local). No cloud APIs on the computer; the phone reads OneDrive and Google Drive through their web APIs, since their apps don't share folders through Files. |
| Visibility | User picks folders; subfolders included. Each source is its own tree. |
| File types | Document types only by default (PDF, images, Office, text); per-source setting to show everything. |
| Online-only files | Indexed without downloading (name, path, size, dates at minimum). |
| Metadata | Embedded in the file by default; hidden sidecar for formats that can't hold it. |
| Organization | Folders + tags (one document, many tags). |
| Search | Full-text search across all documents. |
| Auto-naming / filing | Yes, using only a local model (Local AI: LM Studio, Ollama, …; generalized from LM Studio-only in #26). |
| Chat assistant | Local AI by default; a cloud model (Claude, or OpenAI-compatible) only if the user picks one (decided 2026-11-18). |
| Page editing | Out of scope (no rotate/reorder/merge/split). |
| File operations | Rename/move change the real file right away; delete goes to the system Trash / Recycle Bin (built: right-click ▸ Move to Trash, ⌘⌫ / Delete), taking the sidecar along. Moving is built: drag onto a folder, or right-click ▸ Move to…. |
| Migration | Import from ScanSnap Home. |
| License | Personal first, likely open source, so prefer permissive dependencies. |
| Languages | English, Spanish (Rioplatense, "vos"; used for every Spanish system language) and German (Germany, formal "Sie", with ß; used for every German system language) (decided 2026-11-20). The system's language unless Settings ▸ Language chooses one. Names on disk never depend on the language. |

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

Beside them, a source's `.rupert/duplicates.json` (at its root, synced) records the duplicate groups the user kept as they are (#111's Keep all): each group's key (its content hash, text hash, or size and name) and how many copies it had, so the choice survives renames and moves and a further copy brings the group back. It's a decision, not a cache: Settings ▸ Library ▸ Reset (per source) removes it.

### Finder tags (macOS only)

- **Status: not built yet** (#117). Planned after milestone 3: a correct two-way sync has to tell whether the Finder side or the file changed last, which needs the index.
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
| From (who sent or issued it, #108) | `rupert:sender` | — |
| Amount (#108; a plain decimal with a dot, `1240.50`) | `rupert:amount` | — |
| Currency of the amount (#108; ISO 4217, `EUR`) | `rupert:currency` | — |
| Due date (to pay by, or valid until, #108) | `rupert:dueDate` (`YYYY-MM-DD`) | — |
| (bookkeeping) | `xmp:MetadataDate` updated on every save | — |
| Favorite, source, scan info | `rupert:*` | — |

- Prefer **incremental updates** (appending to the PDF rather than rewriting it). This keeps the original bytes intact and doesn't break digital signatures.
- The embedded `rupert:id` lets Rupert recognize a document after it has been moved or renamed outside the app.
- Rupert's XMP namespace is `https://github.com/micki-b/Rupert/ns/1.0/` (prefix `rupert`).
- From, amount, currency and due date (#108) are `rupert:*` properties, in the sidecar for other formats, and copied into the catalog and the index (schema 7) as caches like the rest. A Rupert older than 0.10 treats unknown `rupert:*` properties as its own, so saving a document's details there drops these four.
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
- **Finding (2026-11-18):** in a large OneDrive library (about 19,000 documents) on a test Mac, 95 % were online-only. The catalog only helps if *some* machine has the files downloaded, so on that machine most documents were searchable by name and details only. **Built:** an opt-in per source, "Download online-only files to index them" (in the source's ⋯ menu, cloud sources only):
  - After regular indexing is done, the indexer downloads online-only PDFs and text files whose text is unknown, 6 at a time (a download is ~2–5 s of mostly waiting, so parallel fetches help), skipping files over 100 MB.
  - Each file is indexed and recorded in the catalog, even when it has no text, so no machine downloads it again. Then its space is freed again: macOS `NSFileManager.evictUbiquitousItem` (tested on iCloud Drive and OneDrive; the modification time is unchanged, so the catalog entry stays valid), Windows by setting `FILE_ATTRIBUTE_UNPINNED`, which OneDrive honors. Files in sources without a catalog stay downloaded.
  - Regular jobs always come first; each file is tried at most once per session; the status bar shows the progress.
- **The inbox (#144)** is the other exception: it works with the files present, so the intake downloads an online-only scan again (reading it through) before anything else, once each time it's online-only; a scan that can't be downloaded keeps a note saying why. Meanwhile its preview is the picture Rupert kept (#62). Settings ▸ Inbox recommends keeping the folder downloaded, with the step for its service; on Windows with OneDrive or Dropbox a button sets it (the documented `FILE_ATTRIBUTE_PINNED`, as "Always keep on this device" does). macOS has no public way for an app to set Finder's Keep Downloaded on another app's folder (`NSFileProviderContentPolicy` is the provider's), so there Rupert only explains it.

## Chat assistant (desktop)

- A chat button in the lower right (⌘J) opens "Ask Rupert": find documents, answer questions about them, and show things in the window ("open the Boat folder", "show everything tagged Taxes and 2025").
- **Engine** (`assistant.rs`): one tool loop for two API shapes: OpenAI-compatible chat completions (Local AI, or a cloud service with an API key) and Anthropic's Messages API over HTTP (no official Rust SDK). At most 8 tool rounds per message; a failed request leaves the conversation as it was.
- **Tools, read-only:** search (text, tags any/all, favorites), list tags, sources and favorite folders, list a folder, read a document's indexed text (from the index: nothing is downloaded), and three window requests (open folder, show document, show results) that the UI carries out. The assistant can't change files; it says so.
- **Models (Settings ▸ Assistant):**
  - **Local AI** (default): the server and model from Settings ▸ Local AI; nothing goes to the internet. An address outside this computer and the local network (`llm::reach`) counts as cloud: ☁ in the panel. Tested with Qwen 3.6 35B: search, read and show all worked, slowly (a minute or two per question, mostly the model's reasoning).
  - **Apple Intelligence** (Local AI on a Mac with macOS 26+): `Backend::Apple`. Apple's framework runs the tool loop itself; the Swift bridge's tools call back into Rust's `ToolBox` (one at a time, while the Rust call blocks). Each turn is a new session, so earlier turns go into the instructions (`recent_turns`), and tool results are cut to fit the short context (8,192 tokens on macOS 27, 4,096 on 26).
  - **Claude:** default `claude-opus-5-5`, adaptive thinking, and server-side refusal fallbacks (`fallbacks: "default"`); assistant turns are echoed back unchanged (thinking included), except reasoning before a fallback boundary, as the API requires. Sign-in with an API key, or with the user's Anthropic account through the `ant` CLI (`ant auth login`, then a short-lived token per message).
  - **OpenAI-compatible service:** address, model and API key.
  - **ChatGPT, with the user's plan** ("Sign in with ChatGPT" plan usage for local and open-source apps, OpenAI preview, added 2026-11-18; `chatgpt.rs`): OAuth with PKCE as a public client, registered dynamically on first sign-in (`client_id=dynamic_agent_client` → an issued `oaiapp_…` id), a per-computer `ext_agent_host_id`, the browser returning to a one-time listener on `127.0.0.1` (port 1455, else any). The ID token's claims are checked (issuer, audience, nonce, the `chatgpt.tokens.use.direct` scope); its signature isn't, since it comes straight from the token endpoint over TLS (OpenID Connect Core 3.1.3.7). Credentials (access token 1 h, rotating 30-day refresh token) are one keychain entry. Requests use the Responses API with `store: false` and `stream: true` (both required), `instructions` for the system prompt, the whole conversation in `input` each time (reasoning items echoed with their encrypted content; other items without ids), function tools; plan-limit errors (`subscription_sharing_*`) get plain messages.
  - **Claude subscriptions** can't be used by other apps (Anthropic's terms); Max, Ultra and Team plans include monthly API credits that Rupert's Claude options spend once linked to a Console organization. The help explains it.
  - **HTTPS:** until 2026-11-18 the desktop build's `ureq` had no TLS (only the phone enabled `rustls`), so the cloud options above failed with "TLS required"; `rustls` is now always on.
  - API keys are kept in the system keychain (`keyring`), never in `settings.json`.
- **Privacy:** with a cloud model, the question and whatever the tools return (titles, tags, folder names, the text of documents the model reads) go to that service. The panel shows "☁" and the model name whenever a cloud model is active, the first screen says what is sent, and Settings repeats it. Inbox suggestions and OCR stay on Local AI and the local OCR engine regardless.

## Views and deleting (desktop)

- **Views** (one choice for all folders, kept in the window's local storage): list; large icons; gallery (the selected document large with page flipping, a filmstrip below). Search results stay a list.
- **Thumbnails** (`thumbs.rs`, served by the `thumb:` URL scheme in `src-tauri/src/thumbs.rs`): a PDF page rendered by PDFium, cached as PNG in the app's cache folder (keyed by path, size, modification time, page and size; trimmed to 150 MB when it passes 200 MB); images are served as they are. Online-only files get none, so nothing is downloaded; the view shows an icon. The scheme only serves files inside a source (`resolve`).
- **Trash** (`fileops::trash`, the `trash` crate): macOS uses `NSFileManager trashItemAtURL` (no Finder automation prompt; "Put Back" isn't offered), Windows the Recycle Bin, Linux the freedesktop Trash. A document's sidecar goes too, and an emptied `.rupert/` folder is removed. Folders go whole, after a confirmation. A source root can't be trashed. The index catches up with a single-file job (or a folder pass for folders). Not on the phone, which stays read-only.
- **Google Drive on Windows (Stream files):** its `G:` drive marks no file as online-only (every file's attributes are `Normal`, checked 2026-11-20), so reading any of them downloads it. `cloud::streams_on_demand` recognizes that drive from its volume information (name "Google Drive", or FAT32 claiming `FILE_SUPPORTS_REMOTE_STORAGE`) and everything on it counts as online-only; text comes from the catalog or the opt-in download. The inbox still reads scans there, and a file the download option just fetched is indexed as local. Mirror mode is an ordinary NTFS folder and unaffected.
- **To verify:** trashing an online-only file in iCloud Drive and OneDrive moves it to the service's deleted items without downloading it first.
- **Move** (`fileops::move_entry`, command `move_entry`): a document (with its sidecar) or a folder into another folder of the same or another source; drag onto a folder in the sidebar or folder view (HTML drag and drop, so the window's `dragDropEnabled` is off: on Windows it would swallow those events), or right-click ▸ Move to…. The name is kept, with " (2)" added if taken; nothing is overwritten. If the sidecar can't follow, the document is moved back. Catalog entries are rewritten under the new paths **before** the rename, so the indexer finds them at once and online-only documents keep their details and text. Nothing is read: an online-only document (or a folder holding one) moves only within its source or between two sources of the same cloud service; documents otherwise copy across disks, folders only rename (same disk). The folder holding the inbox can't be moved (the inbox is kept by path). The index catches up with single-file jobs (old and new path) or two folder passes. Not on the phone.
- **To verify:** moving online-only files within iCloud Drive and within OneDrive (macOS and Windows) without downloading them; drag and drop on Windows.

## Mac App Store edition (in preparation, 2026-11-19)

- The DMG from GitHub is the Mac release. A second edition for the Mac App Store is built from the same code with the `app-store` feature and `src-tauri/tauri.appstore.conf.json`; it shares the iPhone app's App Store record and bundle ID.
- **Sandbox:** access only to folders the user picked. Each source keeps a **security-scoped bookmark** (`bookmark.rs`, `Source::bookmark`, base64 in `sources.json`): made right after the pick, opened at every launch before watching and indexing (`SourceStore::open_bookmarks`, which also follows moved folders and quietly adds bookmarks to older sources, in both editions). A source that can't be opened shows **Allow access…** (macOS) / **Locate…** (elsewhere), a folder picker at its old place (`relocate_source`). The inbox lives inside a source; the ScanSnap import asks for ScanSnap Home's "Managed" folder (its automatic lookup points into the container); "Add as source" from the import goes through the folder picker.
- **Gated off in that edition:** the self-updater (the App Store updates it) and the `ant` command sign-in (a sandboxed app can't run other programs); Claude with an API key works. `app_info` reports the edition; the window hides what's absent.
- **macOS packaging (both editions):** PDFium ships in `Contents/Frameworks` (`tauri.macos.conf.json`), where it's signed with the app; in Resources it wasn't, and notarization refused it.
- **To verify in the sandbox:** freeing iCloud/OneDrive space after indexing, Move to Trash, file watching, Local AI (Apple Intelligence too), the ChatGPT sign-in's local listener.

## Favorite folders

- A folder's own details (for now: favorite) live in its hidden `.rupert/.folder.json` (`folder_info.rs`), so they follow the folder when it's renamed or moved anywhere and sync to other machines with it. The leading dot keeps the name apart from document sidecars, which are named after documents (never hidden). Unfavoriting removes the file, and the `.rupert` folder if it's then empty.
- The index keeps them in a `folders` table (schema 4), filled as folders are walked; a folder pass also forgets subfolders that vanished. The watcher maps the file to its folder like a sidecar, so changes from another machine show up when they sync.
- Desktop: a ☆/★ beside the folder path; favorite folders are listed under Favorites in the sidebar. Phone: a ☆/★ in a folder's header and a Favorites group at the top of Library (favorite folders and favorite documents).
- Starring is the phone's only write to the library besides saving scans and shared documents, and touches only the folder's `.rupert/.folder.json`: through a coordinated write (iCloud Drive, On My iPhone; the `.rupert` folder is created if needed and removed again when empty), or through Graph for OneDrive (the `.rupert` folder is created first since uploads don't create folders; unstarring deletes the file into OneDrive's recycle bin). The Mac picks the change up when it syncs. Desktop Rupert on Windows marks any `.rupert` folder it meets as hidden, since one created by the phone isn't.

## Tags

- **Filter (desktop sidebar, phone sheet):** type to narrow; with nothing typed the 12 most-used tags show, otherwise tags starting with the text, then those with a word starting with it, then those containing it, more-used first; "Show all" for the rest. Several tags can be selected, matching **Any** (at least one) or **All** (every one), ignoring case. In All mode the list only offers tags that occur together with the selection, counted that way, so it narrows as you pick. The selection combines with text search and Favorites. Queries take under 10 ms on 6,900 documents (`json_each` over the stored tag lists; a separate tag table is the upgrade if libraries grow much larger).
- **Tag manager (desktop):** pick tags and rename one, merge several into one (the merged tag takes the place of the first it replaces, once), or delete them from every document. A preview counts the documents by where the change is saved, and changes are written where each document's details live (`tag_edit.rs`): inside local PDFs, in the sidecar for other formats, and as pending sidecar details for online-only PDFs so nothing is downloaded. Pending details can only add or replace, so an online-only PDF that would be left with no tags is downloaded and changed in place; the dialog asks first. The phone stays read-only.

## Scanning and intake

### Pipeline

Inbox folder → OCR if the PDF has no text layer → Local AI suggests title, document date, tags and destination folder → user reviews and accepts → file is renamed, metadata written, file moved. **Built in milestone 4.**

- **Inbox:** a folder inside one of the sources (Settings; e.g. the folder a ScanSnap Home profile saves to). A background thread (`intake.rs`) processes new files one at a time and caches the results by size and modification time in the app's data folder, so nothing is redone after a restart.
- **OCR** (`ocr.rs`, `text_layer.rs`) adds an invisible text layer to every PDF page without text, so other programs can search it too. Scanned images get their recognized text in their sidecar instead.
  - Engines, all local: **Apple Vision** on macOS (built in: nothing to bundle, better than Tesseract on scans, detects the language per text block), and the **`tesseract` command** on Windows and Linux when installed. This replaces the earlier plan of bundling Tesseract, whose native libraries are awkward to ship on all three platforms.
  - Pages are rendered by PDFium at up to 300 dpi; word boxes are mapped back to PDF coordinates with PDFium's own pixel-to-page transform, so rotated and cropped pages work.
  - The layer uses a "glyphless" font as Tesseract does: Type 0, Identity-H (UTF-16 code units), an embedded 600-byte TrueType program whose glyphs are empty, fixed widths and a ToUnicode map. A Type 3 font was tried first; PDFium and Poppler read it but **PDFKit (Preview, Spotlight) ignores Type 3 text**, so it was replaced.
  - **Pages shown turned** (`/Rotate`, e.g. a landscape scan turned in Preview), fixed 2026-11-18 after a scan failed the check (74 of 200 words): (1) pdfium-render's `PdfPageText::all()` only returns text inside the page's *displayed* size, which has width and height swapped on a turned page, so text beyond the shorter side was cut off, for every rotated PDF the indexer read; `text.rs` now asks for the text inside the media box. (2) The glyphless font's glyph had no outline, so text running up or down the page had zero width and PDFium dropped it; the glyph is now a single corner-to-corner line (a real bounding box, no area, so it paints nothing). A test covers 0/90/180/270°. Index version 5 (an in-place upgrade from 4) marks every local PDF as changed, so each is read once more and its catalog entry refreshed: on a test Mac about 3,500 local PDFs took about 80 s; about 1 % had turned pages. Online-only PDFs keep the text their catalog entry has until some machine reads them again.
  - If the text layer still can't be written, the recognized text goes into the sidecar (searchable) and the suggestion uses it.
  - Written as an incremental update like metadata, checked before writing (the updated file must parse and PDFium must read back ≥ 90 % of the words). Tested on real ScanSnap scans: original bytes intact, pages render identically, text found by PDFKit, PDFium and Poppler, selection boxes line up with the printed words.
  - PDFium can be loaded once per process and is then tied to its thread, so it lives on one service thread (`text.rs`); the indexer and the intake share a handle.
- **Local AI** (`llm.rs`): any OpenAI-compatible server (`/v1/models`, `/v1/chat/completions` with a JSON schema for the answer). LM Studio on `localhost:1234` is the default; Ollama (`localhost:11434`) is tested too, and Settings offers both as presets. If it isn't running, the review simply has no suggestion. Suggestions are refused for an address outside this computer and the local network (`llm::reach`, judged from the address alone, no DNS), since inbox suggestions never use a cloud service. `max_tokens` is 8000 because thinking models can spend thousands of tokens before answering; a reply cut off before any JSON gets its own message. Settings were `lmStudioUrl`/`lmStudioModel` up to 0.3 and are still read under those names.
- **Apple Intelligence** (`apple_ai.rs`, `apple/RupertAI.swift`): Apple's on-device model through the Foundation Models framework, on macOS 26+ with Apple silicon and Apple Intelligence on (Settings ▸ Local AI ▸ Server ▸ Apple Intelligence, `Settings.local_engine`). Swift-only API, so `rupert-core/build.rs` compiles a small Swift static library on macOS (skipped, with `cfg(apple_ai)` unset, elsewhere or with an SDK older than Xcode 26). Its C interface passes JSON both ways; JSON schemas become `DynamicGenerationSchema`s (property order from `required`). The framework is weak-linked (`@_weakLinked import`, checked with `otool`: `LC_LOAD_WEAK_DYLIB`) and everything is behind `#available(macOS 26)`, so Rupert still runs on macOS 13. Binaries need `/usr/lib/swift` as a run-path (`src-tauri/build.rs`). For suggestions the folder is an `enum` of the candidates (a small model chooses better than it copies), and the text sent is sized to the context. Trial on the sample library, 2026-11-20 (macOS 27.2, M5 Pro): see #6. **Default for new installs** on a Mac that can run it (`Settings::first_start`, decided 2026-11-20); settings files saved before keep `local_engine` = server, so an LM Studio setup isn't switched.
  - The model sees the start of the text, the file name, the current year (not today's date, which models copy as the document date), the tags in use, and up to 12 **folders where similar documents live** — found by a full-text OR query of the document's most telling words against the index — so the prompt stays small even with 1,200+ folders. It may only pick one of those folders.
  - Some reasoning models return the JSON in `reasoning_content` instead of `content`; both are read.
  - With qwen3.6-35b-a3b: 1–5 s per document; titles, dates and folders were right on 7 of 8 real scans.
- **Review and filing:** the inbox view shows the list, a page preview and a form prefilled from the suggestion (title, date, tags, folder, file name from a pattern, default `{date} {title}`). **File** writes the details into the file, then renames and moves it (`fileops.rs`: sidecar moves along, never overwrites — " (2)" is added — copy + delete across volumes).

### Scanners

- **Phase 1: watched inbox.** Any scanner or app that saves to a folder works, e.g. a ScanSnap Home "save to folder" profile or HP Smart.
- **Phase 2: the phone and scanner software (#123, #124, 0.8.0).** The inbox's **Scan…** opens "Scan with your phone" (`ScanPhonePanel.svelte`). Scanner software that saves to a folder covers the rest, including Image Capture and Windows Scan for network scanners without software of their own.
  - **Continuity Camera (Mac):** **Scan with iPhone…** in the panel, or File ▸ Import from iPhone or iPad: a nearby iPhone's Scan Documents saves a PDF straight into the inbox, no Rupert app needed (`src-tauri/src/continuity.rs`). AppKit takes over a menu item with the identifier `NSMenuItemImportFromDeviceIdentifier` (devices and what each can do), but only one that's in the menu bar when the menu bar is installed (tried with a bare AppKit app, 2026-11-25): not one added later, and never one in another menu. So the item goes into File, the menu bar is installed again (`setMainMenu(nil)`, then the menu), and the panel's button borrows that item for its own menu. AppKit offers what the responder chain accepts of the types registered with `registerServicesMenuSendTypes`: a responder right after the web view (the window hands such questions to its delegate, not to its next responder) accepts PDFs only, so only Scan Documents shows, and only while there's an inbox. Tested with an iPhone 16: the scan arrived as a one-page PDF. Named "Scan 2026-11-25 14.03.07.pdf" like the phones' scans.
  - **The Rupert app:** one QR code (`rupert_core::qr`) for https://rupertlibrarian.com/scan/. On a phone with Rupert it opens the Scan tab: a universal link on the iPhone (associated domain, `/.well-known/apple-app-site-association`, team 9FP28N8GW3) and an App Link on Android (`/.well-known/assetlinks.json` with the upload key's certificate; the Play app-signing key's goes in once Google has approved the closed test). Otherwise the page (`site/scan.html`) offers the App Store or the Android download, and **Open Rupert** (`rupert://scan`; on Android an `intent:` link that falls back to the download). The app takes the link from `RunEvent::Opened` (iPhone) or the activity's intent (Android) and the page asks `take_scan_link` at start, when it comes back, and on `scan-link`. The panel also names the inbox folder to choose there, and notes when the inbox is a folder on this computer (phones can't reach it) or in iCloud Drive (Android can't).
  - **eSCL / AirScan was built (0.5–0.7) and removed in 0.8.0 (#123).** It was never tested with a real scanner, the phone's camera covers most home scanning, and it brought Bonjour discovery (`mdns-sd`, UDP 5353) and a scan dialog of its own to maintain. The JPEG-to-PDF writer it used stays for camera scans (`camera.rs`).
  - **USB scanners:** macOS exposes them through ImageCaptureCore, Windows through WIA, Linux through SANE (none built; their own software saving to the inbox folder does the job).
  - **ScanSnap** (e.g. iX1600 over USB): ScanSnap models don't support TWAIN or ISIS.
    - Phase 1: a ScanSnap Home profile saves to Rupert's inbox folder, or uses ScanSnap Home's "send to application" option to hand the file to Rupert. ScanSnap Home stays installed only as the scanner driver.
    - Phase 2: the ScanSnap SDK (Windows/macOS, needs ScanSnap Home 3.3+, iX1600 listed as supported), through a small native macOS bridge. **Blocked:** the SDK is a licensed download from PFU (registration, terms on redistribution), so it can't be fetched or bundled here. Until then, phase 1 is the way: a ScanSnap Home profile that saves PDFs to Rupert's inbox folder.

## Migrating from ScanSnap Home

Source: `~/Library/Application Support/PFU/ScanSnap Home/Managed/ScanSnapHome.sqlite` (Core Data SQLite; read from a copy). What a real library of about 10,000 items (2026-11-17) showed:

- About half are PDFs; the rest are mixed (HTML, JPEG, ZIP, EXE/DLL, Office…).
- **Titles:** almost always the filename (the few that differ are mostly cut off at 64 characters). Nothing to migrate.
- **Document date:** set on nearly every item, and on about 7 % of PDFs more than a day away from the file date. **This must migrate.**
- **Labels / memos / comments:** unused.
- **Favorites and receipt fields** (vendor, category, amount): a handful each.
- **Trash:** a few items.

Every ScanSnap Home folder records its full disk path (`ZINDEXPATH`, a `file://` URL), and document dates are stored as local midnight (read back with SQLite's `localtime`). The OCR text lives in `ScanSnapHomeSearch_V2.db` (`TextTbl`, joined on `contentId = ZCONTENTID`).

**The importer (built, `rupert_core::scansnap`)**, from a temporary copy of both databases:

- Imports only what isn't already in the files: the document date when it differs from the file's creation date, the favorite flag, receipts (tag "Receipt" plus a notes line such as "Receipt: CVS, 20.50 USD (tax 0.92)"), and OCR text unless the file's own text is already indexed. Fields the user has set are never overwritten, and running it again changes nothing.
- Finds each file by folder path + name. Renamed files are matched by size + type within the same folder, moved files by size + type across Rupert's index. A match must be unique, and never a file that ScanSnap Home knows under another name.
- Local PDFs get the details embedded; other formats use their sidecar. Online-only files are never read: details go into the sidecar as pending, text into the sidecar.
- A review screen comes first (counts, renamed matches, files not found, folders outside Rupert's sources with an "Add as source" button). It is reached from File ▸ Import from ScanSnap Home… on macOS, or the sidebar.
- **Run on a real library 2026-11-18:** about 1,500 documents in iCloud Drive and OneDrive; about 220 PDFs updated (all still readable with Poppler); none of the online-only files were downloaded; 3 items couldn't be found. Documents searchable by content more than doubled.

## Phone app (iPhone)

A Tauri iOS app in `mobile/` (thin commands in `mobile/src/lib.rs`, Swift in `mobile/plugin/ios`), on the same `rupert-core`. The UI is the `/phone` route of the shared SvelteKit front end. Android (#24, decided 2026-11-20, in progress): the same crate and UI as a Tauri Android app (`mobile/gen/android`, package `io.github.micki_b.rupert`). OneDrive and Google Drive there don't hand folders to other apps either, so the first version reads them through their APIs like the iPhone does; iCloud Drive doesn't exist on Android, and folders on the phone (Storage Access Framework) come later. Scanning uses Google's ML Kit document scanner. Tested on the emulator ("Rupert_Pixel", Android 36 with Play services).

- **Phone plugin** (`mobile/plugin`, "rupert-phone"): Swift for iOS and Kotlin for Android behind the same `Device` calls. On Android: `scan` (ML Kit, pages at the chosen resolution, gray as JPEG, black and white as a thresholded PNG, so `pages_to_pdf` and the upload are shared), `signIn` (a Custom Tab; the redirect comes back through the manifest's intent filter for OneDrive's `msauth` scheme), `googleToken` (Google Play services' AuthorizationClient: Google allows the web sign-in only for iOS clients, so `gdrive.rs` takes access tokens from it on Android and keeps a signed-in marker, not a refresh token), the keychain (AES-GCM with a Keystore key), `preview` (FileProvider + ACTION_VIEW). The Files-picker calls are iOS-only.
- **UI on Android** (`src/routes/phone/+page.svelte`, known from the user agent): its own wording (`m.phone.android`), no Files-picker choices, no Ask tab (no Apple Intelligence). The back gesture is handled only while there's something to close or leave (sheets, folders, other tabs); otherwise Android's own gesture leaves the app. The web view doesn't report the navigation bar as a safe area, so the plugin passes it as `--nav-inset`; the status bar it does report. The window background and adaptive icon (`mobile/icons/android/`) match the app.

- **Library:** the user picks folders in the Files picker (iCloud Drive, On My iPhone), the same folders Rupert uses on the computer. Access is kept with bookmarks, which are resolved at every launch because iOS paths can change.
- **OneDrive** can't be used that way: its iOS file provider doesn't let other apps pick folders (they show greyed out; a long-standing limitation of third-party providers). So OneDrive folders are read through **Microsoft Graph** (decided 2026-11-18, phone only; the desktop still uses the synced folders):
  - Sign-in with OAuth 2.0 + PKCE as a public client (personal accounts, `consumers` authority), shown with `ASWebAuthenticationSession`; the refresh token is kept in the Keychain (this device only), the access token only in memory. Scopes: `Files.ReadWrite offline_access` (write is only used to add scans to the inbox, never replacing a file).
  - The app's Microsoft registration (client id) is read at build time from `mobile/onedrive-client-id`; without it the app builds with OneDrive turned off. Redirect URI `msauth.io.github.micki-b.rupert://auth`.
  - A folder's complete listing comes from `delta` (paths rebuilt from parent ids); `remote::sync` indexes it from the catalog entries and sidecars, which are fetched as files (only Rupert's own; never documents). Unchanged documents aren't fetched again. Browsing a OneDrive folder uses the index.
  - Opening a document downloads it into the app's cache (kept 14 days), then Quick Look. Scans upload into a OneDrive inbox folder (upload session above 4 MB).
- **Sharing to the inbox** (#42): documents shared from other apps go to the inbox, under their own names, never replacing a file (`mobile/src/share.rs`).
  - **iPhone:** a share extension (`mobile/gen/apple/RupertShare`, its own target in the Xcode project, added by hand) saves them itself, without opening the app: to a Files folder (its bookmark), or to OneDrive or Google Drive (upload with the app's sign-in). The app writes the inbox's details and the client ids to `share-settings.json` in the app group `group.io.github.micki-b.rupert` whenever the inbox or language changes; the keychain items are shared because both targets list the app's own keychain group. Each document is first kept in the app group's `Pending/<id>/` folder and removed once saved; the app sends whatever is left when it comes to the front (`send_shared`), e.g. after the extension was offline or had no inbox yet. A build step copies the app's version and build number into the extension.
  - **Android:** `SEND`/`SEND_MULTIPLE` intent filters for document types open Rupert; the plugin copies the shared files into the cache's `shared/<id>/` folder (in the background; `sharedFolders` waits for it) and `send_shared` uploads them with the scan code.
  - The share sheet accepts PDFs, pictures, Office, iWork and OpenDocument files, Markdown, CSV and emails; anything else that arrives is removed with a note.
- **Folder views** (#41): list, large icons or gallery, chosen per folder and kept in the web view's storage. Pictures come from the phone's own `thumb:` scheme (`mobile/src/thumbs.rs`, the same URLs as on the computer, which has PDFium; the phone doesn't): OneDrive's thumbnails by path (`thumbnails/0/c<side>x<side>/content`), Google Drive's `thumbnailLink` resized, and on the iPhone `QLThumbnailGenerator` for Files documents that are on the phone; online-only ones in Files get none, since making one would fetch the file. Pictures, and "has none" for a day, are cached by source, path, size and modification time; the cache keeps 30 days.
- **Read-only,** except for saving scans and shared documents, and starring folders. The indexer runs in viewer mode (`indexer::Mode::Viewer`): it reads the shared catalog and sidecars of every folder (the provider isn't visible from iOS paths) and never writes, prunes or downloads. Quick Look's markup is turned off.
- **Text:** there's no PDFium on the phone, so body text comes from the catalog and sidecars that Rupert on a computer writes. Any machine that can't read a file's text now uses a matching catalog entry for it and leaves the entry alone; before, it erased the other machine's text.
- **Downloads:** documents are fetched only when opened (`NSFileCoordinator`), then shown in Quick Look with sharing and printing. Rupert's own `.rupert/` folders are fetched before they're read (old-style iCloud `.name.icloud` placeholders included); a fetch that takes more than 90 s, e.g. offline, is skipped.
- **Online-only files** are recognized like on the Mac (dataless files, `.icloud` placeholders).
- **Ask Rupert** (#27): the **Ask** tab, on an iPhone that can run Apple Intelligence (`apple_ai::Status::capable`; on other iPhones the tab doesn't appear; when it's turned off or downloading, the tab says so). The same engine and read-only `LibraryTools` as the desktop with `Backend::Apple` only (`mobile/src/assistant.rs`): no servers or cloud models on the phone. `rupert-core/build.rs` builds the Swift bridge for iOS too (device and simulator SDKs); it's bundled into the app's static library and linked by Xcode, which provides the Swift runtime. UI actions (open folder, show document, show results) become buttons under the answer, so the answer stays on screen; the conversation is kept when switching tabs. In the iOS 27.2 Simulator on a macOS 27.2.1 host, model calls fail on the host side (`promptTemplateNotFound` for the safety model, surfacing as GenerationError -1), so the model itself needs a real iPhone to test; the tab, chat and error paths were checked in the Simulator.
- **Scanning:** Rupert's own camera (`Scanner.swift`). The system document camera (VisionKit) was tried first, but it gave ~1200×2100-pixel, sometimes out-of-focus pages with no settings, too blurry for OCR.
  - Full-resolution photos from the main lens with quality prioritized; on phones with an ultra-wide lens the virtual camera switches to macro on its own, and focus is restricted to near.
  - The page's edges are shown while aiming and found again on each photo (Vision's document detector, or its rectangle detector as a fallback), then the page is straightened (`CIPerspectiveCorrection`).
  - Options in the Scan tab, remembered: Color, Grayscale or Black & white; 150/200/300/400 dpi (the page's long side taken as 11 inches; never upscaled). Black & white evens out shading (each pixel divided by a heavily blurred copy) before thresholding, and is stored as a 1-bit Flate image (≈20 KB per page versus ≈300 KB for gray or color JPEG).
  - Several pages per scan; tapping the thumbnail removes the last page. The PDF is copied with a coordinated write into the inbox folder chosen on the phone (Files, or OneDrive through Graph), which should be the Mac's inbox. OCR and filing suggestions happen there; the phone runs no AI.
- **Tested in the iOS Simulator** on a made-up library (catalog written by the Mac engine): browsing, search with snippets, details from sidecars, Quick Look, scanning (the simulator has no camera, so it returns a generated page) into the inbox, and Apple Vision OCR of the result on the Mac.
- **Tested on an iPhone (2026-11-18)** with a real library: an iCloud Drive folder through Files, about 6,000 documents, with the same number searchable by text as on the Mac (within 0.2 %); a OneDrive folder through Graph, about 950 documents, an exact match with the Mac; search and opening a OneDrive PDF work. Fetching the big iCloud `.rupert/catalog` folder took longer than the 90 s limit, so the first pass went on without waiting for it; it still came out complete.
- The Microsoft sign-in uses a private (ephemeral) browser session: with Safari's shared cookies it failed with AADSTS165000 ("missing session context cookie").
- Debug builds accept `SIMCTL_CHILD_RUPERT_TEST_FOLDER`, `…_INBOX` and `…_JS` to set up and drive a simulator, because the Files picker can't be scripted.

## Languages

- **Screens:** `src/lib/i18n/`. `en.ts` is the master catalog (nested by screen); every other language is a `Messages` object with exactly the same keys, so `npm run check` catches a missing one. Text with values is a function per language (its own word order and plurals, via `Intl.PluralRules`). Use `m.section.key`; `m` follows the language live. Numbers and dates are formatted in the language's locale (`es-AR`).
- **Rust messages** (errors, the macOS menu, window titles, Ask Rupert's step lines): `rupert_core::i18n`, `tr("English")` / `trf("… {0} …", &[&value])` looked up in a per-language table (`i18n/es.rs`, `i18n/de.rs`). A test reads the source tree and fails on any message without a translation, or with different placeholders, and on unused entries.
- **Which language:** the setting (`Settings.language`; on the phone `Saved.language`) or else the first system language Rupert speaks (`sys-locale`, which on iOS includes the per-app language), else English. Rust holds it in a process-wide atomic; the screens ask for it (`language` command) before the first window draws.
- **Help:** one complete page per language (`src/lib/help/En.svelte`, `Es.svelte`, `De.svelte`) with the same section ids. Never a partial fallback.
- **System texts:** `CFBundleLocalizations` in both Info.plists (macOS and iOS show their own panels in the language, and iOS offers the per-app language), `InfoPlist.strings` per language for the iPhone's camera permission, and NSIS installer languages on Windows. The macOS menu is built by Rupert (Tauri's default one is English only) and rebuilt when the language changes.
- **Stays as it is:** folder and file names Rupert creates (`.rupert/`, scan names, the name pattern), tags written by imports ("Receipt"), and Ask Rupert's instructions to the model (English, with a line to answer in the user's language). Inbox suggestions keep the document's own language.
- **The owl** wears an outfit for the language: the albiceleste in Spanish (white with three sky-blue stripes, navy shoulder bands and V-neck, a 10 in the style of the 2022 shirt; no crest or logo, which are trademarks; approved 2026-11-20) and lederhosen in German (leather braces with stitching along both edges, a chest strap with green trim and an edelweiss, over a white football shirt in the style of the 1990 home shirt: black-red-gold chevron, black collar with a red edge; no crest or logo; approved 2026-11-20), drawn as SVG in the picture's own coordinates, with the picture cut out under it.

## Still to verify

- iCloud Drive and OneDrive sync `.rupert/` dot-folders on all platforms.
- Finder-tag writes on online-only files don't trigger downloads; whether OneDrive keeps Finder tags.
- OneDrive via Graph: token refresh after the first hour (not yet observed), and folders bigger than about 1,000 documents.
- On an iPhone: listing big iCloud Drive folders through the picked bookmarks, fetching `.rupert/` folders, and that no document is downloaded until it is opened.
- ScanSnap Home can save to a folder (or send to an app) without also cataloging the file.

## Milestones

0. ✅ Tauri skeleton, CI builds for all three OSes.
1. ✅ Sources: add folders, browse trees, watch for changes, show online-only status. Folders are listed lazily (one level at a time), so there is no full index yet; that arrives with search in milestone 3.
2. ✅ Metadata: read/write XMP + Info dict, sidecars, tags, `rupert:id` identity. Tested on 26 real PDFs (Poppler, PDFKit and Spotlight read the results; original bytes and rendering unchanged). Finder-tag sync deferred until after milestone 3.
3. ✅ Search: text extraction (PDFium), FTS5 index, shared catalog, search bar, Favorites and Tags in the sidebar. A 19,000-document OneDrive walked in 32 s; searches take 4–30 ms.
4. ✅ Intake: inbox, OCR (Apple Vision / Tesseract) with an invisible text layer, LM Studio suggestions, review screen, renaming and moving.
5. ✅ ScanSnap Home import (done before 4): document dates, favorites, receipts and OCR text, with a review screen; sidecars can hold pending details and imported text.
6. Direct scanning: eSCL/AirScan was built, then removed in 0.8.0 (#123) in favor of scanning with the phone. ScanSnap through the SDK is pending the SDK itself; ScanSnap works through a profile that saves to the inbox. USB scanners via ImageCaptureCore/WIA/SANE not built.
7. ✅ iPhone app: browse and search the same folders (read-only; OneDrive through Microsoft Graph), open documents in Quick Look, scan with the camera into the inbox. Tested on an iPhone with a real library, including camera scans.

## Roadmap (planned 2026-11-25)

Since milestone 7, work is planned as releases; each is a GitHub milestone, and the issues hold the details and open questions. The order within a release isn't fixed.

| Release | Theme | Issues |
|---|---|---|
| 0.7.0 ✅ | Renaming and sending | Released 2026-11-25: Rename documents… (#64); Share… and Email… (#69) |
| 0.8.0 ✅ | Getting documents in | Released 2026-11-25: Print to Rupert (#101); and #60 in three parts: Import into Inbox… (#122), network scanning removed (#123), Scan with your phone through Continuity Camera and a QR link (#124) |
| 0.9.0 ✅ | Working with many documents | Released 2026-11-26: Multi-select and bulk actions (#102, built: actions in Details and the right-click menu, no bar above the list); sorting and search filters (#103, built: clickable columns with a Date column and folders on top, a Sort menu in large icons and the gallery, best match or newest, and a filter row for date, kind and place); Undo for moves, filing and tags (#104, built: one level, also renames; only what's still as Rupert left it); quick open and spacebar preview (#105, built: ⌘K for folders, documents by title or name, tags and commands, with recent documents and favorite folders first; Space opens the page viewer from any view, ↑ ↓ to the neighbors; no system-wide shortcut yet); highlighted hits and page citations (#106, built: text keeps page breaks, an index upgrade re-reads local PDFs; the page viewer marks a search's words with next and previous and ⌘F to find; Ask Rupert cites "[Title, p. N]" and the citation opens the page with the passage marked; local PDFs only); export a bundle (#107, built: Export… for a selection, a folder or the search results; a zip or a folder with index.csv and index.html; copies keep their folders and names; notes as Share… sends them; online-only documents counted in the dialog); a page preview in Details while browsing, and an inbox preview that resizes and makes way for File it (#127, built: the page at the top of Details in the list, large icons and search results for PDFs and pictures, not in the gallery; a divider sets its height, remembered for each of the two; the form keeps its room and the page takes what's left, under 120 px a Show page button instead; × and View ▸ Hide Page Preview hide it in both; the Notes boxes resize only vertically, one-to-one with the drag); resizable, reorderable list columns with a Star column for folders too (#138, built: Star, Name, Kind, Size, Date, Modified; Name takes what the others leave; edges and headers drag with pointer events, apart from dragging documents out; a header menu shows or hides columns and resets them; kept in localStorage; folder stars write the folder's .rupert/.folder.json; Kind still steps aside in a narrow middle; the Star header doesn't sort; search results keep their layout) |
| 0.10.0 | A smarter library | Sender, amount and due date (#108, built: From, Amount with its currency and Due date in File it (suggested by Local AI, each kept only if the text shows it), in Details and on the phone (read-only); search filters for From and due date (from today on / before today); hidden-by-default list columns From, Amount and Due date, sortable; totals for a selection and for a search (all matches, per currency); Ask Rupert searches by them and adds up amounts (`total_amounts`, `list_senders`); Read details… for a selection, folder or source, from indexed text only, with a review table; the ScanSnap import fills From and Amount from receipts; Export's list has the new columns); reminders (#109); saved searches (#110); duplicate finder (#111); filing that learns, File all (#112); library health panel (#113); duplicates and Library health together (#111, #113, built: Library health in the sidebar and from the status line's counts, rows with Show and a fix each: duplicate copies (same bytes by SHA-256 among same-sized local files, same indexed text of at least 300 characters, and "probably the same" online-only by size and name, never trashed), no text (Recognize text in place: PDF text layer, pictures in the sidecar), online-only text unknown (Index this folder per source), no date and no tags (one Suggest details… review from indexed text), scanner-style names (by folder, Rename documents… each), couldn't be read; duplicates clear up into the Trash after the copy kept takes the others' tags and notes; candidates are read under the index lock, files hashed without it); inbox in a cloud folder (#144, built: online-only scans downloaded again by the intake, the kept picture meanwhile, Settings ▸ Inbox recommends Keep Downloaded with the step per service, a button on Windows with OneDrive or Dropbox) |
| Android 0.1 | First Android release | The app (#24); release signing, Play bundle and CI (#39); Ask Rupert with Gemini Nano (#38); Google OAuth verification for Drive (#66) |
| Later | Not scheduled | Search by meaning with local embeddings (#114); importers for Paperless-ngx, DEVONthink, Evernote (#115); first-run tour with a sample library (#116); Finder tags (#117) |

- **Dependencies:** #102 comes first in 0.9.0 (#104, #107 and bulk tagging build on it); #103 before #110; #108 before #109, and it gives #115 a place for correspondents.
- **New details fields** (#108: from, amount, due date; #109: done) follow the metadata rules above: `rupert:*` in XMP or the sidecar, cached in the catalog and index. Saved searches (#110) go in a source's `.rupert/` folder, not only in the database.
- **Would reopen a decision, so not planned until decided:** splitting a stack of scans and saving a page's rotation (#118; page editing is out of scope) and filing from the phone (#119; the phone is read-only).

## Dependencies (licenses)

All permissive; the full list with license texts is `THIRD_PARTY_NOTICES.md`, generated by `scripts/third-party-notices.py` (Rust crates of the desktop and iPhone apps, the window's JavaScript, PDFium) and bundled with both apps. Main ones: Tauri (MIT/Apache), `notify` (CC0/Artistic), `rusqlite` + SQLite FTS5 (MIT/public domain), `lopdf` (MIT), `pdfium-render` + PDFium (MIT/Apache, BSD), `ureq` + rustls (MIT/Apache/ISC), `qrcodegen` (MIT), `png` (MIT/Apache), `objc2-*` (MIT), `keyring` (MIT/Apache), `trash` (MIT), `sys-locale` (MIT/Apache), `marked` (MIT), DOMPurify (MPL-2.0 or Apache-2.0), Tesseract (Apache-2.0, optional, not bundled). Avoid MuPDF and Ghostscript (AGPL). Rupert itself: MIT (`LICENSE`); privacy policy: `PRIVACY.md`.
