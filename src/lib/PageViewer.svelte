<script lang="ts">
  // The page viewer (#83): a document's pages large, over the window, with
  // zooming (buttons, ⌘+ ⌘− ⌘0, pinch or ⌘-scroll, double-click), panning
  // (drag or scroll), page flipping and turning the page for viewing. It
  // shows a quick picture at once and a sharper one when zoomed in. Nothing
  // is changed in the file. Space opens it from any view (#105): then Space
  // closes it too, and ↑ ↓ go to the neighboring documents. An online-only
  // document shows only the picture Rupert kept of it, never downloading.
  import { onMount, tick } from "svelte";
  import Icon from "#lib/Icon.svelte";
  import { thumbUrl } from "#lib/Thumb.svelte";
  import { api, type Entry } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    sourceId,
    entry,
    title = null,
    startPage = 0,
    onclose,
    onstep,
  }: {
    sourceId: string;
    entry: Entry;
    /** The document's title, shown instead of its file name. */
    title?: string | null;
    startPage?: number;
    onclose: () => void;
    /** ↑ ↓: the previous or next document of the view (#105). */
    onstep?: (step: 1 | -1) => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  const online = entry.cloud === "cloudOnly";
  /** No picture to show: a kind without one, or online-only with none kept. */
  let failed = $state(false);

  // svelte-ignore state_referenced_locally
  let page = $state(startPage);
  // svelte-ignore state_referenced_locally
  let pages = $state<number | null>(entry.kind === "pdf" ? null : 1);
  /** Zoom relative to the whole page fitting (1 = Fit page). */
  let zoom = $state(1);
  let rotation = $state(0); // 0, 90, 180, 270
  /** The page's own proportions (height / width), from its picture. */
  let aspect = $state(1.294);
  let stage: HTMLDivElement | undefined = $state();
  let stageW = $state(800);
  let stageH = $state(600);
  /** Sharper pictures already fetched, by page and size. */
  let sharp = $state<Record<string, string>>({});
  let pageInput = $state("");

  const PAD = 24;
  const MIN_ZOOM = 0.2;
  const MAX_ZOOM = 8;
  /** The buttons and keys zoom by this factor, then round (see `tidy`). */
  const STEP = 1.25;

  /** A zoom with its percentage rounded to two significant digits (233 % →
   * 230 %, 61 % → 61 %), up, down or to the nearest. */
  function tidy(z: number, how: "round" | "floor" | "ceil" = "round"): number {
    const pct = z * 100;
    const unit = 10 ** Math.max(0, Math.floor(Math.log10(pct)) - 1);
    return (Math[how](pct / unit + (how === "floor" ? 1e-9 : how === "ceil" ? -1e-9 : 0)) * unit) / 100;
  }
  /** The system's scroll bar thickness (0 where they float over content). */
  let scrollbar = $state(0);
  const turned = $derived(rotation % 180 !== 0);
  /** The page's box (as turned) when it fits the stage. */
  const fit = $derived.by(() => {
    const boxAspect = turned ? 1 / aspect : aspect;
    const w = Math.max(50, stageW - 2 * PAD);
    const h = Math.max(50, stageH - 2 * PAD);
    return w * boxAspect <= h ? { w, h: w * boxAspect } : { w: h / boxAspect, h };
  });
  // Whole pixels, so nothing shifts by fractions.
  const box = $derived({ w: Math.round(fit.w * zoom), h: Math.round(fit.h * zoom) });
  /** The picture itself, before turning. */
  const img = $derived(turned ? { w: box.h, h: box.w } : box);
  const quick = $derived(thumbUrl(sourceId, entry, page, 1600));
  /** The picture to show: the quick one, or once zoomed past it, the
   * smallest sharper one that's big enough (else the biggest so far). */
  const src = $derived.by(() => {
    const need = Math.max(img.w, img.h) * (window.devicePixelRatio || 1);
    if (need <= 1600 * 1.1) return quick;
    const mine = Object.entries(sharp)
      .map(([k, url]) => ({ p: Number(k.split(":")[0]), side: Number(k.split(":")[1]), url }))
      .filter((x) => x.p === page)
      .sort((a, b) => a.side - b.side);
    return (mine.find((x) => x.side >= need) ?? mine.at(-1))?.url ?? quick;
  });
  /** Fit width leaves room for the vertical scroll bar it brings, and
   * rounds down to two digits (233 % → 230 %). */
  const fitWidthZoom = $derived(Math.max(MIN_ZOOM, tidy((stageW - scrollbar - 2 * PAD) / fit.w, "floor")));

  onMount(() => {
    if (entry.kind === "pdf") {
      api
        .pageCount(sourceId, entry.relPath)
        .then((n) => (pages = n))
        .catch(() => (pages = null));
    }
    // The stage's size includes its scroll bars: measured without them,
    // a scroll bar appearing would shrink "fit", which removes the scroll
    // bar, which grows "fit" again… (the page flickered at Fit width).
    const probe = document.createElement("div");
    probe.style.cssText = "position:absolute;visibility:hidden;width:100px;height:100px;overflow:scroll";
    document.body.append(probe);
    scrollbar = probe.offsetWidth - probe.clientWidth;
    probe.remove();
    const ro = new ResizeObserver(() => {
      if (!stage) return;
      stageW = stage.offsetWidth;
      stageH = stage.offsetHeight;
    });
    if (stage) ro.observe(stage);
    return () => ro.disconnect();
  });

  // A sharper picture once zoomed past the quick one (PDFs only; images
  // come at full size already).
  let sharpTimer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const need = Math.max(img.w, img.h) * (window.devicePixelRatio || 1);
    const p = page;
    if (entry.kind !== "pdf" || online || need <= 1600 * 1.1) return;
    const side = Math.min(4000, Math.ceil(need / 800) * 800);
    const key = `${p}:${side}`;
    if (sharp[key]) return;
    clearTimeout(sharpTimer);
    sharpTimer = setTimeout(async () => {
      const url = await api.pageImage(sourceId, entry.relPath, p, side).catch(() => null);
      if (url) sharp = { ...sharp, [key]: url };
    }, 250);
  });

  $effect(() => {
    pageInput = String(page + 1);
  });

  // MARK: Zooming and turning

  /** Zooms to `z`, keeping the point at (`x`, `y`) in the stage where it is. */
  async function zoomTo(z: number, x = stageW / 2, y = stageH / 2) {
    if (!stage) return;
    const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(z * 100) / 100));
    const ratio = next / zoom;
    const cx = stage.scrollLeft + x;
    const cy = stage.scrollTop + y;
    zoom = next;
    await tick();
    stage.scrollLeft = cx * ratio - x;
    stage.scrollTop = cy * ratio - y;
  }
  // ×1.25 or ÷1.25, rounded to two digits: 100 → 130 → 160 → 200 → 250 % and
  // 100 → 80 → 64 → 51 %. Always at least one step on.
  const zoomIn = () => zoomTo(Math.max(tidy(zoom * STEP), tidy(zoom, "floor") + 0.01));
  const zoomOut = () => zoomTo(Math.min(tidy(zoom / STEP), tidy(zoom, "ceil") - 0.01));
  const fitPage = () => (zoom = 1);
  const fitWidth = () => zoomTo(fitWidthZoom, stageW / 2, 0);
  const rotate = () => (rotation = (rotation + 90) % 360);

  function wheel(e: WheelEvent) {
    // Pinch on a trackpad arrives as a wheel with Ctrl; ⌘ or Ctrl + wheel too.
    if (!(e.ctrlKey || e.metaKey) || !stage) return;
    e.preventDefault();
    const r = stage.getBoundingClientRect();
    zoomTo(zoom * Math.exp(-e.deltaY * 0.01), e.clientX - r.left, e.clientY - r.top);
  }

  /** When the viewer opened: the second click of the double-click that
   * opened it mustn't zoom. */
  const openedAt = Date.now();

  function doubleClick(e: MouseEvent) {
    if (!stage || Date.now() - openedAt < 700) return;
    const r = stage.getBoundingClientRect();
    if (Math.abs(zoom - 1) < 0.01) zoomTo(2, e.clientX - r.left, e.clientY - r.top);
    else fitPage();
  }

  // MARK: Panning by dragging

  let drag: { x: number; y: number; left: number; top: number } | null = $state(null);
  function down(e: PointerEvent) {
    if (e.button !== 0 || !stage) return;
    drag = { x: e.clientX, y: e.clientY, left: stage.scrollLeft, top: stage.scrollTop };
    stage.setPointerCapture(e.pointerId);
  }
  function move(e: PointerEvent) {
    if (!drag || !stage) return;
    stage.scrollLeft = drag.left - (e.clientX - drag.x);
    stage.scrollTop = drag.top - (e.clientY - drag.y);
  }
  const up = () => (drag = null);
  const canPan = $derived(box.w > stageW - 2 * PAD + 1 || box.h > stageH - 2 * PAD + 1);

  // MARK: Pages

  function go(p: number) {
    if (pages && p >= 0 && p < pages && p !== page) {
      page = p;
      stage?.scrollTo({ top: 0 });
    }
  }

  function goTyped() {
    const n = parseInt(pageInput, 10);
    if (Number.isFinite(n)) go(n - 1);
    pageInput = String(page + 1);
  }

  function key(e: KeyboardEvent) {
    const typing = (e.target as HTMLElement)?.tagName === "INPUT";
    const cmd = e.metaKey || e.ctrlKey;
    const plain = !cmd && !e.altKey && !e.shiftKey;
    if (e.key === "Escape" || (e.key === " " && !typing && plain)) {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (onstep && !typing && plain && (e.key === "ArrowUp" || e.key === "ArrowDown")) {
      e.preventDefault();
      onstep(e.key === "ArrowDown" ? 1 : -1);
    } else if (cmd && (e.key === "=" || e.key === "+")) {
      e.preventDefault();
      zoomIn();
    } else if (cmd && e.key === "-") {
      e.preventDefault();
      zoomOut();
    } else if (cmd && e.key === "0") {
      e.preventDefault();
      fitPage();
    } else if (!typing && (e.key === "ArrowLeft" || e.key === "PageUp")) {
      e.preventDefault();
      go(page - 1);
    } else if (!typing && (e.key === "ArrowRight" || e.key === "PageDown")) {
      e.preventDefault();
      go(page + 1);
    } else {
      return;
    }
  }
</script>

<svelte:window onkeydown={key} />

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="viewer"
    role="dialog"
    aria-modal="true"
    aria-label={m.pictures.viewer}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <!-- MARK: Toolbar -->
    <header class="bar">
      <strong class="name" title={entry.name}>{title ?? entry.name}</strong>
      {#if pages && pages > 1}
        <span class="group">
          <button class="ib" onclick={() => go(page - 1)} disabled={page === 0} title={m.pictures.previousPage} aria-label={m.pictures.previousPage}>
            <Icon name="back" size={16} />
          </button>
          <label class="pageno">
            <span class="sr">{m.pictures.page}</span>
            <input
              bind:value={pageInput}
              inputmode="numeric"
              aria-label={m.pictures.page}
              onkeydown={(e) => e.key === "Enter" && goTyped()}
              onblur={goTyped}
            />
            <span class="muted">{m.pictures.ofPages(pages)}</span>
          </label>
          <button class="ib" onclick={() => go(page + 1)} disabled={page + 1 >= pages} title={m.pictures.nextPage} aria-label={m.pictures.nextPage}>
            <Icon name="chevron" size={16} />
          </button>
        </span>
      {/if}
      <span class="group">
        <button class="ib" onclick={zoomOut} disabled={zoom <= MIN_ZOOM} title="{m.pictures.zoomOut} (⌘−)" aria-label={m.pictures.zoomOut}>−</button>
        <span class="pct">{Math.round(zoom * 100)} %</span>
        <button class="ib" onclick={zoomIn} disabled={zoom >= MAX_ZOOM} title="{m.pictures.zoomIn} (⌘+)" aria-label={m.pictures.zoomIn}>+</button>
        <button class="txt" class:on={Math.abs(zoom - 1) < 0.01} onclick={fitPage} title="{m.pictures.fitPage} (⌘0)">{m.pictures.fitPage}</button>
        <button class="txt" class:on={Math.abs(zoom - fitWidthZoom) < 0.01} onclick={fitWidth}>{m.pictures.fitWidth}</button>
      </span>
      <button class="ib" onclick={rotate} title={m.pictures.rotate} aria-label={m.pictures.rotate}>
        <svg viewBox="0 0 24 24" width="16" height="16" aria-hidden="true"
          ><path d="M20 12a8 8 0 1 1-2.34-5.66M20 4v4.5h-4.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" /></svg
        >
      </button>
      <span class="space"></span>
      {#if online}<span class="note"><Icon name="cloud" size={14} /> {m.pictures.onlineNote}</span>{/if}
      <button class="txt" onclick={() => api.openEntry(sourceId, entry.relPath)}>{m.common.openInDefaultApp}</button>
      <button class="ib" onclick={onclose} title={m.common.closeEsc} aria-label={m.common.close}>
        <Icon name="close" size={16} />
      </button>
    </header>

    <!-- MARK: The page -->
    <div
      class="stage"
      class:pan={canPan}
      class:dragging={!!drag}
      bind:this={stage}
      role="presentation"
      onwheel={wheel}
      ondblclick={doubleClick}
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
    >
      {#if failed}
        <p class="nothing">{online ? m.pictures.noKeptPicture : m.pictures.noPicture}</p>
      {:else}
      <div class="sizer" style:width="{box.w + 2 * PAD}px" style:height="{box.h + 2 * PAD}px">
        <div class="box" style:width="{box.w}px" style:height="{box.h}px">
          <img
            {src}
            alt={entry.name}
            draggable="false"
            style:width="{img.w}px"
            style:height="{img.h}px"
            style:transform="translate(-50%, -50%) rotate({rotation}deg)"
            onload={(e) => {
              const i = e.currentTarget as HTMLImageElement;
              if (i.naturalWidth && i.src === quick) aspect = i.naturalHeight / i.naturalWidth;
            }}
            onerror={() => (failed = true)}
          />
        </div>
      </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 45;
    background: rgb(0 0 0 / 0.45);
    display: grid;
    place-items: center;
  }
  .viewer {
    width: calc(100vw - 64px);
    height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 22px 60px rgb(0 0 0 / 0.35);
    overflow: hidden;
  }
  .bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .name {
    max-width: 32%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .group {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .space {
    flex: 1;
  }
  button {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  button:disabled {
    opacity: 0.45;
  }
  .ib {
    width: 30px;
    height: 28px;
    padding: 0;
    display: grid;
    place-items: center;
    font-size: 16px;
  }
  .txt {
    padding: 3px 10px;
  }
  .txt.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .pct {
    min-width: 52px;
    text-align: center;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
  }
  .pageno {
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .pageno input {
    width: 40px;
    text-align: center;
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 3px 4px;
    background: var(--bg);
    color: var(--text);
  }
  .muted {
    color: var(--muted);
  }
  .note {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--muted);
  }
  .nothing {
    color: var(--muted);
    max-width: 360px;
    text-align: center;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
  .stage {
    flex: 1;
    min-height: 0;
    overflow: auto;
    background: var(--panel);
    display: grid;
    place-items: center;
    user-select: none;
  }
  .stage.pan {
    cursor: grab;
  }
  .stage.dragging {
    cursor: grabbing;
  }
  .sizer {
    display: grid;
    place-items: center;
  }
  .box {
    position: relative;
  }
  img {
    position: absolute;
    left: 50%;
    top: 50%;
    max-width: none;
    background: #fff;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.25);
    transition: transform 0.15s;
  }
</style>
