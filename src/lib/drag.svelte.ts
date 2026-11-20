// Moving documents and folders by dragging them onto a folder (in the
// sidebar or the folder view). Within the window only: nothing is dragged
// out to other apps, and files dropped from outside aren't taken in.
import type { Entry } from "./api";

/** A document or folder of a source. */
export type MoveItem = { sourceId: string; entry: Entry };

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

/** Handlers for a folder that takes drops; `onmove` does the moving. */
export function dropTarget(sourceId: string, relPath: string, onmove: (item: MoveItem) => void) {
  const key = `${sourceId}\n${relPath}`;
  return {
    ondragover: (ev: DragEvent) => {
      if (!canMoveInto(drag.item, sourceId, relPath)) return;
      ev.preventDefault(); // allows the drop
      if (ev.dataTransfer) ev.dataTransfer.dropEffect = "move";
      drag.over = key;
    },
    ondragleave: () => {
      if (drag.over === key) drag.over = null;
    },
    ondrop: (ev: DragEvent) => {
      ev.preventDefault();
      const item = drag.item;
      drag.item = null;
      drag.over = null;
      if (item && canMoveInto(item, sourceId, relPath)) onmove(item);
    },
  };
}

/** Whether the folder is the one a drag is hovering over (to highlight it). */
export function isDropOver(sourceId: string, relPath: string) {
  return drag.over === `${sourceId}\n${relPath}`;
}
