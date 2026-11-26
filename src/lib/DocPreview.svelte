<script lang="ts">
  // The selected document's page at the top of Details while browsing
  // (#127), as the inbox shows it above File it. Online-only documents show
  // the picture Rupert kept (#62) and never download anything.
  import { api, type Entry } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";
  import PagePicture from "#lib/PagePicture.svelte";
  import PreviewPane from "#lib/PreviewPane.svelte";
  import { thumbUrl } from "#lib/Thumb.svelte";

  let {
    sourceId,
    entry,
    room,
    width,
    onopen,
  }: {
    sourceId: string;
    entry: Entry;
    /** The height Details can spare above its own fields. */
    room: number;
    width: number;
    /** Opens the page viewer at this page. */
    onopen: (page: number) => void;
  } = $props();

  // A picture big enough for the widest Details on a sharp screen.
  const SIDE = 1200;

  let page = $state(0);
  let pages = $state<number | null>(null);
  let aspect = $state(1.294);
  const online = $derived(entry.cloud === "cloudOnly");

  $effect(() => {
    // Another document: its first page, and how many it has (a local PDF's;
    // an online-only one's count would mean reading it).
    const { relPath, kind } = entry;
    const local = !online;
    page = 0;
    pages = null;
    if (kind !== "pdf" || !local) return;
    let live = true;
    api
      .pageCount(sourceId, relPath)
      .then((n) => live && (pages = n))
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  const src = $derived(thumbUrl(sourceId, entry, page, SIDE));
</script>

<PreviewPane
  pane="details"
  {room}
  natural={(width - 24) * aspect + 18 + (pages && pages > 1 ? 20 : 0)}
  onopen={() => onopen(page)}
>
  <div class="preview">
    <PagePicture
      {src}
      alt={m.inbox.firstPage(entry.name)}
      empty={online ? m.pictures.noKeptPicture : m.pictures.noPicture}
      {pages}
      bind:page
      onopen={() => onopen(page)}
      onaspect={(a) => (aspect = a)}
    />
  </div>
</PreviewPane>

<style>
  .preview {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: var(--panel);
    padding: 12px 12px 6px;
  }
</style>
