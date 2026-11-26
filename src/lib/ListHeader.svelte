<script lang="ts">
  // The folder list's column headers (#138), with the table's column widths
  // (`<colgroup>`). Click a header to sort; drag a header's edge to resize
  // (double-click it for the usual width); drag a header sideways to move
  // the column; right-click for which columns show and Reset Columns.
  // Moving uses pointer events, not drag and drop, so it never mixes with
  // dragging documents out of the window.
  import { m } from "#lib/i18n/index.ts";
  import {
    ALL_COLUMNS,
    columnWidth,
    columnsChanged,
    listColumns,
    moveColumn,
    resetColumns,
    saveColumns,
    setColumnShown,
    setColumnWidth,
    type ColumnId,
  } from "#lib/listColumns.svelte.ts";

  type SortKey = "name" | "kind" | "size" | "date" | "modified";

  let {
    columns,
    sort,
    onsort,
  }: {
    /** The columns shown, in order. */
    columns: ColumnId[];
    sort: { key: SortKey; desc: boolean };
    onsort: (key: SortKey) => void;
  } = $props();

  const label = (c: ColumnId): string =>
    ({
      star: m.listColumns.favorite,
      name: m.main.name,
      kind: m.main.kind,
      size: m.main.size,
      date: m.filters.date,
      modified: m.main.modified,
    })[c];

  // MARK: Resizing

  /** The column an edge resizes, and which way: left of Name, the column
   * before the edge grows as it moves right; right of Name, the one after
   * it shrinks (columns there are pinned to the list's right side). Name
   * takes up the difference. */
  function edgeTarget(i: number): { col: ColumnId; sign: 1 | -1 } | null {
    const name = columns.indexOf("name");
    const [a, b] = [columns[i], columns[i + 1]];
    if (!b) return null;
    const left = name < 0 || i < name;
    const col = left ? a : b;
    if (col === "name" || col === "star") return null;
    return { col, sign: left ? 1 : -1 };
  }

  let resizing = $state<{ col: ColumnId; sign: 1 | -1; x: number; w: number } | null>(null);

  function resizeDown(ev: PointerEvent, i: number) {
    const t = edgeTarget(i);
    if (ev.button !== 0 || !t) return;
    ev.preventDefault();
    ev.stopPropagation();
    (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
    resizing = { ...t, x: ev.clientX, w: columnWidth(t.col) ?? 0 };
    document.body.classList.add("resizing");
  }

  function resizeMove(ev: PointerEvent) {
    if (!resizing) return;
    setColumnWidth(resizing.col, resizing.w + resizing.sign * (ev.clientX - resizing.x));
  }

  function resizeUp() {
    resizing = null;
    document.body.classList.remove("resizing");
  }

  function resizeKey(ev: KeyboardEvent, i: number) {
    const t = edgeTarget(i);
    if (!t) return;
    const step = (ev.shiftKey ? 50 : 10) * t.sign;
    const w = columnWidth(t.col) ?? 0;
    if (ev.key === "ArrowRight") setColumnWidth(t.col, w + step);
    else if (ev.key === "ArrowLeft") setColumnWidth(t.col, w - step);
    else if (ev.key === "Enter") setColumnWidth(t.col, null);
    else return;
    ev.preventDefault();
  }

  // MARK: Moving

  let heads: Record<string, HTMLElement> = $state({});
  let pressed: { col: ColumnId; x: number; id: number } | null = null;
  let moving = $state<ColumnId | null>(null);
  /** A move just ended: the click that follows it isn't a sort. */
  let moved = false;

  function moveDown(ev: PointerEvent, col: ColumnId) {
    if (ev.button !== 0 || resizing) return;
    pressed = { col, x: ev.clientX, id: ev.pointerId };
  }

  function moveMove(ev: PointerEvent) {
    if (!pressed) return;
    if (!(ev.buttons & 1)) {
      moveUp();
      return;
    }
    if (!moving) {
      if (Math.abs(ev.clientX - pressed.x) < 5) return;
      moving = pressed.col;
      (ev.currentTarget as HTMLElement).setPointerCapture(pressed.id);
      document.body.classList.add("moving-column");
    }
    // The shown column under the pointer takes this one's place.
    const over = columns.find((c) => {
      const r = heads[c]?.getBoundingClientRect();
      return r && ev.clientX >= r.left && ev.clientX < r.right;
    });
    if (!over || over === moving) return;
    const from = columns.indexOf(moving);
    const to = columns.indexOf(over);
    // Past the middle of the next column, not just onto its edge.
    const r = heads[over].getBoundingClientRect();
    const mid = r.left + r.width / 2;
    if ((to > from && ev.clientX < mid) || (to < from && ev.clientX > mid)) return;
    const order = listColumns.order.filter((c) => c !== moving);
    const at = order.indexOf(over) + (to > from ? 1 : 0);
    moveColumn(moving, at, false);
  }

  function moveUp() {
    if (moving) {
      saveColumns();
      moved = true;
      setTimeout(() => (moved = false));
    }
    pressed = null;
    moving = null;
    document.body.classList.remove("moving-column");
  }

  // MARK: Menu

  let menu = $state<{ x: number; y: number } | null>(null);

  function openMenu(ev: MouseEvent) {
    ev.preventDefault();
    ev.stopPropagation();
    menu = {
      x: Math.min(ev.clientX, window.innerWidth - 220),
      y: Math.min(ev.clientY, window.innerHeight - 240),
    };
  }
</script>

<svelte:window
  onclick={() => (menu = null)}
  oncontextmenu={() => (menu = null)}
  onkeydown={(e) => e.key === "Escape" && menu && (menu = null)}
  onblur={() => (menu = null)}
/>

<colgroup>
  {#each columns as c (c)}
    <col style:width={c === "name" ? null : `${columnWidth(c)}px`} />
  {/each}
</colgroup>
<thead oncontextmenu={openMenu}>
  <tr>
    {#each columns as c, i (c)}
      {@const sortable = c !== "star"}
      <th
        bind:this={heads[c]}
        class={c}
        class:num={c === "size"}
        class:moving={moving === c}
        aria-sort={sortable && sort.key === c ? (sort.desc ? "descending" : "ascending") : undefined}
        onpointerdown={(ev) => moveDown(ev, c)}
        onpointermove={moveMove}
        onpointerup={moveUp}
        onpointercancel={moveUp}
      >
        {#if sortable}
          {@const key = c as SortKey}
          <button
            class="sort-head"
            class:on={sort.key === key}
            onclick={() => !moved && onsort(key)}
            title={m.filters.sortBy(label(c))}
          >
            {label(c)}{#if sort.key === key}<span class="sort-arrow">{sort.desc ? "▼" : "▲"}</span>{/if}
          </button>
        {:else}
          <span class="star-head" title={label(c)} aria-label={label(c)}>★</span>
        {/if}
        {#if edgeTarget(i)}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
          <span
            class="edge"
            class:on={resizing?.col === edgeTarget(i)?.col}
            role="separator"
            aria-orientation="vertical"
            aria-label={m.listColumns.widthOf(label(edgeTarget(i)!.col))}
            tabindex="0"
            title={m.listColumns.resizeTip}
            onpointerdown={(ev) => resizeDown(ev, i)}
            onpointermove={resizeMove}
            onpointerup={resizeUp}
            onpointercancel={resizeUp}
            ondblclick={(ev) => (ev.stopPropagation(), setColumnWidth(edgeTarget(i)!.col, null))}
            onkeydown={(ev) => resizeKey(ev, i)}
            onclick={(ev) => ev.stopPropagation()}
          ></span>
        {/if}
      </th>
    {/each}
  </tr>
</thead>

{#if menu}
  <div
    class="head-menu"
    role="menu"
    tabindex="-1"
    style:left="{menu.x}px"
    style:top="{menu.y}px"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    {#each ALL_COLUMNS.filter((c) => c !== "name") as c (c)}
      {@const shown = !listColumns.hidden.includes(c)}
      <button role="menuitemcheckbox" aria-checked={shown} onclick={() => setColumnShown(c, !shown)}>
        <span class="check">{shown ? "✓" : ""}</span>{label(c)}
      </button>
    {/each}
    <hr />
    <button
      role="menuitem"
      disabled={!columnsChanged()}
      onclick={() => {
        resetColumns();
        menu = null;
      }}><span class="check"></span>{m.listColumns.reset}</button
    >
  </div>
{/if}

<style>
  th {
    position: relative;
    text-align: left;
    font-weight: 500;
    font-size: 11px;
    color: var(--muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    user-select: none;
  }
  th.star {
    padding: 4px 0;
    text-align: center;
  }
  th.moving {
    background: var(--hover);
    color: var(--text);
  }
  .sort-head {
    all: unset;
    cursor: default;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    max-width: 100%;
  }
  .sort-head:hover,
  .sort-head.on {
    color: var(--text);
  }
  .sort-head:focus-visible {
    outline: 2px solid var(--accent-soft);
  }
  th.num .sort-head {
    justify-content: flex-end;
    width: 100%;
  }
  .sort-arrow {
    font-size: 8px;
  }
  .star-head {
    font-size: 11px;
  }
  /* The edge between two headers: drag to resize. */
  .edge {
    position: absolute;
    top: 0;
    bottom: 0;
    right: -1px;
    width: 7px;
    cursor: col-resize;
    touch-action: none;
    z-index: 1;
  }
  .edge::after {
    content: "";
    position: absolute;
    top: 4px;
    bottom: 4px;
    right: 3px;
    width: 1px;
    background: var(--border);
  }
  .edge:hover::after,
  .edge.on::after,
  .edge:focus-visible::after {
    background: var(--accent);
    width: 2px;
  }
  .edge:focus-visible {
    outline: none;
  }
  .head-menu {
    position: fixed;
    z-index: 30;
    min-width: 190px;
    padding: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
    font-size: 13px;
  }
  .head-menu button {
    display: flex;
    width: 100%;
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 8px;
    color: var(--text);
    font: inherit;
  }
  .head-menu button:hover:not(:disabled) {
    background: var(--hover);
  }
  .head-menu button:disabled {
    opacity: 0.45;
  }
  .head-menu .check {
    width: 18px;
  }
  .head-menu hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
  :global(body.moving-column),
  :global(body.moving-column *) {
    cursor: grabbing !important;
    user-select: none;
  }
</style>
