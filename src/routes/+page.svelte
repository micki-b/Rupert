<script lang="ts">
  import { onMount } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    api,
    onDirChanged,
    providerLabel,
    revealLabel,
    type AppInfo,
    type Entry,
    type Source,
  } from "$lib/api";
  import { changes } from "$lib/changes.svelte";
  import { formatDate, formatSize, kindLabel } from "$lib/format";
  import FolderNode from "$lib/FolderNode.svelte";
  import Icon from "$lib/Icon.svelte";

  type Folder = { sourceId: string; relPath: string };

  let info = $state<AppInfo | null>(null);
  let sources = $state<Source[]>([]);
  let folder = $state<Folder | null>(null);
  let entries = $state<Entry[]>([]);
  let entry = $state<Entry | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let menuFor = $state<string | null>(null);
  let renaming = $state<string | null>(null);
  let renameValue = $state("");

  const source = $derived(sources.find((s) => s.id === folder?.sourceId) ?? null);
  const crumbs = $derived.by(() => {
    if (!folder || !source) return [];
    const parts = folder.relPath ? folder.relPath.split("/") : [];
    return [
      { name: source.name, relPath: "" },
      ...parts.map((name, i) => ({ name, relPath: parts.slice(0, i + 1).join("/") })),
    ];
  });

  // MARK: - Loading

  async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      error = null;
      return await action();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadSources(refresh = false) {
    const list = await run(() => (refresh ? api.refreshSources() : api.listSources()));
    if (list) sources = list;
  }

  async function loadEntries() {
    if (!folder) return;
    const current = folder;
    loading = true;
    const list = await run(() => api.listDir(current.sourceId, current.relPath));
    if (folder !== current) return; // user moved on meanwhile
    loading = false;
    entries = list ?? [];
    if (entry) entry = entries.find((e) => e.relPath === entry!.relPath) ?? null;
  }

  function selectFolder(sourceId: string, relPath: string) {
    folder = { sourceId, relPath };
    entry = null;
    entries = [];
    loadEntries();
  }

  onMount(() => {
    api.appInfo().then((i) => (info = i));
    loadSources();

    const unlisten = [
      onDirChanged((batch) => changes.push(batch)),
      getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) loadSources(true);
      }),
    ];
    const closeMenu = () => (menuFor = null);
    window.addEventListener("click", closeMenu);
    return () => {
      unlisten.forEach((p) => p.then((off) => off()));
      window.removeEventListener("click", closeMenu);
    };
  });

  $effect(() => {
    changes.tick;
    if (folder && changes.affects(folder.sourceId, folder.relPath)) loadEntries();
  });

  // MARK: - Source actions

  async function addSource() {
    const path = await open({ directory: true, multiple: false, title: "Add a folder to Rupert" });
    if (typeof path !== "string") return;
    const added = await run(() => api.addSource(path));
    if (!added) return;
    await loadSources();
    selectFolder(added.id, "");
  }

  async function removeSource(s: Source) {
    menuFor = null;
    const ok = await ask(
      `Remove "${s.name}" from Rupert?\n\nThe folder and its files stay exactly where they are.`,
      { title: "Remove source", kind: "warning", okLabel: "Remove" },
    );
    if (!ok) return;
    await run(() => api.removeSource(s.id));
    if (folder?.sourceId === s.id) {
      folder = null;
      entries = [];
      entry = null;
    }
    await loadSources();
  }

  async function toggleAllTypes(s: Source) {
    menuFor = null;
    await run(() => api.updateSource(s.id, { showAllTypes: !s.showAllTypes }));
    await loadSources();
    if (folder?.sourceId === s.id) loadEntries();
  }

  function startRename(s: Source) {
    menuFor = null;
    renaming = s.id;
    renameValue = s.name;
  }

  async function finishRename(s: Source) {
    const name = renameValue.trim();
    renaming = null;
    if (!name || name === s.name) return;
    await run(() => api.updateSource(s.id, { name }));
    await loadSources();
  }

  // MARK: - Entry actions

  function activate(e: Entry) {
    if (!folder) return;
    if (e.isDir) selectFolder(folder.sourceId, e.relPath);
    else run(() => api.openEntry(folder!.sourceId, e.relPath));
  }

  function reveal(e: Entry) {
    if (folder) run(() => api.revealEntry(folder!.sourceId, e.relPath));
  }
</script>

<div class="shell">
  <!-- MARK: Sources -->
  <aside class="sources">
    <header>
      <h2>Sources</h2>
      <button class="icon-btn" onclick={addSource} title="Add a folder" aria-label="Add a folder">
        <Icon name="plus" />
      </button>
    </header>

    <div role="tree" aria-label="Sources">
      {#each sources as s (s.id)}
        {#if renaming === s.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={renameValue}
            autofocus
            onblur={() => finishRename(s)}
            onkeydown={(e) => {
              if (e.key === "Enter") finishRename(s);
              if (e.key === "Escape") renaming = null;
            }}
          />
        {:else if s.available}
          <div class="source">
            <FolderNode
              sourceId={s.id}
              relPath=""
              name={s.name}
              depth={0}
              selected={folder}
              onselect={selectFolder}
              startExpanded
            >
              {#snippet trailing()}
                <span class="provider">{providerLabel[s.provider]}</span>
                <button
                  class="icon-btn more"
                  aria-label="Source options"
                  onclick={(e) => {
                    e.stopPropagation();
                    menuFor = menuFor === s.id ? null : s.id;
                  }}
                >
                  <Icon name="more" size={14} />
                </button>
              {/snippet}
            </FolderNode>
            {#if menuFor === s.id}
              {@render menu(s)}
            {/if}
          </div>
        {:else}
          <div class="source unavailable" title={s.root}>
            <div class="row">
              <Icon name="warning" size={14} />
              <span class="label">{s.name}</span>
              <span class="provider">unavailable</span>
              <button
                class="icon-btn more"
                aria-label="Source options"
                onclick={(e) => {
                  e.stopPropagation();
                  menuFor = menuFor === s.id ? null : s.id;
                }}
              >
                <Icon name="more" size={14} />
              </button>
            </div>
            {#if menuFor === s.id}
              {@render menu(s)}
            {/if}
          </div>
        {/if}
      {/each}
    </div>

    {#if sources.length === 0}
      <p class="empty">No sources yet.</p>
    {/if}
  </aside>

  {#snippet menu(s: Source)}
    <div class="menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="menu-path" title={s.root}>{s.root}</div>
      <button role="menuitemcheckbox" aria-checked={s.showAllTypes} onclick={() => toggleAllTypes(s)}>
        <span class="check">{s.showAllTypes ? "✓" : ""}</span>Show all file types
      </button>
      <button role="menuitem" onclick={() => startRename(s)}>
        <span class="check"></span>Rename…
      </button>
      <button role="menuitem" class="danger" onclick={() => removeSource(s)}>
        <span class="check"></span>Remove from Rupert…
      </button>
    </div>
  {/snippet}

  <!-- MARK: Documents -->
  <main class="documents">
    {#if error}
      <div class="error" role="alert">
        <Icon name="warning" size={14} />
        <span>{error}</span>
        <button class="icon-btn" onclick={() => (error = null)} aria-label="Dismiss">×</button>
      </div>
    {/if}

    {#if folder && source}
      <nav class="crumbs" aria-label="Folder path">
        {#each crumbs as c, i (c.relPath)}
          {#if i > 0}<span class="sep">/</span>{/if}
          <button
            class:current={i === crumbs.length - 1}
            onclick={() => selectFolder(source.id, c.relPath)}>{c.name}</button
          >
        {/each}
      </nav>

      {#if entries.length === 0 && loading}
        <p class="empty center">Loading…</p>
      {:else if entries.length === 0}
        <p class="empty center">
          {source.showAllTypes ? "This folder is empty." : "No documents in this folder."}
        </p>
      {:else}
        <table>
          <thead>
            <tr>
              <th class="name">Name</th>
              <th>Kind</th>
              <th class="num">Size</th>
              <th>Modified</th>
            </tr>
          </thead>
          <tbody>
            {#each entries as e (e.relPath)}
              <tr
                class:selected={entry?.relPath === e.relPath}
                onclick={() => (entry = e)}
                ondblclick={() => activate(e)}
              >
                <td class="name">
                  <span class="file-icon" class:folder={e.isDir}>
                    <Icon name={e.isDir ? "folder" : e.kind === "pdf" ? "pdf" : "file"} size={15} />
                  </span>
                  <span class="file-name">{e.name}</span>
                  {#if e.cloud === "cloudOnly"}
                    <span class="cloud" title="Online-only: opening it downloads it">
                      <Icon name="cloud" size={14} />
                    </span>
                  {/if}
                </td>
                <td class="muted">{e.isDir ? "Folder" : kindLabel[e.kind ?? "other"]}</td>
                <td class="num muted">{e.isDir || e.cloud === "cloudOnly" && !e.size ? "" : formatSize(e.size)}</td>
                <td class="muted">{formatDate(e.modifiedMs)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else}
      <div class="welcome">
        <h1>Rupert</h1>
        <p class="muted">The librarian for your scanned documents.</p>
        {#if sources.length === 0}
          <p>Add a folder to get started. Rupert shows it as it is on disk and never moves your files on its own.</p>
          <button class="primary" onclick={addSource}>Add a folder…</button>
        {:else}
          <p class="muted">Choose a folder on the left.</p>
        {/if}
      </div>
    {/if}
  </main>

  <!-- MARK: Details -->
  <section class="details">
    <h2>Details</h2>
    {#if entry}
      <div class="detail-name">{entry.name}</div>
      <dl>
        <dt>Kind</dt>
        <dd>{entry.isDir ? "Folder" : kindLabel[entry.kind ?? "other"]}</dd>
        {#if !entry.isDir && entry.size}
          <dt>Size</dt>
          <dd>{formatSize(entry.size)}</dd>
        {/if}
        <dt>Modified</dt>
        <dd>{formatDate(entry.modifiedMs)}</dd>
        <dt>Location</dt>
        <dd>{source?.name}/{entry.relPath}</dd>
        {#if entry.cloud === "cloudOnly"}
          <dt>Storage</dt>
          <dd class="cloud-note">
            <Icon name="cloud" size={14} /> Online-only. Opening it downloads it.
          </dd>
        {/if}
      </dl>
      <div class="actions">
        <button class="primary" onclick={() => activate(entry!)}>Open</button>
        <button onclick={() => reveal(entry!)}>{revealLabel(info?.os)}</button>
      </div>
    {:else}
      <p class="empty">Select a document.</p>
    {/if}
  </section>
</div>

<footer>
  {#if info}{info.name} {info.version} · {info.os}/{info.arch}{/if}
</footer>

<style>
  .shell {
    height: calc(100vh - 22px);
    display: grid;
    grid-template-columns: 260px 1fr 280px;
  }

  h2 {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .muted,
  .empty {
    color: var(--muted);
  }

  button {
    border: 1px solid var(--border);
    background: var(--bg);
    border-radius: 6px;
    padding: 4px 10px;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    padding: 2px;
    color: var(--muted);
    border-radius: 4px;
  }
  .icon-btn:hover {
    background: var(--hover);
    color: var(--text);
  }

  /* Sources */
  .sources {
    background: var(--panel);
    border-right: 1px solid var(--border);
    padding: 10px 8px;
    overflow: auto;
  }
  .sources header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 8px 8px;
  }
  .source {
    position: relative;
    margin-bottom: 6px;
  }
  .source :global(.row .label) {
    font-weight: 500;
  }
  .provider {
    font-size: 10px;
    color: var(--muted);
    white-space: nowrap;
  }
  .more {
    opacity: 0;
  }
  .source:hover .more,
  .source:focus-within .more {
    opacity: 1;
  }
  .unavailable .row {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px;
    color: var(--muted);
  }
  .unavailable .label {
    flex: 1;
  }
  .rename {
    width: 100%;
    font: inherit;
    padding: 3px 8px;
    margin-bottom: 6px;
    border: 1px solid var(--accent);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
  }
  .menu {
    position: absolute;
    right: 4px;
    top: 28px;
    z-index: 10;
    min-width: 210px;
    padding: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
  }
  .menu button {
    display: flex;
    width: 100%;
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 8px;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .menu .danger {
    color: var(--danger);
  }
  .menu .check {
    width: 18px;
  }
  .menu-path {
    padding: 4px 8px 6px;
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    border-bottom: 1px solid var(--border);
    margin-bottom: 4px;
  }

  /* Documents */
  .documents {
    overflow: auto;
    padding: 12px 16px;
    min-width: 0;
  }
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    margin-bottom: 10px;
    border-radius: 6px;
    background: var(--danger-soft);
    color: var(--danger);
  }
  .error span {
    flex: 1;
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    margin-bottom: 10px;
  }
  .crumbs button {
    border: 0;
    background: none;
    padding: 2px 4px;
    color: var(--muted);
  }
  .crumbs button:hover {
    color: var(--text);
  }
  .crumbs .current {
    color: var(--text);
    font-weight: 600;
  }
  .sep {
    color: var(--border);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  th {
    text-align: left;
    font-weight: 500;
    font-size: 11px;
    color: var(--muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  th:nth-child(2) {
    width: 70px;
  }
  th:nth-child(3) {
    width: 80px;
  }
  th:nth-child(4) {
    width: 170px;
  }
  td {
    padding: 4px 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  tbody tr {
    cursor: default;
    user-select: none;
  }
  tbody tr:hover {
    background: var(--hover);
  }
  tbody tr.selected {
    background: var(--accent-soft);
  }
  td.name {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .file-icon {
    display: grid;
    color: var(--muted);
    flex: none;
  }
  .file-icon.folder {
    color: var(--folder);
  }
  .cloud {
    display: grid;
    color: var(--cloud);
    flex: none;
  }
  .num {
    text-align: right;
  }
  .center {
    text-align: center;
    margin-top: 40px;
  }
  .welcome {
    max-width: 420px;
    margin: 80px auto 0;
    text-align: center;
  }
  .welcome h1 {
    margin: 0 0 4px;
    font-size: 26px;
  }

  /* Details */
  .details {
    background: var(--panel);
    border-left: 1px solid var(--border);
    padding: 10px 14px;
    overflow: auto;
  }
  .details h2 {
    margin-bottom: 10px;
  }
  .detail-name {
    font-weight: 600;
    font-size: 14px;
    word-break: break-word;
    margin-bottom: 10px;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 10px;
    margin: 0 0 14px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    word-break: break-word;
  }
  .cloud-note {
    display: flex;
    gap: 4px;
    align-items: center;
    color: var(--cloud);
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  footer {
    height: 22px;
    padding: 3px 12px;
    font-size: 11px;
    color: var(--muted);
    border-top: 1px solid var(--border);
  }
</style>
