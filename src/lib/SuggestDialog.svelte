<script lang="ts">
  // Suggest details… (#113): a title, document date and tags for documents
  // missing them, read by Local AI from the text Rupert already has
  // (nothing is downloaded), one document at a time with Stop. What a
  // document has stays; suggestions fill what's missing. Only the ticked
  // rows are saved, and only the fields that changed.
  import { onMount } from "svelte";
  import { api, type DocPlace, type Suggested } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let { docs, title, onclose }: { docs: DocPlace[]; title: string; onclose: () => void } = $props();

  type Row = {
    doc: DocPlace;
    name: string;
    current: Suggested;
    edit: { title: string; date: string; tags: string };
    include: boolean;
    note: string | null;
  };

  let rows = $state<Row[]>([]);
  let done = $state(0);
  let running = $state(true);
  let stopping = false;
  let error = $state<string | null>(null);
  let saving = $state(false);
  let failed = $state<[string, string][]>([]);

  const fileName = (relPath: string) => relPath.slice(relPath.lastIndexOf("/") + 1);
  const tagText = (t: string[] | null | undefined) => (t ?? []).join(", ");
  const tagList = (s: string) => [...new Set(s.split(",").map((t) => t.trim()).filter(Boolean))];

  onMount(() => {
    void run();
    return () => (stopping = true);
  });

  async function run() {
    for (const doc of docs) {
      if (stopping) break;
      const name = fileName(doc.relPath);
      try {
        const p = await api.suggestPropose(doc.sourceId, doc.relPath);
        const c = p.current;
        const s = p.proposed;
        const edit = {
          title: c.title ?? s?.title ?? "",
          date: c.documentDate ?? s?.documentDate ?? "",
          tags: c.tags?.length ? tagText(c.tags) : tagText(s?.tags),
        };
        rows.push({ doc, name, current: c, edit, include: changed({ doc, name, current: c, edit, include: false, note: null }), note: s ? null : m.readDetails.noText });
      } catch (e) {
        const msg = String(e);
        if (!rows.length && /Local AI/i.test(msg)) {
          error = msg;
          break;
        }
        rows.push({ doc, name, current: { title: null, documentDate: null, tags: [] }, edit: { title: "", date: "", tags: "" }, include: false, note: msg });
      }
      done++;
    }
    running = false;
  }

  function changed(r: Row): boolean {
    return (
      r.edit.title.trim() !== (r.current.title ?? "") ||
      r.edit.date !== (r.current.documentDate ?? "") ||
      tagText(tagList(r.edit.tags)) !== tagText(r.current.tags)
    );
  }

  const ticked = $derived(rows.filter((r) => r.include && !r.note));

  async function save() {
    saving = true;
    error = null;
    try {
      const res = await api.suggestApply(
        ticked.map((r) => ({
          sourceId: r.doc.sourceId,
          relPath: r.doc.relPath,
          suggested: {
            title: r.edit.title.trim() !== (r.current.title ?? "") ? r.edit.title.trim() || null : null,
            documentDate: r.edit.date !== (r.current.documentDate ?? "") ? r.edit.date || null : null,
            tags: tagText(tagList(r.edit.tags)) !== tagText(r.current.tags) ? tagList(r.edit.tags) : null,
          },
        })),
      );
      if (res.failed.length) failed = res.failed;
      else onclose();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !saving && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="suggest-title">
    <h1 id="suggest-title">{m.health.suggestTitle(title)}</h1>
    <p class="muted small">{m.health.suggestIntro}</p>
    {#if error}<p class="small error" role="alert">{error}</p>{/if}
    {#if running}
      <div class="progress">
        <progress max={docs.length} value={done}></progress>
        <span class="small muted">{m.readDetails.reading(done, docs.length)}</span>
        <button onclick={() => (stopping = true)}>{m.exportDocs.stop}</button>
      </div>
    {/if}
    {#if rows.length}
      <div class="table">
        <table>
          <thead>
            <tr>
              <th class="tick"></th>
              <th>{m.readDetails.document}</th>
              <th>{m.details.title}</th>
              <th class="date">{m.details.documentDate}</th>
              <th>{m.details.tags}</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.doc.sourceId + "\n" + r.doc.relPath)}
              <tr class:off={!r.include || !!r.note}>
                <td class="tick"><input type="checkbox" bind:checked={r.include} disabled={!!r.note} aria-label={r.name} /></td>
                <td class="name" title={r.doc.relPath}>{r.name}{#if r.note}<div class="small muted">{r.note}</div>{/if}</td>
                {#if !r.note}
                  <td><input class:changed={r.edit.title.trim() !== (r.current.title ?? "")} bind:value={r.edit.title} /></td>
                  <td class="date"><input type="date" class:changed={r.edit.date !== (r.current.documentDate ?? "")} bind:value={r.edit.date} /></td>
                  <td><input class:changed={tagText(tagList(r.edit.tags)) !== tagText(r.current.tags)} bind:value={r.edit.tags} /></td>
                {:else}
                  <td colspan="3"></td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else if !running && !error}
      <p class="small muted">{m.readDetails.nothing}</p>
    {/if}
    {#if failed.length}
      <p class="small error">{m.readDetails.failed(failed.length)}</p>
      <ul class="failed small">{#each failed as [path, why] (path)}<li><b>{path}</b>: {why}</li>{/each}</ul>
    {/if}
    <div class="footer">
      <span class="small muted">{m.readDetails.onlineNote}</span>
      <span class="grow"></span>
      <button onclick={onclose} disabled={saving}>{failed.length ? m.common.done : m.common.cancel}</button>
      {#if !failed.length}
        <button class="primary" onclick={save} disabled={saving || running || ticked.length === 0}>
          {saving ? m.details.saving : m.readDetails.save(ticked.length)}
        </button>
      {/if}
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
    width: min(900px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  p {
    margin: 6px 0 0;
  }
  .small {
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
  }
  .error {
    color: var(--danger, #c0392b);
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }
  .progress progress {
    flex: 1;
  }
  .table {
    margin-top: 12px;
    overflow: auto;
    min-height: 0;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--panel);
    text-align: left;
    font-weight: 500;
    font-size: 11px;
    color: var(--muted);
    padding: 5px 6px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 3px 6px;
    border-bottom: 1px solid var(--border);
    vertical-align: top;
  }
  .tick {
    width: 26px;
  }
  .date {
    width: 132px;
  }
  .name {
    font-size: 12px;
    word-break: break-word;
    padding-top: 6px;
  }
  td input:not([type="checkbox"]) {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid var(--border);
    border-radius: 5px;
    padding: 2px 5px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    font-size: 12px;
  }
  td input.changed {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
  tr.off td:not(.tick) {
    opacity: 0.55;
  }
  .failed {
    margin: 6px 0 0;
    padding-left: 18px;
    max-height: 100px;
    overflow: auto;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
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
