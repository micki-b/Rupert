// Typed wrappers around the phone app's commands (mobile/src/lib.rs).

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { locale, type LanguageInfo } from "#lib/i18n/index.ts";
import type {
  AssistantAction,
  AssistantStep,
  Entry,
  FavoriteFolder,
  Hit,
  IndexStats,
  Progress,
  Provider,
  TagCount,
  TagMode,
} from "#lib/api.ts";

export type { AssistantAction, AssistantStep, FavoriteFolder };

export type { Entry, Hit, Progress, TagCount };

export interface PhoneSource {
  id: string;
  name: string;
  provider: Provider;
  available: boolean;
  /** Added only so the inbox shows in the library; not a library folder. */
  inbox: boolean;
}

export interface AppVersion {
  /** "0.1.1". */
  version: string;
  /** Apple's build number, when it differs from the version. */
  build: string | null;
  /** A debug build, not one from the App Store. */
  development: boolean;
}

export interface Overview {
  /** Saved folders have been reopened. */
  ready: boolean;
  sources: PhoneSource[];
  /** Name of the folder scans go to. */
  inbox: string | null;
  /** The cloud service it's on; null: a folder in Files. */
  inboxService: CloudService | null;
  /** Where to browse the inbox in the library. */
  inboxPlace: { sourceId: string; relPath: string } | null;
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

/** Ask Rupert on this iPhone (Apple Intelligence). */
export interface AssistantStatus {
  /** It can answer now. */
  available: boolean;
  /** The iPhone can run Apple Intelligence: offer Ask Rupert, with `message`. */
  capable: boolean;
  /** Why it can't answer now. */
  message: string | null;
}

export interface AssistantReply {
  text: string;
  steps: AssistantStep[];
  actions: AssistantAction[];
  /** Stopped; nothing was answered or shown. */
  stopped: boolean;
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

/** A cloud service the phone reaches through its web API. */
export type CloudService = "onedrive" | "googledrive";

/** Google Drive's status has the same shape as OneDrive's. */
export type CloudStatus = OneDriveStatus;

export const phone = {
  overview: () => invoke<Overview>("overview"),
  appVersion: () => invoke<AppVersion>("app_version"),
  setLanguage: (language: string | null) => invoke<LanguageInfo>("set_language", { language }),
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
  /** `path`: the folders from the top of OneDrive down to the inbox. */
  setOnedriveInbox: (itemId: string, name: string, path: OneDriveFolder[]) =>
    invoke<void>("set_onedrive_inbox", { itemId, name, path }),
  assistantStatus: () => invoke<AssistantStatus>("assistant_status"),
  assistantSend: (text: string) => invoke<AssistantReply>("assistant_send", { text }),
  assistantCancel: () => invoke<void>("assistant_cancel"),
  assistantReset: () => invoke<void>("assistant_reset"),
  gdriveStatus: () => invoke<CloudStatus>("gdrive_status"),
  /** `false` if cancelled. */
  gdriveSignIn: () => invoke<boolean>("gdrive_sign_in"),
  gdriveSignOut: () => invoke<void>("gdrive_sign_out"),
  /** Subfolders; `null` for the top of My Drive. */
  gdriveFolders: (id: string | null) => invoke<OneDriveFolder[]>("gdrive_folders", { id }),
  addGdriveFolder: (itemId: string, name: string) => invoke<void>("add_gdrive_folder", { itemId, name }),
  /** `path`: the folders from the top of My Drive down to the inbox. */
  setGdriveInbox: (itemId: string, name: string, path: OneDriveFolder[]) =>
    invoke<void>("set_gdrive_inbox", { itemId, name, path }),
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
  return new Date(y, m - 1, d).toLocaleDateString(locale(), { dateStyle: "medium" });
}
export const onAssistantStep = (f: (step: AssistantStep) => void): Promise<UnlistenFn> =>
  listen<AssistantStep>("assistant-step", (e) => f(e.payload));
