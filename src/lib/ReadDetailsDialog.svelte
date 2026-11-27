<script lang="ts">
  // Read details… (#108): who each document is from, its amount and due
  // date, read by Local AI from the text Rupert already has (nothing is
  // downloaded), one document at a time with Stop. The table can be edited;
  // only the ticked rows are saved.
  import { onMount } from "svelte";
  import { api, type DocPlace, type Details } from "#lib/api.ts";
  import { CURRENCIES, senderList } from "#lib/details.svelte.ts";
  import { m } from "#lib/i18n/index.ts";
  import { formatAmountInput, parseAmount, tidyAmountInput } from "#lib/format.ts";

  let {
    docs,
    title,
    onclose,
    onsettings,
  }: {
    docs: DocPlace[];
    title: string;
    onclose: () => void;
    /** Opens Settings ▸ Local AI. */
    onsettings: () => void;
  } = $props();

  type Row = {
    doc: DocPlace;
    name: string;
    current: Details;
    /** The values to save, starting from what the text shows. */
    edit: { sender: string; amount: string; currency: string; dueDate: string };
    include: boolean;
    /** No text to read, or the read failed. */
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
  const same = (a: string | null | undefined, b: string | null | undefined) => (a ?? "") === (b ?? "");

  onMount(() => {
    void run();
    return () => (stopping = true);
  });

  async function run() {
    for (const doc of docs) {
      if (stopping) break;
      const name = fileName(doc.relPath);
      try {
        const p = await api.detailsPropose(doc.sourceId, doc.relPath);
        const c = p.current;
        const n = p.proposed;
        // What the text shows, where it shows something; otherwise as now.
        const pick = (now: string | null, found: string | null | undefined) => found ?? now ?? "";
        const edit = {
          sender: pick(c.sender, n?.sender),
          amount: formatAmountInput(pick(c.amount, n?.amount)),
          currency: pick(c.currency, n?.amount ? n.currency : c.currency),
          dueDate: pick(c.dueDate, n?.dueDate),
        };
        const changes = !same(edit.sender, c.sender) || !same(parseAmount(edit.amount), c.amount) || !same(edit.currency, c.currency) || !same(edit.dueDate, c.dueDate);
        rows.push({ doc, name, current: c, edit, include: changes, note: n ? null : m.readDetails.noText });
      } catch (e) {
        const msg = String(e);
        // Without Local AI there's nothing to do for any of them.
        if (!rows.length && /Local AI/i.test(msg)) {
          error = msg;
          break;
        }
        rows.push({
          doc,
          name,
          current: { sender: null, amount: null, currency: null, dueDate: null },
          edit: { sender: "", amount: "", currency: "", dueDate: "" },
          include: false,
          note: msg,
        });
      }
      done++;
    }
    running = false;
  }

  const ticked = $derived(rows.filter((r) => r.include && !r.note));

  async function save() {
    saving = true;
    error = null;
    try {
      const res = await api.detailsApply(
        ticked.map((r) => ({
          sourceId: r.doc.sourceId,
          relPath: r.doc.relPath,
          details: {
            sender: r.edit.sender.trim() || null,
            amount: r.edit.amount.trim() ? (parseAmount(r.edit.amount) ?? r.edit.amount.trim()) : null,
            currency: r.edit.amount.trim() ? r.edit.currency.trim().toUpperCase() || null : null,
            dueDate: r.edit.dueDate || null,
          },
        })),
      );
      senderList.refresh();
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
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="details-title">
    <h1 id="details-title">{m.readDetails.title(title)}</h1>
    <p class="muted small">{m.readDetails.intro}</p>

    {#if error}
      <p class="small error" role="alert">{error}</p>
      {#if /Local AI/i.test(error)}<button onclick={onsettings}>{m.readDetails.openSettings}</button>{/if}
    {/if}

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
              <th>{m.details.sender}</th>
              <th class="amount">{m.details.amount}</th>
              <th class="cur">{m.details.currency}</th>
              <th class="due">{m.details.dueDate}</th>
            </tr>
          </thead>
          <tbody>
            {#each rows as r (r.doc.sourceId + "\n" + r.doc.relPath)}
              <tr class:off={!r.include || !!r.note}>
                <td class="tick"><input type="checkbox" bind:checked={r.include} disabled={!!r.note} aria-label={r.name} /></td>
                <td class="name" title={r.doc.relPath}>
                  {r.name}
                  {#if r.note}<div class="small muted">{r.note}</div>{/if}
                </td>
                {#if !r.note}
                  <td><input class:changed={!same(r.edit.sender, r.current.sender)} bind:value={r.edit.sender} list="details-senders" /></td>
                  <td class="amount">
                    <input
                      class:changed={!same(parseAmount(r.edit.amount), r.current.amount)}
                      bind:value={r.edit.amount}
                      inputmode="decimal"
                      onblur={() => (r.edit.amount = tidyAmountInput(r.edit.amount))}
                    />
                  </td>
                  <td class="cur">
                    <input
                      class:changed={!same(r.edit.currency, r.current.currency)}
                      bind:value={r.edit.currency}
                      list="details-currencies"
                      maxlength="3"
                      oninput={() => (r.edit.currency = r.edit.currency.toUpperCase())}
                    />
                  </td>
                  <td class="due"><input type="date" class:changed={!same(r.edit.dueDate, r.current.dueDate)} bind:value={r.edit.dueDate} /></td>
                {:else}
                  <td colspan="4"></td>
                {/if}
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
      <datalist id="details-senders">
        {#each senderList.names as name (name)}<option value={name}></option>{/each}
      </datalist>
      <datalist id="details-currencies">
        {#each CURRENCIES as c (c)}<option value={c}></option>{/each}
      </datalist>
    {:else if !running && !error}
      <p class="small muted">{m.readDetails.nothing}</p>
    {/if}

    {#if failed.length}
      <p class="small error">{m.readDetails.failed(failed.length)}</p>
      <ul class="failed small">
        {#each failed as [path, why] (path)}<li><b>{path}</b>: {why}</li>{/each}
      </ul>
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
    width: min(860px, calc(100vw - 32px));
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
  .amount {
    width: 96px;
  }
  .cur {
    width: 58px;
  }
  .due {
    width: 128px;
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
  td.amount input {
    text-align: right;
  }
  /* What reading changed, as in Rename documents… */
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
