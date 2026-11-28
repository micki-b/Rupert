<script lang="ts">
  // File All Sure Ones (#112): the scans whose suggestion agrees with how
  // similar documents were filed, each with its new name, folder and tags.
  // Untick one to leave it in the inbox; nothing is filed before "File".
  import { fileNameFor, type IntakeItem } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    items,
    namePattern,
    onfile,
    onclose,
  }: {
    items: IntakeItem[];
    namePattern: string;
    /** Files these; resolves when done (the dialog then closes). */
    onfile: (chosen: IntakeItem[]) => Promise<void>;
    onclose: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let left = $state<Set<string>>(new Set());
  let working = $state(false);
  let error = $state<string | null>(null);

  const chosen = $derived(items.filter((i) => !left.has(i.relPath)));

  /** The name it's filed under, as File it would make it. */
  function newName(i: IntakeItem): string {
    const ext = i.name.includes(".") ? i.name.slice(i.name.lastIndexOf(".")) : "";
    const s = i.suggestion;
    const stem = s?.title?.trim() ? fileNameFor(namePattern, s.documentDate || null, s.title.trim()) : i.name.replace(/\.[^.]+$/, "");
    return stem + ext;
  }

  function toggle(rel: string) {
    const next = new Set(left);
    if (next.has(rel)) next.delete(rel);
    else next.add(rel);
    left = next;
  }

  async function file() {
    if (!chosen.length || working) return;
    working = true;
    error = null;
    try {
      await onfile(chosen);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      working = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !working && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="file-all-title">
    <h1 id="file-all-title">{m.inbox.fileAllTitle(chosen.length)}</h1>
    <p class="muted">{m.inbox.fileAllIntro}</p>
    <div class="table">
      <div class="row head">
        <span></span><span>{m.inbox.newName}</span><span>{m.inbox.folder}</span><span>{m.inbox.tags}</span>
      </div>
      {#each items as i (i.relPath)}
        <label class="row">
          <input type="checkbox" checked={!left.has(i.relPath)} onchange={() => toggle(i.relPath)} disabled={working} />
          <span class="name">
            <b title={newName(i)}>{newName(i)}</b>
            <span class="muted small">{i.confidence?.reason ?? ""}</span>
          </span>
          <span title={i.suggestedFolder?.label}>{i.suggestedFolder?.label ?? ""}</span>
          <span class="muted">{i.suggestion?.tags.join(", ") ?? ""}</span>
        </label>
      {/each}
    </div>
    <p class="muted small">{m.inbox.fileAllNote}</p>
    {#if error}<p class="small error" role="alert">{error}</p>{/if}
    <div class="footer">
      <span class="grow"></span>
      <button onclick={onclose} disabled={working}>{m.common.cancel}</button>
      <button class="primary" onclick={file} disabled={working || !chosen.length}>
        {working ? m.inbox.filing : m.inbox.fileN(chosen.length)}
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
    width: min(820px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h1 {
    margin: 0;
    font-size: 15px;
  }
  p {
    margin: 0;
  }
  .table {
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .row {
    display: grid;
    grid-template-columns: 24px minmax(0, 1.4fr) minmax(0, 1fr) minmax(0, 0.8fr);
    gap: 10px;
    align-items: center;
    padding: 7px 12px;
    border-top: 1px solid var(--border);
  }
  .row > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.head {
    border-top: none;
    background: var(--panel);
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
  }
  .name {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .name b {
    overflow: hidden;
    text-overflow: ellipsis;
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
  .footer {
    display: flex;
    gap: 8px;
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
