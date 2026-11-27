<script lang="ts">
  // The tag filter: type to narrow a short list of tags (most used first),
  // pick several, and choose whether documents need any or all of them.
  // Used in the desktop sidebar and the phone's tag sheet.
  import { shortCount } from "#lib/format.ts";
  import { api, type TagList, type TagMode } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    selected = $bindable([]),
    mode = $bindable("any"),
    refresh = 0,
    limit = 12,
    variant = "sidebar",
    onchange,
  }: {
    selected?: string[];
    mode?: TagMode;
    /** Bump to reload (e.g. after the index changed). */
    refresh?: number;
    limit?: number;
    variant?: "sidebar" | "sheet";
    onchange?: () => void;
  } = $props();

  let filter = $state("");
  let showAll = $state(false);
  let list = $state<TagList>({ tags: [], matching: 0 });
  let cursor = $state(-1);
  let input: HTMLInputElement | undefined = $state();

  let seq = 0;
  $effect(() => {
    const args = { filter, selected: [...selected], mode, limit: showAll ? 500 : limit };
    refresh; // reload when asked
    const mine = ++seq;
    const timer = setTimeout(async () => {
      const result = await api.tagSuggestions(args.filter, args.selected, args.mode, args.limit).catch(() => null);
      if (result && mine === seq) {
        list = result;
        cursor = Math.min(cursor, result.tags.length - 1);
      }
    }, 60);
    return () => clearTimeout(timer);
  });

  function toggle(tag: string) {
    const i = selected.findIndex((t) => t.toLowerCase() === tag.toLowerCase());
    selected = i >= 0 ? selected.filter((_, j) => j !== i) : [...selected, tag];
    onchange?.();
  }

  function pick(tag: string) {
    toggle(tag);
    filter = "";
    cursor = -1;
    input?.focus();
  }

  function setMode(next: TagMode) {
    if (mode === next) return;
    mode = next;
    onchange?.();
  }

  function clear() {
    selected = [];
    onchange?.();
  }

  function onkeydown(e: KeyboardEvent) {
    const n = list.tags.length;
    if (e.key === "ArrowDown" && n) {
      cursor = (cursor + 1) % n;
      e.preventDefault();
    } else if (e.key === "ArrowUp" && n) {
      cursor = (cursor - 1 + n) % n;
      e.preventDefault();
    } else if (e.key === "Enter") {
      const t = list.tags[cursor >= 0 ? cursor : 0];
      if (t) pick(t.tag);
      e.preventDefault();
    } else if (e.key === "Escape") {
      if (filter) filter = "";
      else if (selected.length) clear();
      cursor = -1;
      e.stopPropagation();
    }
  }
</script>

<div class="tag-filter {variant}">
  <input
    bind:this={input}
    bind:value={filter}
    oninput={() => (cursor = -1)}
    {onkeydown}
    type="search"
    placeholder={m.tagFilter.filterPlaceholder}
    autocapitalize="off"
    autocomplete="off"
    spellcheck="false"
  />

  {#if selected.length}
    <ul class="selected">
      {#each selected as t (t)}
        <li>
          <button class="row on" onclick={() => toggle(t)} title={m.tagFilter.removeFromFilter}>
            <span class="name"># {t}</span>
            <span class="x" aria-label={m.tagFilter.remove}>✕</span>
          </button>
        </li>
      {/each}
    </ul>
    {#if selected.length > 1}
      <div class="mode" role="radiogroup" aria-label={m.tagFilter.mustHave}>
        <span class="mode-label">{m.tagFilter.match}</span>
        <button role="radio" aria-checked={mode === "any"} class:on={mode === "any"} onclick={() => setMode("any")}>
          {m.tagFilter.any}
        </button>
        <button role="radio" aria-checked={mode === "all"} class:on={mode === "all"} onclick={() => setMode("all")}>
          {m.tagFilter.all}
        </button>
        <button class="clear" onclick={clear}>{m.tagFilter.clear}</button>
      </div>
    {:else}
      <div class="mode"><button class="clear" onclick={clear}>{m.tagFilter.clear}</button></div>
    {/if}
  {/if}

  <ul class="tags" role="listbox" aria-label={m.tagFilter.tags}>
    {#each list.tags as t, i (t.tag)}
      <li>
        <button class="row" class:cursor={i === cursor} role="option" aria-selected={false} onclick={() => pick(t.tag)}>
          <span class="name"># {t.tag}</span>
          <span class="count">{shortCount(t.count)}</span>
        </button>
      </li>
    {/each}
  </ul>
  {#if list.matching === 0}
    <p class="empty">
      {filter
        ? m.tagFilter.noneMatch
        : selected.length && mode === "all"
          ? m.tagFilter.noOthersHere
          : selected.length
            ? m.tagFilter.noOthers
            : m.tagFilter.noneYet}
    </p>
  {:else if list.matching > list.tags.length}
    <button class="more" onclick={() => (showAll = true)}>
      {selected.length && mode === "all" ? m.tagFilter.showAllThatGo(list.matching) : m.tagFilter.showAll(list.matching)}
    </button>
  {:else if showAll && list.matching > limit}
    <button class="more" onclick={() => (showAll = false)}>{m.tagFilter.showFewer}</button>
  {/if}
</div>

<style>
  .tag-filter {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  input {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    margin: 2px 0 4px;
    -webkit-user-select: text;
    user-select: text;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    border: 0;
    background: none;
    color: var(--text);
    text-align: left;
    border-radius: 6px;
    padding: 0 8px;
    height: 26px;
  }
  .row:hover,
  .row.cursor {
    background: var(--hover);
  }
  .row.on {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count,
  .x {
    color: var(--muted);
    font-size: 12px;
  }
  .row.on .x {
    color: inherit;
  }
  .mode {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px 6px;
    font-size: 12px;
  }
  .mode-label {
    color: var(--muted);
    margin-right: 2px;
  }
  .mode button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 5px;
    padding: 1px 8px;
    font-size: 12px;
  }
  .mode button.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .mode .clear {
    margin-left: auto;
    border: 0;
    background: none;
    color: var(--accent);
    padding: 1px 2px;
  }
  .empty {
    color: var(--muted);
    font-size: 12px;
    margin: 2px 8px;
  }
  .more {
    border: 0;
    background: none;
    color: var(--accent);
    text-align: left;
    padding: 2px 8px;
    font-size: 12px;
  }

  /* Phone sheet: bigger touch targets. */
  .sheet {
    gap: 6px;
  }
  .sheet input {
    font-size: 16px;
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--panel);
    border-color: transparent;
  }
  .sheet .row {
    height: 44px;
    font-size: 16px;
    padding: 0 12px;
    border-radius: 8px;
  }
  .sheet .count,
  .sheet .x {
    font-size: 14px;
  }
  .sheet .mode {
    font-size: 14px;
    padding: 4px 12px 8px;
  }
  .sheet .mode button {
    font-size: 14px;
    padding: 4px 14px;
  }
  .sheet .empty,
  .sheet .more {
    font-size: 14px;
    padding: 6px 12px;
  }
</style>
