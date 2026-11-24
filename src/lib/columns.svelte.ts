// The desktop window's resizable columns (#61): the Library sidebar, Details,
// and the inbox's preview + File it column. Each remembers its own width on
// this computer.

export type Column = "side" | "details" | "inbox";

/** Default, smallest and largest width of each column, in pixels. */
export const LIMITS: Record<Column, { def: number; min: number; max: number }> = {
  side: { def: 260, min: 180, max: 480 },
  details: { def: 280, min: 220, max: 560 },
  inbox: { def: 380, min: 300, max: 720 },
};

/** The room the middle column always keeps. */
export const MIDDLE = 320;

const KEY = "rupert.columns";

function load(): Record<Column, number> {
  const widths = { side: LIMITS.side.def, details: LIMITS.details.def, inbox: LIMITS.inbox.def };
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    for (const c of Object.keys(widths) as Column[]) {
      const w = saved?.[c];
      if (typeof w === "number" && Number.isFinite(w)) widths[c] = clamp(w, LIMITS[c].min, LIMITS[c].max);
    }
  } catch {
    // Unreadable or missing: the defaults.
  }
  return widths;
}

export const columns = $state(load());

export function clamp(w: number, min: number, max: number) {
  return Math.min(Math.max(w, min), Math.max(min, max));
}

/** Sets a column's width (within its limits) and remembers it. */
export function setWidth(c: Column, w: number) {
  columns[c] = Math.round(clamp(w, LIMITS[c].min, LIMITS[c].max));
  try {
    localStorage.setItem(KEY, JSON.stringify(columns));
  } catch {
    // Not remembered, but still applied.
  }
}
