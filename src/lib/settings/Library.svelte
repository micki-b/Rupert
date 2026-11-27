<script lang="ts">
  // Settings ▸ Library: the sources (the folders Rupert shows), with the
  // same options as their ⋯ menus in the main window's sidebar; and what
  // goes along when a document is sent (#69).
  import { ask, open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "#lib/Icon.svelte";
  import { api, providerLabel, revealLabel, type Settings, type Source } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    settings = $bindable(),
    sources,
    os,
    onchanged,
    onerror,
  }: {
    settings: Settings;
    sources: Source[];
    os: string | undefined;
    /** Reload the sources after a change. */
    onchanged: () => Promise<void>;
    onerror: (message: string | null) => void;
  } = $props();

  let selectedId = $state<string | null>(null);
  const selected = $derived(sources.find((s) => s.id === selectedId) ?? sources[0] ?? null);
  let renaming = $state(false);
  let renameValue = $state("");

  /** Duplicate groups each source keeps as they are (#111), for Reset. */
  let keptDuplicates = $state<Record<string, number>>({});
  $effect(() => {
    const ids = sources.map((s) => s.id);
    Promise.all(ids.map((id) => api.duplicatesKeptCount(id).catch(() => 0))).then((counts) => {
      keptDuplicates = Object.fromEntries(ids.map((id, i) => [id, counts[i]]));
    });
  });
  const keptSources = $derived(sources.filter((s) => (keptDuplicates[s.id] ?? 0) > 0));

  async function resetKept(id: string) {
    await run(() => api.duplicatesReset(id));
    keptDuplicates = { ...keptDuplicates, [id]: 0 };
  }

  async function run(action: () => Promise<unknown>) {
    try {
      onerror(null);
      await action();
      await onchanged();
    } catch (e) {
      onerror(String(e));
    }
  }

  async function add() {
    const path = await openDialog({ directory: true, multiple: false, title: m.main.addFolderTitle });
    if (typeof path !== "string") return;
    await run(async () => {
      const added = await api.addSource(path);
      selectedId = added.id;
    });
  }

  async function remove(s: Source) {
    const ok = await ask(m.main.removeAsk(s.name), { title: m.main.removeTitle, kind: "warning", okLabel: m.main.remove });
    if (!ok) return;
    await run(() => api.removeSource(s.id));
    selectedId = null;
  }

  async function toggleDownload(s: Source) {
    const on = !s.downloadOnlineOnly;
    if (on) {
      const ok = await ask(m.main.downloadAsk(s.name), { title: m.main.downloadTitle, kind: "info", okLabel: m.main.start });
      if (!ok) return;
    }
    await run(() => api.updateSource(s.id, { downloadOnlineOnly: on }));
  }

  function startRename(s: Source) {
    renaming = true;
    renameValue = s.name;
  }

  async function finishRename(s: Source) {
    const name = renameValue.trim();
    renaming = false;
    if (!name || name === s.name) return;
    await run(() => api.updateSource(s.id, { name }));
  }

  /** Arrow keys move through the list. */
  function listKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = sources.findIndex((s) => s.id === selected?.id);
    const next = sources[Math.min(sources.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))];
    if (next) selectedId = next.id;
  }
</script>

<p class="intro">{m.settings.libraryNote}</p>
<div class="sources" role="listbox" tabindex="0" aria-label={m.main.library} onkeydown={listKey}>
  {#each sources as s (s.id)}
    <div
      class="source"
      class:on={s.id === selected?.id}
      role="option"
      aria-selected={s.id === selected?.id}
      tabindex="-1"
      onclick={() => {
        selectedId = s.id;
        renaming = false;
      }}
      onkeydown={() => {}}
    >
      <span class="ficon" class:gone={!s.available}><Icon name={s.available ? "folder" : "warning"} size={18} /></span>
      <span class="names">
        <span class="name">{s.name}</span>
        <span class="root" title={s.root}>{s.root}</span>
      </span>
      <span class="prov">{providerLabel(s.provider)}</span>
    </div>
  {:else}
    <p class="note empty">{m.main.noSources}</p>
  {/each}
</div>
<div class="listbar">
  <button onclick={add} title={m.main.addFolderButton} aria-label={m.main.addFolderButton}>
    <Icon name="plus" size={14} />
  </button>
  <button onclick={() => selected && remove(selected)} disabled={!selected} title={m.main.removeFromRupert} aria-label={m.main.removeFromRupert}>
    <span class="minus">−</span>
  </button>
</div>

{#if selected}
  {@const s = selected}
  <div class="form detail">
    <span class="lab">
      {#if renaming}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="rename"
          bind:value={renameValue}
          autofocus
          onkeydown={(e) => {
            if (e.key === "Enter") finishRename(s);
            if (e.key === "Escape") {
              e.stopPropagation();
              renaming = false;
            }
          }}
          onblur={() => finishRename(s)}
        />
      {:else}
        <strong>{s.name}</strong>
      {/if}
    </span>
    <div>
      {#if !s.available}<p class="note warn">{m.settings.notAvailable}</p>{/if}
      <label class="check">
        <input
          type="checkbox"
          checked={s.showAllTypes}
          onchange={() => run(() => api.updateSource(s.id, { showAllTypes: !s.showAllTypes }))}
        />
        {m.main.showAllTypes}
      </label>
      {#if s.provider !== "local"}
        <label class="check" title={m.main.downloadTip}>
          <input type="checkbox" checked={s.downloadOnlineOnly} onchange={(e) => {
            e.currentTarget.checked = s.downloadOnlineOnly; // until confirmed
            toggleDownload(s);
          }} />
          {m.main.downloadToIndex}
        </label>
        <p class="note">{m.main.downloadTip}</p>
      {/if}
      <div class="line buttons">
        <button onclick={() => run(() => api.indexFolder(s.id, ""))} title={m.main.indexFolderTip} disabled={!s.available}>
          {m.main.indexFolder}
        </button>
        <button onclick={() => startRename(s)}>{m.main.rename}</button>
        <button onclick={() => run(() => api.revealEntry(s.id, ""))} disabled={!s.available}>{revealLabel(os)}</button>
      </div>

    </div>
  </div>
{/if}


<!-- MARK: Sending documents -->
<div class="sending">
  <span class="lab">{m.settings.sending}</span>
  <label class="check">
    <input type="checkbox" bind:checked={settings.sendNotes} />
    {m.settings.sendNotes}
  </label>
  <p class="note">{m.settings.sendNotesNote}</p>
</div>

<!-- MARK: Duplicates kept as they are (#111) -->
<div class="sending">
  <span class="lab">{m.health.keptTitle}</span>
  {#each keptSources as s (s.id)}
    <div class="kept">
      <span>{s.name}: {m.health.keptGroups(keptDuplicates[s.id])}</span>
      <button onclick={() => resetKept(s.id)} title={m.health.resetKeptTip}>{m.health.resetKept}</button>
    </div>
  {:else}
    <p class="note">{m.health.noneKept}</p>
  {/each}
  <p class="note">{m.health.keptExplain}</p>
</div>

<style>
  .sending .lab {
    font-weight: 600;
  }
  .kept {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .sending {
    margin-top: 18px;
    padding-top: 14px;
    border-top: 1px solid var(--border);
    display: grid;
    gap: 6px;
  }
  .sources {
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    overflow: hidden;
  }
  .sources:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .source {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 7px 10px;
    border-bottom: 1px solid var(--border);
    cursor: default;
  }
  .source:last-child {
    border-bottom: 0;
  }
  .source.on {
    background: var(--accent-soft);
  }
  .ficon {
    color: var(--folder);
    display: grid;
  }
  .ficon.gone {
    color: var(--danger);
  }
  .names {
    display: grid;
    min-width: 0;
  }
  .name {
    font-weight: 600;
  }
  .root {
    color: var(--muted);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .prov {
    color: var(--muted);
    font-size: 12px;
  }
  .empty {
    padding: 10px;
  }
  .listbar {
    display: flex;
    margin: 6px 0 0;
  }
  .listbar button {
    width: 28px;
    height: 22px;
    padding: 0;
    display: grid;
    place-items: center;
    border-radius: 0;
  }
  .listbar button:first-child {
    border-radius: 6px 0 0 6px;
  }
  .listbar button:last-child {
    border-radius: 0 6px 6px 0;
    border-left: 0;
  }
  .minus {
    font-size: 16px;
    line-height: 1;
  }
  .detail {
    margin-top: 16px;
  }
  .detail .check {
    margin-bottom: 6px;
  }
  .buttons {
    margin-top: 8px;
  }
  .rename {
    width: 100%;
  }
</style>
