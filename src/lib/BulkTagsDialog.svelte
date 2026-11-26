<script lang="ts">
  // Add tags… / Remove tags… for several selected documents (#102). The
  // change goes where each document's details live, as in the tag manager
  // (inside PDFs, in sidecars, noted beside online-only PDFs), and the same
  // preview says how many of each before anything is written.
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import { api, type ListedEntry, type TagEdit, type TagEditPreview, type TagEditReport } from "#lib/api.ts";
  import { describeTagEdit } from "#lib/tag-edit.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    mode,
    docs,
    onclose,
    ondone,
  }: {
    mode: "add" | "remove";
    docs: { sourceId: string; entry: ListedEntry }[];
    onclose: () => void;
    ondone: (report: TagEditReport) => void;
  } = $props();

  /** Add: the tags typed so far. Remove: the tags ticked. */
  let chosen = $state<string[]>([]);
  let typing = $state("");
  let known = $state<string[]>([]);
  let preview = $state<TagEditPreview | null>(null);
  let working = $state(false);
  let error = $state<string | null>(null);

  /** Remove: the tags the documents have, with how many have each. */
  const present = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const d of docs) for (const t of d.entry.tags ?? []) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  });

  const edit = $derived<TagEdit | null>(
    chosen.length
      ? {
          from: mode === "remove" ? [...chosen] : [],
          action: mode === "add" ? { kind: "add", tags: [...chosen] } : { kind: "delete" },
          only: docs.map((d) => ({ sourceId: d.sourceId, relPath: d.entry.relPath })),
        }
      : null,
  );

  onMount(() => {
    if (mode === "add") api.listTags().then((l) => (known = l.map((t) => t.tag)), () => {});
  });

  // The preview follows the choice.
  $effect(() => {
    const e = edit;
    preview = null;
    if (!e) return;
    let live = true;
    const t = setTimeout(() => {
      api
        .tagEditPreview(e)
        .then((p) => live && (preview = p))
        .catch((err) => live && (error = String(err)));
    }, 150);
    return () => {
      live = false;
      clearTimeout(t);
    };
  });

  function addTyped() {
    const tags = typing
      .split(",")
      .map((t) => t.trim())
      .filter((t) => t && !chosen.some((c) => c.toLowerCase() === t.toLowerCase()));
    chosen = [...chosen, ...tags];
    typing = "";
  }

  function toggle(tag: string) {
    chosen = chosen.includes(tag) ? chosen.filter((t) => t !== tag) : [...chosen, tag];
  }

  async function apply() {
    if (mode === "add" && typing.trim()) addTyped();
    const e = edit;
    if (!e || working) return;
    if (preview?.download) {
      const ok = await ask(m.tags.downloadAsk(preview.download), {
        title: mode === "add" ? m.bulkTags.add : m.bulkTags.remove,
        kind: "warning",
        okLabel: mode === "add" ? m.bulkTags.add : m.bulkTags.remove,
      });
      if (!ok) return;
    }
    working = true;
    error = null;
    try {
      ondone(await api.tagEditApply($state.snapshot(e) as TagEdit));
      onclose();
    } catch (err) {
      error = String(err);
    } finally {
      working = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !working && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="bulk-tags-title">
    <h1 id="bulk-tags-title">{mode === "add" ? m.bulkTags.addTitle(docs.length) : m.bulkTags.removeTitle(docs.length)}</h1>

    {#if mode === "add"}
      <div class="chips">
        {#each chosen as t (t)}
          <button class="chip" onclick={() => toggle(t)} title={m.bulkTags.takeOut}>{t} ×</button>
        {/each}
        <input
          list="bulk-known-tags"
          placeholder={m.bulkTags.typeTag}
          bind:value={typing}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault();
              if (typing.trim()) addTyped();
              else apply();
            }
          }}
        />
        <datalist id="bulk-known-tags">
          {#each known as t (t)}<option value={t}></option>{/each}
        </datalist>
      </div>
    {:else if present.length}
      <ul class="tag-list">
        {#each present as [tag, count] (tag)}
          <li>
            <label>
              <input type="checkbox" checked={chosen.includes(tag)} onchange={() => toggle(tag)} disabled={working} />
              <span class="name"># {tag}</span>
              <span class="count">{count === docs.length ? m.bulkTags.all : m.bulkTags.some(count, docs.length)}</span>
            </label>
          </li>
        {/each}
      </ul>
    {:else}
      <p class="muted small">{m.bulkTags.noTags}</p>
    {/if}

    {#if preview}
      <p class="small">{m.tags.changes(describeTagEdit(preview))}</p>
      {#if preview.download}<p class="small warn">{m.tags.downloadNote}</p>{/if}
    {/if}
    {#if error}<p class="small error" role="alert">{error}</p>{/if}

    <div class="footer">
      <span class="grow"></span>
      <button onclick={onclose} disabled={working}>{m.common.cancel}</button>
      <button class="primary" onclick={apply} disabled={working || (!chosen.length && !typing.trim()) || preview?.documents === 0}>
        {working ? m.bulkTags.working : mode === "add" ? m.bulkTags.add : m.bulkTags.remove}
      </button>
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
    width: min(440px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
  }
  h1 {
    margin: 0 0 12px;
    font-size: 15px;
  }
  p {
    margin: 10px 0 0;
  }
  .small {
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
  }
  .warn {
    color: var(--warning, #a15c00);
  }
  .error {
    color: var(--danger, #c0392b);
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 8px;
  }
  .chips input {
    flex: 1;
    min-width: 140px;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    font: inherit;
  }
  .chip {
    font-size: 12px;
    padding: 1px 8px;
    border-radius: 10px;
    border: 0;
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .tag-list {
    list-style: none;
    margin: 0;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    max-height: 260px;
    overflow: auto;
  }
  .tag-list label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 10px;
  }
  .tag-list .count {
    margin-left: auto;
    color: var(--muted);
    font-size: 12px;
  }
  .footer {
    display: flex;
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
