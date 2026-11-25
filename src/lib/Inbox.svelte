<script lang="ts">
  // The inbox: new scans, their recognized text and Local AI's suggestion,
  // reviewed and filed one at a time. The waiting items fill the middle (a
  // search field, and a list, large icons or a gallery); the right column
  // has the selected item's pages above the File it form (#59).
  import { onMount, tick, untrack } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    api,
    fileNameFor,
    type Entry,
    type FileKind,
    onIntakeChanged,
    revealLabel,
    type Filed,
    type FolderChoice,
    type IntakeItem,
    type Settings,
    type Source,
  } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import Thumb from "#lib/Thumb.svelte";
  import FolderPicker from "#lib/FolderPicker.svelte";
  import FolderPickerDialog from "#lib/FolderPickerDialog.svelte";
  import { crumbs, shortestEndings } from "#lib/folders.ts";
  import { formatDate, formatDay } from "#lib/format.ts";
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

  // MARK: View and search

  type ViewMode = "list" | "icons" | "gallery";
  const VIEW_KEY = "rupert.inboxView";
  let viewMode = $state<ViewMode>(loadViewMode());
  let query = $state("");
  /** Items whose recognized text matches `query`, from the index. */
  let textMatches = $state<Set<string> | null>(null);
  let listBox: HTMLElement | undefined = $state();

  function loadViewMode(): ViewMode {
    try {
      const saved = localStorage.getItem(VIEW_KEY);
      return saved === "icons" || saved === "gallery" ? saved : "list";
    } catch {
      return "list";
    }
  }

  function setViewMode(mode: ViewMode) {
    viewMode = mode;
    try {
      localStorage.setItem(VIEW_KEY, mode);
    } catch {
      /* not essential */
    }
  }

  /** Every word of the query in the name, suggested title, tags or folder. */
  function matchesHere(i: IntakeItem, words: string[]): boolean {
    const hay = [i.name, i.suggestion?.title ?? "", ...(i.suggestion?.tags ?? []), i.suggestedFolder?.label ?? ""]
      .join(" ")
      .toLocaleLowerCase();
    return words.every((w) => hay.includes(w));
  }

  const visible = $derived.by(() => {
    const words = query.toLocaleLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return items;
    return items.filter((i) => matchesHere(i, words) || textMatches?.has(i.relPath));
  });

  // The recognized text is searched in the index, a moment after typing.
  $effect(() => {
    const q = query.trim();
    textMatches = null;
    if (!q) return;
    let live = true;
    const timer = setTimeout(() => {
      api
        .inboxSearch(q)
        .then((paths) => live && (textMatches = new Set(paths)))
        .catch(() => {});
    }, 150);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  });

  // Keep a visible item selected while searching.
  $effect(() => {
    const shown = visible;
    untrack(() => {
      if (shown.length && !shown.some((i) => i.relPath === selectedRel)) selectedRel = shown[0].relPath;
    });
  });

  // Keep the selection in view.
  $effect(() => {
    const rel = selectedRel;
    void viewMode;
    if (!rel || !listBox) return;
    tick().then(() =>
      listBox?.querySelector(".selected")?.scrollIntoView({ block: "nearest", inline: "nearest" }),
    );
  });

  /** A folder in short for the list: the source (unless it's the only
   * one), then the last two folders ("Boating/…/Sol y Mar/Purchase"); the
   * full path goes in a tooltip. */
  function shortFolder(f: FolderChoice): string {
    const parts = crumbs(f, sources);
    if (parts.length <= 3) return parts.join("/");
    return sources.length > 1 ? [parts[0], "…", ...parts.slice(-2)].join("/") : ["…", ...parts.slice(-2)].join("/");
  }
  /** Browse… (the folder tree) is open. */
  let browsing = $state(false);

  /** What the thumbnails need to know about an item. */
  function entryOf(i: IntakeItem): Entry {
    const ext = i.name.slice(i.name.lastIndexOf(".") + 1).toLowerCase();
    const kind: FileKind =
      ext === "pdf"
        ? "pdf"
        : ["jpg", "jpeg", "png", "heic", "heif", "tif", "tiff", "gif", "webp", "bmp"].includes(ext)
          ? "image"
          : "other";
    return { name: i.name, relPath: i.relPath, isDir: false, kind, size: i.size, modifiedMs: i.modifiedMs, cloud: i.cloud };
  }

  /** The item `step` places from the selected one among those shown. */
  function selectBy(step: number) {
    if (!visible.length) return;
    const at = visible.findIndex((i) => i.relPath === selectedRel);
    const next = Math.min(visible.length - 1, Math.max(0, (at < 0 ? -step : at) + step));
    selectedRel = visible[next].relPath;
  }

  /** The item to select after `rel` leaves the inbox: the next one shown. */
  function nextAfter(rel: string): string | null {
    const at = visible.findIndex((i) => i.relPath === rel);
    const rest = visible.filter((i) => i.relPath !== rel);
    return rest[Math.min(Math.max(at, 0), rest.length - 1)]?.relPath ?? null;
  }

  /** The selected item, while the search shows it. */
  const selected = $derived(visible.find((i) => i.relPath === selectedRel) ?? null);
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

  // MARK: Preview

  /** The page shown, and how many there are (null until known). */
  let page = $state(0);
  let pages = $state<number | null>(null);

  $effect(() => {
    const item = selected;
    page = 0;
    pages = null;
    if (!item || item.cloud === "cloudOnly" || entryOf(item).kind !== "pdf") return;
    let live = true;
    api
      .pageCount(item.sourceId, item.relPath)
      .then((n) => live && (pages = n))
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  function flip(step: number) {
    if (pages && page + step >= 0 && page + step < pages) page += step;
  }

  // Preview when the selection or page changes (or the file changed, e.g.
  // after OCR).
  $effect(() => {
    const item = selected;
    const key = item ? `${item.relPath}\n${item.size}\n${page}` : null;
    const p = page;
    untrack(() => loadPreview(item, key, p));
  });
  let previewKey: string | null = null;
  async function loadPreview(item: IntakeItem | null, key: string | null, p: number) {
    if (key === previewKey) return;
    previewKey = key;
    if (!item || item.cloud === "cloudOnly") {
      previewUrl = null;
      return;
    }
    previewLoading = true;
    try {
      const url = await api.preview(item.sourceId, item.relPath, p);
      if (previewKey === key) previewUrl = url;
    } catch {
      if (previewKey === key) previewUrl = null;
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
      selectedRel = nextAfter(item.relPath);
      items = items.filter((i) => i.relPath !== item.relPath);
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
      if (selectedRel === item.relPath) {
        selectedRel = nextAfter(item.relPath);
        formFor = null;
      }
      items = items.filter((i) => i.relPath !== item.relPath);
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
    } else if (!typing && !menu && !e.metaKey && !e.ctrlKey && !e.altKey) {
      arrow(e);
    }
  }

  /** Arrow keys: the next or previous item; in the gallery, up and down
   * (and Page Up/Down) flip pages. */
  function arrow(e: KeyboardEvent) {
    let step = 0;
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      if (viewMode === "gallery") {
        e.preventDefault();
        flip(e.key === "ArrowDown" ? 1 : -1);
        return;
      }
      step = (viewMode === "icons" ? columns() : 1) * (e.key === "ArrowDown" ? 1 : -1);
    } else if ((e.key === "ArrowRight" || e.key === "ArrowLeft") && viewMode !== "list") {
      step = e.key === "ArrowRight" ? 1 : -1;
    } else if (e.key === "PageDown" || e.key === "PageUp") {
      e.preventDefault();
      flip(e.key === "PageDown" ? 1 : -1);
      return;
    } else return;
    e.preventDefault();
    selectBy(step);
  }

  /** Tiles per row in the large icons. */
  function columns(): number {
    const first = listBox?.querySelector(".tile") as HTMLElement | null;
    if (!listBox || !first) return 1;
    return Math.max(1, Math.round(listBox.clientWidth / first.offsetWidth));
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
  <div class="inbox" class:gallery={viewMode === "gallery"}>
    <!-- MARK: Items -->
    <section class="items">
      <header>
        <div class="title-row">
          <h2>{m.inbox.title}</h2>
          <span class="muted small path" title={settings.inbox.relPath}>
            {inboxSource.name}{settings.inbox.relPath ? "/" + settings.inbox.relPath : ""}
          </span>
          <span class="spacer"></span>
          <button class="scan-btn" onclick={onscan}>{m.inbox.scan}</button>
        </div>
        <div class="toolbar">
          <label class="search">
            <Icon name="search" size={14} />
            <input
              type="search"
              bind:value={query}
              placeholder={m.inbox.searchPlaceholder}
              aria-label={m.inbox.searchPlaceholder}
              onkeydown={(e) => {
                if (e.key === "Escape" && query) {
                  e.stopPropagation();
                  query = "";
                }
              }}
            />
          </label>
          <span class="view-switch" role="group" aria-label={m.main.view}>
            {#each [["list", "view-list", m.main.list], ["icons", "view-grid", m.main.largeIcons], ["gallery", "view-gallery", m.main.gallery]] as const as [mode, icon, label] (mode)}
              <button class:on={viewMode === mode} onclick={() => setViewMode(mode)} title={label} aria-label={label} aria-pressed={viewMode === mode}>
                <Icon name={icon} size={16} />
              </button>
            {/each}
          </span>
        </div>
      </header>
      {#if lastFiled}
        <div class="filed">
          {m.inbox.filedIn(lastFiled.label)}
          <button class="link" onclick={() => onfiled(lastFiled!.filed)}>{m.inbox.show}</button>
          <button class="link" onclick={() => (lastFiled = null)} aria-label={m.inbox.dismiss}>×</button>
        </div>
      {/if}

      {#if items.length === 0}
        <p class="muted pad">{m.inbox.nothingWaiting}</p>
      {:else if visible.length === 0}
        <p class="muted pad">{m.inbox.noMatches(query.trim())}</p>
      {:else if viewMode === "list"}
        <div class="scroll" bind:this={listBox}>
          <table>
            <thead>
              <tr>
                <th class="c-name">{m.main.name}</th>
                <th class="c-suggestion">{m.inbox.suggestion}</th>
                <th>{m.inbox.folder}</th>
                <th class="c-when">{m.main.modified}</th>
              </tr>
            </thead>
            <tbody>
              {#each visible as i (i.relPath)}
                <tr
                  class:selected={i.relPath === selectedRel}
                  onclick={() => (selectedRel = i.relPath)}
                  ondblclick={() => api.openEntry(i.sourceId, i.relPath)}
                  oncontextmenu={(e) => openMenu(i, e)}
                >
                  <td class="name">
                    <Icon name={entryOf(i).kind === "pdf" ? "pdf" : "file"} size={15} />
                    <span title={i.name}>{i.name}</span>
                  </td>
                  <td class="state" class:busy={i.stage !== "ready"} title={stageText(i)}>{stageText(i)}</td>
                  <td class="muted" title={i.suggestedFolder?.label}>{i.suggestedFolder ? shortFolder(i.suggestedFolder) : ""}</td>
                  <td class="muted when" title={formatDate(i.modifiedMs)}>{formatDay(i.modifiedMs)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else if viewMode === "icons"}
        <div class="scroll grid" bind:this={listBox}>
          {#each visible as i (i.relPath)}
            <button
              class="tile"
              class:selected={i.relPath === selectedRel}
              onclick={() => (selectedRel = i.relPath)}
              ondblclick={() => api.openEntry(i.sourceId, i.relPath)}
              oncontextmenu={(e) => openMenu(i, e)}
              title={i.suggestion?.title ? `${i.suggestion.title}\n${i.name}` : i.name}
            >
              <span class="pic"><Thumb sourceId={i.sourceId} entry={entryOf(i)} side={320} iconSize={48} /></span>
              <span class="label">{i.suggestion?.title ?? i.name}</span>
              <span class="label muted small" class:busy={i.stage !== "ready"}>
                {i.stage === "ready" ? i.name : stageText(i)}
              </span>
            </button>
          {/each}
        </div>
      {:else}
        <div class="stage">{@render pagePreview(true)}</div>
        <div class="strip" bind:this={listBox}>
          {#each visible as i (i.relPath)}
            <button
              class="frame"
              class:selected={i.relPath === selectedRel}
              onclick={() => (selectedRel = i.relPath)}
              ondblclick={() => api.openEntry(i.sourceId, i.relPath)}
              oncontextmenu={(e) => openMenu(i, e)}
              title={i.suggestion?.title ?? i.name}
            >
              <Thumb sourceId={i.sourceId} entry={entryOf(i)} side={200} iconSize={26} />
            </button>
          {/each}
        </div>
      {/if}
    </section>

    <!-- MARK: Right column -->
    <section class="side">
      {#if selected && viewMode !== "gallery"}
        <div class="preview">{@render pagePreview(false)}</div>
      {/if}
      <!-- MARK: Filing form -->
      <div class="form">
      {#if selected}
        <h2>{m.inbox.fileIt}</h2>
        {#if selected.stage !== "ready"}
          <p class="working"><span class="spinner"></span>{stageText(selected)}</p>
        {/if}
        {#if selected.ocr}<p class="muted small">{selected.ocr}.</p>{/if}
        {#if selected.note}<p class="note small"><Icon name="warning" size={12} /> {selected.note}</p>{/if}

        <label class="field">
          <span>{m.inbox.title_}</span>
          <input
            bind:value={form.title}
            oninput={onTitleOrDate}
            placeholder={selected.name}
            title={form.title.trim() || undefined}
          />
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
        <div class="field">
          <span>{m.inbox.folder}</span>
          <FolderPicker
            bind:value={form.folder}
            suggested={selected.folders}
            all={allFolders}
            {sources}
            onbrowse={() => (browsing = true)}
          />
        </div>
        {#if selected.folders.length}
          {@const ends = shortestEndings(selected.folders.slice(0, 4), sources)}
          <div class="choices">
            {#each selected.folders.slice(0, 4) as f (f.label)}
              <button
                class="choice"
                class:on={f.label === form.folder}
                onclick={() => (form.folder = f.label)}
                title={f.label}>{ends.get(f.label)}</button
              >
            {/each}
          </div>
        {/if}
        {#if selected.suggestion?.reason}<p class="muted small">{selected.suggestion.reason}</p>{/if}
        <label class="field">
          <span>{m.inbox.fileName}</span>
          <div class="name-row">
            <input
              bind:value={form.fileName}
              oninput={() => (nameEdited = true)}
              title={form.fileName.trim() ? form.fileName.trim() + ext : undefined}
            />
            <span class="muted">{ext}</span>
          </div>
        </label>
        <label class="field">
          <span>{m.inbox.notes}</span>
          <textarea bind:value={form.notes} rows="2" title={form.notes.trim() || undefined}></textarea>
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
      </div>
    </section>
  </div>
{/if}

<!-- MARK: Page preview -->
{#snippet pagePreview(big: boolean)}
  {#if selected}
    <div class="page" class:big role="presentation" ondblclick={() => api.openEntry(selected!.sourceId, selected!.relPath)}>
      {#if previewUrl}
        <img src={previewUrl} alt={m.inbox.firstPage(selected.name)} />
      {:else if selected.cloud === "cloudOnly"}
        <p class="muted center">{m.inbox.onlineOnly}</p>
      {:else if previewLoading}
        <p class="muted center">{m.inbox.loadingPreview}</p>
      {:else}
        <p class="muted center">{m.inbox.noPreview}</p>
      {/if}
      {#if pages && pages > 1}
        <button class="flip prev" onclick={() => flip(-1)} disabled={page === 0} title={m.pictures.previousPageKey} aria-label={m.pictures.previousPage}>
          <Icon name="back" size={18} />
        </button>
        <button class="flip next" onclick={() => flip(1)} disabled={page + 1 >= pages} title={m.pictures.nextPageKey} aria-label={m.pictures.nextPage}>
          <Icon name="chevron" size={18} />
        </button>
      {/if}
    </div>
    {#if pages && pages > 1}<p class="muted small page-of">{m.pictures.pageOf(page + 1, pages)}</p>{/if}
  {/if}
{/snippet}

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


{#if browsing && selected}
  {@const current = [...selected.folders, ...allFolders].find((f) => f.label === form.folder) ?? null}
  <FolderPickerDialog
    {sources}
    {current}
    title={form.title}
    onchoose={(f) => {
      if (!allFolders.some((x) => x.label === f.label)) allFolders = [...allFolders, f];
      form.folder = f.label;
      browsing = false;
    }}
    onclose={() => (browsing = false)}
  />
{/if}
<style>
  .inbox {
    display: grid;
    grid-template-columns: minmax(0, 1fr) var(--inbox-side, 380px);
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
    padding: 16px 4px;
  }
  .center {
    text-align: center;
    margin-top: 30%;
  }
  .busy {
    font-style: italic;
  }
  /* MARK: Items */
  .items {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    padding: 10px 14px 12px;
    gap: 8px;
  }
  .items header {
    display: grid;
    gap: 8px;
  }
  .title-row,
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .scan-btn {
    flex: none;
    font-size: 12px;
    padding: 2px 10px;
  }
  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0 8px;
    background: var(--bg);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    border: 0;
    padding: 4px 0;
    background: none;
  }
  .search input:focus {
    outline: none;
  }
  .view-switch {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 6px;
    overflow: hidden;
  }
  .view-switch button {
    border: 0;
    border-radius: 0;
    padding: 3px 8px;
    color: var(--muted);
    display: grid;
    place-items: center;
  }
  .view-switch button.on {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  table {
    width: 100%;
    table-layout: fixed;
    border-collapse: collapse;
    font-size: 13px;
  }
  th.c-name {
    width: 28%;
  }
  th.c-suggestion {
    width: 32%;
  }
  th.c-when {
    width: 110px;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--bg);
    text-align: left;
    font-weight: 500;
    font-size: 12px;
    color: var(--muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  td {
    padding: 5px 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  td.name :global(svg) {
    vertical-align: -3px;
    margin-right: 6px;
    color: var(--muted);
  }
  tbody tr:hover {
    background: var(--hover);
  }
  tbody tr.selected {
    background: var(--accent-soft);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    align-content: start;
    gap: 10px;
    padding: 2px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 8px 6px;
    border: 0;
    border-radius: 8px;
    background: none;
    min-width: 0;
  }
  .tile:hover {
    background: var(--hover);
  }
  .tile.selected {
    background: var(--accent-soft);
  }
  .pic {
    position: relative;
    width: 110px;
    height: 140px;
    margin-bottom: 2px;
  }
  .label {
    max-width: 100%;
    font-size: 12px;
    line-height: 1.3;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .strip {
    flex: none;
    display: flex;
    gap: 8px;
    overflow-x: auto;
    padding: 4px 2px;
  }
  .frame {
    flex: none;
    width: 64px;
    height: 84px;
    padding: 3px;
    border: 2px solid transparent;
    border-radius: 6px;
    background: none;
    position: relative;
  }
  .frame.selected {
    border-color: var(--accent);
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
  }
  .link {
    border: 0;
    background: none;
    color: var(--accent-strong);
    padding: 0 2px;
    text-decoration: underline;
  }
  /* MARK: Right column */
  .side {
    border-left: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: auto;
  }
  .preview {
    flex: 1 1 auto;
    min-height: 220px;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: var(--panel);
    padding: 12px 12px 6px;
    border-bottom: 1px solid var(--border);
  }
  .page {
    position: relative;
    flex: 1;
    min-height: 0;
    width: 100%;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .page img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    box-shadow: 0 2px 12px rgb(0 0 0 / 0.18);
    background: #fff;
  }
  .page.big {
    background: var(--panel);
    border-radius: 8px;
    padding: 12px;
  }
  .page-of {
    margin-top: 4px;
  }
  .flip {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 32px;
    height: 32px;
    border-radius: 50%;
    padding: 0;
    display: grid;
    place-items: center;
    opacity: 0.85;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.2);
  }
  .flip:disabled {
    opacity: 0.25;
  }
  .flip.prev {
    left: 4px;
  }
  .flip.next {
    right: 4px;
  }
  .form {
    flex: none;
    /* Room under the last line for the Ask Rupert button. */
    padding: 12px 14px 64px;
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
