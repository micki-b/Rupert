<script lang="ts">
  // The tag manager: pick tags, then rename (one tag), merge (several) or
  // delete them in every document that has them. Shows what will change,
  // including online-only files that would have to be downloaded.
  import { onMount } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    api,
    onIndexChanged,
    onTagEditProgress,
    type TagCount,
    type TagEdit,
    type TagEditPreview,
    type TagEditReport,
  } from "#lib/api.ts";
  import { describeTagEdit } from "#lib/tag-edit.ts";
  import { m } from "#lib/i18n/index.ts";

  let { onclose, onchanged }: { onclose: () => void; onchanged: () => void } = $props();

  let all = $state<TagCount[]>([]);
  let filter = $state("");
  let sortByCount = $state(true);
  let picked = $state<string[]>([]);
  let newName = $state("");
  let preview = $state<TagEditPreview | null>(null);
  let previewError = $state<string | null>(null);
  let working = $state(false);
  let progress = $state<{ done: number; total: number } | null>(null);
  let report = $state<TagEditReport | null>(null);
  let error = $state<string | null>(null);

  const shown = $derived.by(() => {
    const f = filter.trim().toLowerCase();
    const list = all.filter((t) => !f || t.tag.toLowerCase().includes(f));
    return sortByCount
      ? [...list].sort((a, b) => b.count - a.count || a.tag.localeCompare(b.tag))
      : [...list].sort((a, b) => a.tag.localeCompare(b.tag, undefined, { sensitivity: "base" }));
  });
  const isPicked = (tag: string) => picked.includes(tag);
  const countOf = (tag: string) => all.find((t) => t.tag === tag)?.count ?? 0;
  const renameEdit = $derived<TagEdit | null>(
    picked.length && newName.trim() ? { from: picked, action: { kind: "rename", to: newName.trim() } } : null,
  );
  const verb = $derived(picked.length > 1 ? m.tags.merge : m.tags.rename);

  async function load() {
    all = await api.listTags();
    picked = picked.filter((p) => all.some((t) => t.tag === p));
  }

  onMount(() => {
    load();
    // The index catches up with the changed files a moment later.
    const un = [onTagEditProgress((p) => (progress = p)), onIndexChanged(() => load())];
    return () => un.forEach((u) => u.then((f) => f()));
  });

  function toggle(tag: string) {
    report = null;
    if (isPicked(tag)) {
      picked = picked.filter((t) => t !== tag);
    } else {
      picked = [...picked, tag];
      // Suggest the most-used picked spelling as the merged name.
      if (picked.length > 1 && !newName.trim()) {
        newName = [...picked].sort((a, b) => countOf(b) - countOf(a))[0];
      }
    }
  }

  // Preview whenever the rename changes.
  let seq = 0;
  $effect(() => {
    const edit = renameEdit;
    preview = null;
    previewError = null;
    if (!edit) return;
    const mine = ++seq;
    const t = setTimeout(async () => {
      try {
        const p = await api.tagEditPreview($state.snapshot(edit) as TagEdit);
        if (mine === seq) preview = p;
      } catch (e) {
        if (mine === seq) previewError = String(e);
      }
    }, 200);
    return () => clearTimeout(t);
  });

  const describe = describeTagEdit;

  async function run(edit: TagEdit) {
    working = true;
    error = null;
    report = null;
    progress = null;
    try {
      report = await api.tagEditApply(edit);
      picked = [];
      newName = "";
      await load();
      onchanged();
    } catch (e) {
      error = String(e);
    } finally {
      working = false;
    }
  }

  async function rename() {
    if (!renameEdit || !preview) return;
    if (preview.download > 0) {
      const ok = await ask(
        m.tags.downloadAsk(preview.download),
        { title: picked.length > 1 ? m.tags.mergeTitle : m.tags.renameTitle, kind: "warning", okLabel: verb },
      );
      if (!ok) return;
    }
    await run($state.snapshot(renameEdit) as TagEdit);
  }

  async function remove() {
    if (!picked.length) return;
    const edit: TagEdit = { from: [...picked], action: { kind: "delete" } };
    let p: TagEditPreview;
    try {
      p = await api.tagEditPreview(edit);
    } catch (e) {
      error = String(e);
      return;
    }
    const names = picked.map((t) => `“${t}”`).join(", ");
    const ok = await ask(
      m.tags.deleteAsk(names, describe(p), p.download > 0),
      { title: m.tags.deleteTitle, kind: "warning", okLabel: m.tags.delete },
    );
    if (ok) await run(edit);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !working) onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="tm-title">
    <h1 id="tm-title">{m.tags.title}</h1>
    <p class="muted small">{m.tags.intro}</p>
    <div class="columns">
      <div class="list-col">
        <div class="list-tools">
          <input type="search" placeholder={m.tags.filter(all.length)} bind:value={filter} />
          <button class="link" onclick={() => (sortByCount = !sortByCount)}>
            {sortByCount ? m.tags.byUse : m.tags.aToZ}
          </button>
        </div>
        <ul class="tag-list">
          {#each shown as t (t.tag)}
            <li>
              <label class:on={isPicked(t.tag)}>
                <input type="checkbox" checked={isPicked(t.tag)} onchange={() => toggle(t.tag)} disabled={working} />
                <span class="name"># {t.tag}</span>
                <span class="count">{t.count}</span>
              </label>
            </li>
          {:else}
            <li class="muted small pad">{all.length ? m.tags.noneMatch : m.tags.noneYet}</li>
          {/each}
        </ul>
      </div>

      <div class="action-col">
        {#if !picked.length}
          <p class="muted">{m.tags.pickHint}</p>
        {:else}
          <h2>{m.tags.selected(picked.length)}</h2>
          <ul class="picked">
            {#each picked as t (t)}
              <li><span># {t}</span><span class="count">{countOf(t)}</span></li>
            {/each}
          </ul>
          <label class="field">
            <span>{picked.length > 1 ? m.tags.mergeInto : m.tags.renameTo}</span>
            <input bind:value={newName} list="tm-tags" placeholder={m.tags.newName} disabled={working} />
            <datalist id="tm-tags">
              {#each all as t (t.tag)}<option value={t.tag}></option>{/each}
            </datalist>
          </label>
          {#if preview}
            <p class="small">{m.tags.changes(describe(preview))}</p>
            {#if preview.download}
              <p class="small warn">{m.tags.downloadNote}</p>
            {/if}
          {:else if previewError}
            <p class="small error">{previewError}</p>
          {/if}
          <div class="buttons">
            <button class="danger" onclick={remove} disabled={working}>{m.tags.deleteButton}</button>
            <span class="spacer"></span>
            <button class="primary" onclick={rename} disabled={working || !preview || preview.documents === 0}>
              {verb}
            </button>
          </div>
        {/if}

        {#if working}
          <p class="small progress">
            <span class="spinner"></span>
            {progress
              ? `${m.tags.saving} ${m.tags.savingProgress(progress.done, progress.total)}`
              : m.common.ellipsis(m.tags.saving)}
          </p>
        {/if}
        {#if report}
          <p class="small ok">
            {m.tags.changed(report.changed)}
          </p>
          {#if report.failed.length}
            <p class="small error">{m.tags.failed(report.failed.length)}</p>
            <ul class="failures">
              {#each report.failed.slice(0, 20) as f (f.path)}<li><strong>{f.path}</strong>: {f.error}</li>{/each}
            </ul>
          {/if}
        {/if}
        {#if error}<p class="small error">{error}</p>{/if}
      </div>
    </div>
    <div class="actions">
      <span class="spacer"></span>
      <button onclick={onclose} disabled={working}>{m.common.done}</button>
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
    width: min(760px, calc(100vw - 32px));
    height: min(620px, calc(100vh - 64px));
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
    font-size: 16px;
    margin: 0 0 4px;
  }
  h2 {
    font-size: 11px;
    margin: 0 0 6px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
  }
  p {
    margin: 6px 0;
    line-height: 1.4;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .columns {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 16px;
    margin-top: 10px;
  }
  .list-col {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .list-tools {
    display: flex;
    gap: 6px;
    padding: 6px;
    border-bottom: 1px solid var(--border);
  }
  .list-tools input {
    flex: 1;
  }
  .tag-list {
    list-style: none;
    margin: 0;
    padding: 4px;
    overflow-y: auto;
    flex: 1;
  }
  .tag-list label {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 6px;
    border-radius: 5px;
    cursor: default;
  }
  .tag-list label:hover {
    background: var(--hover);
  }
  .tag-list label.on {
    background: var(--accent-soft);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .count {
    color: var(--muted);
    font-size: 11px;
  }
  .pad {
    padding: 6px;
  }
  .action-col {
    overflow-y: auto;
  }
  .picked {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    max-height: 140px;
    overflow-y: auto;
  }
  .picked li {
    display: flex;
    justify-content: space-between;
    padding: 2px 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--muted);
  }
  input:not([type="checkbox"]) {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 13px;
  }
  .buttons,
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
  }
  .spacer {
    flex: 1;
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 12px;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  button.danger {
    color: var(--danger);
  }
  button.link {
    border: 0;
    background: none;
    color: var(--accent);
    padding: 0 4px;
    font-size: 12px;
  }
  button:disabled {
    opacity: 0.5;
  }
  .warn {
    color: var(--muted);
  }
  .error {
    color: var(--danger);
  }
  .ok {
    color: var(--accent-strong);
  }
  .failures {
    font-size: 12px;
    margin: 4px 0;
    padding-left: 18px;
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
