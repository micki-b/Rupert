<script lang="ts" module>
  /** Something quick open can go to, besides folders, documents and tags. */
  export type QuickCommand = { id: string; label: string; keys?: string };
</script>

<script lang="ts">
  // Quick open (#105, ⌘K / Ctrl+K): a field over the window that goes
  // straight to a folder, a document (by title or file name), a tag or a
  // command. Before anything is typed it offers the recent documents and
  // the favorite folders. Folders are found the way filing finds them (#63).
  import { onMount, tick } from "svelte";
  import Icon from "#lib/Icon.svelte";
  import { api, type FolderChoice, type Hit, type Source } from "#lib/api.ts";
  import { crumbs, searchFolders } from "#lib/folders.ts";
  import { recentDocuments } from "#lib/recent.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    sources,
    commands,
    mac,
    onfolder,
    ondocument,
    ontag,
    oncommand,
    onclose,
  }: {
    sources: Source[];
    commands: QuickCommand[];
    /** ⌘ (Mac) or Ctrl in the hint. */
    mac: boolean;
    onfolder: (sourceId: string, relPath: string) => void;
    /** `open`: open it in its app (⌘/Ctrl+Return) instead of showing it. */
    ondocument: (hit: Hit, open: boolean) => void;
    ontag: (tag: string) => void;
    oncommand: (id: string) => void;
    onclose: () => void;
  } = $props();

  type Row =
    | { kind: "folder"; sourceId: string; relPath: string; name: string; where: string; favorite?: boolean }
    | { kind: "doc"; hit: Hit; name: string; where: string }
    | { kind: "tag"; tag: string; count: number }
    | { kind: "command"; id: string; label: string; keys?: string };
  type Section = { title: string; rows: Row[] };

  let text = $state("");
  let input: HTMLInputElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();
  let active = $state(0);
  let sections = $state<Section[]>([]);
  let allFolders: FolderChoice[] = [];
  let ready = $state(false);

  const rows = $derived(sections.flatMap((s) => s.rows));

  /** "Boat › Insurance": where something is, the source first when there are several. */
  function whereOf(sourceId: string, dir: string): string {
    const source = sources.find((s) => s.id === sourceId)?.name ?? "";
    return crumbs({ sourceId, dir, label: dir ? `${source}/${dir}` : source }, sources).join(" › ");
  }

  function folderRow(sourceId: string, relPath: string, favorite = false): Row {
    const cut = relPath.lastIndexOf("/");
    const name = relPath ? relPath.slice(cut + 1) : (sources.find((s) => s.id === sourceId)?.name ?? "");
    return { kind: "folder", sourceId, relPath, name, where: relPath ? whereOf(sourceId, cut < 0 ? "" : relPath.slice(0, cut)) : "", favorite };
  }

  function docRow(hit: Hit): Row {
    const cut = hit.relPath.lastIndexOf("/");
    const where = whereOf(hit.sourceId, cut < 0 ? "" : hit.relPath.slice(0, cut));
    return { kind: "doc", hit, name: hit.title || hit.name, where: hit.title ? `${where ? where + " › " : ""}${hit.name}` : where };
  }

  /** Folds case and accents, as the folder search does. */
  const fold = (s: string) => s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();

  async function startPage() {
    const [recent, favorites] = await Promise.all([
      api.documentsInfo(recentDocuments()).catch(() => [] as Hit[]),
      api.favoriteFolders().catch(() => []),
    ]);
    if (text.trim()) return;
    sections = [
      { title: m.quickOpen.recent, rows: recent.map(docRow) },
      { title: m.quickOpen.favoriteFolders, rows: favorites.map((f) => folderRow(f.sourceId, f.relPath, true)) },
      { title: m.quickOpen.commands, rows: commands.map((c) => ({ kind: "command" as const, ...c })) },
    ].filter((s) => s.rows.length);
    active = 0;
    ready = true;
  }

  let asked = 0;
  async function find(q: string) {
    const mine = ++asked;
    const words = fold(q).split(/\s+/).filter(Boolean);
    const [docs, tagList] = await Promise.all([
      api.quickFind(q).catch(() => [] as Hit[]),
      api.tagSuggestions(q, [], "any", 4).catch(() => null),
    ]);
    if (mine !== asked) return;
    const folders = searchFolders(q, allFolders, 6).map((f) => folderRow(f.sourceId, f.dir));
    sections = [
      { title: m.quickOpen.folders, rows: folders },
      { title: m.quickOpen.documents, rows: docs.slice(0, 8).map(docRow) },
      { title: m.quickOpen.tags, rows: (tagList?.tags ?? []).map((t) => ({ kind: "tag" as const, tag: t.tag, count: t.count })) },
      {
        title: m.quickOpen.commands,
        rows: commands
          .filter((c) => words.every((w) => fold(c.label).includes(w)))
          .map((c) => ({ kind: "command" as const, ...c })),
      },
    ].filter((s) => s.rows.length);
    active = 0;
    ready = true;
  }

  onMount(() => {
    input?.focus();
    startPage();
    api.allFolders().then((f) => {
      allFolders = f;
      if (text.trim()) find(text);
    }, () => {});
  });

  let timer: ReturnType<typeof setTimeout> | undefined;
  function typed() {
    clearTimeout(timer);
    const q = text.trim();
    if (!q) {
      asked++;
      startPage();
      return;
    }
    timer = setTimeout(() => find(q), 80);
  }

  function choose(row: Row | undefined, open = false) {
    if (!row) return;
    onclose();
    if (row.kind === "folder") onfolder(row.sourceId, row.relPath);
    else if (row.kind === "doc") ondocument(row.hit, open);
    else if (row.kind === "tag") ontag(row.tag);
    else oncommand(row.id);
  }

  function move(step: number) {
    if (!rows.length) return;
    active = (active + step + rows.length) % rows.length;
    tick().then(() => list?.querySelector(".row.active")?.scrollIntoView({ block: "nearest" }));
  }

  function key(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      move(e.key === "ArrowDown" ? 1 : -1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(rows[active], mac ? e.metaKey : e.ctrlKey);
    }
  }
</script>

<div class="backdrop" role="presentation" onclick={onclose}>
  <div
    class="quick"
    role="dialog"
    aria-modal="true"
    aria-label={m.quickOpen.title}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <div class="field">
      <Icon name="search" size={16} />
      <input
        bind:this={input}
        bind:value={text}
        oninput={typed}
        onkeydown={key}
        placeholder={m.quickOpen.placeholder}
        aria-label={m.quickOpen.title}
        role="combobox"
        aria-expanded="true"
        aria-controls="quick-list"
        aria-activedescendant={rows.length ? `quick-${active}` : undefined}
        spellcheck="false"
        autocomplete="off"
      />
    </div>
    <div class="list" id="quick-list" role="listbox" bind:this={list}>
      {#each sections as s (s.title)}
        <div class="group" role="presentation">{s.title}</div>
        {#each s.rows as row (row)}
          {@const i = rows.indexOf(row)}
          <div
            id="quick-{i}"
            class="row"
            class:active={i === active}
            role="option"
            aria-selected={i === active}
            tabindex="-1"
            onpointermove={() => (active = i)}
            onclick={(e) => choose(row, mac ? e.metaKey : e.ctrlKey)}
            onkeydown={() => {}}
          >
            {#if row.kind === "folder"}
              <span class="icon folder"><Icon name="folder" size={16} /></span>
              <span class="name">{row.name}</span>
              {#if row.where}<span class="where">{row.where}</span>{/if}
            {:else if row.kind === "doc"}
              <span class="icon"><Icon name={row.hit.kind === "pdf" ? "pdf" : "file"} size={16} /></span>
              <span class="name">{row.name}</span>
              {#if row.hit.cloud === "cloudOnly"}<span class="cloud" title={m.quickOpen.onlineOnly}><Icon name="cloud" size={13} /></span>{/if}
              {#if row.where}<span class="where">{row.where}</span>{/if}
            {:else if row.kind === "tag"}
              <span class="icon"><Icon name="tag" size={16} /></span>
              <span class="name"># {row.tag}</span>
              <span class="where">{m.quickOpen.documentCount(row.count)}</span>
            {:else}
              <span class="icon"><Icon name="chevron" size={16} /></span>
              <span class="name">{row.label}</span>
              {#if row.keys}<span class="where keys">{row.keys}</span>{/if}
            {/if}
          </div>
        {/each}
      {/each}
      {#if ready && !rows.length}
        <p class="none">{text.trim() ? m.quickOpen.nothing(text.trim()) : m.quickOpen.typeToFind}</p>
      {/if}
    </div>
    <footer class="hint">{m.quickOpen.hint(mac ? "⌘↩" : "Ctrl+Enter")}</footer>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    background: rgb(0 0 0 / 0.18);
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
  }
  .quick {
    width: min(560px, calc(100vw - 32px));
    max-height: 70vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 22px 60px rgb(0 0 0 / 0.3);
    overflow: hidden;
  }
  .field {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
  }
  .field input {
    flex: 1;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: 16px;
  }
  .list {
    overflow: auto;
    padding: 4px 6px 6px;
  }
  .group {
    padding: 8px 8px 3px;
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px;
    border-radius: 6px;
    cursor: default;
    min-width: 0;
  }
  .row.active {
    background: var(--accent);
    color: #fff;
  }
  .icon {
    display: grid;
    place-items: center;
    color: var(--muted);
    flex: none;
  }
  .icon.folder {
    color: var(--folder);
  }
  .row.active .icon,
  .row.active .where,
  .row.active .cloud {
    color: rgb(255 255 255 / 0.85);
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 0 1 auto;
    min-width: 0;
  }
  .cloud {
    color: var(--muted);
    display: grid;
    place-items: center;
    flex: none;
  }
  .where {
    margin-left: auto;
    padding-left: 12px;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 0 2 auto;
    min-width: 0;
  }
  .none {
    margin: 0;
    padding: 14px 10px;
    color: var(--muted);
    font-size: 13px;
  }
  .hint {
    padding: 6px 14px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--muted);
    background: var(--panel);
  }
</style>
