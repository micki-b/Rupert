// The star and the cloud badge on a document in a list: starring saves at
// once (into the file, or beside it), and clicking the cloud downloads that
// one online-only file so Rupert can read it.
import { SvelteSet } from "svelte/reactivity";
import { api, type Entry } from "#lib/api.ts";

type Doc = Entry & { favorite?: boolean };

/** Documents with a star or download under way ("sourceId\nrelPath"). */
const busy = new SvelteSet<string>();
const key = (sourceId: string, relPath: string) => `${sourceId}\n${relPath}`;

export function isBusy(sourceId: string, relPath: string): boolean {
  return busy.has(key(sourceId, relPath));
}

async function guarded(sourceId: string, doc: Doc, action: () => Promise<void>) {
  const k = key(sourceId, doc.relPath);
  if (busy.has(k)) return;
  busy.add(k);
  try {
    await action();
  } finally {
    busy.delete(k);
  }
}

/** Stars or unstars a document; the entry shows the result. */
export function toggleFavorite(sourceId: string, doc: Doc): Promise<void> {
  return guarded(sourceId, doc, async () => {
    const meta = await api.setFavorite(sourceId, doc.relPath, !doc.favorite);
    doc.favorite = meta.favorite;
  });
}

/** Downloads an online-only document; the entry then shows it as local. */
export function download(sourceId: string, doc: Doc): Promise<void> {
  return guarded(sourceId, doc, async () => {
    await api.downloadFile(sourceId, doc.relPath);
    doc.cloud = "local";
  });
}

/** The star's tooltip: unstarring an online-only PDF downloads it. */
export function starTitle(doc: Doc, t: { add: string; remove: string; removeDownloads: string }): string {
  if (!doc.favorite) return t.add;
  return doc.kind === "pdf" && doc.cloud === "cloudOnly" ? t.removeDownloads : t.remove;
}
