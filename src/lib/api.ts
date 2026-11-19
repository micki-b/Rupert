// Typed wrappers around the Rust commands in src-tauri/src/lib.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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
  action: { kind: "rename"; to: string } | { kind: "delete" };
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
  lmStudioUrl: string;
  lmStudioModel: string | null;
  /** `{date}` and `{title}` are replaced. */
  namePattern: string;
  assistant: AssistantSettings;
  /** Desktop: look for a new version on GitHub now and then. */
  checkUpdates: boolean;
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

export interface AssistantReply {
  text: string;
  steps: AssistantStep[];
  actions: AssistantAction[];
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

export interface Scanner {
  name: string;
  /** eSCL root, e.g. http://192.168.1.20/eSCL */
  url: string;
}

export interface SourceCaps {
  colorModes: string[];
  resolutions: number[];
  formats: string[];
  maxWidth: number;
  maxHeight: number;
}

export interface Capabilities {
  makeAndModel: string | null;
  platen: SourceCaps | null;
  feeder: SourceCaps | null;
  duplex: boolean;
}

export type InputSource = "platen" | "feeder";
export type PageSize = "letter" | "legal" | "a4" | "max";

export interface ScanRequest {
  source: InputSource;
  duplex: boolean;
  color: "color" | "gray";
  resolution: number;
  pageSize: PageSize;
}

export interface ScanDone {
  sourceId: string | null;
  relPath: string | null;
  error: string | null;
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
  search: (query: SearchArgs) => invoke<Hit[]>("search", { query }),
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
  indexStatus: () => invoke<{ stats: IndexStats; progress: Progress }>("index_status"),
  scansnapLibrary: () => invoke<string | null>("scansnap_library"),
  scansnapPlan: (library: string) => invoke<ImportSummary>("scansnap_plan", { library }),
  scansnapApply: () => invoke<void>("scansnap_apply"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<Settings>("set_settings", { settings }),
  ocrEngine: () => invoke<string | null>("ocr_engine"),
  lmModels: (url: string) => invoke<string[]>("lm_models", { url }),
  inboxItems: () => invoke<IntakeItem[]>("inbox_items"),
  inboxRedo: (relPath: string) => invoke<void>("inbox_redo", { relPath }),
  allFolders: () => invoke<FolderChoice[]>("all_folders"),
  fileDocument: (sourceId: string, relPath: string, filing: Filing) =>
    invoke<Filed>("file_document", { sourceId, relPath, filing }),
  scannersDiscover: () => invoke<Scanner[]>("scanners_discover"),
  scannerAt: (address: string) => invoke<Scanner>("scanner_at", { address }),
  scannerCapabilities: (scanner: Scanner) => invoke<Capabilities>("scanner_capabilities", { scanner }),
  scanStart: (scanner: Scanner, request: ScanRequest, name: string) =>
    invoke<void>("scan_start", { scanner, request, name }),
  preview: (sourceId: string, relPath: string, page = 0) =>
    invoke<string | null>("preview", { sourceId, relPath, page }),
};

export function onIntakeChanged(handler: () => void): Promise<UnlistenFn> {
  return listen("intake-changed", () => handler());
}

export function onScanProgress(handler: (pages: number) => void): Promise<UnlistenFn> {
  return listen<number>("scan-progress", (e) => handler(e.payload));
}

export function onScanDone(handler: (done: ScanDone) => void): Promise<UnlistenFn> {
  return listen<ScanDone>("scan-done", (e) => handler(e.payload));
}

export function onMenuScan(handler: () => void): Promise<UnlistenFn> {
  return listen("menu-scan", () => handler());
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

export const providerLabel: Record<Provider, string> = {
  local: "This computer",
  icloud: "iCloud Drive",
  onedrive: "OneDrive",
  googledrive: "Google Drive",
  dropbox: "Dropbox",
};

/** Which build someone runs, for support: "GitHub" (installers from
 * GitHub releases), "App Store", or "development" (run from the source). */
export function editionLabel(info: AppInfo | null): string {
  if (import.meta.env.DEV) return "development";
  return info?.edition === "app-store" ? "App Store" : "GitHub";
}

export function revealLabel(os: string | undefined): string {
  if (os === "macos") return "Show in Finder";
  if (os === "windows") return "Show in Explorer";
  return "Show in Files";
}

export const onTagEditProgress = (f: (p: { done: number; total: number }) => void): Promise<UnlistenFn> =>
  listen<{ done: number; total: number }>("tag-edit-progress", (e) => f(e.payload));

export const onAssistantStep = (f: (step: AssistantStep) => void): Promise<UnlistenFn> =>
  listen<AssistantStep>("assistant-step", (e) => f(e.payload));
