<script lang="ts">
  // The star on a document in a list. Shown when the document is a favorite,
  // and otherwise while pointing at it (an ancestor with class `doc-host`) or
  // when it's selected; one click stars or unstars it at once.
  import { isBusy, starTitle } from "#lib/doc-actions.svelte.ts";
  import type { Entry } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    sourceId,
    doc,
    round = false,
    onclick,
  }: { sourceId: string; doc: Entry & { favorite?: boolean }; round?: boolean; onclick: () => void } = $props();

  const title = $derived(
    starTitle(doc, { add: m.main.addFavorite, remove: m.main.removeFavorite, removeDownloads: m.main.removeFavoriteDownloads }),
  );
</script>

<button
  type="button"
  class="doc-star"
  class:on={doc.favorite}
  class:round
  class:busy={isBusy(sourceId, doc.relPath)}
  {title}
  aria-label={title}
  aria-pressed={!!doc.favorite}
  draggable="false"
  onclick={(ev) => {
    ev.stopPropagation();
    onclick();
  }}
  ondblclick={(ev) => ev.stopPropagation()}>{doc.favorite ? "★" : "☆"}</button
>

<style>
  .doc-star {
    flex: none;
    border: 0;
    background: none;
    padding: 0 1px;
    line-height: 1;
    font-size: 14px;
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .doc-star.on {
    color: #d9a520;
  }
  /* Over a picture: on a small disc, like the cloud badge. */
  .doc-star.round {
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    background: var(--bg);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.25);
  }
  .doc-star.on,
  .doc-star:focus-visible,
  :global(.doc-host:hover) .doc-star,
  :global(.doc-host.selected) .doc-star {
    opacity: 1;
  }
  .doc-star:hover {
    color: #d9a520;
  }
  .doc-star.busy {
    opacity: 0.5;
    pointer-events: none;
  }
</style>
