<script lang="ts">
  // File it ▸ Browse… (#63): the folder tree (as in Move to…), opened at the
  // current choice, with New folder… for a folder that doesn't exist yet.
  import { api, type FolderChoice, type Source } from "#lib/api.ts";
  import FolderNode from "#lib/FolderNode.svelte";
  import { m } from "#lib/i18n/index.ts";

  let {
    sources,
    current,
    title,
    onchoose,
    onclose,
  }: {
    sources: Source[];
    /** The folder chosen so far, if any. */
    current: FolderChoice | null;
    /** The document's title, for the heading. */
    title: string;
    onchoose: (f: FolderChoice) => void;
    onclose: () => void;
  } = $props();

  const available = $derived(sources.filter((s) => s.available));
  // svelte-ignore state_referenced_locally
  let dest = $state<{ sourceId: string; relPath: string } | null>(
    current ? { sourceId: current.sourceId, relPath: current.dir } : null,
  );
  // svelte-ignore state_referenced_locally
  const opened = current ? { sourceId: current.sourceId, relPath: current.dir } : null;
  let naming = $state(false);
  let newName = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const destSource = $derived(available.find((s) => s.id === dest?.sourceId) ?? null);
  /** The chosen folder's own name, for "New folder in “…”". */
  const destName = $derived(dest ? (dest.relPath.split("/").at(-1) || destSource?.name) : "");

  function choiceFor(sourceId: string, relPath: string): FolderChoice | null {
    const s = available.find((x) => x.id === sourceId);
    if (!s) return null;
    return { sourceId, dir: relPath, label: relPath ? `${s.name}/${relPath}` : s.name };
  }

  function choose() {
    const c = dest && choiceFor(dest.sourceId, dest.relPath);
    if (c) onchoose(c);
  }

  async function create() {
    const name = newName.trim();
    if (!dest || !name || busy) return;
    busy = true;
    try {
      const rel = await api.createFolder(dest.sourceId, dest.relPath, name);
      error = null;
      naming = false;
      newName = "";
      // Chosen at once: making a folder here means filing into it.
      const c = choiceFor(dest.sourceId, rel);
      if (c) onchoose(c);
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && !busy) {
      e.stopPropagation();
      if (naming) naming = false;
      else onclose();
    }
  }}
/>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="pick-title">
    <h1 id="pick-title">{m.inbox.chooseFolder}</h1>
    {#if title}<p class="muted small">{m.inbox.forDocument(title)}</p>{/if}
    <div class="tree" role="tree">
      {#each available as s (s.id)}
        <FolderNode
          sourceId={s.id}
          relPath=""
          name={s.name}
          depth={0}
          selected={dest}
          expandTo={opened}
          onselect={(sourceId, relPath) => ((dest = { sourceId, relPath }), (error = null))}
          startExpanded={available.length === 1}
        />
      {/each}
    </div>
    {#if naming}
      <div class="new">
        <label for="new-folder" class="small muted">{m.inbox.newFolderIn(destName ?? "")}</label>
        <div class="row">
          <!-- svelte-ignore a11y_autofocus -->
          <input
            id="new-folder"
            bind:value={newName}
            autofocus
            autocomplete="off"
            onkeydown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                create();
              }
            }}
          />
          <button class="primary" onclick={create} disabled={!newName.trim() || busy}>{m.inbox.create}</button>
        </div>
      </div>
    {/if}
    {#if error}<p class="error small" role="alert">{error}</p>{/if}
    <div class="footer">
      <button onclick={() => (naming = true)} disabled={!dest || naming}>{m.inbox.newFolder}</button>
      <span class="space"></span>
      <button onclick={onclose} disabled={busy}>{m.common.cancel}</button>
      <button class="primary" onclick={choose} disabled={!dest || busy}>{m.inbox.choose}</button>
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
    z-index: 40;
  }
  .dialog {
    width: min(460px, calc(100vw - 32px));
    height: min(580px, calc(100vh - 64px));
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
    margin: 0 0 2px;
    font-size: 16px;
  }
  p {
    margin: 0 0 10px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .small {
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
  }
  .tree {
    flex: 1;
    min-height: 0;
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 4px;
  }
  .new {
    margin-top: 10px;
  }
  .row {
    display: flex;
    gap: 6px;
    margin-top: 4px;
  }
  .row input {
    flex: 1;
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
  }
  .error {
    margin: 10px 0 0;
    color: var(--danger, #c0392b);
    white-space: normal;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  .space {
    flex: 1;
  }
</style>
