// The folder list's columns (#138): their order, widths and which are
// shown, kept on this computer (like the sidebar and Details widths, #61),
// not in the library. Name has no width of its own: it takes what the others
// leave, and never less than NAME_MIN (the list scrolls sideways then).

export type ColumnId = "star" | "name" | "kind" | "size" | "date" | "modified";

export const ALL_COLUMNS: ColumnId[] = ["star", "name", "kind", "size", "date", "modified"];

/** Default and smallest width of each column, in pixels. The star's is
 * fixed. */
export const COLUMN_WIDTHS: Record<Exclude<ColumnId, "name">, { def: number; min: number }> = {
  star: { def: 28, min: 28 },
  kind: { def: 70, min: 44 },
  size: { def: 70, min: 48 },
  date: { def: 104, min: 64 },
  modified: { def: 104, min: 64 },
};

export const NAME_MIN = 140;

type Layout = {
  order: ColumnId[];
  widths: Partial<Record<ColumnId, number>>;
  hidden: ColumnId[];
};

const KEY = "rupert.listColumns";

function defaults(): Layout {
  return { order: [...ALL_COLUMNS], widths: {}, hidden: [] };
}

function load(): Layout {
  const out = defaults();
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!saved) return out;
    // Every column once, in the saved order, with any new ones at the end.
    const order = (Array.isArray(saved.order) ? saved.order : []).filter(
      (c: unknown, i: number, a: unknown[]) => ALL_COLUMNS.includes(c as ColumnId) && a.indexOf(c) === i,
    ) as ColumnId[];
    out.order = [...order, ...ALL_COLUMNS.filter((c) => !order.includes(c))];
    for (const c of ALL_COLUMNS) {
      const w = saved.widths?.[c];
      if (c !== "name" && c !== "star" && typeof w === "number" && Number.isFinite(w)) {
        out.widths[c] = Math.max(COLUMN_WIDTHS[c].min, Math.round(w));
      }
    }
    out.hidden = (Array.isArray(saved.hidden) ? saved.hidden : []).filter(
      (c: unknown) => ALL_COLUMNS.includes(c as ColumnId) && c !== "name",
    );
  } catch {
    // Unreadable or missing: the defaults.
  }
  return out;
}

export const listColumns = $state(load());

function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(listColumns));
  } catch {
    // Not remembered, but still applied.
  }
}

/** A column's width (Name: none, it takes the rest). */
export function columnWidth(c: ColumnId): number | null {
  if (c === "name") return null;
  return listColumns.widths[c] ?? COLUMN_WIDTHS[c].def;
}

export function setColumnWidth(c: ColumnId, w: number | null) {
  if (c === "name" || c === "star") return;
  if (w === null) delete listColumns.widths[c];
  else listColumns.widths[c] = Math.max(COLUMN_WIDTHS[c].min, Math.round(w));
  save();
}

/** Moves column `c` to position `to` in the order. */
export function moveColumn(c: ColumnId, to: number, keep = true) {
  const order = listColumns.order.filter((x) => x !== c);
  order.splice(Math.max(0, Math.min(to, order.length)), 0, c);
  listColumns.order = order;
  if (keep) save();
}

export function saveColumns() {
  save();
}

export function setColumnShown(c: ColumnId, shown: boolean) {
  if (c === "name") return;
  listColumns.hidden = shown ? listColumns.hidden.filter((x) => x !== c) : [...new Set([...listColumns.hidden, c])];
  save();
}

/** Back to the usual columns, order and widths. */
export function resetColumns() {
  Object.assign(listColumns, defaults());
  save();
}

/** Whether the layout differs from the usual one. */
export function columnsChanged(): boolean {
  return (
    listColumns.hidden.length > 0 ||
    Object.keys(listColumns.widths).length > 0 ||
    listColumns.order.some((c, i) => c !== ALL_COLUMNS[i])
  );
}
