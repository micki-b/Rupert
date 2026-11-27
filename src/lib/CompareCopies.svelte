<script lang="ts">
  // Compare duplicates (#111): up to three copies side by side, each with
  // its page large, its details, Keep this one, Open and Show in Finder.
  // ← → turn the page in all of them together, so the same page sits side
  // by side. Online-only copies show the picture Rupert kept (nothing is
  // downloaded); Open downloads, as anywhere.
  import { onMount } from "svelte";
  import { api, revealLabel, type Hit, type Source } from "#lib/api.ts";
  import { formatDay, formatSize } from "#lib/format.ts";
  import { m } from "#lib/i18n/index.ts";
  import Icon from "#lib/Icon.svelte";
  import { thumbUrl } from "#lib/Thumb.svelte";

  let {
    docs,
    keep,
    first = 0,
    sources,
    os,
    canChoose,
    cantrash,
    onkeep,
    ontrash,
    onclose,
  }: {
    docs: Hit[];
    /** Index of the copy to keep. */
    keep: number;
    /** The copy clicked to open this (shown first). */
    first?: number;
    sources: Source[];
    os: string | undefined;
    /** Whether the keep choice can change ("probably the same" can't). */
    canChoose: boolean;
    /** Whether a copy is one that goes to the Trash. */
    cantrash: (index: number) => boolean;
    onkeep: (index: number) => void;
    /** Moves that copy to the Trash now (after the kept one takes its tags). */
    ontrash: (index: number) => void;
    onclose: () => void;
  } = $props();

  /** At most three shown: the one clicked, the one kept, then the rest. */
  // svelte-ignore state_referenced_locally
  let shown = $state<number[]>([...new Set([first, keep, ...docs.map((_, i) => i)])].slice(0, 3));
  let page = $state(0);
  // One entry per copy; the group doesn't change while this is open.
  // svelte-ignore state_referenced_locally
  let pages = $state<(number | null)[]>(docs.map(() => null));
  // svelte-ignore state_referenced_locally
  let failed = $state<boolean[]>(docs.map(() => false));

  onMount(() => {
    docs.forEach((d, i) => {
      if (d.kind === "pdf" && d.cloud !== "cloudOnly") {
        api
          .pageCount(d.sourceId, d.relPath)
          .then((n) => (pages[i] = n))
          .catch(() => {});
      } else {
        pages[i] = 1;
      }
    });
  });

  const most = $derived(Math.max(1, ...shown.map((i) => pages[i] ?? 1)));

  function turn(step: number) {
    const next = page + step;
    if (next >= 0 && next < most) {
      page = next;
      failed = docs.map(() => false);
    }
  }

  function toggle(i: number) {
    if (shown.includes(i)) {
      if (shown.length > 1) shown = shown.filter((x) => x !== i);
    } else if (shown.length < 3) {
      shown = [...shown, i];
    }
  }

  const sourceName = (id: string) => sources.find((s) => s.id === id)?.name ?? "";
  const folderOf = (d: Hit) => [sourceName(d.sourceId), ...d.relPath.split("/").slice(0, -1)].join(" › ");

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    } else if (e.key === "ArrowRight" || e.key === "PageDown") {
      e.preventDefault();
      turn(1);
    } else if (e.key === "ArrowLeft" || e.key === "PageUp") {
      e.preventDefault();
      turn(-1);
    }
  }
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div class="compare" role="dialog" aria-modal="true" aria-label={m.health.compareTitle}>
  <div class="bar">
    <h1>{m.health.compareTitle}</h1>
    {#if docs.length > 3}
      <span class="small muted">{m.health.compareChoose}</span>
      {#each docs as d, i (d.sourceId + "\n" + d.relPath)}
        <label class="pick small" title={d.relPath}>
          <input type="checkbox" checked={shown.includes(i)} disabled={!shown.includes(i) && shown.length >= 3} onchange={() => toggle(i)} />
          {d.name}
        </label>
      {/each}
    {/if}
    <span class="grow"></span>
    <button class="icon" onclick={() => turn(-1)} disabled={page === 0} aria-label={m.pictures.previousPage}><Icon name="back" size={16} /></button>
    <span class="small">{m.pictures.pageOf(page + 1, most)}</span>
    <button class="icon" onclick={() => turn(1)} disabled={page + 1 >= most} aria-label={m.pictures.nextPage}><Icon name="chevron" size={16} /></button>
    <button onclick={onclose}>{m.common.done}</button>
  </div>
  <div class="panes" style:grid-template-columns="repeat({shown.length}, minmax(0, 1fr))">
    {#each shown as i (i)}
      {@const d = docs[i]}
      <section class="pane" class:kept={i === keep}>
        <header>
          <span class="name" title={d.relPath}>{d.name}</span>
          <span class="small muted">{folderOf(d)}</span>
          <span class="small muted">
            {d.documentDate ?? formatDay(d.modifiedMs)} · {formatSize(d.size)}{d.cloud === "cloudOnly" ? " · " + m.health.onlineOnly : ""}{#if d.title}
              · {d.title}{/if}
          </span>
          {#if d.tags.length}<span class="small muted">{d.tags.join(", ")}</span>{/if}
        </header>
        <div class="page">
          {#if page >= (pages[i] ?? 1)}
            <p class="muted small">{m.health.noPage(page + 1)}</p>
          {:else if failed[i]}
            <p class="muted small">{d.cloud === "cloudOnly" ? m.pictures.noKeptPicture : m.pictures.noPicture}</p>
          {:else}
            <img src={thumbUrl(d.sourceId, d, page, 1600)} alt={d.name} onerror={() => (failed[i] = true)} />
          {/if}
        </div>
        <footer>
          {#if i === keep}
            <span class="verdict">✓ {m.health.keepIt}</span>
          {:else if canChoose}
            <button onclick={() => onkeep(i)}>{m.health.keepThis}</button>
          {:else}
            <span></span>
          {/if}
          <span class="grow"></span>
          {#if cantrash(i)}
            <button class="icon danger" onclick={() => ontrash(i)} title={m.health.trashThis} aria-label={m.health.trashThis}
              ><Icon name="trash" size={15} /></button
            >
          {/if}
          <button onclick={() => api.openEntry(d.sourceId, d.relPath)}>{m.main.open}</button>
          <button onclick={() => api.revealEntry(d.sourceId, d.relPath)}>{revealLabel(os)}</button>
        </footer>
      </section>
    {/each}
  </div>
</div>

<style>
  .compare {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding: 10px 16px;
    border-bottom: 1px solid var(--border);
  }
  h1 {
    margin: 0;
    font-size: 15px;
  }
  .small {
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
  }
  .grow {
    flex: 1;
  }
  .pick {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .panes {
    flex: 1;
    min-height: 0;
    display: grid;
    gap: 1px;
    background: var(--border);
  }
  .pane {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--bg);
  }
  .pane.kept header {
    background: var(--accent-soft);
  }
  header {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border);
  }
  .name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .page {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    padding: 12px;
    background: var(--panel);
    overflow: auto;
  }
  .page img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    background: #fff;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.18);
  }
  footer {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-top: 1px solid var(--border);
  }
  .verdict {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent-strong);
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 10px;
    font: inherit;
  }
  button.icon {
    width: 28px;
    height: 26px;
    padding: 0;
    display: grid;
    place-items: center;
  }
  button.icon.danger {
    color: var(--danger, #c0392b);
  }
  button:disabled {
    opacity: 0.4;
  }
</style>
