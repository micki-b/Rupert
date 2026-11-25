// Dragging onto a folder (in the sidebar or the folder view):
// - documents and folders of Rupert's are moved there;
// - files dropped from another app (the Finder, File Explorer, …) are
//   copied there, keeping their names.
// Dragging out (#84): a drag of Rupert's documents is the system's own,
// carrying the files as a copy, so Mail, Messages, a browser or the Finder
// get them. The web view doesn't hand such a drag back to the page, so the
// app reports where the pointer is (`drag-out-pointer`) and where it was let
// go (`drag-out-ended`): over one of Rupert's folders, the document moves.
import { listen } from "@tauri-apps/api/event";
import { api, type Entry } from "./api";

/** A document or folder of a source. */
export type MoveItem = { sourceId: string; entry: Entry };

/** What was dropped from another app: its files, and the names of any
 * folders (which aren't copied). */
export type Dropped = { files: File[]; folders: string[] };

export const drag = $state<{
  item: MoveItem | null;
  over: string | null;
  /** The system's drag (a copy), not the page's own. */
  native: boolean;
  /** Only to other apps: an inbox scan is filed with File it, not dropped. */
  outOnly: boolean;
}>({ item: null, over: null, native: false, outOnly: false });

function endDrag() {
  drag.item = null;
  drag.over = null;
  drag.native = false;
  drag.outOnly = false;
}

/** The folders that take drops, by key: found under the pointer by their
 * `data-drop-key` while the system's drag is on. */
const targets = new Map<string, { sourceId: string; relPath: string; move?: (item: MoveItem) => void }>();

type Pointer = { x: number; y: number; inside: boolean; dropped: boolean };

/** The folder under the pointer that would take the dragged item, if any. */
function targetAt(p: Pointer) {
  if (!drag.item || !p.inside) return null;
  const el = document.elementFromPoint(p.x, p.y)?.closest("[data-drop-key]");
  const key = el?.getAttribute("data-drop-key");
  const t = key ? targets.get(key) : undefined;
  return key && t?.move && canMoveInto(drag.item, t.sourceId, t.relPath) ? { key, t } : null;
}

/** The names of the files last dragged out, and until when a drop of them
 * on the window is that same drag coming back (#96): the web view can hand
 * the page its drop after the app has said the drag ended, and it would
 * look like files from another app, to be copied. */
let ours = { names: new Set<string>(), until: 0 };

/** Whether a drop brings back the files Rupert is dragging out (or just did). */
function isOurs(ev: DragEvent): boolean {
  if (drag.native) return true;
  if (Date.now() > ours.until) return false;
  const files = [...(ev.dataTransfer?.files ?? [])];
  return files.length > 0 && files.every((f) => ours.names.has(f.name));
}

let listening = false;
/** Hands a drag to the system with the files at `relPaths` of a source. */
function startDragOut(sourceId: string, relPaths: string[], ev: DragEvent) {
  if (!listening) {
    listening = true;
    listen<Pointer>("drag-out-pointer", (e) => {
      if (drag.native) drag.over = targetAt(e.payload)?.key ?? null;
    });
    listen<Pointer>("drag-out-ended", (e) => {
      ours.until = Date.now() + 3000;
      // Taken by another app (e.g. a window over Rupert's): not moved here.
      const hit = e.payload.dropped ? null : targetAt(e.payload);
      const item = drag.item;
      endDrag();
      if (hit && item) hit.t.move?.(item);
    });
  }
  drag.native = true;
  ours = { names: new Set(relPaths.map((p) => p.slice(p.lastIndexOf("/") + 1))), until: Number.MAX_SAFE_INTEGER };
  // Where the page and the screen see the pointer, so the app can tell
  // where on the page it is later.
  const start = { clientX: ev.clientX, clientY: ev.clientY, screenX: ev.screenX, screenY: ev.screenY };
  api.dragOut(sourceId, relPaths, start).catch(() => endDrag());
}

/** Handlers for something that can be dragged out of Rupert only (an inbox
 * scan). */
export function dragOutSource(sourceId: string, relPath: string) {
  return {
    draggable: true,
    ondragstart: (ev: DragEvent) => {
      ev.preventDefault(); // the system's drag instead
      drag.outOnly = true;
      startDragOut(sourceId, [relPath], ev);
    },
  };
}

function dirOf(relPath: string) {
  return relPath.includes("/") ? relPath.slice(0, relPath.lastIndexOf("/")) : "";
}

/** Whether `item` can go into the folder `relPath` of `sourceId`: not where
 * it already is, and a folder not into itself. */
export function canMoveInto(item: MoveItem | null, sourceId: string, relPath: string): boolean {
  if (!item) return false;
  if (item.sourceId !== sourceId) return true;
  if (dirOf(item.entry.relPath) === relPath) return false;
  const self = item.entry.relPath;
  return !(item.entry.isDir && (relPath === self || relPath.startsWith(self + "/")));
}

/** Whether files from another app are being dragged (not Rupert's own,
 * which arrive as files too). */
function bringsFiles(ev: DragEvent): boolean {
  return !drag.item && !isOurs(ev) && !!ev.dataTransfer?.types.includes("Files");
}

/** The files and folders of a drop from another app. */
function dropped(ev: DragEvent): Dropped {
  const result: Dropped = { files: [], folders: [] };
  const items = ev.dataTransfer?.items;
  if (!items) return result;
  for (const item of items) {
    if (item.kind !== "file") continue;
    const entry = item.webkitGetAsEntry?.();
    if (entry?.isDirectory) {
      result.folders.push(entry.name);
      continue;
    }
    const file = item.getAsFile();
    if (file) result.files.push(file);
  }
  return result;
}

/** Handlers for something that can be dragged. */
export function dragSource(item: MoveItem) {
  return {
    draggable: true,
    ondragstart: (ev: DragEvent) => {
      ev.preventDefault(); // the system's drag instead, so other apps get the file
      drag.item = item;
      startDragOut(item.sourceId, [item.entry.relPath], ev);
    },
  };
}

/** Handlers for a folder that takes drops: `move` moves Rupert's documents
 * and folders into it, `files` copies files from another app. A drop goes
 * to the innermost folder that takes it. */
export function dropTarget(
  sourceId: string,
  relPath: string,
  on: { move?: (item: MoveItem) => void; files?: (dropped: Dropped) => void },
) {
  const key = `${sourceId}\n${relPath}`;
  targets.set(key, { sourceId, relPath, move: on.move });
  const takes = (ev: DragEvent) =>
    on.files && bringsFiles(ev) ? "copy" : on.move && canMoveInto(drag.item, sourceId, relPath) ? "move" : null;
  return {
    "data-drop-key": key,
    ondragover: (ev: DragEvent) => {
      const effect = takes(ev);
      if (!effect) return;
      ev.preventDefault(); // allows the drop
      ev.stopPropagation();
      // The system's drag only offers a copy (for other apps); here it moves.
      if (ev.dataTransfer) ev.dataTransfer.dropEffect = drag.native ? "copy" : effect;
      drag.over = key;
    },
    ondragleave: (ev: DragEvent) => {
      // Not when moving onto something inside this folder's element.
      if (ev.currentTarget instanceof Node && ev.relatedTarget instanceof Node && ev.currentTarget.contains(ev.relatedTarget)) return;
      if (drag.over === key) drag.over = null;
    },
    ondrop: (ev: DragEvent) => {
      if (drag.native || isOurs(ev)) {
        // Rupert's own drag: where it ended decides (`drag-out-ended`).
        ev.preventDefault();
        ev.stopPropagation();
        drag.over = null;
        return;
      }
      const effect = takes(ev);
      if (!effect) return;
      ev.preventDefault();
      ev.stopPropagation();
      drag.over = null;
      if (effect === "copy") {
        on.files?.(dropped(ev));
        return;
      }
      const item = drag.item;
      drag.item = null;
      if (item) on.move?.(item);
    },
  };
}

/** Whether the folder is the one a drag is hovering over (to highlight it). */
export function isDropOver(sourceId: string, relPath: string) {
  return drag.over === `${sourceId}\n${relPath}`;
}
