<script lang="ts">
  // Import into Inbox (#122): the window that collects what to import.
  // Drop files or folders on it, or Add files… / Add folder… (the same on
  // every system); it lists them with how many documents each brings, then
  // Import closes it and the inbox shows the progress (inbox-import).
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, type ImportChosen } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import { droppedItems, runImport, type DroppedItem } from "#lib/inbox-import.svelte.ts";
  import { m, num } from "#lib/i18n/index.ts";

  let { inboxName, onclose }: { inboxName: string; onclose: () => void } = $props();

  /** Chosen with the buttons: copied by path. */
  let chosen = $state<ImportChosen[]>([]);
  let paths = $state<string[]>([]);
  let leftOut = $state(0);
  /** Dropped: copied by contents. */
  let dropped = $state<DroppedItem[]>([]);
  let over = $state(false);
  let reading = $state(false);
  let error = $state<string | null>(null);

  const count = $derived(paths.length + dropped.reduce((n, d) => n + d.files.length, 0));

  async function add(directory: boolean) {
    const picked = await openDialog({ multiple: true, directory, title: directory ? m.importInbox.addFolder : m.importInbox.addFiles });
    const list = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
    if (!list.length) return;
    reading = true;
    try {
      const plan = await api.inboxImportPlan(list);
      const known = new Set(chosen.map((c) => c.path));
      chosen = [...chosen, ...plan.chosen.filter((c) => !known.has(c.path))];
      paths = [...new Set([...paths, ...plan.files])];
      leftOut += plan.leftOut;
    } catch (e) {
      error = String(e);
    } finally {
      reading = false;
    }
  }

  async function drop(ev: DragEvent) {
    ev.preventDefault();
    over = false;
    if (!ev.dataTransfer) return;
    reading = true;
    try {
      dropped = [...dropped, ...(await droppedItems(ev.dataTransfer.items))];
    } catch (e) {
      error = String(e);
    } finally {
      reading = false;
    }
  }

  function start() {
    if (!count) return;
    runImport({ paths: $state.snapshot(paths), files: dropped.flatMap((d) => d.files) }, leftOut);
    onclose();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="import-title">
    <h1 id="import-title">{m.importInbox.title}</h1>
    <p class="muted small">{m.importInbox.intro(inboxName)}</p>

    <div
      class="drop"
      class:over
      role="region"
      aria-label={m.importInbox.dropHere}
      ondragover={(e) => {
        e.preventDefault();
        e.stopPropagation();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
        over = true;
      }}
      ondragleave={() => (over = false)}
      ondrop={(e) => {
        e.stopPropagation();
        drop(e);
      }}
    >
      {reading ? m.importInbox.reading : m.importInbox.dropHere}
    </div>

    {#if chosen.length || dropped.length}
      <ul class="list">
        {#each chosen as c (c.path)}
          <li title={c.path}>
            <Icon name={c.isDir ? "folder" : "file"} size={14} />
            <span class="name">{c.name}</span>
            <span class="count">{c.isDir ? m.importInbox.documents(c.documents) : c.documents ? "1" : m.importInbox.notADocument}</span>
          </li>
        {/each}
        {#each dropped as d, i (i)}
          <li>
            <Icon name={d.isDir ? "folder" : "file"} size={14} />
            <span class="name">{d.name}</span>
            <span class="count">{d.isDir ? m.importInbox.files(d.files.length) : "1"}</span>
          </li>
        {/each}
      </ul>
    {/if}
    {#if error}<p class="error small" role="alert">{error}</p>{/if}

    <div class="footer">
      <button onclick={() => add(false)} disabled={reading}>{m.importInbox.addFiles}</button>
      <button onclick={() => add(true)} disabled={reading}>{m.importInbox.addFolder}</button>
      <span class="grow"></span>
      <button onclick={onclose}>{m.common.cancel}</button>
      <button class="primary" onclick={start} disabled={!count || reading}>{m.importInbox.import(num(count), count)}</button>
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
    width: min(540px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  .small {
    font-size: 12px;
  }
  p {
    margin: 0;
  }
  .drop {
    margin-top: 12px;
    border: 2px dashed var(--border);
    border-radius: 10px;
    padding: 22px;
    text-align: center;
    color: var(--muted);
    background: var(--panel);
  }
  .drop.over {
    border-color: var(--accent);
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .list {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: auto;
    max-height: 220px;
  }
  .list li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
    border-bottom: 1px solid var(--border);
    color: var(--folder);
  }
  .list li:last-child {
    border-bottom: 0;
  }
  .name {
    color: var(--text);
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    margin-left: auto;
    color: var(--muted);
    white-space: nowrap;
  }
  .error {
    margin-top: 10px;
    color: var(--danger, #c0392b);
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .grow {
    flex: 1;
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 12px;
    font: inherit;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    font-weight: 600;
  }
  button:disabled {
    opacity: 0.5;
  }
</style>
