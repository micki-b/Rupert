import { locale, m } from "#lib/i18n/index.ts";
import type { FileKind } from "#lib/api.ts";

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit++;
  }
  const shown = value < 10 ? Math.round(value * 10) / 10 : Math.round(value);
  return `${shown.toLocaleString(locale(), { minimumFractionDigits: value < 10 ? 1 : 0 })} ${units[unit]}`;
}

export function formatDate(ms: number | null): string {
  return ms == null ? "" : new Date(ms).toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" });
}

/** The day only ("Nov 24, 2026"), for narrow columns. */
export function formatDay(ms: number | null): string {
  return ms == null ? "" : new Date(ms).toLocaleDateString(locale(), { dateStyle: "medium" });
}

/** "PDF", "Image", … for a file kind (or "Folder"). */
export function kindLabel(kind: FileKind | "folder" | null): string {
  return m.kinds[kind ?? "other"];
}
