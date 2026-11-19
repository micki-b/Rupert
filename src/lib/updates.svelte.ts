// Updates for the desktop app: Tauri's updater reads `latest.json` from the
// newest GitHub release, checks its signature against the public key in
// tauri.conf.json, then downloads and installs, and Rupert restarts.
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

type Status = "idle" | "checking" | "none" | "available" | "downloading" | "restarting" | "error";

const LAST_CHECK = "rupert.lastUpdateCheck";
const DAY = 24 * 60 * 60 * 1000;

class Updates {
  status = $state<Status>("idle");
  /** The newer version, once found. */
  version = $state<string | null>(null);
  /** Download progress, 0–1 (null while the size is unknown). */
  progress = $state<number | null>(null);
  problem = $state<string | null>(null);
  #update: Update | null = null;

  /** Looks for a newer version. A check you start yourself reports
   * problems; the automatic one stays quiet. */
  async check(byHand = false) {
    if (this.status === "checking" || this.status === "downloading") return;
    this.status = "checking";
    this.problem = null;
    try {
      const found = await check();
      this.#update = found;
      this.version = found?.version ?? null;
      this.status = found ? "available" : "none";
    } catch (e) {
      this.status = byHand ? "error" : "idle";
      this.problem = String(e);
    }
  }

  /** Downloads and installs the update found, then restarts Rupert. */
  async install() {
    const update = this.#update;
    if (!update || this.status === "downloading") return;
    this.status = "downloading";
    this.progress = null;
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        else if (event.event === "Progress") {
          done += event.data.chunkLength;
          this.progress = total ? Math.min(1, done / total) : null;
        }
      });
      this.status = "restarting";
      await relaunch();
    } catch (e) {
      this.status = "error";
      this.problem = String(e);
    }
  }
}

export const updates = new Updates();

/** At startup: checks at most once a day, if allowed in Settings. Never
 * when running from the source tree. */
export function checkNowAndThen(allowed: boolean) {
  if (!allowed || import.meta.env.DEV) return;
  let last = 0;
  try {
    last = Number(localStorage.getItem(LAST_CHECK) ?? 0);
  } catch {
    /* not essential */
  }
  if (Date.now() - last < DAY) return;
  try {
    localStorage.setItem(LAST_CHECK, String(Date.now()));
  } catch {
    /* not essential */
  }
  updates.check(false);
}
