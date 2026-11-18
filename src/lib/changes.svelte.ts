// Shared record of folders that changed on disk. Views read `changes.tick`
// inside an $effect and call `affects()` to decide whether to reload.

import type { DirChange } from "./api";

let tick = $state(0);
let latest: DirChange[] = [];

export const changes = {
  get tick() {
    return tick;
  },
  push(batch: DirChange[]) {
    latest = batch;
    tick++;
  },
  /** Whether the most recent batch touched this folder. */
  affects(sourceId: string, relDir: string): boolean {
    return latest.some((c) => c.sourceId === sourceId && c.relDir === relDir);
  },
};
