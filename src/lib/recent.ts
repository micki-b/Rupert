// Recent documents for quick open (#105): the last few opened or looked at
// from Rupert, newest first. A convenience of this computer's window, kept
// in its local storage; nothing about the documents themselves. One that
// moved or went away drops out (the index no longer has it there).

const KEY = "rupert.recentDocuments";
const KEEP = 8;

export type DocPlace = { sourceId: string; relPath: string };

export function recentDocuments(): DocPlace[] {
  try {
    const list = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((d) => typeof d?.sourceId === "string" && typeof d?.relPath === "string") : [];
  } catch {
    return [];
  }
}

/** Puts a document at the top of the list. */
export function rememberDocument(sourceId: string, relPath: string) {
  const rest = recentDocuments().filter((d) => d.sourceId !== sourceId || d.relPath !== relPath);
  try {
    localStorage.setItem(KEY, JSON.stringify([{ sourceId, relPath }, ...rest].slice(0, KEEP)));
  } catch {
    // Not kept: quick open just shows fewer.
  }
}

