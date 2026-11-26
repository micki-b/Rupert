// Typed wrappers around the Rust commands in src-tauri/src/lib.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { m } from "#lib/i18n/index.ts";

export type Provider = "local" | "icloud" | "onedrive" | "googledrive" | "dropbox";
export type FileKind = "pdf" | "image" | "office" | "text" | "email" | "other";
export type CloudState = "local" | "cloudOnly";

export interface Source {
  id: string;
  name: string;
  root: string;
  provider: Provider;
  showAllTypes: boolean;
  /** Download online-only documents to read their text, then free the space. */
  downloadOnlineOnly: boolean;
  available: boolean;
}

export interface Entry {
  name: string;
  relPath: string;
  isDir: boolean;
  kind: FileKind | null;
  size: number;
  modifiedMs: number | null;
  cloud: CloudState;
}

/** A folder entry plus what the index knows about it. */
export interface ListedEntry extends Entry {
  title?: string | null;
  tags?: string[];
  favorite?: boolean;
  documentDate?: string | null;
}

export interface Hit extends Entry {
  sourceId: string;
  title: string | null;
  tags: string[];
  documentDate: string | null;
  favorite: boolean;
  /** Body excerpt; matches wrapped in U+E000 … U+E001. */
  snippet: string | null;
}

export interface TagCount {
  tag: string;
  count: number;
}

export interface Progress {
  active: boolean;
  done: number;
  total: number;
  current: string | null;
  pdfText: boolean;
  downloadDone: number;
  downloadTotal: number;
  /** Stopped with Stop in the status line (#78), until Resume or the next launch. */
  stopped: boolean;
  /** Pictures of online-only-to-be documents made in idle time (#62). */
  pictureDone: number;
  pictureTotal: number;
}

export interface IndexStats {
  documents: number;
  withText: number;
  withoutText: number;
  cloudOnlyUnindexed: number;
  favorites: number;
}

/** How several selected tags combine: documents with any of them, or all. */
export type TagMode = "any" | "all";

export interface SearchArgs {
  text?: string;
  sourceId?: string | null;
  tags?: string[];
  tagMode?: TagMode;
  favoritesOnly?: boolean;
  /** #103: dated from / to (YYYY-MM-DD), only these kinds, only in a folder
   * of `sourceId` (and below), and newest first instead of best match. */
  dateFrom?: string;
  dateTo?: string;
  kinds?: string[];
  folder?: string | null;
  byDate?: boolean;
}

/** A folder marked as a favorite (kept in the folder's own `.rupert`). */
export interface FavoriteFolder {
  sourceId: string;
  relPath: string;
  name: string;
  sourceName: string;
}

/** A tag manager edit: rename (several tags: merge) or delete. */
export interface TagEdit {
  from: string[];
  action: { kind: "rename"; to: string } | { kind: "delete" } | { kind: "add"; tags: string[] };
  /** Only these documents (#102); every document with the tags otherwise. */
  only?: { sourceId: string; relPath: string }[];
}

/** What a tag edit would change, by where each document keeps its details. */
export interface TagEditPreview {
  documents: number;
  inFile: number;
  sidecar: number;
  pending: number;
  download: number;
}

export interface TagEditReport {
  changed: number;
  failed: { path: string; error: string }[];
  /** What changed, document by document, for Undo (#104). */
  changes: TagChange[];
}

/** A document's tags before and after an edit. */
export interface TagChange {
  sourceId: string;
  relPath: string;
  before: string[];
  after: string[];
}

/** Something to put back where it was: Undo of a move or of File it (#104). */
export interface MoveBackItem {
  nowSourceId: string;
  nowRelPath: string;
  wasSourceId: string;
  wasRelPath: string;
}

/** A page of tags for the tag filter, and how many matched in all. */
export interface TagList {
  tags: TagCount[];
  matching: number;
}

export interface DirChange {
  sourceId: string;
  relDir: string;
}

export interface DocMeta {
  id: string | null;
  title: string | null;
  tags: string[];
  notes: string | null;
  /** YYYY-MM-DD, the date on the paper. */
  documentDate: string | null;
  favorite: boolean;
}

export type Storage = "embedded" | "sidecar" | "none";

export interface MetaRecord {
  meta: DocMeta;
  storage: Storage;
  note: string | null;
}

export interface MetaResponse {
  available: boolean;
  record: MetaRecord | null;
}

export interface FolderCount {
  path: string;
  count: number;
}

export interface RenamedFile {
  from: string;
  sourceId: string;
  relPath: string;
}

/** What a ScanSnap Home import would do (see rupert_core::scansnap). */
export interface ImportSummary {
  library: string;
  items: number;
  inTrash: number;
  withDetails: number;
  ready: number;
  dates: number;
  favorites: number;
  receipts: number;
  texts: number;
  onlineOnly: number;
  alreadySearchable: number;
  renamed: RenamedFile[];
  outsideSources: FolderCount[];
  notFound: string[];
}

export interface ImportProgress {
  done: number;
  total: number;
}

export interface ImportReport {
  embedded: number;
  sidecar: number;
  unchanged: number;
  failed: { sourceId: string; relPath: string; error: string }[];
}

export interface InboxLocation {
  sourceId: string;
  relPath: string;
}

export interface Settings {
  inbox: InboxLocation | null;
  ocr: boolean;
  suggestions: boolean;
  /** Local AI: a server (below) or Apple Intelligence. */
  localEngine: "server" | "apple";
  /** Local AI: an OpenAI-compatible server (LM Studio, Ollama, …). */
  localUrl: string;
  localModel: string | null;
  /** `{date}` and `{title}` are replaced. */
  /** Sending a PDF: include the notes kept inside it (off: a copy without them). */
  sendNotes: boolean;
  /** Print to Rupert (#101): the entry in the Print dialog's PDF menu / Send to. */
  printEntry: boolean;
  printEntryBookmark: string | null;
  namePattern: string;
  assistant: AssistantSettings;
  /** Desktop: look for a new version on GitHub now and then. */
  checkUpdates: boolean;
  /** Rupert's language ("en", "es"); null follows the system. */
  language: string | null;
}

/** Which model the chat assistant uses. Keys live in the keychain. */
export type AssistantProvider = "local" | "anthropic" | "openai" | "chatgpt";

/** The ChatGPT plan sign-in (Settings ▸ Assistant). */
export interface ChatGptStatus {
  signedIn: boolean;
  email: string | null;
  /** Models the plan offers other apps (slug for the API, name to show). */
  models: { slug: string; displayName: string }[];
  /** Why the models couldn't be listed, if they couldn't. */
  problem: string | null;
}

export interface AssistantSettings {
  provider: AssistantProvider;
  /** Cloud model (Anthropic: empty means its default). */
  model: string | null;
  /** OpenAI-compatible service address (empty: OpenAI). */
  baseUrl: string | null;
  /** Claude through the `ant` command-line login instead of an API key. */
  anthropicCliLogin: boolean;
}

/** Whether Apple Intelligence can be used here. */
export interface AppleAiStatus {
  available: boolean;
  /** "notSupported": not a Mac, or built without it (don't offer it). */
  reason: string | null;
  contextSize: number;
  /** Why it can't be used, to show. */
  message: string | null;
}

/** Where a Local AI address points. */
export interface LocalAiPlace {
  reach: "thisComputer" | "localNetwork" | "elsewhere";
  host: string;
}

export interface AssistantInfo {
  /** Documents' details and text go to a cloud service. */
  cloud: boolean;
  label: string;
}

export interface AssistantStep {
  tool: string;
  summary: string;
}

/** Something the assistant asks the main window to show. */
export type AssistantAction =
  | { kind: "openFolder"; sourceId: string; relPath: string }
  | { kind: "showDocument"; sourceId: string; relPath: string }
  | { kind: "search"; text: string; tags: string[]; tagMode: TagMode; favoritesOnly: boolean };

// Export… (#107)
export type DocPlace = { sourceId: string; relPath: string };

export interface ExportPlan {
  documents: number;
  size: number;
  /** Online-only documents, which copying downloads. */
  online: number;
  /** Notes go along by default as Share… sends them (Settings). */
  includeNotes: boolean;
}

export interface ExportOptions {
  format: "zip" | "folder";
  includeNotes: boolean;
}

export interface ExportDone {
  path: string;
  copied: number;
  failed: [string, string][];
}

export interface ExportProgress {
  done: number;
  total: number;
  name: string;
  downloading: boolean;
}

export function onExportProgress(handler: (p: ExportProgress) => void): Promise<UnlistenFn> {
  return listen<ExportProgress>("export-progress", (e) => handler(e.payload));
}

/** A page an answer cites ("[Lease 2024, p. 3]", #106). */
export interface Citation {
  /** As written in the answer. */
  text: string;
  sourceId: string;
  relPath: string;
  /** 0-based. */
  page: number;
  /** The line on that page to highlight. */
  passage: string | null;
}

/** Where words are on a page (#106): rectangles `[x, y, width, height]` in
 * fractions of the page as shown, from the top left. */
export interface FoundHit {
  page: number;
  rects: [number, number, number, number][];
}

export interface AssistantReply {
  text: string;
  steps: AssistantStep[];
  actions: AssistantAction[];
  citations: Citation[];
  model: string;
  cloud: boolean;
  /** Stopped by the user: nothing was answered or shown. */
  stopped: boolean;
}

export interface FolderChoice {
  sourceId: string;
  dir: string;
  /** "Source/dir" */
  label: string;
}

export interface Suggestion {
  title: string | null;
  documentDate: string | null;
  tags: string[];
  folder: string | null;
  reason: string | null;
  model: string;
}

export type Stage = "waiting" | "recognizing" | "suggesting" | "ready";

export interface IntakeItem {
  sourceId: string;
  relPath: string;
  name: string;
  size: number;
  modifiedMs: number | null;
  cloud: CloudState;
  stage: Stage;
  progress: [number, number] | null;
  ocr: string | null;
  suggestion: Suggestion | null;
  suggestedFolder: FolderChoice | null;
  folders: FolderChoice[];
  note: string | null;
}

export interface Filing {
  title: string | null;
  documentDate: string | null;
  tags: string[];
  notes: string | null;
  favorite: boolean;
  destSourceId: string;
  destDir: string;
  fileName: string;
}

export interface Filed {
  sourceId: string;
  relPath: string;
}

export interface AppInfo {
  name: string;
  version: string;
  coreVersion: string;
  os: string;
  arch: string;
  /** "direct" (GitHub installers, self-updating) or "app-store". */
  edition: "direct" | "app-store";
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  listSources: () => invoke<Source[]>("list_sources"),
  refreshSources: () => invoke<Source[]>("refresh_sources"),
  addSource: (path: string) => invoke<Source>("add_source", { path }),
  /** Picks an unavailable source's folder again (moved, or access lost). */
  relocateSource: (id: string, path: string) => invoke<Source>("relocate_source", { id, path }),
  removeSource: (id: string) => invoke<void>("remove_source", { id }),
  updateSource: (
    id: string,
    changes: { name?: string; showAllTypes?: boolean; downloadOnlineOnly?: boolean },
  ) =>
    invoke<Source>("update_source", { id, ...changes }),
  listDir: (sourceId: string, relPath: string) =>
    invoke<ListedEntry[]>("list_dir", { sourceId, relPath }),
  openEntry: (sourceId: string, relPath: string) =>
    invoke<void>("open_entry", { sourceId, relPath }),
  /** Moves a document (with its sidecar) or a folder to the Trash / Recycle Bin. */
  trashEntry: (sourceId: string, relPath: string) => invoke<void>("trash_entry", { sourceId, relPath }),
  /** Moves a document (with its details) or a folder into `destDir` of
   * `destSourceId`; resolves to its new path there. */
  moveEntry: (sourceId: string, relPath: string, destSourceId: string, destDir: string) =>
    invoke<string>("move_entry", { sourceId, relPath, destSourceId, destDir }),
  /** Copies a file dropped from another app into a folder, keeping its name. Resolves to its path. */
  copyIn: async (sourceId: string, destDir: string, file: File) => {
    const into = { sourceId, destDir, name: file.name, modifiedMs: file.lastModified || null };
    // Headers are ASCII: escape everything else the JSON way.
    const header = JSON.stringify(into).replace(/[\u007f-￿]/g, (c) => "\\u" + c.charCodeAt(0).toString(16).padStart(4, "0"));
    return invoke<string>("copy_in", new Uint8Array(await file.arrayBuffer()), { headers: { "rupert-copy": header } });
  },
  // Import into Inbox (#122)
  /** What chosen files and folders bring in. */
  inboxImportPlan: (paths: string[]) => invoke<ImportPlan>("inbox_import_plan", { paths }),
  /** Copies one document into the inbox, unless it's already in the library. */
  inboxImportFile: (path: string) => invoke<Imported>("inbox_import_file", { path }),
  /** The same for a file dropped on the Import window (its contents). */
  inboxImportDropped: async (file: File) => {
    const header = JSON.stringify({ name: file.name, modifiedMs: file.lastModified || null }).replace(
      /[\u007f-\uffff]/g,
      (c) => "\\u" + c.charCodeAt(0).toString(16).padStart(4, "0"),
    );
    return invoke<Imported>("inbox_import_dropped", new Uint8Array(await file.arrayBuffer()), {
      headers: { "rupert-import": header },
    });
  },
  /** A Markdown file's text (its first `limit` bytes if given); null when it's online-only. */
  readMarkdown: (sourceId: string, relPath: string, limit?: number) =>
    invoke<string | null>("read_markdown", { sourceId, relPath, limit: limit ?? null }),
  /** Pages in a document (1 for non-PDFs); null for online-only files. */
  pageCount: (sourceId: string, relPath: string) => invoke<number | null>("page_count", { sourceId, relPath }),
  revealEntry: (sourceId: string, relPath: string) =>
    invoke<void>("reveal_entry", { sourceId, relPath }),
  readMeta: (sourceId: string, relPath: string, allowDownload = false) =>
    invoke<MetaResponse>("read_meta", { sourceId, relPath, allowDownload }),
  writeMeta: (sourceId: string, relPath: string, meta: DocMeta) =>
    invoke<MetaRecord>("write_meta", { sourceId, relPath, meta }),
  /** Stars or unstars a document at once (an online-only PDF is downloaded only to unstar it). */
  setFavorite: (sourceId: string, relPath: string, favorite: boolean) =>
    invoke<DocMeta>("set_favorite", { sourceId, relPath, favorite }),
  /** Downloads an online-only document; Rupert then reads it. */
  downloadFile: (sourceId: string, relPath: string) => invoke<void>("download_file", { sourceId, relPath }),
  search: (query: SearchArgs) => invoke<Hit[]>("search", { query }),
  /** Where words (or, with `phrase`, a passage) are on a local PDF's pages (#106). */
  findInDocument: (sourceId: string, relPath: string, text: string, phrase = false) =>
    invoke<FoundHit[]>("find_in_document", { sourceId, relPath, text, phrase }),
  /** Every indexed document in a folder and its subfolders (Export folder…). */
  exportFolderDocs: (sourceId: string, relPath: string) =>
    invoke<DocPlace[]>("export_folder_docs", { sourceId, relPath }),
  exportPlan: (docs: DocPlace[]) => invoke<ExportPlan>("export_plan", { docs }),
  /** Copies `docs` into `dest` (a new folder or a zip) with a list headed `title`. */
  exportRun: (docs: DocPlace[], options: ExportOptions, dest: string, title: string) =>
    invoke<ExportDone>("export_run", { docs, options, dest, title }),
  exportCancel: () => invoke<void>("export_cancel"),
  /** Show in Finder / File Explorer for an export. */
  exportReveal: (path: string) => invoke<void>("export_reveal", { path }),
  /** Share… for a file Rupert made (an exported zip). */
  shareFile: (path: string, at: ShareAt) => invoke<void>("share_file", { path, at }),
  /** Quick open (#105): documents by title or file name, best first. */
  quickFind: (text: string) => invoke<Hit[]>("quick_find", { text }),
  /** These documents' indexed details, in order, without any no longer indexed. */
  documentsInfo: (docs: { sourceId: string; relPath: string }[]) => invoke<Hit[]>("documents_info", { docs }),
  listTags: () => invoke<TagCount[]>("list_tags"),
  /** Tags matching `filter`, best first, without the selected ones (with
   * "all", only those that occur together with the selection). */
  tagSuggestions: (filter: string, selected: string[], mode: TagMode, limit: number) =>
    invoke<TagList>("tag_suggestions", { filter, selected, mode, limit }),
  assistantInfo: () => invoke<AssistantInfo>("assistant_info"),
  assistantSend: (text: string) => invoke<AssistantReply>("assistant_send", { text }),
  assistantReset: () => invoke<void>("assistant_reset"),
  assistantCancel: () => invoke<void>("assistant_cancel"),
  /** Opens the help window, at a section of it if given. */
  openHelp: (section?: string) => invoke<void>("open_help", { section: section ?? null }),
  /** Opens the Settings window (#80) at a tab: "general", "library", "inbox",
   * "text", "local-ai", "assistant", or "updates" (General, checking now). */
  openSettings: (tab?: string) => invoke<void>("open_settings", { tab: tab ?? null }),
  assistantSetKey: (provider: AssistantProvider, key: string | null) =>
    invoke<void>("assistant_set_key", { provider, key }),
  assistantHasKey: (provider: AssistantProvider) => invoke<boolean>("assistant_has_key", { provider }),
  assistantCliStatus: () => invoke<{ installed: boolean; signedIn: boolean }>("assistant_cli_status"),
  assistantCliLogin: () => invoke<void>("assistant_cli_login"),
  chatgptStatus: () => invoke<ChatGptStatus>("chatgpt_status"),
  chatgptSignIn: () => invoke<ChatGptStatus>("chatgpt_sign_in"),
  chatgptSignOut: () => invoke<void>("chatgpt_sign_out"),
  favoriteFolders: () => invoke<FavoriteFolder[]>("favorite_folders"),
  folderFavorite: (sourceId: string, relPath: string) => invoke<boolean>("folder_favorite", { sourceId, relPath }),
  setFolderFavorite: (sourceId: string, relPath: string, favorite: boolean) =>
    invoke<void>("set_folder_favorite", { sourceId, relPath, favorite }),
  tagEditPreview: (edit: TagEdit) => invoke<TagEditPreview>("tag_edit_preview", { edit }),
  tagEditApply: (edit: TagEdit) => invoke<TagEditReport>("tag_edit_apply", { edit }),
  /** Undo (#104): tags back as they were, where they're still as the edit left them. */
  tagUndo: (changes: TagChange[]) => invoke<TagEditReport>("tag_undo", { changes }),
  /** Undo (#104): items back to their old place and name, if they're still where Rupert left them. */
  moveBack: (items: MoveBackItem[]) =>
    invoke<{ done: number; failed: { path: string; error: string }[] }>("move_back", { items }),
  /** Edit ▸ Undo's title on the Mac ("Undo Move of 3 Documents"); null for plain "Undo". */
  setUndoLabel: (label: string | null) => invoke<void>("set_undo_label", { label }),
  indexStatus: () => invoke<{ stats: IndexStats; progress: Progress }>("index_status"),
  stopIndexing: () => invoke<void>("stop_indexing"),
  resumeIndexing: () => invoke<void>("resume_indexing"),
  indexFolder: (sourceId: string, relPath: string) => invoke<void>("index_folder", { sourceId, relPath }),
  /** Starts the system's drag of documents out of the window (#84). */
  dragOut: (
    sourceId: string,
    relPaths: string[],
    start: { clientX: number; clientY: number; screenX: number; screenY: number },
  ) => invoke<void>("drag_out", { sourceId, relPaths, start }),
  /** A sharp picture of a page for the page viewer: a data: URL, or null. */
  pageImage: (sourceId: string, relPath: string, page: number, side: number) =>
    invoke<string | null>("page_image", { sourceId, relPath, page, side }),
  /** Makes a folder inside `parent`; resolves to its path (#63). */
  createFolder: (sourceId: string, parent: string, name: string) =>
    invoke<string>("create_folder", { sourceId, parent, name }),
  /** Renames a document or folder in place; resolves to its new path (#65). */
  renameEntry: (sourceId: string, relPath: string, newName: string) =>
    invoke<string>("rename_entry", { sourceId, relPath, newName }),
  /** Turns Print to Rupert on or off; resolves to the settings as saved. */
  setPrintEntry: (on: boolean) => invoke<Settings>("set_print_entry", { on }),
  // Share… and Email… (#69)
  /** The system's share menu for a document, at `at` on the page (the
   * pointer, or a button). `name`: send it under this name (an inbox scan). */
  shareDocument: (sourceId: string, relPath: string, name: string | null, at: ShareAt) =>
    invoke<void>("share_document", { sourceId, relPath, name, at }),
  /** A new email with the document attached and its title as the subject. */
  emailDocument: (sourceId: string, relPath: string, name: string | null) =>
    invoke<void>("email_document", { sourceId, relPath, name }),
  /** Share… and Email… for several documents at once (#102): one share, one email. */
  shareDocuments: (docs: { sourceId: string; relPath: string }[], at: ShareAt) =>
    invoke<void>("share_documents", { docs, at }),
  emailDocuments: (docs: { sourceId: string; relPath: string }[]) => invoke<void>("email_documents", { docs }),
  /** Drops a send that's waiting for an online-only document to download. */
  shareCancel: () => invoke<void>("share_cancel"),
  // Rename documents… (#64)
  /** A folder's documents, with names from the name pattern; `ai`: whether
   * Local AI can propose names. */
  renameRows: (sourceId: string, relDir: string) =>
    invoke<{ rows: RenameRow[]; ai: boolean }>("rename_rows", { sourceId, relDir }),
  /** The names documents would really get (taken → " (2)", not allowed, unchanged). */
  renameCheck: (sourceId: string, relDir: string, planned: { relPath: string; stem: string }[]) =>
    invoke<RenameChecked[]>("rename_check", { sourceId, relDir, planned }),
  /** Names "the same way" as the typed ones, if they're a pattern of
   * details; null when they aren't. */
  renameSameWay: (typed: { details: RenameDetails; name: string }[], targets: RenameDetails[]) =>
    invoke<(string | null)[] | null>("rename_same_way", { typed, targets }),
  /** Starts Local AI's proposals; they arrive with `onRenameProposal`. Resolves to the job's number. */
  renamePropose: (
    sourceId: string,
    instruction: string,
    examples: { details: RenameDetails; name: string }[],
    relPaths: string[],
  ) => invoke<number>("rename_propose", { sourceId, instruction, examples, relPaths }),
  renameStop: () => invoke<void>("rename_stop"),
  renameApply: (sourceId: string, steps: { relPath: string; name: string }[], setTitle: boolean) =>
    invoke<RenameOutcome>("rename_apply", { sourceId, steps, setTitle }),
  renameUndo: (sourceId: string, renamed: Renamed[]) => invoke<RenameOutcome>("rename_undo", { sourceId, renamed }),
  scansnapLibrary: () => invoke<string | null>("scansnap_library"),
  scansnapPlan: (library: string) => invoke<ImportSummary>("scansnap_plan", { library }),
  scansnapApply: () => invoke<void>("scansnap_apply"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  ocrEngine: () => invoke<string | null>("ocr_engine"),
  lmModels: (url: string) => invoke<string[]>("lm_models", { url }),
  localAiPlace: (url: string) => invoke<LocalAiPlace>("local_ai_place", { url }),
  appleAiStatus: () => invoke<AppleAiStatus>("apple_ai_status"),
  inboxItems: () => invoke<IntakeItem[]>("inbox_items"),
  /** Inbox items whose name or recognized text match `text` (relative paths). */
  inboxSearch: (text: string) => invoke<string[]>("inbox_search", { text }),
  inboxRedo: (relPath: string) => invoke<void>("inbox_redo", { relPath }),
  allFolders: () => invoke<FolderChoice[]>("all_folders"),
  fileDocument: (sourceId: string, relPath: string, filing: Filing) =>
    invoke<Filed>("file_document", { sourceId, relPath, filing }),
  /** A QR code as SVG; null if the text is too long for one. */
  qrSvg: (text: string) => invoke<string | null>("qr_svg", { text }),
  /** macOS: the Continuity Camera menu (nearby iPhones' Scan Documents)
   * at a point of the page (a button's bottom left). */
  continuityMenu: (x: number, y: number) => invoke<void>("continuity_menu", { x, y }),
  preview: (sourceId: string, relPath: string, page = 0) =>
    invoke<string | null>("preview", { sourceId, relPath, page }),
};

export function onIntakeChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("intake-changed", () => handler());
}

/** Where on the page the share menu opens. */
export type ShareAt = { x: number; y: number; width?: number; height?: number };

/** Where a button is, for the share menu to open beside it. */
export function atElement(el: Element): ShareAt {
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, width: r.width, height: r.height };
}

/** While an online-only document downloads to be sent: its name; then null. */
export function onShareProgress(handler: (downloading: string | null) => void): Promise<UnlistenFn> {
  return listen<{ downloading: string | null }>("share-progress", (e) => handler(e.payload.downloading));
}

export interface ImportChosen {
  path: string;
  name: string;
  isDir: boolean;
  documents: number;
}

export interface ImportPlan {
  chosen: ImportChosen[];
  files: string[];
  leftOut: number;
}

export type Imported =
  | { kind: "copied"; relPath: string }
  | { kind: "duplicate"; sourceId: string; relPath: string }
  | { kind: "leftOut" };

/** File ▸ Import into Inbox… (macOS menu). */
export function onMenuImportInbox(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-import-inbox", () => handler());
}

export interface RenameRow {
  relPath: string;
  name: string;
  stem: string;
  /** With its dot (".pdf"), or "". */
  ext: string;
  title: string | null;
  documentDate: string | null;
  tags: string[];
  cloud: CloudState;
  kind: FileKind | null;
  size: number;
  modifiedMs: number | null;
  hasText: boolean;
  patternName: string | null;
}

export interface RenameDetails {
  stem: string;
  title: string | null;
  documentDate: string | null;
  tags: string[];
}

export type RenameProblem = { kind: "unchanged" } | { kind: "taken" } | { kind: "invalid"; message: string };

export interface RenameChecked {
  relPath: string;
  /** The full new name, with extension (and " (2)" if needed). */
  name: string;
  problem: RenameProblem | null;
}

export interface Renamed {
  from: string;
  to: string;
  titleChanged: boolean;
  oldTitle: string | null;
}

export interface RenameOutcome {
  renamed: Renamed[];
  failed: { relPath: string; message: string }[];
}

export interface RenameProposal {
  job: number;
  relPath: string;
  name: string | null;
  error: string | null;
}

export function onRenameProposal(handler: (p: RenameProposal) => void): Promise<UnlistenFn> {
  return listen<RenameProposal>("rename-proposal", (e) => handler(e.payload));
}

export function onRenameProposalsDone(handler: (d: { job: number; stopped: boolean }) => void): Promise<UnlistenFn> {
  return listen<{ job: number; stopped: boolean }>("rename-proposals-done", (e) => handler(e.payload));
}

/** A Continuity Camera scan arrived in the inbox (`name`), or couldn't be
 * saved (`error`). macOS. */
export function onContinuityScan(handler: (r: { name: string | null; error: string | null }) => void): Promise<UnlistenFn> {
  return listen<{ name: string | null; error: string | null }>("continuity-scan", (e) => handler(e.payload));
}

/** File ▸ Quick Open… (macOS menu, #105). */
export function onMenuQuickOpen(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-quick-open", () => handler());
}

/** Edit ▸ Undo (macOS menu). */
export function onMenuUndo(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-undo", () => handler());
}

/** Settings were saved, in any window. */
export function onSettingsChanged(handler: (s: Settings) => void): Promise<UnlistenFn> {
  return listen<Settings>("settings-changed", (e) => handler(e.payload));
}

/** Sources were added, removed, renamed or changed, in any window. */
export function onSourcesChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("sources-changed", () => handler());
}

/** The Settings window is asked to show a tab. */
export function onSettingsShow(handler: (tab: string) => void): Promise<UnlistenFn> {
  return listen<string>("settings-show", (e) => handler(e.payload));
}

export function onMenuSettings(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-settings", () => handler());
}

export function onMenuCheckUpdates(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-check-updates", () => handler());
}

/** The file name (without extension) from the naming pattern. */
export function fileNameFor(pattern: string, date: string | null, title: string): string {
  return pattern
    .replace("{date}", date ?? "")
    .replace("{title}", title)
    .split(/\s+/)
    .filter(Boolean)
    .join(" ")
    .replace(/^[\s_-]+|[\s_-]+$/g, "");
}

export function onImportProgress(handler: (p: ImportProgress) => void): Promise<UnlistenFn> {
  return listen<ImportProgress>("import-progress", (e) => handler(e.payload));
}

export function onMenuImport(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-import-scansnap", () => handler());
}

export function onImportDone(handler: (r: ImportReport) => void): Promise<UnlistenFn> {
  return listen<ImportReport>("import-done", (e) => handler(e.payload));
}

export function onIndexProgress(handler: (p: Progress) => void): Promise<UnlistenFn> {
  return listen<Progress>("index-progress", (e) => handler(e.payload));
}

export function onIndexChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("index-changed", () => handler());
}

/** Splits a snippet into plain and highlighted runs. */
export function snippetParts(snippet: string): { text: string; hit: boolean }[] {
  const parts: { text: string; hit: boolean }[] = [];
  let hit = false;
  let buf = "";
  for (const ch of snippet) {
    if (ch === "\uE000" || ch === "\uE001") {
      if (buf) parts.push({ text: buf, hit });
      buf = "";
      hit = ch === "\uE000";
    } else buf += ch;
  }
  if (buf) parts.push({ text: buf, hit });
  return parts;
}

export function onDirChanged(handler: (changes: DirChange[]) => void): Promise<UnlistenFn> {
  return listen<DirChange[]>("dir-changed", (e) => handler(e.payload));
}

export function providerLabel(provider: Provider): string {
  const names: Record<Provider, string> = {
    local: m.places.thisComputer,
    icloud: "iCloud Drive",
    onedrive: "OneDrive",
    googledrive: "Google Drive",
    dropbox: "Dropbox",
  };
  return names[provider];
}

/** Which build someone runs, for support: "GitHub" (installers from
 * GitHub releases), "App Store", or "development" (run from the source). */
export function editionLabel(info: AppInfo | null): string {
  if (import.meta.env.DEV) return m.main.development;
  return info?.edition === "app-store" ? "App Store" : "GitHub";
}

export function revealLabel(os: string | undefined): string {
  if (os === "macos") return m.places.showInFinder;
  if (os === "windows") return m.places.showInExplorer;
  return m.places.showInFiles;
}

export const onTagEditProgress = (f: (p: { done: number; total: number }) => void): Promise<UnlistenFn> =>
  listen<{ done: number; total: number }>("tag-edit-progress", (e) => f(e.payload));

export const onAssistantStep = (f: (step: AssistantStep) => void): Promise<UnlistenFn> =>
  listen<AssistantStep>("assistant-step", (e) => f(e.payload));
