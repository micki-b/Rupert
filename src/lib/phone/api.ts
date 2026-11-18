// Typed wrappers around the phone app's commands (mobile/src/lib.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Entry, FavoriteFolder, Hit, IndexStats, Progress, Provider, TagCount, TagMode } from "$lib/api";

export type { FavoriteFolder };

export type { Entry, Hit, Progress, TagCount };

export interface PhoneSource {
  id: string;
  name: string;
  provider: Provider;
  available: boolean;
}

export interface Overview {
  /** Saved folders have been reopened. */
  ready: boolean;
  sources: PhoneSource[];
  /** Name of the folder scans go to. */
  inbox: string | null;
  inboxOnOnedrive: boolean;
  stats: IndexStats;
  progress: Progress;
}

export interface Listed extends Entry {
  meta: { title: string | null; tags: string[]; favorite: boolean; documentDate: string | null } | null;
}

export interface DocInfo extends Hit {
  notes: string | null;
  textState: "extracted" | "catalog" | "sidecar" | "empty" | "cloudOnly" | "unsupported" | "failed";
}

export type ScanMode = "color" | "gray" | "bw";

export interface ScanSettings {
  mode: ScanMode;
  /** Output resolution (the page's long side taken as 11 inches). */
  dpi: number;
}

export interface OneDriveStatus {
  /** This build has the app's Microsoft registration. */
  configured: boolean;
  signedIn: boolean;
  account: string | null;
  /** The last indexing problem, if any. */
  error: string | null;
  progress: Progress;
}

export interface OneDriveFolder {
  id: string;
  name: string;
}

export const phone = {
  overview: () => invoke<Overview>("overview"),
  refresh: () => invoke<void>("refresh"),
  addFolder: () => invoke<boolean>("add_folder"),
  removeFolder: (id: string) => invoke<void>("remove_folder", { id }),
  chooseInbox: () => invoke<boolean>("choose_inbox"),
  listDir: (sourceId: string, relPath: string) => invoke<Listed[]>("list_dir", { sourceId, relPath }),
  search: (text: string, tags: string[], tagMode: TagMode, favoritesOnly: boolean) =>
    invoke<Hit[]>("search", { query: { text, tags, tagMode, favoritesOnly } }),
  tags: () => invoke<TagCount[]>("list_tags"),
  favoriteFolders: () => invoke<FavoriteFolder[]>("favorite_folders"),
  setFolderFavorite: (sourceId: string, relPath: string, favorite: boolean) =>
    invoke<void>("set_folder_favorite", { sourceId, relPath, favorite }),
  document: (sourceId: string, relPath: string) => invoke<DocInfo | null>("document", { sourceId, relPath }),
  open: (sourceId: string, relPath: string) => invoke<void>("open_document", { sourceId, relPath }),
  /** The saved file name, or null if the scan was cancelled. */
  scan: (name: string, settings: ScanSettings) => invoke<string | null>("scan", { name, settings }),
  onedriveStatus: () => invoke<OneDriveStatus>("onedrive_status"),
  /** `false` if cancelled. */
  onedriveSignIn: () => invoke<boolean>("onedrive_sign_in"),
  onedriveSignOut: () => invoke<void>("onedrive_sign_out"),
  /** Subfolders; `null` for the top level. */
  onedriveFolders: (id: string | null) => invoke<OneDriveFolder[]>("onedrive_folders", { id }),
  addOnedriveFolder: (itemId: string, name: string) => invoke<void>("add_onedrive_folder", { itemId, name }),
  setOnedriveInbox: (itemId: string, name: string) => invoke<void>("set_onedrive_inbox", { itemId, name }),
};

export const onProgress = (f: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("index-progress", (e) => f(e.payload));
export const onIndexChanged = (f: () => void): Promise<UnlistenFn> => listen("index-changed", () => f());
export const onRemoteProgress = (f: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("remote-progress", (e) => f(e.payload));
export const onFoldersReady = (f: () => void): Promise<UnlistenFn> => listen("folders-ready", () => f());

/** "Scan 2026-11-18 15.04.12", in local time (same as the desktop). */
export function scanName(d = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `Scan ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
}

/** "2026-03-14" → "Mar 14, 2026". */
export function formatDocDate(iso: string | null): string {
  if (!iso) return "";
  const [y, m, d] = iso.split("-").map(Number);
  return new Date(y, m - 1, d).toLocaleDateString(undefined, { dateStyle: "medium" });
}
