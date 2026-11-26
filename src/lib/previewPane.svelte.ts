// The page preview above File it (inbox) and above Details while browsing
// (#127): the height of each and whether it's hidden, remembered on this
// computer. The height set by dragging is the most it takes;
// a short window shrinks it, and under `MIN_HEIGHT` it gives way entirely.

export type Pane = "inbox" | "details";

/** Under this, there's no useful preview: it gives way to the form. */
export const MIN_HEIGHT = 120;

/** The usual height of each (double-click on the divider): the inbox's
 * fills what File it leaves; Details' stops at a page's worth. */
export const DEFAULT_HEIGHT: Record<Pane, number> = {
  inbox: Number.POSITIVE_INFINITY,
  details: 300,
};

const KEY = "rupert.preview";

type Saved = { height: Record<Pane, number | null>; hidden: boolean };

function load(): Saved {
  const out: Saved = { height: { inbox: null, details: null }, hidden: false };
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    for (const p of Object.keys(out.height) as Pane[]) {
      const h = saved?.height?.[p];
      if (typeof h === "number" && Number.isFinite(h)) out.height[p] = Math.max(MIN_HEIGHT, Math.round(h));
    }
    out.hidden = saved?.hidden === true;
  } catch {
    // Unreadable or missing: the defaults.
  }
  return out;
}

/** The heights set, and whether the preview is hidden (in both places). */
export const preview = $state(load());

function save() {
  try {
    localStorage.setItem(KEY, JSON.stringify(preview));
  } catch {
    // Not remembered, but still applied.
  }
}

/** The most the preview may take: what was set, or the usual height. */
export function wantedHeight(p: Pane): number {
  return preview.height[p] ?? DEFAULT_HEIGHT[p];
}

/** Sets the height (`null`: back to the usual one). */
export function setPreviewHeight(p: Pane, h: number | null) {
  preview.height[p] = h === null ? null : Math.max(MIN_HEIGHT, Math.round(h));
  save();
}

/** Hides or shows the preview, in the inbox and in Details alike. */
export function setPreviewHidden(hidden: boolean) {
  preview.hidden = hidden;
  save();
}
