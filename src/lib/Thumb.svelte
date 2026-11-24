<script lang="ts" module>
  import { convertFileSrc } from "@tauri-apps/api/core";
  import type { Entry } from "#lib/api.ts";

  /** The `thumb:` URL of a document's picture (see `src-tauri/src/thumbs.rs`). */
  export function thumbUrl(sourceId: string, entry: Entry, page: number, side: number): string {
    return (
      convertFileSrc(`${sourceId}/${entry.relPath}`, "thumb") + `?w=${side}&p=${page}&m=${entry.modifiedMs ?? 0}`
    );
  }
</script>

<script lang="ts">
  // A document's picture (first page of a PDF, or the image itself) from the
  // `thumb:` URL scheme, or its kind icon when there's none: folders, other
  // kinds, and anything that fails to load. Online-only PDFs are asked for
  // too: the computer answers from its cache without reading the file (#62),
  // or with a 404. Online-only images aren't (nothing is kept for them).
  // With `tryAll` (the phone) every document is asked for: there the server
  // knows which have pictures (a cloud service's thumbnails download nothing).
  import Icon from "#lib/Icon.svelte";

  let {
    sourceId,
    entry,
    page = 0,
    side = 320,
    iconSize = 40,
    eager = false,
    tryAll = false,
  }: {
    sourceId: string;
    entry: Entry;
    page?: number;
    side?: number;
    iconSize?: number;
    eager?: boolean;
    tryAll?: boolean;
  } = $props();

  let failed = $state(false);
  const hasPicture = $derived(
    !entry.isDir &&
      (tryAll || entry.kind === "pdf" || (entry.kind === "image" && entry.cloud !== "cloudOnly")),
  );
  const src = $derived(thumbUrl(sourceId, entry, page, side));
  $effect(() => {
    void src;
    failed = false;
  });
</script>

{#if hasPicture && !failed}
  <img {src} alt="" loading={eager ? "eager" : "lazy"} draggable="false" onerror={() => (failed = true)} />
{:else}
  <span class="icon" class:folder={entry.isDir}>
    <Icon name={entry.isDir ? "folder" : entry.kind === "pdf" ? "pdf" : "file"} size={iconSize} />
  </span>
{/if}

<style>
  img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: contain;
    /* Follows the page's outline, not the letterboxing. */
    filter: drop-shadow(0 1px 3px rgb(0 0 0 / 0.28));
    user-select: none;
    -webkit-user-drag: none;
  }
  .icon {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .icon.folder {
    color: var(--folder);
  }
</style>
