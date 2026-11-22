// Dragging onto a folder (in the sidebar or the folder view):
// - documents and folders of Rupert's are moved there;
// - files dropped from another app (the Finder, File Explorer, …) are
//   copied there, keeping their names.
// Nothing is dragged out to other apps.
import type { Entry } from "./api";

/** A document or folder of a source. */
export type MoveItem = { sourceId: string; entry: Entry };

/** What was dropped from another app: its files, and the names of any
 * folders (which aren't copied). */
export type Dropped = { files: File[]; folders: string[] };

export const drag = $state<{ item: MoveItem | null; over: string | null }>({ item: null, over: null });

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

/** Whether files from another app are being dragged. */
function bringsFiles(ev: DragEvent): boolean {
  return !drag.item && !!ev.dataTransfer?.types.includes("Files");
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
      drag.item = item;
      if (ev.dataTransfer) {
        ev.dataTransfer.effectAllowed = "move";
        ev.dataTransfer.setData("text/plain", item.entry.name);
      }
    },
    ondragend: () => {
      drag.item = null;
      drag.over = null;
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
  const takes = (ev: DragEvent) =>
    on.files && bringsFiles(ev) ? "copy" : on.move && canMoveInto(drag.item, sourceId, relPath) ? "move" : null;
  return {
    ondragover: (ev: DragEvent) => {
      const effect = takes(ev);
      if (!effect) return;
      ev.preventDefault(); // allows the drop
      ev.stopPropagation();
      if (ev.dataTransfer) ev.dataTransfer.dropEffect = effect;
      drag.over = key;
    },
    ondragleave: (ev: DragEvent) => {
      // Not when moving onto something inside this folder's element.
      if (ev.currentTarget instanceof Node && ev.relatedTarget instanceof Node && ev.currentTarget.contains(ev.relatedTarget)) return;
      if (drag.over === key) drag.over = null;
    },
    ondrop: (ev: DragEvent) => {
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
