<script lang="ts">
  // Editable details for one document. Saving writes into the file (PDF) or
  // its sidecar, so it only happens when the user presses Save.
  import { untrack } from "svelte";
  import { api, type DocMeta, type Entry, type MetaRecord } from "./api";
  import Icon from "./Icon.svelte";
  import NotesBox from "#lib/NotesBox.svelte";
  import { m } from "#lib/i18n/index.ts";
  import { formatAmountInput, formatIsoDay, formatMoney, parseAmount, tidyAmountInput } from "#lib/format.ts";
  import { CURRENCIES, senderList } from "#lib/details.svelte.ts";

  let { sourceId, entry }: { sourceId: string; entry: Entry } = $props();

  type Form = {
    title: string;
    documentDate: string;
    tags: string[];
    notes: string;
    favorite: boolean;
    sender: string;
    amount: string;
    currency: string;
    dueDate: string;
  };

  let record = $state<MetaRecord | null>(null);
  let available = $state(true);
  let loading = $state(false);
  let saving = $state(false);
  let error = $state<string | null>(null);
  let form = $state<Form>(empty());
  let saved = $state<Form>(empty());
  let tagDraft = $state("");

  const dirty = $derived(JSON.stringify(form) !== JSON.stringify(saved) || tagDraft.trim() !== "");

  function empty(): Form {
    return {
      title: "",
      documentDate: "",
      tags: [],
      notes: "",
      favorite: false,
      sender: "",
      amount: "",
      currency: "",
      dueDate: "",
    };
  }

  function toForm(d: DocMeta): Form {
    return {
      title: d.title ?? "",
      documentDate: d.documentDate ?? "",
      tags: [...d.tags],
      notes: d.notes ?? "",
      favorite: d.favorite,
      sender: d.sender ?? "",
      amount: formatAmountInput(d.amount),
      currency: d.currency ?? "",
      dueDate: d.dueDate ?? "",
    };
  }

  function apply(r: MetaRecord) {
    record = r;
    form = toForm(r.meta);
    saved = toForm(r.meta);
    tagDraft = "";
  }

  async function load(allowDownload = false) {
    const key = entry.relPath;
    loading = true;
    error = null;
    try {
      const res = await api.readMeta(sourceId, key, allowDownload);
      if (key !== entry.relPath) return; // selection moved on
      available = res.available;
      if (res.record) apply(res.record);
      else {
        record = null;
        form = empty();
        saved = empty();
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  senderList.ensure();

  // Reload only when a different file is selected, not when the folder
  // listing refreshes (which replaces the `entry` object).
  let loadedFor = "";
  $effect(() => {
    const key = `${sourceId}\n${entry.relPath}`;
    if (key !== loadedFor) {
      loadedFor = key;
      load();
    }
  });

  // The star saves at once, here as on the document in the list. When the
  // list's star changes this document, follow it.
  let seenFavorite: boolean | undefined;
  $effect(() => {
    const f = (entry as Entry & { favorite?: boolean }).favorite;
    const before = seenFavorite;
    seenFavorite = f;
    if (f === undefined || before === undefined || f === before) return;
    untrack(() => {
      saved.favorite = f;
      form.favorite = f;
      refreshStorage();
    });
  });

  // Starring can move details kept beside a PDF into it; show where they
  // are now, leaving any unsaved edits in the form alone.
  async function refreshStorage() {
    const key = entry.relPath;
    try {
      const res = await api.readMeta(sourceId, key, false);
      if (key === entry.relPath && res.record) record = res.record;
    } catch {
      // The storage line just stays as it was.
    }
  }

  let starring = $state(false);
  async function toggleStar() {
    starring = true;
    error = null;
    try {
      const meta = await api.setFavorite(sourceId, entry.relPath, !form.favorite);
      saved.favorite = meta.favorite;
      form.favorite = meta.favorite;
      refreshStorage();
    } catch (e) {
      error = String(e);
    } finally {
      starring = false;
    }
  }

  function addTag() {
    const parts = tagDraft.split(",").map((t) => t.trim()).filter(Boolean);
    for (const t of parts) {
      if (!form.tags.some((x) => x.toLowerCase() === t.toLowerCase())) form.tags.push(t);
    }
    tagDraft = "";
  }

  function removeTag(i: number) {
    form.tags.splice(i, 1);
  }

  async function save() {
    addTag();
    saving = true;
    error = null;
    try {
      const meta: DocMeta = {
        id: record?.meta.id ?? null,
        title: form.title || null,
        tags: form.tags,
        notes: form.notes || null,
        documentDate: form.documentDate || null,
        favorite: form.favorite,
        sender: form.sender.trim() || null,
        amount: form.amount.trim() ? (parseAmount(form.amount) ?? form.amount.trim()) : null,
        currency: form.amount.trim() ? form.currency.trim() || null : null,
        dueDate: form.dueDate || null,
      };
      apply(await api.writeMeta(sourceId, entry.relPath, meta));
      senderList.refresh();
      available = true;
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  function revert() {
    form = JSON.parse(JSON.stringify(saved));
    tagDraft = "";
  }

  function onkeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === "s") {
      e.preventDefault();
      if (dirty && !saving) save();
    }
  }

  const storageText = $derived(
    record?.storage === "embedded"
      ? m.details.inPdf
      : record?.storage === "sidecar"
        ? m.details.inSidecar
        : m.details.noneYet,
  );
</script>

{#if !available}
  <div class="offline">
    <Icon name="cloud" size={16} />
    <p>{m.details.onlineOnly}</p>
    {#if record}
      {@const known = record.meta}
      <p class="beside">
        {m.details.alreadyKnown}
        {[
          known.title,
          known.documentDate && m.details.dated(known.documentDate),
          known.favorite && m.details.favorite,
          known.tags.length && known.tags.map((t) => `#${t}`).join(" "),
          known.sender && m.details.fromWho(known.sender),
          known.amount && formatMoney(known.amount, known.currency),
          known.dueDate && m.details.dueOn(formatIsoDay(known.dueDate)),
          known.notes,
        ]
          .filter(Boolean)
          .join(" · ")}
      </p>
    {/if}
    <button onclick={() => load(true)} disabled={loading}>
      {loading ? m.details.downloading : m.details.downloadAndShow}
    </button>
  </div>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="meta" onsubmit={(e) => { e.preventDefault(); save(); }} {onkeydown}>
    <div class="title-row">
      <label class="field grow">
        <span>{m.details.title}</span>
        <input bind:value={form.title} placeholder={entry.name} disabled={loading} />
      </label>
      <button
        type="button"
        class="star"
        class:on={form.favorite}
        title={form.favorite ? m.details.removeFavorite : m.details.markFavorite}
        aria-pressed={form.favorite}
        disabled={loading || starring}
        onclick={toggleStar}>{form.favorite ? "★" : "☆"}</button
      >
    </div>

    <label class="field">
      <span>{m.details.documentDate}</span>
      <input
        type="date"
        class:unset={!form.documentDate}
        bind:value={form.documentDate}
        disabled={loading}
      />
    </label>

    <!-- #108: who it's from, the amount on it, and when it's due. -->
    <label class="field">
      <span>{m.details.sender}</span>
      <input bind:value={form.sender} list="rupert-senders" placeholder={m.details.senderHint} disabled={loading} />
    </label>
    <div class="money">
      <label class="field grow">
        <span>{m.details.amount}</span>
        <input
          bind:value={form.amount}
          inputmode="decimal"
          placeholder={formatAmountInput("0.00")}
          disabled={loading}
          onblur={() => (form.amount = tidyAmountInput(form.amount))}
        />
      </label>
      <label class="field currency">
        <span>{m.details.currency}</span>
        <input
          bind:value={form.currency}
          list="rupert-currencies"
          maxlength="3"
          placeholder="EUR"
          disabled={loading || !form.amount.trim()}
          oninput={() => (form.currency = form.currency.toUpperCase())}
        />
      </label>
    </div>
    <label class="field">
      <span>{m.details.dueDate}</span>
      <input type="date" class:unset={!form.dueDate} bind:value={form.dueDate} disabled={loading} />
    </label>
    <datalist id="rupert-senders">
      {#each senderList.names as name (name)}<option value={name}></option>{/each}
    </datalist>
    <datalist id="rupert-currencies">
      {#each CURRENCIES as c (c)}<option value={c}></option>{/each}
    </datalist>

    <div class="field">
      <span id="tags-label">{m.details.tags}</span>
      <div class="tags" role="group" aria-labelledby="tags-label">
        {#each form.tags as tag, i (tag)}
          <span class="tag">
            {tag}
            <button type="button" aria-label={m.details.removeTag(tag)} onclick={() => removeTag(i)}>×</button>
          </span>
        {/each}
        <input
          class="tag-input"
          bind:value={tagDraft}
          placeholder={form.tags.length ? "" : m.details.addTags}
          disabled={loading}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === ",") {
              e.preventDefault();
              addTag();
            } else if (e.key === "Backspace" && !tagDraft && form.tags.length) {
              form.tags.pop();
            }
          }}
          onblur={addTag}
        />
      </div>
    </div>

    <label class="field">
      <span>{m.details.notes}</span>
      <NotesBox bind:value={form.notes} rows={3} disabled={loading} />
    </label>

    {#if error}<p class="error">{error}</p>{/if}

    <div class="actions">
      <button type="submit" class="primary" disabled={!dirty || saving || loading}>
        {saving ? m.details.saving : m.common.save}
      </button>
      <button type="button" onclick={revert} disabled={!dirty || saving}>{m.details.revert}</button>
    </div>

    <p class="storage">
      {loading ? m.details.reading : storageText}
      {#if record?.note}<br /><span class="note">{record.note}</span>{/if}
    </p>
  </form>
{/if}

<style>
  .meta {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .field > span {
    font-size: 11px;
    color: var(--muted);
  }
  .grow {
    flex: 1;
    min-width: 0;
  }
  input,
  .field :global(textarea) {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 7px;
    width: 100%;
  }
  input:focus,
  .field :global(textarea:focus),
  .tags:focus-within {
    outline: 2px solid var(--accent-soft);
    border-color: var(--accent);
  }
  .money {
    display: flex;
    gap: 6px;
    align-items: flex-end;
  }
  .currency {
    width: 64px;
    flex: none;
  }
  .title-row {
    display: flex;
    gap: 6px;
    align-items: flex-end;
  }
  .star {
    border: 0;
    background: none;
    font-size: 20px;
    line-height: 1;
    padding: 4px;
    color: var(--muted);
  }
  .star.on {
    color: #d9a520;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    padding: 1px 4px 1px 8px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-size: 12px;
  }
  .tag button {
    border: 0;
    background: none;
    padding: 0 3px;
    color: inherit;
    line-height: 1;
  }
  .tag-input {
    flex: 1;
    min-width: 60px;
    border: 0;
    padding: 2px 4px;
    outline: none !important;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    border-radius: 6px;
    padding: 4px 10px;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  button:disabled {
    opacity: 0.5;
  }
  /* WebKit draws today's date in an empty date field; hide it until focused
     so an unset date doesn't look like a real one. */
  input.unset:not(:focus)::-webkit-datetime-edit,
  input.unset:not(:focus)::-webkit-datetime-edit-fields-wrapper,
  input.unset:not(:focus)::-webkit-datetime-edit-month-field,
  input.unset:not(:focus)::-webkit-datetime-edit-day-field,
  input.unset:not(:focus)::-webkit-datetime-edit-year-field,
  input.unset:not(:focus)::-webkit-datetime-edit-text {
    color: transparent;
  }
  .beside {
    font-size: 12px;
    color: var(--muted);
  }
  .storage {
    margin: 0;
    font-size: 11px;
    color: var(--muted);
  }
  .note {
    font-style: italic;
  }
  .error {
    margin: 0;
    color: var(--danger);
  }
  .offline {
    display: flex;
    flex-direction: column;
    gap: 8px;
    color: var(--cloud);
  }
  .offline p {
    margin: 0;
    color: var(--text);
  }
  .offline button {
    align-self: flex-start;
  }
</style>
