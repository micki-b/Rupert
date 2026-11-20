<script lang="ts">
  // "Move to…": pick a folder (in any source) for a document or folder.
  // The window does the moving; this only chooses where.
  import type { Source } from "$lib/api";
  import FolderNode from "$lib/FolderNode.svelte";
  import { canMoveInto, type MoveItem } from "$lib/drag.svelte";

  let {
    item,
    sources,
    onmove,
    onclose,
  }: {
    item: MoveItem;
    sources: Source[];
    /** Moves the item there; resolves to whether it worked. */
    onmove: (sourceId: string, relPath: string) => Promise<boolean>;
    onclose: () => void;
  } = $props();

  let dest = $state<{ sourceId: string; relPath: string } | null>(null);
  let busy = $state(false);

  const ok = $derived(!!dest && canMoveInto(item, dest.sourceId, dest.relPath));
  const why = $derived.by(() => {
    if (!dest || ok) return "";
    return item.entry.isDir && dest.sourceId === item.sourceId && dest.relPath !== parentOf(item.entry.relPath)
      ? "A folder can't be moved into itself."
      : "It's already in this folder.";
  });

  function parentOf(relPath: string) {
    return relPath.includes("/") ? relPath.slice(0, relPath.lastIndexOf("/")) : "";
  }

  async function move() {
    if (!dest || !ok || busy) return;
    busy = true;
    const done = await onmove(dest.sourceId, dest.relPath);
    busy = false;
    if (done) onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !busy && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="move-title">
    <h1 id="move-title">Move “{item.entry.name}”</h1>
    <p class="muted small">
      Choose a folder. {item.entry.isDir ? "The folder moves with everything in it" : "The document moves with its details"},
      on disk, right away.
    </p>
    <div class="tree" role="tree">
      {#each sources.filter((s) => s.available) as s (s.id)}
        <FolderNode
          sourceId={s.id}
          relPath=""
          name={s.name}
          depth={0}
          selected={dest}
          onselect={(sourceId, relPath) => (dest = { sourceId, relPath })}
          startExpanded
        />
      {/each}
    </div>
    <div class="footer">
      <span class="muted small">{why}</span>
      <button onclick={onclose} disabled={busy}>Cancel</button>
      <button class="primary" onclick={move} disabled={!ok || busy}>{busy ? "Moving…" : "Move"}</button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.3);
    display: grid;
    place-items: center;
    z-index: 40; /* above the chat panel (20) and menus (30) */
  }
  .dialog {
    width: min(460px, calc(100vw - 32px));
    height: min(560px, calc(100vh - 64px));
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.25);
    padding: 18px 20px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 16px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 12px;
  }
  p {
    margin: 0 0 12px;
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  .footer span {
    flex: 1;
  }
</style>
