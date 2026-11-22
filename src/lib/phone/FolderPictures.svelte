<script lang="ts">
  // A folder's contents as pictures on the phone (#41): large icons (a grid
  // of tiles) or a gallery (one document large, with a strip of the others
  // below; swipe or tap the strip to move). Pictures come from the phone's
  // `thumb:` scheme (mobile/src/thumbs.rs): the cloud service's thumbnails,
  // or the system's for documents on the phone; otherwise the kind's icon.
  import { tick } from "svelte";
  import Icon from "#lib/Icon.svelte";
  import Thumb from "#lib/Thumb.svelte";
  import { formatDocDate, type Listed } from "#lib/phone/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    mode,
    sourceId,
    entries,
    onopen,
  }: {
    mode: "icons" | "gallery";
    sourceId: string;
    entries: Listed[];
    /** Opens a folder or document (tapped). */
    onopen: (e: Listed) => void;
  } = $props();

  // MARK: Gallery

  let at = $state(0);
  const current = $derived(entries[Math.min(at, entries.length - 1)] ?? null);
  let strip: HTMLDivElement | undefined = $state();

  // A new folder starts at its first document.
  let shownFor = "";
  $effect(() => {
    const key = entries.map((e) => e.relPath).join("\n");
    if (key !== shownFor) {
      shownFor = key;
      const firstDoc = entries.findIndex((e) => !e.isDir);
      at = firstDoc >= 0 ? firstDoc : 0;
    }
  });

  async function go(i: number) {
    at = Math.max(0, Math.min(entries.length - 1, i));
    await tick();
    strip?.querySelector(".here")?.scrollIntoView({ inline: "center", block: "nearest", behavior: "smooth" });
  }

  let touchX: number | null = null;
  function ontouchstart(ev: TouchEvent) {
    touchX = ev.touches[0]?.clientX ?? null;
  }
  function ontouchend(ev: TouchEvent) {
    const start = touchX;
    touchX = null;
    const end = ev.changedTouches[0]?.clientX;
    if (start === null || end === undefined) return;
    const dx = end - start;
    if (Math.abs(dx) > 50) go(at + (dx < 0 ? 1 : -1));
  }

  function caption(e: Listed): string {
    return [formatDocDate(e.meta?.documentDate ?? null), ...(e.meta?.tags ?? [])].filter(Boolean).join(" · ");
  }
</script>

{#if mode === "icons"}
  <div class="grid">
    {#each entries as e (e.relPath)}
      <button class="tile" onclick={() => onopen(e)}>
        <span class="pic">
          <Thumb {sourceId} entry={e} side={360} iconSize={36} tryAll />
          {#if e.meta?.favorite}<span class="fav" aria-hidden="true"><Icon name="star" size={11} /></span>{/if}
          {#if e.cloud === "cloudOnly" && !e.isDir}<span class="cloud" title={m.phone.inTheCloud}><Icon name="cloud" size={12} /></span>{/if}
        </span>
        <span class="name">{e.meta?.title || e.name}</span>
      </button>
    {/each}
  </div>
{:else if current}
  <div class="gallery">
    <button class="big" onclick={() => onopen(current)} {ontouchstart} {ontouchend} aria-label={current.meta?.title || current.name}>
      {#key current.relPath}
        <Thumb {sourceId} entry={current} side={1000} iconSize={72} tryAll eager />
      {/key}
    </button>
    <div class="caption">
      <strong>
        {#if current.meta?.favorite}<span class="fav-inline"><Icon name="star" size={12} /></span>{/if}
        {current.meta?.title || current.name}
      </strong>
      {#if !current.isDir}<span class="sub">{caption(current) || current.name}</span>{/if}
    </div>
    <div class="strip" bind:this={strip}>
      {#each entries as e, i (e.relPath)}
        <button class="mini" class:here={i === at} onclick={() => (i === at ? onopen(e) : go(i))} aria-label={e.meta?.title || e.name}>
          <Thumb {sourceId} entry={e} side={200} iconSize={22} tryAll />
        </button>
      {/each}
    </div>
  </div>
{/if}

<style>
  /* MARK: Large icons */
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(104px, 1fr));
    gap: 14px 10px;
    padding: 12px 16px calc(16px + var(--nav-inset, 0px));
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    min-width: 0;
  }
  .pic {
    position: relative;
    width: 100%;
    aspect-ratio: 3 / 4;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 8px;
    background: var(--panel);
    padding: 6px;
    box-sizing: border-box;
  }
  .name {
    width: 100%;
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
  .fav,
  .cloud {
    position: absolute;
    display: flex;
    padding: 3px;
    border-radius: 999px;
    background: var(--bg);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.2);
  }
  .fav {
    top: 4px;
    right: 4px;
    color: var(--accent);
  }
  .cloud {
    bottom: 4px;
    right: 4px;
    color: var(--cloud);
  }

  /* MARK: Gallery */
  .gallery {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 16px calc(12px + var(--nav-inset, 0px));
  }
  .big {
    height: min(46vh, 460px);
    display: flex;
    align-items: center;
    justify-content: center;
    border: 0;
    border-radius: 10px;
    background: var(--panel);
    padding: 10px;
    touch-action: pan-y;
  }
  .caption {
    display: flex;
    flex-direction: column;
    gap: 2px;
    text-align: center;
  }
  .caption strong {
    font-size: 16px;
    overflow-wrap: anywhere;
  }
  .fav-inline {
    color: var(--accent);
  }
  .sub {
    color: var(--muted);
    font-size: 13px;
  }
  .strip {
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 4px 2px 8px;
    scrollbar-width: none;
  }
  .mini {
    flex: 0 0 auto;
    width: 56px;
    height: 74px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 4px;
    border: 2px solid transparent;
    border-radius: 6px;
    background: var(--panel);
  }
  .mini.here {
    border-color: var(--accent);
  }
</style>
