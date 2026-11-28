<script lang="ts">
  // The inbox: new scans, their recognized text and Local AI's suggestion,
  // reviewed and filed one at a time. The waiting items fill the middle (a
  // search field, and a list, large icons or a gallery); the right column
  // has the selected item's pages above the File it form (#59).
  import { onMount, tick, untrack } from "svelte";
  import { ask } from "@tauri-apps/plugin-dialog";
  import {
    api,
    atElement,
    fileNameFor,
    type Entry,
    type FileKind,
    onIntakeChanged,
    revealLabel,
    type Filed,
    type FolderChoice,
    type IntakeItem,
    type MoveBackItem,
    type Settings,
    type Source,
  } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import Thumb, { thumbUrl } from "#lib/Thumb.svelte";
  import FolderPicker from "#lib/FolderPicker.svelte";
  import FolderPickerDialog from "#lib/FolderPickerDialog.svelte";
  import PageViewer from "#lib/PageViewer.svelte";
  import PagePicture from "#lib/PagePicture.svelte";
  import PreviewPane from "#lib/PreviewPane.svelte";
  import NotesBox from "#lib/NotesBox.svelte";
  import FileAllDialog from "#lib/FileAllDialog.svelte";
  import { CURRENCIES, senderList } from "#lib/details.svelte.ts";
  import { crumbs, shortestEndings } from "#lib/folders.ts";
  import { dragOutSource } from "#lib/drag.svelte.ts";
  import { formatAmountInput, formatDate, formatDay, parseAmount, tidyAmountInput } from "#lib/format.ts";
  import { m, num } from "#lib/i18n/index.ts";
  import { dismissImport, importing, stopImport } from "#lib/inbox-import.svelte.ts";

  let {
    sources,
    settings,
    onfiled,
    onsettings,
    onscan,
    onimport,
    onshow,
    onfiling,
    onundo,
    onfiledmany,
    canUndo = false,
    filingUndone = 0,
  }: {
    sources: Source[];
    settings: Settings;
    onfiled: (filed: Filed) => void;
    onsettings: () => void;
    onscan: () => void;
    /** Import into Inbox… (#122). */
    onimport: () => void;
    /** Shows a document where it is in the library. */
    onshow: (sourceId: string, relPath: string) => void;
    /** A scan was filed (from `relPath` of `sourceId`, to `filed`), for Undo (#104). */
    onfiling?: (sourceId: string, relPath: string, filed: Filed, label: string) => void;
    /** Undoes the last filing (the notice's Undo). */
    onundo?: () => void;
    /** File All Sure Ones (#112) filed these, for one Undo. */
    onfiledmany?: (moves: MoveBackItem[]) => void;
    /** Whether the last filing can still be undone. */
    canUndo?: boolean;
    /** Goes up when a filing was undone, from here or Edit ▸ Undo. */
    filingUndone?: number;
  } = $props();

  // A filing undone (here or with Edit ▸ Undo): its notice goes.
  $effect(() => {
    if (filingUndone) {
      lastFiled = null;
      filedMany = 0;
    }
  });

  /** Import's summary: the documents that were already in the library. */
  let showDuplicates = $state(false);
  function placeOf(sourceId: string, relPath: string): string {
    const dir = relPath.includes("/") ? relPath.slice(0, relPath.lastIndexOf("/")) : "";
    const name = sources.find((s) => s.id === sourceId)?.name ?? "";
    return [name, ...(dir ? dir.split("/") : [])].join(" › ");
  }

  type Form = {
    title: string;
    documentDate: string;
    tags: string[];
    notes: string;
    favorite: boolean;
    /** #108 */
    sender: string;
    amount: string;
    currency: string;
    dueDate: string;
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

  // MARK: Sure ones (#112)

  /** How many File All filed last (its notice). */
  let filedMany = $state(0);
  let fileAllOpen = $state(false);
  /** File it: the filed documents the suggestion learned from, shown on click. */
  let showLearned = $state(false);
  // Another scan: its "learned from" starts folded again.
  $effect(() => {
    void selectedRel;
    showLearned = false;
  });
  /** Scans whose suggestion agrees with how similar documents were filed. */
  const sureItems = $derived(
    items.filter((i) => i.stage === "ready" && i.confidence?.sure && i.suggestedFolder && i.cloud !== "cloudOnly"),
  );

  /** Files each as its suggestion says (as File it would with the form
   * untouched), one after the other; one Undo takes them all back. */
  async function fileAll(chosen: IntakeItem[]) {
    const moves: MoveBackItem[] = [];
    const failed: string[] = [];
    for (const item of chosen) {
      const s = item.suggestion;
      const dest = item.suggestedFolder;
      if (!s || !dest) continue;
      try {
        const filed = await api.fileDocument(item.sourceId, item.relPath, {
          title: s.title?.trim() || null,
          documentDate: s.documentDate || null,
          tags: s.tags,
          notes: null,
          favorite: false,
          sender: s.sender?.trim() || null,
          amount: s.amount || null,
          currency: s.amount ? s.currency || null : null,
          dueDate: s.dueDate || null,
          destSourceId: dest.sourceId,
          destDir: dest.dir,
          fileName: s.title?.trim()
            ? fileNameFor(settings.namePattern, s.documentDate || null, s.title.trim())
            : item.name.replace(/\.[^.]+$/, ""),
        });
        moves.push({ nowSourceId: filed.sourceId, nowRelPath: filed.relPath, wasSourceId: item.sourceId, wasRelPath: item.relPath });
        items = items.filter((i) => i.relPath !== item.relPath);
      } catch (e) {
        failed.push(`${item.name}: ${e}`);
      }
    }
    if (moves.length) {
      lastFiled = null;
      filedMany = moves.length;
      onfiledmany?.(moves);
      if (selectedRel && !items.some((i) => i.relPath === selectedRel)) selectedRel = items[0]?.relPath ?? null;
      formFor = null;
    }
    if (failed.length) throw new Error(`${m.inbox.notFiled(failed.length)}: ${failed[0]}`);
  }
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
  /** The page viewer is open (#83). */
  let viewing = $state(false);

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
      folder: "",
      fileName: "",
    };
  }

  async function load() {
    items = await api.inboxItems();
    if (!selectedRel || !items.some((i) => i.relPath === selectedRel)) {
      selectedRel = items[0]?.relPath ?? null;
    }
  }

  onMount(() => {
    load();
    senderList.ensure();
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
  /** The page's height / width, from its picture (a letter page until then). */
  let aspect = $state(1.294);
  /** The right column's size and the form's height, for the preview's share. */
  let sideH = $state(0);
  let sideW = $state(0);
  let formH = $state(0);
  /** `.fab-room`'s height: the preview leaves it below the form. */
  const FAB_ROOM = 52;

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
    if (!item) {
      previewUrl = null;
      return;
    }
    // Online-only (#144): the picture Rupert kept of it, if any, while the
    // inbox downloads it again; nothing is read from the file.
    if (item.cloud === "cloudOnly") {
      previewUrl = thumbUrl(item.sourceId, entryOf(item), p, 1200);
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
      form.sender ||= s?.sender ?? "";
      if (!form.amount) {
        form.amount = formatAmountInput(s?.amount);
        form.currency = s?.currency ?? "";
      }
      form.dueDate ||= s?.dueDate ?? "";
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
        sender: form.sender.trim() || null,
        amount: form.amount.trim() ? (parseAmount(form.amount) ?? form.amount.trim()) : null,
        currency: form.amount.trim() ? form.currency.trim() || null : null,
        dueDate: form.dueDate || null,
        destSourceId: destination.sourceId,
        destDir: destination.dir,
        fileName: form.fileName.trim(),
      });
      lastFiled = { filed, label: destination.label };
      onfiling?.(item.sourceId, item.relPath, filed, destination.label);
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
  /** The name a scan is sent under (#69): the one it would be filed
   * under, if there is one yet. */
  function sendName(): string | null {
    return form.fileName.trim() || null;
  }

  /** Share… or Email…, showing what went wrong here. */
  async function send(action: () => Promise<void>) {
    try {
      error = null;
      await action();
    } catch (e) {
      error = String(e);
    }
  }

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

  /** A menu command for the menu's scan. The scan is taken before the menu
   * closes: `mn` follows `menu`, so after `menu = null` it's gone (Open and
   * Show in Finder did nothing). */
  function fromMenu(action: (item: IntakeItem) => Promise<unknown>) {
    const item = menu?.item;
    menu = null;
    if (!item) return;
    error = null;
    action(item).catch((e) => (error = String(e)));
  }

  function openMenu(item: IntakeItem, e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    selectedRel = item.relPath;
    // Keep it inside the window (the menu is about 200 × 120).
    menu = { x: Math.min(e.clientX, window.innerWidth - 210), y: Math.min(e.clientY, window.innerHeight - 130), item };
  }

  function onkeydown(e: KeyboardEvent) {
    // The page viewer and the folder tree have their own keys.
    if (viewing || browsing) return;
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
          {#if sureItems.length}
            <button class="scan-btn sure-btn" onclick={() => (fileAllOpen = true)} title={m.inbox.fileAllTip}>
              <Icon name="check" size={12} />{m.inbox.fileAllSure(sureItems.length)}
            </button>
          {/if}
          <button class="scan-btn" onclick={onimport} title={m.importInbox.buttonTip}>{m.importInbox.button}</button>
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
      <!-- MARK: Import's progress (#122) -->
      {#if importing.active || importing.finished}
        <div class="import-box" role="status">
          <div class="import-row">
            {#if importing.active}
              <b>{m.importInbox.progress(num(importing.done), num(importing.total))}</b>
              <span class="spacer"></span>
              <button class="link" onclick={stopImport}>{m.importInbox.stop}</button>
            {:else}
              <b>{importing.stopped ? m.importInbox.stopped + " " : ""}{m.importInbox.copied(importing.copied)}</b>
              <span class="spacer"></span>
              <button class="link" onclick={() => ((showDuplicates = false), dismissImport())}>{m.importInbox.dismiss}</button>
            {/if}
          </div>
          {#if importing.active}
            <div class="import-bar"><i style:width="{(100 * importing.done) / Math.max(importing.total, 1)}%"></i></div>
          {/if}
          {#if importing.duplicates.length}
            <p class="small muted">
              {m.importInbox.duplicates(importing.duplicates.length)}
              <button class="link" onclick={() => (showDuplicates = !showDuplicates)}
                >{showDuplicates ? m.importInbox.hide : m.importInbox.show}</button
              >
            </p>
            {#if showDuplicates}
              <ul class="dupes">
                {#each importing.duplicates as d, i (i)}
                  <li>
                    <button class="link" onclick={() => onshow(d.sourceId, d.relPath)} title={m.importInbox.openFolder}
                      >{d.name}</button
                    >
                    <span class="muted">{placeOf(d.sourceId, d.relPath)}</span>
                  </li>
                {/each}
              </ul>
            {/if}
          {/if}
          {#if importing.leftOut}<p class="small muted">{m.importInbox.leftOut(importing.leftOut)}</p>{/if}
          {#if importing.failed.length}
            <p class="small error" title={importing.failed.map((f) => `${f.name}: ${f.message}`).join("\n")}>
              {m.importInbox.failed(importing.failed.length)}
              {importing.failed[0].name}: {importing.failed[0].message}
            </p>
          {/if}
        </div>
      {/if}
      {#if filedMany && !lastFiled}
        <div class="filed">
          {m.inbox.filedMany(filedMany)}
          {#if canUndo}<button class="link" onclick={() => onundo?.()}>{m.renameDocs.undo}</button>{/if}
          <button class="link" onclick={() => (filedMany = 0)} aria-label={m.inbox.dismiss}>×</button>
        </div>
      {/if}
      {#if lastFiled}
        <div class="filed">
          {m.inbox.filedIn(lastFiled.label)}
          <button class="link" onclick={() => onfiled(lastFiled!.filed)}>{m.inbox.show}</button>
          {#if canUndo}<button class="link" onclick={() => onundo?.()}>{m.renameDocs.undo}</button>{/if}
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
              {...dragOutSource(i.sourceId, i.relPath)}
                >
                  <td class="name">
                    <Icon name={entryOf(i).kind === "pdf" ? "pdf" : "file"} size={15} />
                    <span title={i.name}>{i.name}</span>
                  </td>
                  <td class="state" class:busy={i.stage !== "ready"} title={i.confidence ? `${stageText(i)}\n${i.confidence.reason}` : stageText(i)}>
                    {#if i.stage === "ready" && i.confidence}<span class="mark" class:sure={i.confidence.sure}
                        >{#if i.confidence.sure}<Icon name="check" size={10} />{m.inbox.sure}{:else}{m.inbox.check}{/if}</span
                      >{/if}{stageText(i)}
                  </td>
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
              {...dragOutSource(i.sourceId, i.relPath)}
              title={i.suggestion?.title ? `${i.suggestion.title}\n${i.name}` : i.name}
            >
              <span class="pic"><Thumb sourceId={i.sourceId} entry={entryOf(i)} side={320} iconSize={48} /></span>
              <span class="label">
                {#if i.stage === "ready" && i.confidence?.sure}<span class="mark sure"><Icon name="check" size={10} />{m.inbox.sure}</span>{/if}{i.suggestion?.title ??
                  i.name}
              </span>
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
              {...dragOutSource(i.sourceId, i.relPath)}
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
      <div class="side-scroll" bind:clientHeight={sideH} bind:clientWidth={sideW}>
      {#if selected}
        <!-- The page gets what File it leaves (#127). -->
        <PreviewPane
          pane="inbox"
          room={sideH - formH - FAB_ROOM}
          natural={(sideW - 24) * aspect + 18 + (pages && pages > 1 ? 20 : 0)}
          onopen={previewUrl ? () => (viewing = true) : undefined}
        >
          <div class="preview">{@render pagePreview(false)}</div>
        </PreviewPane>
      {/if}
      <!-- MARK: Filing form -->
      <div class="form" bind:offsetHeight={formH}>
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
        <!-- #108: who it's from, the amount on it, and when it's due. -->
        <label class="field">
          <span>{m.details.sender}</span>
          <input bind:value={form.sender} list="inbox-senders" placeholder={m.details.senderHint} title={form.sender.trim() || undefined} />
        </label>
        <div class="money">
          <label class="field">
            <span>{m.details.amount}</span>
            <input
              bind:value={form.amount}
              inputmode="decimal"
              placeholder={formatAmountInput("0.00")}
              onblur={() => (form.amount = tidyAmountInput(form.amount))}
            />
          </label>
          <label class="field">
            <span>{m.details.currency}</span>
            <input
              bind:value={form.currency}
              list="inbox-currencies"
              maxlength="3"
              placeholder="EUR"
              disabled={!form.amount.trim()}
              oninput={() => (form.currency = form.currency.toUpperCase())}
            />
          </label>
          <label class="field">
            <span>{m.details.dueDate}</span>
            <input type="date" class:unset={!form.dueDate} bind:value={form.dueDate} />
          </label>
        </div>
        <datalist id="inbox-senders">
          {#each senderList.names as name (name)}<option value={name}></option>{/each}
        </datalist>
        <datalist id="inbox-currencies">
          {#each CURRENCIES as c (c)}<option value={c}></option>{/each}
        </datalist>
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
        {#if selected.stage === "ready" && selected.confidence}
          {@const c = selected.confidence}
          <div class="confidence" class:sure={c.sure}>
            {#if c.learnedFrom.length}
              <button class="why" onclick={() => (showLearned = !showLearned)} aria-expanded={showLearned} title={m.inbox.learnedTip}>
                <b>{#if c.sure}<Icon name="check" size={12} />{m.inbox.sure}{:else}{m.inbox.check}{/if}</b>
                <span>{c.reason}</span>
              </button>
              {#if showLearned}
                <p class="small">{m.inbox.learnedFrom}</p>
                <ul>
                  {#each c.learnedFrom as l, n (n)}
                    <li>{l.title} → {l.folder}{l.tags.length ? ` · ${l.tags.join(", ")}` : ""}</li>
                  {/each}
                </ul>
              {/if}
            {:else}
              <p class="why"><b>{#if c.sure}<Icon name="check" size={12} />{m.inbox.sure}{:else}{m.inbox.check}{/if}</b> <span>{c.reason}</span></p>
            {/if}
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
          <NotesBox bind:value={form.notes} rows={2} title={form.notes.trim() || undefined} />
        </label>

        {#if error}<p class="error small">{error}</p>{/if}
        <div class="actions">
          <button
            class="primary"
            disabled={!folderChoice || filing}
            onclick={file}
            title={`${m.inbox.whatFilingDoes} (${os === "macos" ? "⌘↩" : "Ctrl+Enter"})`}
          >
            {filing ? m.inbox.filing : m.inbox.file}
          </button>
          <button onclick={() => api.openEntry(selected!.sourceId, selected!.relPath)}>{m.inbox.open}</button>
          <!-- Send it before (or without) filing (#69), named as it would be filed. -->
          {#if os !== "linux"}
            <button
              class="icon-action send"
              onclick={(ev) =>
                send(() => api.shareDocument(selected!.sourceId, selected!.relPath, sendName(), atElement(ev.currentTarget)))}
              title={m.common.ellipsis(os === "windows" ? m.share.shareTipWindows : m.share.shareTip)}
              aria-label={m.share.share}><Icon name="share" size={16} /></button
            >
          {/if}
          <button
            class="icon-action send"
            onclick={() => send(() => api.emailDocument(selected!.sourceId, selected!.relPath, sendName()))}
            title={m.share.emailTip}
            aria-label={m.share.email}><Icon name="mail" size={16} /></button
          >
          <span class="spacer"></span>
          <button
            class="icon-action"
            class:spinning={selected.stage !== "ready"}
            onclick={() => api.inboxRedo(selected!.relPath)}
            disabled={selected.stage !== "ready"}
            title={m.inbox.suggestAgainTip}
            aria-label={m.inbox.suggestAgain}
          >
            <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"
              ><path
                d="M19.5 11a7.5 7.5 0 0 0-13.6-4.3M4.5 4v4h4M4.5 13a7.5 7.5 0 0 0 13.6 4.3M19.5 20v-4h-4"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
              /></svg
            >
          </button>
          <button
            class="icon-action trash"
            onclick={() => trash(selected!)}
            disabled={filing}
            title={m.common.ellipsis(trashLabel)}
            aria-label={m.common.ellipsis(trashLabel)}
          >
            <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true"
              ><path
                d="M4 7h16M10 11v6M14 11v6M5.5 7l1 12a2 2 0 0 0 2 2h7a2 2 0 0 0 2-2l1-12M9 7V4.5A.5.5 0 0 1 9.5 4h5a.5.5 0 0 1 .5.5V7"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
              /></svg
            >
          </button>
        </div>
      {/if}
      </div>
      <div class="fab-room" aria-hidden="true"></div>
      </div>
    </section>
  </div>
{/if}

<!-- MARK: Page preview -->
{#snippet pagePreview(big: boolean)}
  {#if selected}
    <PagePicture
      src={previewUrl}
      alt={m.inbox.firstPage(selected.name)}
      loading={previewLoading}
      empty={selected.cloud === "cloudOnly" ? m.inbox.onlineOnly : m.inbox.noPreview}
      {pages}
      bind:page
      {big}
      onopen={() => (viewing = true)}
      onaspect={(a) => (aspect = a)}
    />
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
    <button role="menuitem" onclick={() => fromMenu((i) => api.openEntry(i.sourceId, i.relPath))}>{m.inbox.open}</button>
    <button role="menuitem" onclick={() => fromMenu((i) => api.revealEntry(i.sourceId, i.relPath))}
      >{revealLabel(os)}</button
    >
    <hr />
    <button role="menuitem" class="danger" onclick={() => trash(mn.item)}>{m.common.ellipsis(trashLabel)}</button>
  </div>
{/if}


{#if fileAllOpen}
  <FileAllDialog items={sureItems} namePattern={settings.namePattern} onfile={fileAll} onclose={() => (fileAllOpen = false)} />
{/if}

{#if viewing && selected}
  <PageViewer
    sourceId={selected.sourceId}
    entry={entryOf(selected)}
    title={form.title.trim() || selected.suggestion?.title || null}
    startPage={page}
    onclose={() => (viewing = false)}
  />
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
  .import-box {
    margin: 0 0 10px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    display: grid;
    gap: 4px;
  }
  .import-box p {
    margin: 0;
  }
  .import-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .import-bar {
    height: 5px;
    border-radius: 3px;
    background: var(--border);
    overflow: hidden;
  }
  .import-bar i {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .dupes {
    margin: 0;
    padding: 0 0 0 4px;
    list-style: none;
    max-height: 140px;
    overflow: auto;
    font-size: 12px;
  }
  .dupes li {
    display: flex;
    gap: 8px;
    align-items: baseline;
  }
  .scan-btn {
    flex: none;
    font-size: 12px;
    padding: 2px 10px;
  }
  /* #112: File All Sure Ones, and the Sure / Check marks. */
  .sure-btn {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    font-weight: 600;
  }
  .mark {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    margin-right: 6px;
    padding: 0 6px;
    border-radius: 8px;
    font-size: 11px;
    border: 1px solid var(--border);
    color: var(--muted);
    vertical-align: 1px;
  }
  .mark.sure {
    border-color: var(--ok-border, #b9d4b3);
    background: var(--ok-soft, #eef5ec);
    color: var(--ok, #2f6b2a);
  }
  .confidence {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 7px 10px;
    font-size: 12px;
  }
  .confidence.sure {
    border-color: var(--ok-border, #b9d4b3);
    background: var(--ok-soft, #eef5ec);
  }
  .confidence .why {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 2px 6px;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    width: 100%;
  }
  button.why {
    cursor: pointer;
  }
  .confidence b {
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
  .confidence.sure b {
    color: var(--ok, #2f6b2a);
  }
  .confidence p {
    margin: 6px 0 2px;
    color: var(--muted);
  }
  .confidence ul {
    margin: 0;
    padding-left: 16px;
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
  }
  .side-scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow: auto;
    /* As in Details: a scroll bar that takes space (Windows, Linux) would
       otherwise come and go as the preview resizes with the column. */
    scrollbar-gutter: stable;
    display: flex;
    flex-direction: column;
  }
  /* Beside the Ask Rupert button (46px, 18px from the window's right edge,
     2px past this column's): the text wraps 20px before it. */
  /* Room for the Ask Rupert button (46px, 34px from the window's bottom)
     at the end of what scrolls: the form can always be scrolled clear of
     it, and nothing else of the column is given up to it. */
  .fab-room {
    flex: none;
    height: 52px;
  }
  .preview {
    flex: 1 1 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    background: var(--panel);
    padding: 12px 12px 6px;
  }








  .form {
    flex: none;
    padding: 12px 14px;
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
  /* Amount, currency and due date on one line (#108). */
  .money {
    display: grid;
    grid-template-columns: 1fr 58px 1.3fr;
    gap: 6px;
  }
  .money input {
    width: 100%;
    box-sizing: border-box;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
  }
  input,
  .field :global(textarea) {
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
  /* Suggest again and Move to Trash as icons (#83). */
  .icon-action {
    width: 32px;
    height: 30px;
    padding: 0;
    display: grid;
    place-items: center;
  }
  .icon-action.send {
    color: var(--accent-strong);
  }
  .icon-action.trash {
    color: var(--danger);
  }
  .icon-action.trash:hover:not(:disabled) {
    background: var(--danger-soft);
    border-color: var(--danger);
  }
  .icon-action.spinning svg {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
