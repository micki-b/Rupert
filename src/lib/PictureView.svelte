<script lang="ts">
  // A folder's contents as pictures: large icons (a grid of tiles) or a
  // gallery (the selected document large, with its pages to flip through,
  // and a filmstrip below). The list view stays in the page itself.
  import { tick } from "svelte";
  import Icon from "#lib/Icon.svelte";
  import Thumb, { thumbUrl } from "#lib/Thumb.svelte";
  import MarkdownView from "#lib/MarkdownView.svelte";
  import DocStar from "#lib/DocStar.svelte";
  import CloudBadge from "#lib/CloudBadge.svelte";
  import RenameField from "#lib/RenameField.svelte";
  import { isMarkdown } from "#lib/markdown.ts";
  import { api, type ListedEntry } from "#lib/api.ts";
  import { formatDate } from "#lib/format.ts";
  import { dragSource, dropTarget, isDropOver, type Dropped, type MoveItem } from "#lib/drag.svelte.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    mode,
    sourceId,
    entries,
    selectedRelPath,
    onselect,
    onactivate,
    onmenu,
    onmove,
    oncopy,
    onstar,
    ondownload,
    renaming = null,
    onrename,
    onrenamecancel,
    enterOpens = true,
  }: {
    mode: "icons" | "gallery";
    sourceId: string;
    entries: ListedEntry[];
    selectedRelPath: string | null;
    onselect: (e: ListedEntry) => void;
    onactivate: (e: ListedEntry) => void;
    onmenu: (e: ListedEntry, ev: MouseEvent) => void;
    /** Moves a dropped document or folder into a folder here. */
    onmove: (item: MoveItem, sourceId: string, relPath: string) => void;
    /** Copies files dropped from another app into a folder here. */
    oncopy: (dropped: Dropped, sourceId: string, relPath: string) => void;
    /** Stars or unstars a document (its star was clicked). */
    onstar: (e: ListedEntry) => void;
    /** Downloads an online-only document (its cloud was clicked). */
    ondownload: (e: ListedEntry) => void;
    /** The path whose name is being edited in place (#65). */
    renaming?: string | null;
    onrename: (e: ListedEntry, name: string) => void;
    onrenamecancel: () => void;
    /** Return opens the selected item (on the Mac it renames, as in the Finder). */
    enterOpens?: boolean;
  } = $props();

  let strip: HTMLDivElement | undefined = $state();
  let grid: HTMLDivElement | undefined = $state();
  const index = $derived(entries.findIndex((e) => e.relPath === selectedRelPath));
  const current = $derived(index >= 0 ? entries[index] : null);

  // MARK: Pages (gallery)

  const BIG = 1400;
  let page = $state(0);
  /** Pages in the shown PDF; null until known (or for other files). */
  let pages = $state<number | null>(null);

  $effect(() => {
    const c = current;
    page = 0;
    pages = null;
    if (mode !== "gallery" || !c || c.kind !== "pdf" || c.cloud === "cloudOnly") return;
    let live = true;
    api
      .pageCount(sourceId, c.relPath)
      .then((n) => live && (pages = n))
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  // Render the next page ahead, so flipping is quick.
  $effect(() => {
    if (current && pages && page + 1 < pages) new Image().src = thumbUrl(sourceId, current, page + 1, BIG);
  });

  function flip(step: number) {
    if (pages && page + step >= 0 && page + step < pages) page += step;
  }

  // The gallery always shows something: the first entry to begin with.
  $effect(() => {
    if (mode === "gallery" && !current && entries.length) onselect(entries[0]);
  });

  // Keep the selection in view.
  $effect(() => {
    const i = index;
    const box = mode === "gallery" ? strip : grid;
    if (i < 0 || !box) return;
    tick().then(() => box.children[i]?.scrollIntoView({ block: "nearest", inline: "nearest" }));
  });

  /** Arrow keys move the selection (not while typing somewhere). */
  function onkeydown(ev: KeyboardEvent) {
    const t = ev.target;
    if (t instanceof Element && t.closest("input, textarea, select, [contenteditable], [role=dialog]")) return;
    if (!entries.length || ev.metaKey || ev.ctrlKey || ev.altKey) return;
    let step = 0;
    if (ev.key === "ArrowRight") step = 1;
    else if (ev.key === "ArrowLeft") step = -1;
    else if (mode === "icons" && (ev.key === "ArrowDown" || ev.key === "ArrowUp")) {
      step = columns() * (ev.key === "ArrowDown" ? 1 : -1);
    } else if (mode === "gallery" && ["ArrowDown", "ArrowUp", "PageDown", "PageUp"].includes(ev.key)) {
      ev.preventDefault();
      flip(ev.key === "ArrowDown" || ev.key === "PageDown" ? 1 : -1);
      return;
    } else if (ev.key === "Enter" && current && enterOpens && !renaming) {
      ev.preventDefault();
      onactivate(current);
      return;
    } else return;
    ev.preventDefault();
    const next = Math.min(entries.length - 1, Math.max(0, (index < 0 ? -step : index) + step));
    onselect(entries[next]);
  }

  /** Tiles per row in the grid. */
  function columns(): number {
    const first = grid?.children[0] as HTMLElement | undefined;
    if (!grid || !first) return 1;
    return Math.max(1, Math.round(grid.clientWidth / first.offsetWidth));
  }
</script>

<svelte:window {onkeydown} />

{#if mode === "icons"}
  <div class="grid" bind:this={grid}>
    {#each entries as e (e.relPath)}
      <div class="tile-wrap doc-host" class:selected={e.relPath === selectedRelPath}>
      <button
        class="tile"
        class:selected={e.relPath === selectedRelPath}
        class:drop={e.isDir && isDropOver(sourceId, e.relPath)}
        onclick={() => onselect(e)}
        ondblclick={() => onactivate(e)}
        oncontextmenu={(ev) => onmenu(e, ev)}
        {...renaming === e.relPath ? {} : dragSource({ sourceId, entry: e })}
        {...e.isDir ? dropTarget(sourceId, e.relPath, { move: (item) => onmove(item, sourceId, e.relPath), files: (d) => oncopy(d, sourceId, e.relPath) }) : {}}
        title={e.title ?? e.name}
      >
        <span class="pic">
          {#if isMarkdown(e) && e.cloud !== "cloudOnly"}
            <span class="md-page"><MarkdownView {sourceId} relPath={e.relPath} compact /></span>
          {:else}
            <Thumb {sourceId} entry={e} side={320} iconSize={48} />
          {/if}
        </span>
        <span class="label" class:hidden={renaming === e.relPath}>{e.name}</span>
      </button>
      {#if renaming === e.relPath}
        <!-- Outside the tile's button: buttons can't hold inputs. -->
        <span class="rename-at">
          <RenameField name={e.name} isDir={e.isDir} oncommit={(name) => onrename(e, name)} oncancel={onrenamecancel} />
        </span>
      {/if}
      <!-- Over the picture, outside the tile's button (buttons can't hold buttons). -->
      <span class="pic-marks">
        {#if !e.isDir}
          <span class="star"><DocStar {sourceId} doc={e} round onclick={() => onstar(e)} /></span>
        {/if}
        {#if e.cloud === "cloudOnly"}
          <span class="badge"><CloudBadge {sourceId} doc={e} size={13} round onclick={() => ondownload(e)} /></span>
        {/if}
      </span>
      </div>
    {/each}
  </div>
{:else}
  <div class="gallery">
    <div
      class="stage"
      role="presentation"
      ondblclick={() => current && onactivate(current)}
      oncontextmenu={(ev) => current && onmenu(current, ev)}
    >
      {#if current}
        <div class="big" class:md-big={isMarkdown(current) && current.cloud !== "cloudOnly"}>
          {#if isMarkdown(current) && current.cloud !== "cloudOnly"}
            {#key current.relPath}
              <div class="md-sheet"><MarkdownView {sourceId} relPath={current.relPath} /></div>
            {/key}
          {:else}
            <Thumb {sourceId} entry={current} {page} side={BIG} iconSize={96} eager />
          {/if}
          {#if pages && pages > 1}
            <button class="flip prev" onclick={() => flip(-1)} disabled={page === 0} title={m.pictures.previousPageKey} aria-label={m.pictures.previousPage}>
              <Icon name="back" size={20} />
            </button>
            <button class="flip next" onclick={() => flip(1)} disabled={page + 1 >= pages} title={m.pictures.nextPageKey} aria-label={m.pictures.nextPage}>
              <Icon name="chevron" size={20} />
            </button>
          {/if}
        </div>
        <div class="caption doc-host selected">
          <strong class="cap-title">
            {#if !current.isDir}<DocStar {sourceId} doc={current} onclick={() => onstar(current!)} />{/if}
            {#if renaming === current.relPath}
              {@const doc = current}
              <RenameField name={doc.name} isDir={doc.isDir} oncommit={(name) => onrename(doc, name)} oncancel={onrenamecancel} />
            {:else}
              <span>{current.title ?? current.name}</span>
            {/if}
            {#if current.cloud === "cloudOnly"}<CloudBadge {sourceId} doc={current} onclick={() => ondownload(current!)} />{/if}
          </strong>
          <span class="muted">
            {#if current.title}{current.name} · {/if}{current.documentDate ?? formatDate(current.modifiedMs)}
            {#if pages && pages > 1}· {m.pictures.pageOf(page + 1, pages)}{/if}
            {#if current.cloud === "cloudOnly"}· {m.pictures.onlineOnly}{/if}
          </span>
        </div>
      {/if}
    </div>
    <div class="strip" bind:this={strip}>
      {#each entries as e (e.relPath)}
        <button
          class="frame"
          class:selected={e.relPath === selectedRelPath}
          onclick={() => onselect(e)}
          ondblclick={() => onactivate(e)}
          oncontextmenu={(ev) => onmenu(e, ev)}
          {...dragSource({ sourceId, entry: e })}
          title={e.title ?? e.name}
        >
          <Thumb {sourceId} entry={e} side={200} iconSize={26} />
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  /* MARK: Large icons */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
    gap: 10px;
    padding: 2px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 8px 6px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: inherit;
    min-width: 0;
    cursor: default;
  }
  .tile:hover {
    background: var(--hover);
  }
  .tile.selected {
    background: var(--accent-soft);
  }
  .tile.drop {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .pic {
    position: relative;
    width: 110px;
    height: 140px;
  }
  /* A Markdown file's tile: the start of it on a little page. */
  .md-page {
    position: absolute;
    inset: 4px 8px;
    overflow: hidden;
    padding: 6px 7px;
    border-radius: 2px;
    background: var(--bg);
    border: 1px solid var(--border);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.2);
  }
  .tile-wrap {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .tile-wrap > .tile {
    flex: 1;
  }
  /* The picture's place in the tile (8px padding, 110 × 140, centered). */
  .pic-marks {
    position: absolute;
    top: 8px;
    left: 50%;
    width: 110px;
    height: 140px;
    transform: translateX(-50%);
    pointer-events: none;
  }
  .pic-marks > span {
    position: absolute;
    pointer-events: auto;
  }
  .pic-marks .star {
    top: 2px;
    right: 2px;
  }
  .pic-marks .badge {
    right: 2px;
    bottom: 2px;
  }
  .label {
    max-width: 100%;
    font-size: 12px;
    line-height: 1.3;
    text-align: center;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }
  /* MARK: Gallery */
  .gallery {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .big {
    flex: 1;
    min-height: 0;
    width: 100%;
  }
  .big {
    position: relative;
  }
  .flip {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 36px;
    height: 36px;
    border-radius: 50%;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    display: grid;
    place-items: center;
    opacity: 0.85;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.2);
  }
  .flip:hover:not(:disabled) {
    opacity: 1;
  }
  .flip:disabled {
    opacity: 0.25;
  }
  .flip.prev {
    left: 6px;
  }
  .flip.next {
    right: 6px;
  }
  /* The gallery shows a Markdown file as a scrollable page. */
  .big.md-big {
    display: flex;
    justify-content: center;
  }
  .md-sheet {
    width: min(720px, 100%);
    height: 100%;
    overflow-y: auto;
    padding: 18px 24px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.15);
  }
  .caption {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    text-align: center;
    max-width: 100%;
  }
  .caption strong,
  .caption span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cap-title {
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .caption .muted {
    font-size: 12px;
    color: var(--muted);
  }
  .strip {
    flex: none;
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding: 6px 2px 8px;
    border-top: 1px solid var(--border);
  }
  .frame {
    flex: none;
    width: 64px;
    height: 80px;
    padding: 4px;
    border: 2px solid transparent;
    border-radius: 6px;
    background: none;
    cursor: default;
  }
  .frame:hover {
    background: var(--hover);
  }
  .frame.selected {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .label.hidden {
    visibility: hidden;
  }
  .rename-at {
    position: absolute;
    left: 6px;
    right: 6px;
    bottom: 8px;
    z-index: 2;
  }
</style>
