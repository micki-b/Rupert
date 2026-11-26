<script lang="ts">
  // Export… (#107): copies of a set of documents in a zip or a folder, with
  // a list (index.csv and index.html). Copies keep their folders and names.
  // Zip or folder and notes, then where to save it (the save dialog), then
  // progress, then Show in Finder / Share….
  import { onMount } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import {
    api,
    atElement,
    onExportProgress,
    revealLabel,
    type DocPlace,
    type ExportDone,
    type ExportPlan,
    type ExportProgress,
  } from "#lib/api.ts";
  import { formatSize } from "#lib/format.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    docs,
    title,
    os,
    onclose,
  }: {
    docs: DocPlace[];
    /** Names the zip or folder and heads the list. */
    title: string;
    os: string | undefined;
    onclose: () => void;
  } = $props();

  let plan = $state<ExportPlan | null>(null);
  let format = $state<"zip" | "folder">("zip");
  let includeNotes = $state(false);
  let stage = $state<"options" | "working" | "done">("options");
  let progress = $state<ExportProgress | null>(null);
  let done = $state<ExportDone | null>(null);
  let error = $state<string | null>(null);
  let stopping = $state(false);

  onMount(() => {
    api
      .exportPlan($state.snapshot(docs) as DocPlace[])
      .then((p) => {
        plan = p;
        includeNotes = p.includeNotes;
      })
      .catch((e) => (error = String(e)));
    const off = onExportProgress((p) => (progress = p));
    return () => {
      off.then((f) => f());
    };
  });

  /** A file name from the title (no characters a disk won't take). */
  const safeTitle = $derived(title.replace(/[/\\:*?"<>|]+/g, " ").replace(/\s+/g, " ").trim() || m.exportDocs.defaultName);

  async function start() {
    if (!plan) return;
    let dest: string | null;
    try {
      dest = await save({
        title: m.exportDocs.saveTitle,
        defaultPath: format === "zip" ? `${safeTitle}.zip` : safeTitle,
        filters: format === "zip" ? [{ name: "Zip", extensions: ["zip"] }] : undefined,
      });
    } catch (e) {
      error = String(e);
      return;
    }
    if (!dest) return;
    stage = "working";
    error = null;
    try {
      done = await api.exportRun(
        $state.snapshot(docs) as DocPlace[],
        { format, includeNotes },
        format === "zip" && !dest.toLowerCase().endsWith(".zip") ? `${dest}.zip` : dest,
        title,
      );
      stage = "done";
    } catch (e) {
      stage = "options";
      error = stopping ? null : String(e);
    } finally {
      stopping = false;
    }
  }

  function stop() {
    stopping = true;
    api.exportCancel();
  }

  // svelte-ignore state_referenced_locally
  const reveal = revealLabel(os);
  const fileName = $derived(done ? done.path.split(/[/\\]/).pop() : "");
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && stage !== "working" && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="export-title">
    <h1 id="export-title">{m.exportDocs.title(docs.length)}</h1>

    {#if stage === "options"}
      <p class="muted small">
        {#if plan}{m.exportDocs.summary(formatSize(plan.size))}{:else}{m.common.loading}{/if}
      </p>

      <fieldset>
        <legend>{m.exportDocs.as}</legend>
        <label><input type="radio" bind:group={format} value="zip" /> {m.exportDocs.zip}</label>
        <label><input type="radio" bind:group={format} value="folder" /> {m.exportDocs.folder}</label>
      </fieldset>
      <label class="check">
        <input type="checkbox" bind:checked={includeNotes} />
        {m.exportDocs.includeNotes}
      </label>

      <p class="muted small">{m.exportDocs.listNote}</p>
      {#if plan?.online}<p class="small warn">{m.exportDocs.onlineNote(plan.online)}</p>{/if}
      {#if error}<p class="small error" role="alert">{error}</p>{/if}

      <div class="footer">
        <span class="grow"></span>
        <button onclick={onclose}>{m.common.cancel}</button>
        <button class="primary" onclick={start} disabled={!plan || plan.documents === 0}>{m.common.ellipsis(m.exportDocs.export)}</button>
      </div>
    {:else if stage === "working"}
      <progress max={progress?.total ?? docs.length} value={progress?.done ?? 0}></progress>
      <p class="small muted name">
        {#if progress}{progress.downloading ? m.exportDocs.downloading(progress.name) : m.exportDocs.copying(progress.name)}{/if}
      </p>
      <div class="footer">
        <span class="grow"></span>
        <button onclick={stop} disabled={stopping}>{stopping ? m.exportDocs.stopping : m.exportDocs.stop}</button>
      </div>
    {:else if done}
      <p>{m.exportDocs.done(done.copied, fileName ?? "")}</p>
      {#if done.failed.length}
        <p class="small warn">{m.exportDocs.failed(done.failed.length)}</p>
        <ul class="failed small">
          {#each done.failed as [path, why] (path)}<li><b>{path}</b>: {why}</li>{/each}
        </ul>
      {/if}
      <div class="footer">
        <button onclick={() => api.exportReveal(done!.path)}>{reveal}</button>
        {#if format === "zip" && os !== "linux"}
          <button onclick={(ev) => api.shareFile(done!.path, atElement(ev.currentTarget))}>{m.common.ellipsis(m.share.share)}</button>
        {/if}
        <span class="grow"></span>
        <button class="primary" onclick={onclose}>{m.common.done}</button>
      </div>
    {/if}
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
    width: min(460px, calc(100vw - 32px));
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
    margin: 0 0 6px;
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
  fieldset {
    border: 0;
    margin: 12px 0 0;
    padding: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
  }
  legend {
    width: 100%;
    padding: 0;
    margin-bottom: 4px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  label.check {
    margin-top: 12px;
  }
  progress {
    width: 100%;
    margin-top: 12px;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .failed {
    margin: 6px 0 0;
    padding-left: 18px;
    max-height: 120px;
    overflow: auto;
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
