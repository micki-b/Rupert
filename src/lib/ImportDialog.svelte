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
  const n = (x: number) => x.toLocaleString();
  /** The last two folders, e.g. "…/OneDrive/Taxes"; full path on hover. */
  const shortPath = (p: string) => {
    const parts = p.split(/[\\/]/).filter(Boolean);
    return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : p;
  };
  const plural = (x: number, one: string, many = one + "s") => `${n(x)} ${x === 1 ? one : many}`;

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
    const picked = await openDialog({ directory: true, title: "ScanSnap Home's library folder (“Managed”)" });
    if (typeof picked === "string") library = picked;
  }

  const review = () =>
    attempt(async () => {
      summary = await api.scansnapPlan(library!);
      step = "review";
    });

  const addSource = (path: string) =>
    attempt(async () => {
      await api.addSource(path);
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
    <h1 id="import-title">Import from ScanSnap Home</h1>

    {#if step === "find"}
      <p>
        Rupert brings over what ScanSnap Home knows about your documents that isn't in the files themselves:
        document dates, favorites, receipt details, and the text it recognized in scans. Nothing is moved or
        renamed, and ScanSnap Home isn't changed.
      </p>
      {#if library}
        <p class="path">{library}</p>
      {:else if library === null}
        <p class="muted">ScanSnap Home's library wasn't found in the usual place. Choose its “Managed” folder.</p>
      {/if}
      <div class="actions">
        <button onclick={chooseLibrary}>Choose folder…</button>
        <span class="spacer"></span>
        <button onclick={onclose}>Cancel</button>
        <button class="primary" disabled={!library || busy} onclick={review}>
          {busy ? "Reading…" : "Continue"}
        </button>
      </div>
    {:else if step === "review" && summary}
      {@const s = summary}
      <p>
        ScanSnap Home lists {plural(s.items, "document")}. {n(s.withDetails)} have details to bring over;
        {#if s.ready}{n(s.ready)} of them are in Rupert's sources:{:else}none of them are in Rupert's sources yet.{/if}
      </p>
      {#if s.ready}
        <ul class="counts">
          {#if s.dates}<li><b>{n(s.dates)}</b> document dates</li>{/if}
          {#if s.favorites}<li><b>{n(s.favorites)}</b> favorites</li>{/if}
          {#if s.receipts}<li><b>{n(s.receipts)}</b> receipts (tagged “Receipt”, details in the notes)</li>{/if}
          {#if s.texts}<li><b>{n(s.texts)}</b> scans' recognized text, so they can be searched</li>{/if}
        </ul>
        <p class="muted small">
          Details go into the PDF itself where possible; the recognized text goes into a hidden file next to
          the document.
          {#if s.onlineOnly}
            {plural(s.onlineOnly, "document is", "documents are")} online-only: they aren't downloaded, so their
            details wait in the hidden file and move into the PDF the next time you save it in Rupert.
          {/if}
          Details you've already set in Rupert are kept.
        </p>
      {/if}

      {#if s.outsideSources.length}
        <h2>Not in Rupert's sources</h2>
        <ul class="folders">
          {#each s.outsideSources as f (f.path)}
            <li>
              <span class="path" title={f.path}>{shortPath(f.path)}</span>
              <span class="muted">{n(f.count)}</span>
              <button disabled={busy} onclick={() => addSource(f.path)}>Add as source</button>
            </li>
          {/each}
        </ul>
      {/if}

      {#if s.renamed.length}
        <details>
          <summary>{plural(s.renamed.length, "document was", "documents were")} renamed or moved since; matched by size</summary>
          <ul class="list">
            {#each s.renamed as r (r.from)}
              <li><span class="muted">{r.from}</span><br />→ {sourceName(r.sourceId)}/{r.relPath}</li>
            {/each}
          </ul>
        </details>
      {/if}
      {#if s.notFound.length}
        <details>
          <summary>{plural(s.notFound.length, "document")} couldn't be found</summary>
          <ul class="list">
            {#each s.notFound as p (p)}<li>{p}</li>{/each}
          </ul>
        </details>
      {/if}

      <div class="actions">
        <span class="spacer"></span>
        <button onclick={onclose}>Cancel</button>
        <button class="primary" disabled={!s.ready || busy} onclick={start}>
          Import {plural(s.ready, "document")}
        </button>
      </div>
    {:else if step === "running"}
      <p>Importing… {#if progress}{n(progress.done)} of {n(progress.total)}{/if}</p>
      <progress max={progress?.total || 1} value={progress?.done ?? 0}></progress>
      <p class="muted small">You can keep using Rupert; it re-indexes the changed folders afterwards.</p>
    {:else if step === "done" && report}
      <p>Done.</p>
      <ul class="counts">
        {#if report.embedded}<li><b>{n(report.embedded)}</b> PDFs updated</li>{/if}
        {#if report.sidecar}<li><b>{n(report.sidecar)}</b> documents got a hidden details file</li>{/if}
        {#if report.unchanged}<li><b>{n(report.unchanged)}</b> already up to date</li>{/if}
      </ul>
      {#if report.failed.length}
        <details open>
          <summary>{plural(report.failed.length, "document")} couldn't be updated</summary>
          <ul class="list">
            {#each report.failed as f (f.relPath)}
              <li>{sourceName(f.sourceId)}/{f.relPath}<br /><span class="muted">{f.error}</span></li>
            {/each}
          </ul>
        </details>
      {/if}
      <div class="actions">
        <span class="spacer"></span>
        <button class="primary" onclick={onclose}>Close</button>
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
