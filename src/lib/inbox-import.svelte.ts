// Import into Inbox (#122): copies what the Import window collected into
// the inbox one document at a time, so the inbox can show the progress at
// the top of its list (with Cancel) and fill as documents arrive. Files
// and folders chosen with Add files… / Add folder… are copied by path;
// dropped ones (whose paths the page doesn't get) by their contents.
import { api } from "./api";

/** What to import: chosen paths, and dropped files. */
export type Batch = { paths: string[]; files: File[] };

/** A document that was already in the library, and where. */
export type Duplicate = { name: string; sourceId: string; relPath: string };

export const importing = $state<{
  /** Copying now. */
  active: boolean;
  /** Finished (or stopped), with its summary still showing. */
  finished: boolean;
  stopped: boolean;
  total: number;
  done: number;
  copied: number;
  duplicates: Duplicate[];
  failed: { name: string; message: string }[];
  /** Not documents (or hidden), left out. */
  leftOut: number;
}>({
  active: false,
  finished: false,
  stopped: false,
  total: 0,
  done: 0,
  copied: 0,
  duplicates: [],
  failed: [],
  leftOut: 0,
});

let stopRequested = false;

/** Stops after the document in hand. */
export function stopImport() {
  stopRequested = true;
}

/** Hides the summary. */
export function dismissImport() {
  if (!importing.active) importing.finished = false;
}

function nameOf(path: string) {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Copies a batch into the inbox. `leftOut`: what the plan already left out. */
export async function runImport(batch: Batch, leftOut = 0) {
  if (importing.active) return;
  stopRequested = false;
  Object.assign(importing, {
    active: true,
    finished: false,
    stopped: false,
    total: batch.paths.length + batch.files.length,
    done: 0,
    copied: 0,
    duplicates: [],
    failed: [],
    leftOut,
  });
  const jobs: { name: string; run: () => ReturnType<typeof api.inboxImportFile> }[] = [
    ...batch.paths.map((p) => ({ name: nameOf(p), run: () => api.inboxImportFile(p) })),
    ...batch.files.map((f) => ({ name: f.name, run: () => api.inboxImportDropped(f) })),
  ];
  for (const job of jobs) {
    if (stopRequested) {
      importing.stopped = true;
      break;
    }
    try {
      const r = await job.run();
      if (r.kind === "copied") importing.copied++;
      else if (r.kind === "duplicate") importing.duplicates.push({ name: job.name, sourceId: r.sourceId, relPath: r.relPath });
      else importing.leftOut++;
    } catch (e) {
      importing.failed.push({ name: job.name, message: String(e) });
    }
    importing.done++;
  }
  importing.active = false;
  importing.finished = true;
}

/** What one dropped item brings: its name, and its files (a folder's read
 * through). */
export type DroppedItem = { name: string; isDir: boolean; files: File[] };

/** The items of a drop. The entries are taken at once, while the drop
 * event lasts; their files are read after. */
export async function droppedItems(items: DataTransferItemList): Promise<DroppedItem[]> {
  const entries = [...items].map((i) => i.webkitGetAsEntry?.()).filter((e): e is FileSystemEntry => !!e);
  async function walk(entry: FileSystemEntry, into: File[]): Promise<void> {
    if (entry.name.startsWith(".")) return;
    if (entry.isFile) {
      into.push(await new Promise<File>((ok, fail) => (entry as FileSystemFileEntry).file(ok, fail)));
    } else if (entry.isDirectory) {
      const reader = (entry as FileSystemDirectoryEntry).createReader();
      // readEntries returns them in chunks until an empty one.
      for (;;) {
        const chunk = await new Promise<FileSystemEntry[]>((ok, fail) => reader.readEntries(ok, fail));
        if (!chunk.length) break;
        for (const child of chunk) await walk(child, into);
      }
    }
  }
  const out: DroppedItem[] = [];
  for (const e of entries) {
    const files: File[] = [];
    await walk(e, files);
    out.push({ name: e.name, isDir: e.isDirectory, files });
  }
  return out;
}
