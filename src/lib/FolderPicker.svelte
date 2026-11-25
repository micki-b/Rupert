<script lang="ts">
  // File it's Folder field (#63): at rest the chosen folder as a breadcrumb
  // that wraps (the last folder in bold); clicked or typed into, Rupert's
  // own folder search, suggested folders first; the folder button and
  // "Browse all folders…" open the folder tree.
  import { tick } from "svelte";
  import Icon from "#lib/Icon.svelte";
  import type { FolderChoice, Source } from "#lib/api.ts";
  import { crumbs, searchFolders } from "#lib/folders.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    value = $bindable(),
    suggested,
    all,
    sources,
    onbrowse,
  }: {
    /** The chosen folder's label ("Source/dir"), or "". */
    value: string;
    suggested: FolderChoice[];
    all: FolderChoice[];
    sources: Source[];
    onbrowse: () => void;
  } = $props();

  let editing = $state(false);
  let query = $state("");
  let active = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();

  const choice = $derived([...suggested, ...all].find((f) => f.label === value) ?? null);
  const parts = $derived(choice ? crumbs(choice, sources) : []);

  /** What the list shows: suggested folders that match, then the others. */
  const results = $derived.by(() => {
    const q = query.trim();
    if (!q) return { suggested, others: [] as FolderChoice[] };
    const hits = searchFolders(q, [...suggested, ...all.filter((f) => !suggested.some((s) => s.label === f.label))]);
    const isSuggested = (f: FolderChoice) => suggested.some((s) => s.label === f.label);
    return { suggested: hits.filter(isSuggested), others: hits.filter((f) => !isSuggested(f)) };
  });
  const flat = $derived([...results.suggested, ...results.others]);

  async function open() {
    editing = true;
    query = "";
    active = 0;
    await tick();
    input?.focus();
  }

  function pick(f: FolderChoice) {
    value = f.label;
    editing = false;
  }

  function browse() {
    editing = false;
    onbrowse();
  }

  function key(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      // The last place is "Browse all folders…".
      const n = flat.length + 1;
      active = (active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      tick().then(() => list?.querySelector(".active")?.scrollIntoView({ block: "nearest" }));
    } else if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation(); // not ⌘↩ (File) by accident
      if (active === flat.length) browse();
      else if (flat[active]) pick(flat[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      editing = false;
    }
  }

  /** Where a folder is: its parents, without the source if there's only one. */
  function whereIs(f: FolderChoice) {
    return crumbs(f, sources).slice(0, -1).join(" › ");
  }
  function nameOf(f: FolderChoice) {
    return crumbs(f, sources).at(-1) ?? f.label;
  }
</script>

<div class="picker">
  <div class="line">
    {#if editing}
      <input
        bind:this={input}
        bind:value={query}
        class="search"
        placeholder={m.inbox.searchFolders}
        aria-label={m.inbox.folder}
        aria-expanded="true"
        aria-controls="folder-results"
        role="combobox"
        autocomplete="off"
        spellcheck="false"
        oninput={() => (active = 0)}
        onkeydown={key}
        onblur={() => setTimeout(() => (editing = false), 120)}
      />
    {:else}
      <button class="crumbs" class:empty={!value} class:invalid={!!value && !choice} onclick={open} title={value ? `${value}\n${m.inbox.changeFolder}` : undefined}>
        {#if choice}
          {#each parts as p, i (i)}
            {#if i > 0}<span class="sep">›</span>{/if}<span class:last={i === parts.length - 1}>{p}</span>
          {/each}
        {:else if value}
          {value}
        {:else}
          <span class="muted">{m.inbox.searchFolders}</span>
        {/if}
      </button>
    {/if}
    <button class="browse-btn" onclick={browse} title={m.inbox.browseTip} aria-label={m.inbox.browseTip}>
      <Icon name="folder" size={16} />
    </button>
  </div>

  {#if editing}
    <div class="results" id="folder-results" role="listbox" bind:this={list}>
      {#if results.suggested.length}
        <div class="group">{m.inbox.suggestedFolders}</div>
        {#each results.suggested as f, i (f.label)}
          {@render option(f, i)}
        {/each}
      {/if}
      {#if query.trim()}
        {#if results.others.length}
          <div class="group">{m.inbox.matching(query.trim())}</div>
          {#each results.others as f, i (f.label)}
            {@render option(f, results.suggested.length + i)}
          {/each}
        {:else if !results.suggested.length}
          <p class="none">{m.inbox.noFolders(query.trim())}</p>
        {/if}
      {/if}
      <button
        class="browse-all"
        class:active={active === flat.length}
        onmousedown={(e) => e.preventDefault()}
        onclick={browse}
      >
        <Icon name="folder" size={14} />
        {m.inbox.browseAll}
      </button>
    </div>
  {/if}
</div>

{#snippet option(f: FolderChoice, i: number)}
  <button
    class="option"
    class:active={i === active}
    role="option"
    aria-selected={i === active}
    title={f.label}
    onmousedown={(e) => e.preventDefault()}
    onmouseenter={() => (active = i)}
    onclick={() => pick(f)}
  >
    <span class="ficon"><Icon name="folder" size={15} /></span>
    <span class="text">
      <span class="name">{nameOf(f)}</span>
      {#if whereIs(f)}<span class="where">{whereIs(f)}</span>{/if}
    </span>
  </button>
{/snippet}

<style>
  .picker {
    position: relative;
  }
  .line {
    display: flex;
    gap: 6px;
    align-items: stretch;
  }
  .crumbs,
  .search {
    flex: 1;
    min-width: 0;
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 9px;
    text-align: left;
    line-height: 1.45;
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 5px;
    cursor: text;
  }
  .crumbs:hover {
    border-color: var(--muted);
  }
  .crumbs.invalid {
    border-color: var(--danger);
  }
  .search {
    outline: none;
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
  .sep,
  .muted {
    color: var(--muted);
  }
  .last {
    font-weight: 650;
  }
  .browse-btn {
    flex: none;
    width: 34px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--folder);
    display: grid;
    place-items: center;
    padding: 0;
  }
  .browse-btn:hover {
    border-color: var(--muted);
  }
  .results {
    position: absolute;
    left: 0;
    right: 0;
    top: calc(100% + 4px);
    z-index: 5;
    max-height: 320px;
    overflow: auto;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 12px 30px rgb(0 0 0 / 0.18);
    padding: 4px;
  }
  .group {
    font-size: 10.5px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    padding: 6px 8px 2px;
  }
  .option,
  .browse-all {
    display: flex;
    gap: 7px;
    width: 100%;
    text-align: left;
    font: inherit;
    color: var(--text);
    background: none;
    border: 0;
    border-radius: 6px;
    padding: 5px 8px;
  }
  .option.active,
  .browse-all.active {
    background: var(--accent);
    color: #fff;
  }
  .ficon {
    color: var(--folder);
    padding-top: 1px;
  }
  @media (prefers-color-scheme: dark) {
    .option.active,
    .browse-all.active {
      color: #1e1c1a;
    }
  }
  .option.active .ficon,
  .option.active .where {
    color: inherit;
    opacity: 0.9;
  }
  .text {
    display: grid;
    min-width: 0;
  }
  .name {
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .where {
    color: var(--muted);
    font-size: 11.5px;
    overflow-wrap: anywhere;
  }
  .browse-all {
    border-top: 1px solid var(--border);
    border-radius: 0 0 6px 6px;
    margin-top: 4px;
    padding-top: 7px;
    color: var(--accent);
    align-items: center;
  }
  .none {
    color: var(--muted);
    font-size: 12px;
    padding: 6px 8px;
    margin: 0;
  }
</style>
