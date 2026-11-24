<script lang="ts">
  // The edge between two columns (#61): drag it to resize the column next to
  // it, double-click it for the usual width, or focus it and use the arrow keys.
  import { clamp, LIMITS, setWidth, type Column } from "#lib/columns.svelte.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    column,
    width,
    max,
    at,
    label,
    inset = 0,
  }: {
    column: Column;
    /** The column's width as shown. */
    width: number;
    /** The largest width that leaves the middle its room. */
    max: number;
    /** Which side of the window the column is on. */
    at: "left" | "right";
    /** What the screen reader says, e.g. "Library width". */
    label: string;
    /** Room between the column and the window's edge (the inbox's padding). */
    inset?: number;
  } = $props();

  const limits = $derived(LIMITS[column]);
  const top = $derived(Math.max(limits.min, Math.min(limits.max, max)));
  let dragging = $state<{ x: number; w: number } | null>(null);

  function set(w: number) {
    setWidth(column, clamp(w, limits.min, top));
  }

  function down(ev: PointerEvent) {
    if (ev.button !== 0) return;
    ev.preventDefault();
    (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
    dragging = { x: ev.clientX, w: width };
    document.body.classList.add("resizing");
  }

  function move(ev: PointerEvent) {
    if (!dragging) return;
    const dx = ev.clientX - dragging.x;
    set(at === "left" ? dragging.w + dx : dragging.w - dx);
  }

  function up() {
    dragging = null;
    document.body.classList.remove("resizing");
  }

  function key(ev: KeyboardEvent) {
    const step = ev.shiftKey ? 50 : 10;
    const grow = at === "left" ? "ArrowRight" : "ArrowLeft";
    const shrink = at === "left" ? "ArrowLeft" : "ArrowRight";
    if (ev.key === grow) set(width + step);
    else if (ev.key === shrink) set(width - step);
    else if (ev.key === "Home") set(limits.min);
    else if (ev.key === "End") set(top);
    else if (ev.key === "Enter") set(limits.def);
    else return;
    ev.preventDefault();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuemin={limits.min}
  aria-valuemax={top}
  aria-valuenow={width}
  tabindex="0"
  title={m.main.resizeTip}
  style:left={at === "left" ? `${width + inset}px` : null}
  style:right={at === "right" ? `${width + inset}px` : null}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  ondblclick={() => setWidth(column, limits.def)}
  onkeydown={key}
></div>

<style>
  .splitter {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 7px;
    margin: 0 -3px;
    z-index: 5;
    cursor: col-resize;
    touch-action: none;
  }
  .splitter::after {
    content: "";
    position: absolute;
    top: 0;
    bottom: 0;
    left: 2px;
    width: 3px;
    background: transparent;
    transition: background 0.12s;
  }
  .splitter:hover::after,
  .splitter.dragging::after,
  .splitter:focus-visible::after {
    background: var(--accent);
  }
  .splitter:focus-visible {
    outline: none;
  }
  :global(body.resizing) {
    cursor: col-resize;
    user-select: none;
  }
  :global(body.resizing *) {
    cursor: col-resize !important;
  }
</style>
