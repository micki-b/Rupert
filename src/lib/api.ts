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
    invoke<Entry[]>("list_dir", { sourceId, relPath }),
  openEntry: (sourceId: string, relPath: string) =>
    invoke<void>("open_entry", { sourceId, relPath }),
  revealEntry: (sourceId: string, relPath: string) =>
    invoke<void>("reveal_entry", { sourceId, relPath }),
  readMeta: (sourceId: string, relPath: string, allowDownload = false) =>
    invoke<MetaResponse>("read_meta", { sourceId, relPath, allowDownload }),
  writeMeta: (sourceId: string, relPath: string, meta: DocMeta) =>
    invoke<MetaRecord>("write_meta", { sourceId, relPath, meta }),
};

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
