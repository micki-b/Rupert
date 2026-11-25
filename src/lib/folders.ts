// Showing and finding folders when filing (#63): the path as a breadcrumb,
// the shortest endings that tell suggestions apart, and search.
import type { FolderChoice, Source } from "#lib/api.ts";

/** A folder's path as its parts: the source's name first, unless the
 * library has only one source (then it says nothing). */
export function crumbs(choice: FolderChoice, sources: Source[]): string[] {
  const dirs = choice.dir.split("/").filter(Boolean);
  const source = sources.find((s) => s.id === choice.sourceId)?.name ?? choice.label.split("/")[0];
  return sources.length > 1 || !dirs.length ? [source, ...dirs] : dirs;
}

/** For each folder, the fewest last parts that no other folder in the list
 * ends with too ("Sol y Mar/Purchase", "Boats", "Insurance/Boat"). */
export function shortestEndings(choices: FolderChoice[], sources: Source[]): Map<string, string> {
  const parts = choices.map((c) => crumbs(c, sources));
  const out = new Map<string, string>();
  choices.forEach((c, i) => {
    const mine = parts[i];
    let n = 1;
    while (
      n < mine.length &&
      parts.some((other, j) => j !== i && other.slice(-n).join("/").toLowerCase() === mine.slice(-n).join("/").toLowerCase())
    ) {
      n++;
    }
    out.set(c.label, mine.slice(-n).join("/"));
  });
  return out;
}

/** Folds case and accents, so "maerz" finds nothing but "marz" finds "März". */
function fold(s: string): string {
  return s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

/** Folders whose path holds every word typed, best first: the folder's own
 * name starting with the first word, then its name holding it, then the rest. */
export function searchFolders(query: string, choices: FolderChoice[], limit = 40): FolderChoice[] {
  const words = fold(query).split(/\s+/).filter(Boolean);
  if (!words.length) return [];
  const scored: { c: FolderChoice; score: number }[] = [];
  for (const c of choices) {
    const path = fold(c.label);
    if (!words.every((w) => path.includes(w))) continue;
    const name = fold(c.label.slice(c.label.lastIndexOf("/") + 1));
    const score = name.startsWith(words[0]) ? 0 : name.includes(words[0]) ? 1 : 2;
    scored.push({ c, score });
  }
  return scored
    .sort((a, b) => a.score - b.score || a.c.label.length - b.c.label.length || a.c.label.localeCompare(b.c.label))
    .slice(0, limit)
    .map((x) => x.c);
}
