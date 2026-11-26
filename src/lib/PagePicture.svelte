<script lang="ts">
  // A document's page as a picture with ‹ › to flip through its pages and a
  // click to open it in the page viewer: the inbox's preview, and Details'
  // while browsing (#127). Where the picture comes from is up to the caller.
  import Icon from "#lib/Icon.svelte";
  import { m } from "#lib/i18n/index.ts";

  let {
    src,
    alt,
    loading = false,
    empty,
    pages = null,
    page = $bindable(0),
    big = false,
    onopen,
    onaspect,
  }: {
    src: string | null;
    alt: string;
    loading?: boolean;
    /** Said where there's no picture. */
    empty: string;
    pages?: number | null;
    page?: number;
    big?: boolean;
    /** Opens the page viewer on this page. */
    onopen: () => void;
    /** The picture's height / width, once it's loaded. */
    onaspect?: (aspect: number) => void;
  } = $props();

  let failed = $state(false);
  $effect(() => {
    void src;
    failed = false;
  });
  const showing = $derived(!!src && !failed);

  function flip(step: number) {
    if (pages && page + step >= 0 && page + step < pages) page += step;
  }
</script>

<div
  class="page"
  class:big
  class:zoomable={showing}
  role="button"
  tabindex="-1"
  title={showing ? m.pictures.openViewer : undefined}
  onclick={() => showing && onopen()}
  onkeydown={() => {}}
>
  {#if showing}
    <img
      {src}
      {alt}
      onload={(e) => {
        const i = e.currentTarget as HTMLImageElement;
        if (i.naturalWidth) onaspect?.(i.naturalHeight / i.naturalWidth);
      }}
      onerror={() => (failed = true)}
    />
  {:else if loading}
    <p class="muted center">{m.inbox.loadingPreview}</p>
  {:else}
    <p class="muted center">{empty}</p>
  {/if}
  {#if pages && pages > 1}
    <button class="flip prev" onclick={(e) => (e.stopPropagation(), flip(-1))} disabled={page === 0} title={m.pictures.previousPageKey} aria-label={m.pictures.previousPage}>
      <Icon name="back" size={18} />
    </button>
    <button class="flip next" onclick={(e) => (e.stopPropagation(), flip(1))} disabled={page + 1 >= pages} title={m.pictures.nextPageKey} aria-label={m.pictures.nextPage}>
      <Icon name="chevron" size={18} />
    </button>
  {/if}
</div>
{#if pages && pages > 1}<p class="muted small page-of">{m.pictures.pageOf(page + 1, pages)}</p>{/if}

<style>
  .page {
    position: relative;
    flex: 1;
    min-height: 0;
    width: 100%;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .page.zoomable {
    cursor: zoom-in;
  }
  .page img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.18);
    background: #fff;
  }
  .page.big {
    background: var(--panel);
    border-radius: 8px;
    padding: 12px;
  }
  .center {
    align-self: center;
    text-align: center;
    margin: 0;
    padding: 0 12px;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .page-of {
    flex: none;
    margin: 4px 0 0;
  }
  .flip {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 32px;
    height: 32px;
    border-radius: 50%;
    padding: 0;
    display: grid;
    place-items: center;
    opacity: 0.85;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.2);
  }
  .flip:disabled {
    opacity: 0.25;
  }
  .flip.prev {
    left: 4px;
  }
  .flip.next {
    right: 4px;
  }
</style>
