<script lang="ts">
  // Library health (#113): what could be better in the library, row by
  // row, each with its fix. "Show" lists a row's documents here; selecting
  // one shows its details on the right, as in a folder.
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, onHealthProgress, type DocPlace, type HealthCounts, type HealthRow, type Hit, type Source } from "#lib/api.ts";
  import { formatDay } from "#lib/format.ts";
  import { m, num } from "#lib/i18n/index.ts";
  import DuplicatesDialog from "#lib/DuplicatesDialog.svelte";
  import SuggestDialog from "#lib/SuggestDialog.svelte";

  let {
    sources,
    os,
    tick,
    selectedKey,
    onselect,
    onopen,
    onrename,
    onindex,
    oncounts,
  }: {
    sources: Source[];
    os: string | undefined;
    /** Changes when the index does: counts are read again. */
    tick: number;
    /** The selected document ("source\\npath"), to mark it in a list. */
    selectedKey: string | null;
    onselect: (h: Hit) => void;
    onopen: (h: Hit) => void;
    /** Rename documents… for these documents of one folder. */
    onrename: (sourceId: string, relDir: string, only: string[]) => void;
    /** Downloads a source's online-only documents to index them. */
    onindex: (sourceId: string) => void;
    /** How many rows have something to fix (the sidebar's badge). */
    oncounts?: (rows: number) => void;
  } = $props();

  type RowId = HealthRow | "duplicates";
  let sourceId = $state<string | null>(null);
  let counts = $state<HealthCounts | null>(null);
  /** Duplicate groups found by the last search (null: not looked yet). */
  let dupGroups = $state<number | null>(null);
  let dupDocs = $state(0);
  let showing = $state<{ row: HealthRow; hits: Hit[] } | null>(null);
  let working = $state<{ label: string; done: number; total: number; name: string } | null>(null);
  let note = $state<string | null>(null);
  let duplicates = $state(false);
  let suggest = $state<{ docs: DocPlace[]; title: string } | null>(null);

  async function load() {
    try {
      counts = await api.healthCounts(sourceId);
      const rows = Object.values(counts).filter((n) => n > 0).length + (dupGroups ? 1 : 0);
      oncounts?.(rows);
    } catch (e) {
      note = String(e);
    }
  }

  $effect(() => {
    void tick;
    void sourceId;
    load();
  });

  onMount(() => {
    const off = onHealthProgress((p) => {
      if (working) working = { ...working, done: p.done, total: p.total, name: p.name };
    });
    return () => {
      off.then((f) => f());
    };
  });

  const rows = $derived<{ id: RowId; icon: string; title: string; detail: string; count: string; action: string }[]>([
    {
      id: "duplicates",
      icon: "⧉",
      title: m.health.duplicates,
      detail: m.health.duplicatesDetail,
      count: dupGroups === null ? "" : m.health.inGroups(dupDocs, dupGroups),
      action: m.common.ellipsis(dupGroups === null ? m.health.lookForDuplicates : m.health.reviewDuplicates),
    },
    { id: "noText", icon: "T", title: m.health.noText, detail: m.health.noTextDetail, count: n(counts?.noText), action: m.common.ellipsis(m.health.recognize) },
    { id: "onlineUnknown", icon: "☁", title: m.health.onlineUnknown, detail: m.health.onlineUnknownDetail, count: n(counts?.onlineUnknown), action: m.common.ellipsis(m.health.download) },
    { id: "noDate", icon: "◷", title: m.health.noDate, detail: m.health.noDateDetail, count: n(counts?.noDate), action: m.common.ellipsis(m.health.suggest) },
    { id: "noTags", icon: "#", title: m.health.noTags, detail: m.health.noTagsDetail, count: n(counts?.noTags), action: m.common.ellipsis(m.health.suggest) },
    { id: "scannerNames", icon: "✎", title: m.health.scannerNames, detail: m.health.scannerNamesDetail, count: n(counts?.scannerNames), action: m.common.ellipsis(m.renameDocs.menu.replace(/…$/, "")) },
    { id: "unreadable", icon: "!", title: m.health.unreadable, detail: m.health.unreadableDetail, count: n(counts?.unreadable), action: m.health.showList },
  ]);

  function n(x: number | undefined): string {
    return x === undefined ? "…" : num(x);
  }
  const zero = (id: RowId) => (id === "duplicates" ? dupGroups === 0 : counts !== null && counts[id] === 0);

  async function show(row: HealthRow) {
    note = null;
    showing = { row, hits: await api.healthDocs(row, sourceId) };
  }

  const places = (hits: Hit[]): DocPlace[] => hits.map((h) => ({ sourceId: h.sourceId, relPath: h.relPath }));

  async function fix(id: RowId) {
    note = null;
    if (id === "duplicates") {
      duplicates = true;
    } else if (id === "noText") {
      const hits = await api.healthDocs("noText", sourceId);
      if (!hits.length) return;
      const ok = await ask(m.health.recognizeAsk(hits.length), { title: m.health.noText, kind: "info", okLabel: m.health.recognize });
      if (!ok) return;
      working = { label: m.health.recognizing, done: 0, total: hits.length, name: "" };
      try {
        const r = await api.healthRecognize(places(hits));
        note = m.health.recognized(r.done, r.nothing, r.failed.length);
      } catch (e) {
        note = String(e);
      } finally {
        working = null;
        load();
      }
    } else if (id === "onlineUnknown") {
      const hits = await api.healthDocs("onlineUnknown", sourceId);
      if (!hits.length) return;
      const ok = await ask(m.health.downloadAsk(hits.length), { title: m.health.onlineUnknown, kind: "info", okLabel: m.health.download });
      if (!ok) return;
      for (const s of new Set(hits.map((h) => h.sourceId))) onindex(s);
      note = m.health.downloading;
    } else if (id === "noDate" || id === "noTags") {
      const hits = await api.healthDocs(id, sourceId);
      if (hits.length) suggest = { docs: places(hits), title: id === "noDate" ? m.health.noDate : m.health.noTags };
    } else {
      show(id);
    }
  }

  /** Scanner-style names, by folder: Rename documents… works on one. */
  const byFolder = $derived.by(() => {
    if (!showing) return [];
    const groups = new Map<string, Hit[]>();
    for (const h of showing.hits) {
      const dir = h.relPath.includes("/") ? h.relPath.slice(0, h.relPath.lastIndexOf("/")) : "";
      const k = h.sourceId + "\n" + dir;
      groups.set(k, [...(groups.get(k) ?? []), h]);
    }
    return [...groups.entries()].map(([k, hits]) => {
      const [sid, dir] = k.split("\n");
      return { sourceId: sid, dir, hits };
    });
  });

  const sourceName = (id: string) => sources.find((s) => s.id === id)?.name ?? "";
  const where = (sid: string, dir: string) => [sourceName(sid), ...(dir ? dir.split("/") : [])].join(" › ");
  const rowTitle = (r: HealthRow) => rows.find((x) => x.id === r)?.title ?? "";
</script>

<div class="health">
  {#if !showing}
    <div class="head">
      <h1>{m.health.title}</h1>
      <span class="muted">{m.health.intro}</span>
      <span class="grow"></span>
      {#if sources.length > 1}
        <label class="for">
          {m.health.showFor}
          <select value={sourceId ?? ""} onchange={(e) => (sourceId = e.currentTarget.value || null)}>
            <option value="">{m.health.allSources}</option>
            {#each sources as s (s.id)}<option value={s.id}>{s.name}</option>{/each}
          </select>
        </label>
      {/if}
    </div>
    {#if working}
      <div class="working">
        <progress max={working.total} value={working.done}></progress>
        <span class="small muted">{working.label} {working.name}</span>
        <button onclick={() => api.healthStop()}>{m.exportDocs.stop}</button>
      </div>
    {/if}
    {#if note}<p class="small note">{note}</p>{/if}
    <div class="rows">
      {#each rows as r (r.id)}
        <div class="row" class:done={zero(r.id)}>
          <span class="icon" aria-hidden="true">{r.icon}</span>
          <div class="text">
            <span class="title">{r.title}</span>
            <span class="detail">{r.detail}</span>
          </div>
          <span class="count">{zero(r.id) ? "✓" : r.count}</span>
          {#if r.id !== "duplicates" && !zero(r.id)}
            <button class="link" onclick={() => show(r.id as HealthRow)}>{m.health.show}</button>
          {:else}
            <span class="link-space"></span>
          {/if}
          <button class="fix" disabled={!!working || (zero(r.id) && r.id !== "duplicates")} onclick={() => fix(r.id)}>{r.action}</button>
        </div>
      {/each}
    </div>
  {:else}
    <div class="head">
      <button class="link back" onclick={() => (showing = null)}>‹ {m.health.title}</button>
      <h1>{rowTitle(showing.row)}</h1>
      <span class="muted">{num(showing.hits.length)}{showing.hits.length >= 2000 ? "+" : ""}</span>
    </div>
    <div class="list">
      {#each byFolder as g (g.sourceId + "\n" + g.dir)}
        <div class="folder">
          <span>{where(g.sourceId, g.dir)}</span>
          {#if showing.row === "scannerNames"}
            <button class="link" onclick={() => onrename(g.sourceId, g.dir, g.hits.map((h) => h.relPath))}
              >{m.common.ellipsis(m.health.renameHere)}</button
            >
          {/if}
        </div>
        {#each g.hits as h (h.sourceId + "\n" + h.relPath)}
          <button
            class="doc"
            class:selected={selectedKey === h.sourceId + "\n" + h.relPath}
            onclick={() => onselect(h)}
            ondblclick={() => onopen(h)}
          >
            <span class="name">{h.title ?? h.name}</span>
            {#if h.title}<span class="muted small">{h.name}</span>{/if}
            <span class="grow"></span>
            <span class="muted small">{h.documentDate ?? formatDay(h.modifiedMs)}</span>
          </button>
        {/each}
      {/each}
    </div>
  {/if}
</div>

{#if duplicates}
  <DuplicatesDialog
    {sourceId}
    {sources}
    {os}
    onfound={(groups, docs) => ((dupGroups = groups), (dupDocs = docs))}
    onclose={() => ((duplicates = false), load())}
  />
{/if}

{#if suggest}
  <SuggestDialog docs={suggest.docs} title={suggest.title} onclose={() => ((suggest = null), load())} />
{/if}

<style>
  .health {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: 17px;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .grow {
    flex: 1;
  }
  .for {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .for select {
    font: inherit;
    padding: 2px 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
  }
  .working {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .working progress {
    flex: 0 0 200px;
  }
  .note {
    margin: 0;
  }
  .rows {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
    overflow: hidden;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--border);
  }
  .row:last-child {
    border-bottom: 0;
  }
  .row.done .title,
  .row.done .count {
    color: var(--muted);
  }
  .icon {
    width: 28px;
    height: 28px;
    flex: none;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent-strong);
    display: grid;
    place-items: center;
    font-size: 14px;
  }
  .text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-weight: 600;
  }
  .detail {
    font-size: 12px;
    color: var(--muted);
  }
  .count {
    min-width: 80px;
    text-align: right;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-strong);
    text-decoration: underline;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .link-space {
    width: 34px;
  }
  .fix {
    min-width: 160px;
    font: inherit;
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
  }
  .fix:disabled {
    opacity: 0.45;
  }
  .back {
    font-size: 13px;
  }
  .list {
    display: flex;
    flex-direction: column;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 8px 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .doc {
    display: flex;
    align-items: baseline;
    gap: 8px;
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 8px;
    border-radius: 6px;
    font: inherit;
    color: var(--text);
  }
  .doc:hover {
    background: var(--hover);
  }
  .doc.selected {
    background: var(--accent-soft);
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
