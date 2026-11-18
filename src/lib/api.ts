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
}

export interface IndexStats {
  documents: number;
  withText: number;
  withoutText: number;
  cloudOnlyUnindexed: number;
  favorites: number;
}

export interface SearchArgs {
  text?: string;
  sourceId?: string | null;
  tag?: string | null;
  favoritesOnly?: boolean;
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

export interface AppInfo {
  name: string;
  version: string;
  coreVersion: string;
  os: string;
  arch: string;
}

export const api = {
  appInfo: () => invoke<AppInfo>("app_info"),
  listSources: () => invoke<Source[]>("list_sources"),
  refreshSources: () => invoke<Source[]>("refresh_sources"),
  addSource: (path: string) => invoke<Source>("add_source", { path }),
  removeSource: (id: string) => invoke<void>("remove_source", { id }),
  updateSource: (id: string, changes: { name?: string; showAllTypes?: boolean }) =>
    invoke<Source>("update_source", { id, ...changes }),
  listDir: (sourceId: string, relPath: string) =>
    invoke<ListedEntry[]>("list_dir", { sourceId, relPath }),
  openEntry: (sourceId: string, relPath: string) =>
    invoke<void>("open_entry", { sourceId, relPath }),
  revealEntry: (sourceId: string, relPath: string) =>
    invoke<void>("reveal_entry", { sourceId, relPath }),
  readMeta: (sourceId: string, relPath: string, allowDownload = false) =>
    invoke<MetaResponse>("read_meta", { sourceId, relPath, allowDownload }),
  writeMeta: (sourceId: string, relPath: string, meta: DocMeta) =>
    invoke<MetaRecord>("write_meta", { sourceId, relPath, meta }),
  search: (query: SearchArgs) => invoke<Hit[]>("search", { query }),
  listTags: () => invoke<TagCount[]>("list_tags"),
  indexStatus: () => invoke<{ stats: IndexStats; progress: Progress }>("index_status"),
  scansnapLibrary: () => invoke<string | null>("scansnap_library"),
  scansnapPlan: (library: string) => invoke<ImportSummary>("scansnap_plan", { library }),
  scansnapApply: () => invoke<void>("scansnap_apply"),
};

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

export function revealLabel(os: string | undefined): string {
  if (os === "macos") return "Show in Finder";
  if (os === "windows") return "Show in Explorer";
  return "Show in Files";
}
