<script lang="ts">
  // The inbox: new scans, their recognized text and LM Studio's suggestion,
  // reviewed and filed one at a time.
  import { onMount, untrack } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    api,
    fileNameFor,
    onIntakeChanged,
    revealLabel,
    type Filed,
    type FolderChoice,
    type IntakeItem,
    type Settings,
    type Source,
  } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import { m } from "#lib/i18n/index.ts";

  let {
    sources,
    settings,
    onfiled,
    onsettings,
    onscan,
  }: {
    sources: Source[];
    settings: Settings;
    onfiled: (filed: Filed) => void;
    onsettings: () => void;
    onscan: () => void;
  } = $props();

  type Form = {
    title: string;
    documentDate: string;
    tags: string[];
    notes: string;
    favorite: boolean;
    folder: string; // label
    fileName: string;
  };

  let items = $state<IntakeItem[]>([]);
  let selectedRel = $state<string | null>(null);
  let previewUrl = $state<string | null>(null);
  let previewLoading = $state(false);
  let allFolders = $state<FolderChoice[]>([]);
  let form = $state<Form>(blank());
  let formFor = $state<string | null>(null); // "rel\nsuggestion model" the form was filled from
  let nameEdited = $state(false);
  let tagDraft = $state("");
  let filing = $state(false);
  let error = $state<string | null>(null);
  let lastFiled = $state<{ filed: Filed; label: string } | null>(null);
  let os = $state<string | undefined>();
  /** The right-click menu on a scan in the list. */
  let menu = $state<{ x: number; y: number; item: IntakeItem } | null>(null);
  const trashLabel = $derived(os === "windows" ? m.trash.delete : m.trash.moveToTrash);
  const binName = $derived(os === "windows" ? m.trash.recycleBin : m.trash.trash);

  const selected = $derived(items.find((i) => i.relPath === selectedRel) ?? null);
  const inboxSource = $derived(sources.find((s) => s.id === settings.inbox?.sourceId) ?? null);
  const folderChoice = $derived(
    [...(selected?.folders ?? []), ...allFolders].find((f) => f.label === form.folder.trim()) ?? null,
  );
  const ext = $derived(selected?.name.includes(".") ? selected.name.slice(selected.name.lastIndexOf(".")) : "");

  function blank(): Form {
    return { title: "", documentDate: "", tags: [], notes: "", favorite: false, folder: "", fileName: "" };
  }

  async function load() {
    items = await api.inboxItems();
    if (!selectedRel || !items.some((i) => i.relPath === selectedRel)) {
      selectedRel = items[0]?.relPath ?? null;
    }
  }

  onMount(() => {
    load();
    api.appInfo().then((i) => (os = i.os));
    api.allFolders().then((f) => (allFolders = f));
    const unlisten = onIntakeChanged(load);
    return () => {
      unlisten.then((f) => f());
    };
  });

  // Preview when the selection changes (or the file changed, e.g. after OCR).
  $effect(() => {
    const item = selected;
    const key = item ? `${item.relPath}\n${item.size}` : null;
    untrack(() => loadPreview(item, key));
  });
  let previewKey: string | null = null;
  async function loadPreview(item: IntakeItem | null, key: string | null) {
    if (key === previewKey) return;
    previewKey = key;
    previewUrl = null;
    if (!item || item.cloud === "cloudOnly") return;
    previewLoading = true;
    try {
      const url = await api.preview(item.sourceId, item.relPath);
      if (previewKey === key) previewUrl = url;
    } catch {
      /* no preview */
    } finally {
      previewLoading = false;
    }
  }

  // Fill the form from the suggestion, once per document (and again when a
  // new suggestion arrives), without overwriting what the user typed.
  $effect(() => {
    const item = selected;
    const key = item ? `${item.relPath}\n${item.suggestion?.model ?? ""}\n${item.stage}` : null;
    untrack(() => {
      if (!item || key === formFor) return;
      const sameDoc = formFor?.split("\n")[0] === item.relPath;
      formFor = key;
      if (sameDoc && item.stage !== "ready") return;
      const s = item.suggestion;
      if (!sameDoc) {
        form = blank();
        nameEdited = false;
        error = null;
      }
      form.title ||= s?.title ?? "";
      form.documentDate ||= s?.documentDate ?? "";
      if (!form.tags.length) form.tags = [...(s?.tags ?? [])];
      form.folder ||= item.suggestedFolder?.label ?? "";
      if (!nameEdited) form.fileName = defaultName();
    });
  });

  function defaultName() {
    const stem = selected?.name.replace(/\.[^.]+$/, "") ?? "";
    return form.title.trim() ? fileNameFor(settings.namePattern, form.documentDate || null, form.title.trim()) : stem;
  }

  function onTitleOrDate() {
    if (!nameEdited) form.fileName = defaultName();
  }

  function addTag() {
    const t = tagDraft.replace(/,/g, " ").trim();
    if (t && !form.tags.some((x) => x.toLowerCase() === t.toLowerCase())) form.tags = [...form.tags, t];
    tagDraft = "";
  }

  async function file() {
    if (!selected || !folderChoice) return;
    filing = true;
    error = null;
    const item = selected;
    const destination = folderChoice;
    try {
      const filed = await api.fileDocument(item.sourceId, item.relPath, {
        title: form.title.trim() || null,
        documentDate: form.documentDate || null,
        tags: form.tags,
        notes: form.notes.trim() || null,
        favorite: form.favorite,
        destSourceId: destination.sourceId,
        destDir: destination.dir,
        fileName: form.fileName.trim(),
      });
      lastFiled = { filed, label: destination.label };
      const next = items.findIndex((i) => i.relPath === item.relPath);
      items = items.filter((i) => i.relPath !== item.relPath);
      selectedRel = items[Math.min(next, items.length - 1)]?.relPath ?? null;
      formFor = null;
    } catch (e) {
      error = String(e);
    } finally {
      filing = false;
    }
  }

  function stageText(i: IntakeItem): string {
    switch (i.stage) {
      case "waiting":
        return m.inbox.waiting;
      case "recognizing":
        return i.progress ? m.inbox.readingPage(i.progress[0] + 1, i.progress[1]) : m.inbox.reading;
      case "suggesting":
        return m.inbox.asking;
      default:
        return i.suggestion?.title ?? (i.note ? m.inbox.needsInput : m.inbox.ready);
    }
  }

  // MARK: Deleting

  /** Sends a scan to the Trash / Recycle Bin, after asking: a new scan may
   * not be saved anywhere else yet. Then the next one is selected. */
  async function trash(item: IntakeItem) {
    menu = null;
    const ok = await ask(
      m.trash.askScan(item.name, binName),
      { title: trashLabel, kind: "warning", okLabel: trashLabel },
    );
    if (!ok) return;
    try {
      error = null;
      await api.trashEntry(item.sourceId, item.relPath);
      const next = items.findIndex((i) => i.relPath === item.relPath);
      items = items.filter((i) => i.relPath !== item.relPath);
      if (selectedRel === item.relPath) {
        selectedRel = items[Math.min(next, items.length - 1)]?.relPath ?? null;
        formFor = null;
      }
    } catch (e) {
      error = String(e);
    }
  }

  function openMenu(item: IntakeItem, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    selectedRel = item.relPath;
    // Keep it inside the window (the menu is about 200 × 120).
    menu = { x: Math.min(e.clientX, window.innerWidth - 210), y: Math.min(e.clientY, window.innerHeight - 130), item };
  }

  function onkeydown(e: KeyboardEvent) {
    const typing =
      e.target instanceof Element && e.target.closest("input, textarea, select, [contenteditable]");
    if (e.key === "Escape" && menu) {
      menu = null;
    } else if ((e.metaKey || e.ctrlKey) && e.key === "Enter" && folderChoice && !filing) {
      e.preventDefault();
      file();
    } else if (
      !typing &&
      selected &&
      !filing &&
      (os === "macos" ? e.metaKey && e.key === "Backspace" : e.key === "Delete")
    ) {
      e.preventDefault();
      trash(selected);
    }
  }
</script>

<svelte:window {onkeydown} onclick={() => (menu = null)} />

{#if !settings.inbox || !inboxSource}
  <div class="empty-state">
    <h1>{m.inbox.title}</h1>
    <p class="muted">{m.inbox.setUpIntro}</p>
    <button class="primary" onclick={onsettings}>{m.inbox.setUp}</button>
  </div>
{:else}
  <div class="inbox">
    <!-- MARK: List -->
    <aside class="list">
      <header>
        <div class="title-row">
          <h2>{m.inbox.title}</h2>
          <button class="scan-btn" onclick={onscan}>{m.inbox.scan}</button>
        </div>
        <span class="muted small" title={settings.inbox.relPath}>
          {inboxSource.name}{settings.inbox.relPath ? "/" + settings.inbox.relPath : ""}
        </span>
      </header>
      {#if lastFiled}
        <div class="filed">
          {m.inbox.filedIn(lastFiled.label)}
          <button class="link" onclick={() => onfiled(lastFiled!.filed)}>{m.inbox.show}</button>
          <button class="link" onclick={() => (lastFiled = null)} aria-label={m.inbox.dismiss}>×</button>
        </div>
      {/if}
      {#each items as i (i.relPath)}
        <button
          class="item"
          class:selected={i.relPath === selectedRel}
          onclick={() => (selectedRel = i.relPath)}
          oncontextmenu={(e) => openMenu(i, e)}
        >
          <span class="name">{i.name}</span>
          <span class="state muted" class:busy={i.stage !== "ready"}>{stageText(i)}</span>
        </button>
      {:else}
        <p class="muted small pad">{m.inbox.nothingWaiting}</p>
      {/each}
    </aside>

    <!-- MARK: Preview -->
    <section class="preview">
      {#if selected}
        {#if previewUrl}
          <img src={previewUrl} alt={m.inbox.firstPage(selected.name)} />
        {:else if selected.cloud === "cloudOnly"}
          <p class="muted center">{m.inbox.onlineOnly}</p>
        {:else if previewLoading}
          <p class="muted center">{m.inbox.loadingPreview}</p>
        {:else}
          <p class="muted center">{m.inbox.noPreview}</p>
        {/if}
      {/if}
    </section>

    <!-- MARK: Filing form -->
    <section class="form">
      {#if selected}
        <h2>{m.inbox.fileIt}</h2>
        {#if selected.stage !== "ready"}
          <p class="working"><span class="spinner"></span>{stageText(selected)}</p>
        {/if}
        {#if selected.ocr}<p class="muted small">{selected.ocr}.</p>{/if}
        {#if selected.note}<p class="note small"><Icon name="warning" size={12} /> {selected.note}</p>{/if}

        <label class="field">
          <span>{m.inbox.title_}</span>
          <input bind:value={form.title} oninput={onTitleOrDate} placeholder={selected.name} />
        </label>
        <div class="two">
          <label class="field">
            <span>{m.inbox.documentDate}</span>
            <input
              type="date"
              class:unset={!form.documentDate}
              bind:value={form.documentDate}
              oninput={onTitleOrDate}
            />
          </label>
          <label class="field fav">
            <span>{m.inbox.favorite}</span>
            <button
              class="star"
              class:on={form.favorite}
              onclick={() => (form.favorite = !form.favorite)}
              aria-pressed={form.favorite}>★</button
            >
          </label>
        </div>
        <div class="field">
          <span>{m.inbox.tags}</span>
          <div class="tags">
            {#each form.tags as t (t)}
              <span class="chip"
                >{t}<button onclick={() => (form.tags = form.tags.filter((x) => x !== t))} aria-label={m.inbox.remove(t)}
                  >×</button
                ></span
              >
            {/each}
            <input
              bind:value={tagDraft}
              placeholder={m.inbox.addTag}
              onkeydown={(e) => {
                if (e.key === "Enter" || e.key === ",") {
                  e.preventDefault();
                  addTag();
                } else if (e.key === "Backspace" && !tagDraft && form.tags.length) {
                  form.tags = form.tags.slice(0, -1);
                }
              }}
              onblur={addTag}
            />
          </div>
        </div>
        <label class="field">
          <span>{m.inbox.folder}</span>
          <input
            list="inbox-folders"
            bind:value={form.folder}
            placeholder={m.inbox.findFolder}
            class:invalid={form.folder.trim() !== "" && !folderChoice}
          />
          <datalist id="inbox-folders">
            {#each selected.folders as f (f.label)}<option value={f.label}>{m.inbox.similar}</option>{/each}
            {#each allFolders as f (f.sourceId + f.dir)}<option value={f.label}></option>{/each}
          </datalist>
        </label>
        {#if selected.folders.length}
          <div class="choices">
            {#each selected.folders.slice(0, 4) as f (f.label)}
              <button class="choice" class:on={f.label === form.folder} onclick={() => (form.folder = f.label)}
                >{f.label.split("/").slice(-2).join("/")}</button
              >
            {/each}
          </div>
        {/if}
        {#if selected.suggestion?.reason}<p class="muted small">{selected.suggestion.reason}</p>{/if}
        <label class="field">
          <span>{m.inbox.fileName}</span>
          <div class="name-row">
            <input bind:value={form.fileName} oninput={() => (nameEdited = true)} />
            <span class="muted">{ext}</span>
          </div>
        </label>
        <label class="field">
          <span>{m.inbox.notes}</span>
          <textarea bind:value={form.notes} rows="2"></textarea>
        </label>

        {#if error}<p class="error small">{error}</p>{/if}
        <div class="actions">
          <button class="primary" disabled={!folderChoice || filing} onclick={file} title="⌘↩">
            {filing ? m.inbox.filing : m.inbox.file}
          </button>
          <button onclick={() => api.openEntry(selected!.sourceId, selected!.relPath)}>{m.inbox.open}</button>
          <button onclick={() => api.inboxRedo(selected!.relPath)} disabled={selected.stage !== "ready"}
            >{m.inbox.suggestAgain}</button
          >
          <span class="spacer"></span>
          <button class="danger" onclick={() => trash(selected!)} disabled={filing}>{m.common.ellipsis(trashLabel)}</button>
        </div>
        <p class="muted small">{m.inbox.whatFilingDoes}</p>
      {/if}
    </section>
  </div>
{/if}

{#if menu}
  {@const mn = menu}
  <div
    class="menu"
    role="menu"
    tabindex="-1"
    style:left="{mn.x}px"
    style:top="{mn.y}px"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <button role="menuitem" onclick={() => ((menu = null), api.openEntry(mn.item.sourceId, mn.item.relPath))}>{m.inbox.open}</button>
    <button role="menuitem" onclick={() => ((menu = null), api.revealEntry(mn.item.sourceId, mn.item.relPath))}
      >{revealLabel(os)}</button
    >
    <hr />
    <button role="menuitem" class="danger" onclick={() => trash(mn.item)}>{m.common.ellipsis(trashLabel)}</button>
  </div>
{/if}

<style>
  .inbox {
    display: grid;
    grid-template-columns: 230px 1fr 300px;
    height: 100%;
    min-height: 0;
  }
  .empty-state {
    max-width: 460px;
    margin: 80px auto;
    text-align: center;
  }
  .empty-state p {
    margin: 12px 0 20px;
    line-height: 1.5;
  }
  h2 {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .pad {
    padding: 8px;
  }
  .center {
    text-align: center;
    margin-top: 40%;
  }
  .list {
    border-right: 1px solid var(--border);
    overflow: auto;
    padding: 10px 6px;
  }
  .list header {
    padding: 0 6px 8px;
    display: grid;
    gap: 2px;
  }
  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .scan-btn {
    flex: none;
    font-size: 12px;
    padding: 2px 10px;
  }
  .item {
    display: grid;
    width: 100%;
    text-align: left;
    border: 0;
    background: none;
    border-radius: 6px;
    padding: 6px 8px;
    gap: 2px;
  }
  .item:hover {
    background: var(--hover);
  }
  .item.selected {
    background: var(--accent-soft);
  }
  .item .name,
  .item .state {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .item .state {
    font-size: 12px;
  }
  .item .state.busy {
    font-style: italic;
  }
  .filed {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 0 4px;
    font-size: 12px;
    background: var(--accent-soft);
    border-radius: 6px;
    padding: 6px 8px;
    margin: 0 2px 8px;
  }
  .link {
    border: 0;
    background: none;
    color: var(--accent-strong);
    padding: 0 2px;
    text-decoration: underline;
  }
  .preview {
    overflow: auto;
    background: var(--panel);
    padding: 16px;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .preview img {
    max-width: 100%;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.18);
    background: #fff;
  }
  .form {
    border-left: 1px solid var(--border);
    padding: 12px 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .field {
    display: grid;
    gap: 3px;
  }
  .field > span {
    font-size: 12px;
    color: var(--muted);
  }
  .two {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
  }
  input,
  textarea {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
    min-width: 0;
  }
  input.invalid {
    border-color: #c0392b;
  }
  input.unset:not(:focus)::-webkit-datetime-edit,
  input.unset:not(:focus)::-webkit-datetime-edit-fields-wrapper,
  input.unset:not(:focus)::-webkit-datetime-edit-month-field,
  input.unset:not(:focus)::-webkit-datetime-edit-day-field,
  input.unset:not(:focus)::-webkit-datetime-edit-year-field,
  input.unset:not(:focus)::-webkit-datetime-edit-text {
    color: transparent;
  }
  .star {
    border: 1px solid var(--border);
    background: var(--bg);
    border-radius: 6px;
    color: var(--muted);
    font-size: 15px;
    padding: 2px 10px;
  }
  .star.on {
    color: #d9a520;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 3px;
  }
  .tags input {
    border: 0;
    flex: 1;
    min-width: 60px;
    padding: 2px 4px;
  }
  .tags input:focus {
    outline: none;
  }
  .chip {
    background: var(--accent-soft);
    color: var(--accent-strong);
    border-radius: 10px;
    padding: 1px 4px 1px 8px;
    font-size: 12px;
    display: inline-flex;
    align-items: center;
  }
  .chip button {
    border: 0;
    background: none;
    color: inherit;
    padding: 0 3px;
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .choice {
    font-size: 12px;
    border: 1px solid var(--border);
    background: var(--bg);
    border-radius: 10px;
    padding: 1px 8px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .choice.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .name-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .name-row input {
    flex: 1;
  }
  .actions {
    display: flex;
    flex-wrap: wrap; /* longer labels in other languages */
    gap: 6px;
    margin-top: 4px;
  }
  .actions button {
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  button.danger {
    color: var(--danger);
  }
  /* The right-click menu on a scan. */
  .menu {
    position: fixed;
    z-index: 30;
    min-width: 190px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgb(0 0 0 / 0.15);
  }
  .menu button {
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 8px;
    border-radius: 5px;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .menu hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 4px 0;
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
  .working {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    margin: 0;
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
  .note {
    color: #b07a10;
    margin: 0;
  }
  .error {
    color: #c0392b;
  }
  p {
    margin: 0;
  }
</style>
