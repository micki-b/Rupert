<script lang="ts">
  // Import from ScanSnap Home: find the library, review what would change,
  // then apply in the background (see rupert_core::scansnap).
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import {
    api,
    onImportDone,
    onImportProgress,
    type ImportProgress,
    type ImportReport,
    type ImportSummary,
    type Source,
  } from "$lib/api";
  import { m, num } from "$lib/i18n";

  let {
    sources,
    onclose,
    onsourceadded,
  }: { sources: Source[]; onclose: () => void; onsourceadded: () => void } = $props();

  type Step = "find" | "review" | "running" | "done";
  let step = $state<Step>("find");
  let library = $state<string | null>(null);
  let summary = $state<ImportSummary | null>(null);
  let progress = $state<ImportProgress | null>(null);
  let report = $state<ImportReport | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  const sourceName = (id: string) => sources.find((s) => s.id === id)?.name ?? "";
  const n = (x: number) => num(x);
  /** The last two folders, e.g. "…/OneDrive/Taxes"; full path on hover. */
  const shortPath = (p: string) => {
    const parts = p.split(/[\\/]/).filter(Boolean);
    return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : p;
  };

  onMount(() => {
    api.scansnapLibrary().then((l) => (library = l));
    const unlisten = [
      onImportProgress((p) => (progress = p)),
      onImportDone((r) => {
        report = r;
        step = "done";
      }),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  async function attempt(action: () => Promise<void>) {
    busy = true;
    error = null;
    try {
      await action();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function chooseLibrary() {
    const picked = await openDialog({ directory: true, title: m.import.pickLibrary });
    if (typeof picked === "string") library = picked;
  }

  const review = () =>
    attempt(async () => {
      summary = await api.scansnapPlan(library!);
      step = "review";
    });

  // The folder picker opens at the suggested folder: adding a source is
  // always the user's pick (a sandboxed app may only use picked folders).
  const addSource = (path: string) =>
    attempt(async () => {
      const picked = await openDialog({ directory: true, defaultPath: path, title: m.import.pickSource });
      if (typeof picked !== "string") return;
      await api.addSource(picked);
      onsourceadded();
      summary = await api.scansnapPlan(library!);
    });

  const start = () =>
    attempt(async () => {
      progress = { done: 0, total: summary?.ready ?? 0 };
      step = "running";
      await api.scansnapApply();
    });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && step !== "running") onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="import-title">
    <h1 id="import-title">{m.import.title}</h1>

    {#if step === "find"}
      <p>{m.import.intro}</p>
      {#if library}
        <p class="path">{library}</p>
      {:else if library === null}
        <p class="muted">{m.import.notFound}</p>
      {/if}
      <div class="actions">
        <button onclick={chooseLibrary}>{m.import.chooseFolder}</button>
        <span class="spacer"></span>
        <button onclick={onclose}>{m.common.cancel}</button>
        <button class="primary" disabled={!library || busy} onclick={review}>
          {busy ? m.import.reading : m.import.continue}
        </button>
      </div>
    {:else if step === "review" && summary}
      {@const s = summary}
      <p>
        {m.import.lists(s.items, s.withDetails)}
        {#if s.ready}{m.import.inSources(s.ready)}{:else}{m.import.noneInSources}{/if}
      </p>
      {#if s.ready}
        <ul class="counts">
          {#if s.dates}<li><b>{n(s.dates)}</b> {m.import.dates(s.dates)}</li>{/if}
          {#if s.favorites}<li><b>{n(s.favorites)}</b> {m.import.favorites(s.favorites)}</li>{/if}
          {#if s.receipts}<li><b>{n(s.receipts)}</b> {m.import.receipts(s.receipts)}</li>{/if}
          {#if s.texts}<li><b>{n(s.texts)}</b> {m.import.texts(s.texts)}</li>{/if}
        </ul>
        <p class="muted small">
          {m.import.whereDetailsGo}
          {#if s.onlineOnly}
            {m.import.onlineOnly(s.onlineOnly)}
          {/if}
          {m.import.keptDetails}
        </p>
      {/if}

      {#if s.outsideSources.length}
        <h2>{m.import.notInSources}</h2>
        <ul class="folders">
          {#each s.outsideSources as f (f.path)}
            <li>
              <span class="path" title={f.path}>{shortPath(f.path)}</span>
              <span class="muted">{n(f.count)}</span>
              <button disabled={busy} onclick={() => addSource(f.path)}>{m.import.addAsSource}</button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if s.renamed.length}
        <details>
          <summary>{m.import.renamed(s.renamed.length)}</summary>
          <ul class="list">
            {#each s.renamed as r (r.from)}
              <li><span class="muted">{r.from}</span><br />→ {sourceName(r.sourceId)}/{r.relPath}</li>
            {/each}
          </ul>
        </details>
      {/if}
      {#if s.notFound.length}
        <details>
          <summary>{m.import.notFoundDocs(s.notFound.length)}</summary>
          <ul class="list">
            {#each s.notFound as p (p)}<li>{p}</li>{/each}
          </ul>
        </details>
      {/if}

      <div class="actions">
        <span class="spacer"></span>
        <button onclick={onclose}>{m.common.cancel}</button>
        <button class="primary" disabled={!s.ready || busy} onclick={start}>
          {m.import.importN(s.ready)}
        </button>
      </div>
    {:else if step === "running"}
      <p>{m.import.importing} {#if progress}{m.import.progress(progress.done, progress.total)}{/if}</p>
      <progress max={progress?.total || 1} value={progress?.done ?? 0}></progress>
      <p class="muted small">{m.import.keepUsing}</p>
    {:else if step === "done" && report}
      <p>{m.import.done}</p>
      <ul class="counts">
        {#if report.embedded}<li><b>{n(report.embedded)}</b> {m.import.pdfsUpdated(report.embedded)}</li>{/if}
        {#if report.sidecar}<li><b>{n(report.sidecar)}</b> {m.import.sidecars(report.sidecar)}</li>{/if}
        {#if report.unchanged}<li><b>{n(report.unchanged)}</b> {m.import.unchanged}</li>{/if}
      </ul>
      {#if report.failed.length}
        <details open>
          <summary>{m.import.failed(report.failed.length)}</summary>
          <ul class="list">
            {#each report.failed as f (f.relPath)}
              <li>{sourceName(f.sourceId)}/{f.relPath}<br /><span class="muted">{f.error}</span></li>
            {/each}
          </ul>
        </details>
      {/if}
      <div class="actions">
        <span class="spacer"></span>
        <button class="primary" onclick={onclose}>{m.common.close}</button>
      </div>
    {/if}

    {#if error}<p class="error">{error}</p>{/if}
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
    width: min(620px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.25);
    padding: 18px 20px;
  }
  h1 {
    font-size: 16px;
    margin: 0 0 10px;
  }
  h2 {
    font-size: 12px;
    margin: 14px 0 6px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  p {
    margin: 0 0 10px;
    line-height: 1.45;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .path {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    word-break: break-all;
  }
  .counts {
    margin: 0 0 10px;
    padding-left: 20px;
    line-height: 1.6;
  }
  .folders {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .folders li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 0;
  }
  .folders .path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  details {
    margin: 8px 0;
  }
  summary {
    cursor: pointer;
  }
  .list {
    max-height: 180px;
    overflow: auto;
    font-size: 12px;
    padding-left: 18px;
    line-height: 1.4;
  }
  .list li {
    margin: 3px 0;
    word-break: break-all;
  }
  progress {
    width: 100%;
  }
  .actions {
    display: flex;
    gap: 8px;
    margin-top: 16px;
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
  button:disabled {
    opacity: 0.5;
  }
  .error {
    color: #c0392b;
    margin-top: 10px;
  }
</style>
