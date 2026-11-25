// Rename documents… (#64): what changes between a name and its new name,
// for highlighting in the dialog's table.

export interface Part {
  text: string;
  changed: boolean;
}

/** Words, spaces and separators, each its own piece. */
function pieces(s: string): string[] {
  return s.split(/(\s+|[-_.,;()[\]])/).filter((p) => p !== "");
}

/** `next` in pieces, marking those that aren't in `before` (in order, by the
 * longest common run of pieces; case counts). */
export function changedParts(before: string, next: string): Part[] {
  const a = pieces(before);
  const b = pieces(next);
  // Longest common subsequence, by table (names are short).
  const n = a.length;
  const k = b.length;
  const len: number[][] = Array.from({ length: n + 1 }, () => new Array<number>(k + 1).fill(0));
  for (let i = n - 1; i >= 0; i--) {
    for (let j = k - 1; j >= 0; j--) {
      len[i][j] = a[i] === b[j] ? len[i + 1][j + 1] + 1 : Math.max(len[i + 1][j], len[i][j + 1]);
    }
  }
  const kept = new Array<boolean>(k).fill(false);
  let i = 0;
  let j = 0;
  while (i < n && j < k) {
    if (a[i] === b[j]) {
      kept[j] = true;
      i++;
      j++;
    } else if (len[i + 1][j] >= len[i][j + 1]) {
      i++;
    } else {
      j++;
    }
  }
  // A lone space or separator between two changed pieces is part of the change.
  const out: Part[] = [];
  b.forEach((text, at) => {
    const filler = /^(\s+|[-_.,;()[\]])$/.test(text);
    const changed = !kept[at] || (filler && at > 0 && at < k - 1 && !kept[at - 1] && !kept[at + 1]);
    const last = out[out.length - 1];
    if (last && last.changed === changed) last.text += text;
    else out.push({ text, changed });
  });
  return out;
}
