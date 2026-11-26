<script lang="ts">
  // The page preview's place at the top of a column (#127): as tall as it
  // was set, less whatever the form below needs, and gone when that leaves
  // under MIN_HEIGHT. A divider below it resizes it (drag, arrow keys,
  // double-click for the usual height); × hides it until it's shown again.
  import Icon from "#lib/Icon.svelte";
  import { m } from "#lib/i18n/index.ts";
  import {
    MIN_HEIGHT,
    preview,
    setPreviewHeight,
    setPreviewHidden,
    wantedHeight,
    type Pane,
  } from "#lib/previewPane.svelte.ts";
  import type { Snippet } from "svelte";

  let {
    pane,
    room,
    natural = Number.POSITIVE_INFINITY,
    onopen,
    children,
  }: {
    pane: Pane;
    /** The height the column can spare for the preview and its divider. */
    room: number;
    /** The most the page's own shape can use at this width. */
    natural?: number;
    /** Opens the page large, when there's no room to show it here. */
    onopen?: () => void;
    children: Snippet;
  } = $props();

  /** The divider's height. */
  const BAR = 7;

  const hidden = $derived(preview.hidden);
  const fits = $derived(Math.floor(Math.min(room - BAR, natural)));
  const height = $derived(Math.min(wantedHeight(pane), fits));
  const shown = $derived(!hidden && height >= MIN_HEIGHT);

  let dragging = $state<{ y: number; h: number } | null>(null);

  function set(h: number) {
    setPreviewHeight(pane, Math.min(Math.max(h, MIN_HEIGHT), Math.max(MIN_HEIGHT, fits)));
  }

  function down(ev: PointerEvent) {
    if (ev.button !== 0) return;
    ev.preventDefault();
    (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
    dragging = { y: ev.clientY, h: height };
    document.body.classList.add("resizing-rows");
  }

  function move(ev: PointerEvent) {
    if (dragging) set(dragging.h + ev.clientY - dragging.y);
  }

  function up() {
    dragging = null;
    document.body.classList.remove("resizing-rows");
  }

  function key(ev: KeyboardEvent) {
    const step = ev.shiftKey ? 50 : 10;
    if (ev.key === "ArrowDown") set(height + step);
    else if (ev.key === "ArrowUp") set(height - step);
    else if (ev.key === "Enter") setPreviewHeight(pane, null);
    else return;
    ev.preventDefault();
  }
</script>

{#if shown}
  <div class="pane" style:height="{height}px">
    {@render children()}
    <button class="hide" onclick={() => setPreviewHidden(true)} title={m.preview.hideTip} aria-label={m.preview.hide}>
      <Icon name="close" size={12} />
    </button>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="bar"
    class:dragging
    role="separator"
    aria-orientation="horizontal"
    aria-label={m.preview.heightLabel}
    aria-valuemin={MIN_HEIGHT}
    aria-valuemax={Math.max(MIN_HEIGHT, fits)}
    aria-valuenow={height}
    tabindex="0"
    title={m.preview.resizeTip}
    style:height="{BAR}px"
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    ondblclick={() => setPreviewHeight(pane, null)}
    onkeydown={key}
  ></div>
{:else if hidden}
  <button class="show" onclick={() => setPreviewHidden(false)}>
    <Icon name="file" size={13} />
    {m.preview.show}
  </button>
{:else if onopen}
  <button class="show" onclick={onopen} title={m.preview.noRoomTip}>
    <Icon name="file" size={13} />
    {m.preview.openPage}
  </button>
{/if}

<style>
  .pane {
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  .hide {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    opacity: 0;
    transition: opacity 0.12s;
    z-index: 2;
  }
  .pane:hover .hide,
  .hide:focus-visible {
    opacity: 0.85;
  }
  .bar {
    flex: none;
    position: relative;
    cursor: row-resize;
    touch-action: none;
    border-top: 1px solid var(--border);
  }
  .bar::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    top: -2px;
    height: 3px;
    background: transparent;
    transition: background 0.12s;
  }
  .bar:hover::after,
  .bar.dragging::after,
  .bar:focus-visible::after {
    background: var(--accent);
  }
  .bar:focus-visible {
    outline: none;
  }
  .show {
    flex: none;
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin: 8px 14px 0;
    padding: 2px 8px;
    font-size: 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
  }
  :global(body.resizing-rows) {
    cursor: row-resize;
    user-select: none;
  }
  :global(body.resizing-rows *) {
    cursor: row-resize !important;
  }
</style>
