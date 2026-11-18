// Typed wrappers around the phone app's commands (mobile/src/lib.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Entry, Hit, IndexStats, Progress, Provider, TagCount } from "$lib/api";

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

export const phone = {
  overview: () => invoke<Overview>("overview"),
  refresh: () => invoke<void>("refresh"),
  addFolder: () => invoke<boolean>("add_folder"),
  removeFolder: (id: string) => invoke<void>("remove_folder", { id }),
  chooseInbox: () => invoke<boolean>("choose_inbox"),
  listDir: (sourceId: string, relPath: string) => invoke<Listed[]>("list_dir", { sourceId, relPath }),
  search: (text: string, tag: string | null, favoritesOnly: boolean) =>
    invoke<Hit[]>("search", { query: { text, tag, favoritesOnly } }),
  tags: () => invoke<TagCount[]>("list_tags"),
  document: (sourceId: string, relPath: string) => invoke<DocInfo | null>("document", { sourceId, relPath }),
  open: (sourceId: string, relPath: string) => invoke<void>("open_document", { sourceId, relPath }),
  /** The saved file name, or null if the scan was cancelled. */
  scan: (name: string) => invoke<string | null>("scan", { name }),
};

export const onProgress = (f: (p: Progress) => void): Promise<UnlistenFn> =>
  listen<Progress>("index-progress", (e) => f(e.payload));
export const onIndexChanged = (f: () => void): Promise<UnlistenFn> => listen("index-changed", () => f());
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
