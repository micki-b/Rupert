<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    api,
    onDirChanged,
    onIndexChanged,
    onIndexProgress,
    onIntakeChanged,
    onMenuImport,
    onMenuImportInbox,
    onMenuQuickOpen,
    onMenuTogglePreview,
    onContinuityScan,
    onMenuSettings,
    onMenuCheckUpdates,
    onSettingsChanged,
    onSourcesChanged,
    onRemindersShow,
    providerLabel,
    revealLabel,
    editionLabel,
    snippetParts,
    type AppInfo,
    type ReminderCounts,
    type SavedSearch,
    type Filed,
    type Settings,
    type TagEditReport,
    type TagChange,
    type MoveBackItem,
    type Entry,
    type Hit,
    type Total,
    type IndexStats,
    type ListedEntry,
    type Progress,
    type Source,
    type TagCount,
    type TagMode,
    type FavoriteFolder,
    type AssistantAction,
    type Citation,
    type DocPlace,
    type RenameOutcome,
    type Renamed,
    type ShareAt,
    atElement,
    onShareProgress,
  } from "#lib/api.ts";
  import { changes } from "#lib/changes.svelte.ts";
  import { addUp, formatDate, formatDay, formatMoney, formatSize, formatTotals, kindLabel, shortCount } from "#lib/format.ts";
  import { senderList } from "#lib/details.svelte.ts";
  import FolderNode from "#lib/FolderNode.svelte";
  import MoveDialog from "#lib/MoveDialog.svelte";
  import { canMoveInto, dragSource, dropTarget, isDropOver, onMoveMany, type Dropped, type MoveItem } from "#lib/drag.svelte.ts";
  import Icon from "#lib/Icon.svelte";
  import ImportDialog from "#lib/ImportDialog.svelte";
  import Inbox from "#lib/Inbox.svelte";
  import TagFilter from "#lib/TagFilter.svelte";
  import TagManager from "#lib/TagManager.svelte";
  import Assistant from "#lib/Assistant.svelte";
  import ScanPhonePanel from "#lib/ScanPhonePanel.svelte";
  import MetaEditor from "#lib/MetaEditor.svelte";
  import PictureView from "#lib/PictureView.svelte";
  import DocStar from "#lib/DocStar.svelte";
  import CloudBadge from "#lib/CloudBadge.svelte";
  import * as docActions from "#lib/doc-actions.svelte.ts";
  import MarkdownReader from "#lib/MarkdownReader.svelte";
  import { isMarkdown } from "#lib/markdown.ts";
  import { checkNowAndThen, updates } from "#lib/updates.svelte.ts";
  import RupertOwl from "#lib/RupertOwl.svelte";
  import Splitter from "#lib/Splitter.svelte";
  import RenameField from "#lib/RenameField.svelte";
  import RenameDialog from "#lib/RenameDialog.svelte";
  import InboxImportDialog from "#lib/InboxImportDialog.svelte";
  import BulkTagsDialog from "#lib/BulkTagsDialog.svelte";
  import ExportDialog from "#lib/ExportDialog.svelte";
  import ReadDetailsDialog from "#lib/ReadDetailsDialog.svelte";
  import Health from "#lib/Health.svelte";
  import ComingUp from "#lib/ComingUp.svelte";
  import SaveSearchDialog from "#lib/SaveSearchDialog.svelte";
  import PageViewer from "#lib/PageViewer.svelte";
  import DocPreview from "#lib/DocPreview.svelte";
  import ListHeader from "#lib/ListHeader.svelte";
  import { columnWidth, listColumns, NAME_MIN } from "#lib/listColumns.svelte.ts";
  import { preview as pagePreview, setPreviewHidden } from "#lib/previewPane.svelte.ts";
  import QuickOpen, { type QuickCommand } from "#lib/QuickOpen.svelte";
  import { rememberDocument } from "#lib/recent.ts";
  import { clamp, columns, LIMITS, MIDDLE } from "#lib/columns.svelte.ts";
  import { loadLanguage, locale, m } from "#lib/i18n/index.ts";

  type Folder = { sourceId: string; relPath: string };
  type Selected = { sourceId: string; entry: Entry };

  let info = $state<AppInfo | null>(null);
  let sources = $state<Source[]>([]);
  let folder = $state<Folder | null>(null);
  let entries = $state<ListedEntry[]>([]);
  let selected = $state<Selected | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let menuFor = $state<string | null>(null);
  let renaming = $state<string | null>(null);
  let renameValue = $state("");
  let importing = $state(false);
  let scanOpen = $state(false);
  let appSettings = $state<Settings | null>(null);
  let inboxOpen = $state(false);
  let inboxCount = $state(0);
  /** The Import into Inbox window (#122). */
  let importInboxOpen = $state(false);

  // MARK: - Selecting several (#102)

  /** Everything selected, in the order picked. `selected` is the one picked
   * last (what Details shows when it's alone); a plain `selected = …`
   * anywhere selects just that one. */
  let picked = $state<Selected[]>([]);
  /** Where a Shift-click range starts. */
  let anchor: Selected | null = null;
  const keyOf = (sourceId: string, relPath: string) => sourceId + "\n" + relPath;
  const pickedKeys = $derived(new Set(picked.map((p) => keyOf(p.sourceId, p.entry.relPath))));
  const isPicked = (sourceId: string, relPath: string) => pickedKeys.has(keyOf(sourceId, relPath));
  const several = $derived(picked.length > 1);
  /** The picked paths in one source (for the picture views). */
  const pickedIn = (sourceId: string) =>
    new Set(picked.filter((p) => p.sourceId === sourceId).map((p) => p.entry.relPath));

  $effect(() => {
    const s = selected;
    untrack(() => {
      if (!s) {
        if (picked.length) picked = [];
        anchor = null;
      } else if (!pickedKeys.has(keyOf(s.sourceId, s.entry.relPath))) {
        picked = [s];
        anchor = s;
      }
    });
  });

  /** What the middle shows, in order: search results or the folder. */
  function shownItems(): Selected[] {
    if (inboxOpen) return [];
    if (searching) return hits.map((h) => ({ sourceId: h.sourceId, entry: h }));
    if (folder && source) return sortedEntries.map((e) => ({ sourceId: source.id, entry: e }));
    return [];
  }

  type Mods = { shiftKey: boolean; metaKey: boolean; ctrlKey: boolean };

  /** A click (or arrow key) on an item: just it; with ⌘ (Ctrl elsewhere)
   * added or taken away; with Shift, the range from the last one picked. */
  function pick(sel: Selected, ev?: Mods) {
    const k = keyOf(sel.sourceId, sel.entry.relPath);
    if (ev?.shiftKey && anchor) {
      const list = shownItems();
      const at = (x: Selected) => list.findIndex((y) => keyOf(y.sourceId, y.entry.relPath) === keyOf(x.sourceId, x.entry.relPath));
      const [a, b] = [at(anchor), at(sel)];
      if (a >= 0 && b >= 0) {
        picked = list.slice(Math.min(a, b), Math.max(a, b) + 1);
        selected = sel;
        return;
      }
    }
    if (ev && (info?.os === "macos" ? ev.metaKey : ev.ctrlKey) && picked.length) {
      if (pickedKeys.has(k)) {
        const rest = picked.filter((p) => keyOf(p.sourceId, p.entry.relPath) !== k);
        picked = rest;
        selected = rest.at(-1) ?? null;
      } else {
        picked = [...picked, sel];
        selected = sel;
      }
      anchor = sel;
      return;
    }
    picked = [sel];
    selected = sel;
    anchor = sel;
  }

  /** What a drag starting on an item carries: the selection, if the item is
   * part of it. */
  function dragGroup(sourceId: string, relPath: string): Selected[] {
    return several && isPicked(sourceId, relPath) ? picked.slice() : [];
  }

  /** ⌘A / Ctrl+A: everything shown. */
  function pickAll() {
    const all = shownItems();
    if (!all.length) return;
    picked = all;
    if (!selected || !all.some((x) => x.sourceId === selected!.sourceId && x.entry.relPath === selected!.entry.relPath)) {
      selected = all[0];
      anchor = all[0];
    }
  }

  /** ↑ ↓ in the list and search results (Shift extends the selection). */
  function stepPick(step: number, shift: boolean) {
    const list = shownItems();
    if (!list.length) return;
    const i = selected ? list.findIndex((x) => x.sourceId === selected!.sourceId && x.entry.relPath === selected!.entry.relPath) : -1;
    const next = list[Math.min(list.length - 1, Math.max(0, i < 0 ? 0 : i + step))];
    pick(next, { shiftKey: shift, metaKey: false, ctrlKey: false });
    tick().then(() =>
      document.querySelector(`[data-pick="${CSS.escape(keyOf(next.sourceId, next.entry.relPath))}"]`)?.scrollIntoView({ block: "nearest" }),
    );
  }

  // MARK: - Column widths (#61)
  // The sidebar and the right column (Details, or the inbox's preview and
  // File it) as shown: what was dragged, but never so wide that the middle
  // loses its room.
  let windowWidth = $state(1200);
  /** The inbox pane's padding (`.documents`), outside its right column. */
  const INBOX_PADDING = 16;
  const rightColumn = $derived(inboxOpen && appSettings ? "inbox" : "details");
  const sideWidth = $derived(
    clamp(columns.side, LIMITS.side.min, windowWidth - columns[rightColumn] - MIDDLE),
  );
  const rightWidth = $derived(
    clamp(columns[rightColumn], LIMITS[rightColumn].min, windowWidth - sideWidth - MIDDLE),
  );

  /** The chat panel, to open it from the welcome screen. */
  let assistant: { show: () => Promise<void>; refreshInfo: () => Promise<void> } | undefined = $state();

  // Search & library filters
  let query = $state("");
  let selectedTags = $state<string[]>([]);
  let tagMode = $state<TagMode>("any");
  let tagManagerOpen = $state(false);
  let favoriteFolders = $state<FavoriteFolder[]>([]);
  /** Whether the open folder is a favorite. */
  let folderFav = $state(false);
  let favoritesOnly = $state(false);
  let hits = $state<Hit[]>([]);
  let searchedOnce = $state(false);
  let tags = $state<TagCount[]>([]);
  /** The lower part of the sidebar: source folders or the tag filter. */
  const VIEW_KEY = "rupert.sidebarView";
  let sidebarView = $state<"folders" | "tags">(loadSidebarView());

  function loadSidebarView(): "folders" | "tags" {
    try {
      return localStorage.getItem(VIEW_KEY) === "tags" ? "tags" : "folders";
    } catch {
      return "folders";
    }
  }

  function setSidebarView(v: "folders" | "tags") {
    sidebarView = v;
    try {
      localStorage.setItem(VIEW_KEY, v);
    } catch {
      /* not essential */
    }
  }
  /** How folders are shown: one choice for all folders, kept on this computer. */
  type FolderMode = "list" | "icons" | "gallery";
  const MODE_KEY = "rupert.folderView";
  let folderMode = $state<FolderMode>(loadFolderMode());

  function loadFolderMode(): FolderMode {
    try {
      const saved = localStorage.getItem(MODE_KEY);
      return saved === "icons" || saved === "gallery" ? saved : "list";
    } catch {
      return "list";
    }
  }

  function setFolderMode(mode: FolderMode) {
    folderMode = mode;
    try {
      localStorage.setItem(MODE_KEY, mode);
    } catch {
      /* not essential */
    }
  }
  let stats = $state<IndexStats | null>(null);
  let progress = $state<Progress | null>(null);
  let indexTick = $state(0);
  let searchInput: HTMLInputElement | undefined = $state();

  // MARK: - Sorting and search filters (#103)

  type SortKey = "name" | "kind" | "size" | "modified" | "date" | "sender" | "amount" | "due";
  type Sort = { key: SortKey; desc: boolean };
  const SORT_KEY = "rupert.sort";
  /** How folders are sorted (folders stay on top), remembered like the view. */
  let sort = $state<Sort>(loadSort());
  function loadSort(): Sort {
    try {
      const s = JSON.parse(localStorage.getItem(SORT_KEY) ?? "null");
      if (s && ["name", "kind", "size", "modified", "date", "sender", "amount", "due"].includes(s.key)) return { key: s.key, desc: !!s.desc };
    } catch {
      /* first time */
    }
    return { key: "name", desc: false };
  }
  /** A column header: sort by it, or reverse if it already is. Dates and
   * sizes start with the newest and biggest. */
  function sortBy(key: SortKey, desc?: boolean) {
    sort = {
      key,
      desc: desc ?? (sort.key === key ? !sort.desc : key === "modified" || key === "date" || key === "size" || key === "amount"),
    };
    try {
      localStorage.setItem(SORT_KEY, JSON.stringify(sort));
    } catch {
      /* not essential */
    }
  }
  /** A document's date: the one on the paper, else the file's day. */
  const docDate = (e: ListedEntry) => e.documentDate ?? (e.modifiedMs ? formatIsoDay(e.modifiedMs) : "");
  /** "2026-03-14" as the reader's date (as in Details). */
  function formatDocDay(iso: string | null | undefined): string {
    if (!iso) return "";
    const [y, mo, d] = iso.split("-").map(Number);
    return new Date(y, mo - 1, d).toLocaleDateString(locale(), { dateStyle: "medium" });
  }
  function formatIsoDay(ms: number) {
    const d = new Date(ms);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  const collator = $derived(new Intl.Collator(locale(), { numeric: true, sensitivity: "base" }));
  /** The open folder in the chosen order, folders first. */
  const sortedEntries = $derived.by(() => {
    const { key, desc } = sort;
    const byName = (a: ListedEntry, b: ListedEntry) => collator.compare(a.name, b.name);
    // #108's columns: documents without the detail go last either way.
    const detail = (e: ListedEntry): string | number | null =>
      key === "sender" ? (e.sender ?? null) : key === "amount" ? (e.amount ? Number(e.amount) : null) : key === "due" ? (e.dueDate ?? null) : null;
    const cmp = (a: ListedEntry, b: ListedEntry): number => {
      if (key === "sender" || key === "amount" || key === "due") {
        const [x, y] = [detail(a), detail(b)];
        if (x === null || y === null) return x === y ? byName(a, b) : x === null ? 1 : -1;
        const c = typeof x === "number" && typeof y === "number" ? x - y : collator.compare(String(x), String(y));
        return (desc ? -c : c) || byName(a, b);
      }
      let c = 0;
      if (key === "kind") c = collator.compare(kindLabel(a.isDir ? "folder" : a.kind), kindLabel(b.isDir ? "folder" : b.kind));
      else if (key === "size") c = (a.size || 0) - (b.size || 0);
      else if (key === "modified") c = (a.modifiedMs ?? 0) - (b.modifiedMs ?? 0);
      else if (key === "date") c = docDate(a).localeCompare(docDate(b));
      if (c === 0) c = byName(a, b);
      return desc ? -c : c;
    };
    return [...entries].sort((a, b) => (a.isDir !== b.isDir ? (a.isDir ? -1 : 1) : cmp(a, b)));
  });

  /** Search results: best match first, or newest first. */
  let resultsByDate = $state(false);

  type DateFilter = { kind: "thisYear" } | { kind: "lastYear" } | { kind: "range"; from: string; to: string };
  let dateFilter = $state<DateFilter | null>(null);
  let kindFilter = $state<string | null>(null);
  /** #108: documents from this sender, and due from today on / before today. */
  let fromFilter = $state<string | null>(null);
  let dueFilter = $state<"upcoming" | "past" | null>(null);
  /** Where to search: a source, or a folder and its subfolders. */
  let whereFilter = $state<{ scope: "source" | "folder"; sourceId: string; relPath: string } | null>(null);
  const filtering = $derived(!!dateFilter || !!kindFilter || !!whereFilter || !!fromFilter || !!dueFilter);
  /** Today as YYYY-MM-DD (local), for due dates. */
  function todayIso(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  /** The day before `iso`. */
  function dayBefore(iso: string): string {
    const [y, mo, d] = iso.split("-").map(Number);
    const t = new Date(y, mo - 1, d - 1);
    return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, "0")}-${String(t.getDate()).padStart(2, "0")}`;
  }

  function dateRange(f: DateFilter | null): { dateFrom?: string; dateTo?: string } {
    if (!f) return {};
    const y = new Date().getFullYear();
    if (f.kind === "thisYear") return { dateFrom: `${y}-01-01`, dateTo: `${y}-12-31` };
    if (f.kind === "lastYear") return { dateFrom: `${y - 1}-01-01`, dateTo: `${y - 1}-12-31` };
    return { dateFrom: f.from || undefined, dateTo: f.to || undefined };
  }

  /** "In this folder" / "In this source" start from what's open. */
  function setWhere(scope: "everywhere" | "source" | "folder") {
    const at = whereFilter ?? (folder ? { sourceId: folder.sourceId, relPath: folder.relPath } : null);
    whereFilter = scope === "everywhere" || !at ? null : { scope, sourceId: at.sourceId, relPath: at.relPath };
  }

  function whereLabelOf(w: NonNullable<typeof whereFilter>) {
    if (w.scope === "source") return m.filters.inSource(sourceName(w.sourceId));
    const name = w.relPath ? w.relPath.slice(w.relPath.lastIndexOf("/") + 1) : sourceName(w.sourceId);
    return m.filters.inFolder(name);
  }

  /** A date range being chosen in the filter row. */
  let rangeFrom = $state("");
  let rangeTo = $state("");
  const KINDS = ["pdf", "image", "office", "text", "email"] as const;

  const searching = $derived(query.trim() !== "" || selectedTags.length > 0 || favoritesOnly || filtering);
  /** The folder whose contents are on screen, for highlighting in the sidebar
   * (none while searching or in the inbox). */
  const shownFolder = $derived(searching || inboxOpen ? null : folder);
  const source = $derived(sources.find((s) => s.id === folder?.sourceId) ?? null);
  const sourceName = (id: string) => sources.find((s) => s.id === id)?.name ?? "";
  const crumbs = $derived.by(() => {
    if (!folder || !source) return [];
    const parts = folder.relPath ? folder.relPath.split("/") : [];
    return [
      { name: source.name, relPath: "" },
      ...parts.map((name, i) => ({ name, relPath: parts.slice(0, i + 1).join("/") })),
    ];
  });

  // MARK: - Loading

  async function run<T>(action: () => Promise<T>): Promise<T | undefined> {
    try {
      error = null;
      return await action();
    } catch (e) {
      error = String(e);
    }
  }

  async function loadSources(refresh = false) {
    const list = await run(() => (refresh ? api.refreshSources() : api.listSources()));
    if (list) sources = list;
  }

  async function loadEntries() {
    if (!folder) return;
    const current = folder;
    loading = true;
    const list = await run(() => api.listDir(current.sourceId, current.relPath));
    if (folder !== current) return; // user moved on meanwhile
    loading = false;
    entries = list ?? [];
    if (pendingSelect) {
      const want = pendingSelect;
      pendingSelect = null;
      const entry = entries.find((e) => e.relPath === want);
      if (entry) {
        selected = { sourceId: current.sourceId, entry };
        tick().then(() =>
          document.querySelector(`[data-pick="${CSS.escape(keyOf(current.sourceId, want))}"]`)?.scrollIntoView({ block: "nearest" }),
        );
      }
    } else if (selected && !searching) {
      const rel = selected.entry.relPath;
      const still = entries.find((e) => e.relPath === rel);
      selected = still ? { sourceId: current.sourceId, entry: still } : null;
    }
    if (!searching) refreshPicked(entries.map((e) => ({ sourceId: current.sourceId, entry: e })));
  }

  /** Points the selection at freshly loaded entries (new tags, stars and
   * sizes), dropping any that went away. */
  function refreshPicked(fresh: Selected[]) {
    if (picked.length < 2) return;
    const byKey = new Map(fresh.map((f) => [keyOf(f.sourceId, f.entry.relPath), f]));
    picked = picked.flatMap((p) => {
      const f = byKey.get(keyOf(p.sourceId, p.entry.relPath));
      return f ? [f] : [];
    });
  }

  /** A document to select once its folder has loaded (from the assistant). */
  let pendingSelect: string | null = null;

  /** Carries out what the assistant asked the window to show. */
  function assistantAction(a: AssistantAction) {
    if (a.kind === "openFolder") {
      selectFolder(a.sourceId, a.relPath);
    } else if (a.kind === "showDocument") {
      const cut = a.relPath.lastIndexOf("/");
      pendingSelect = a.relPath;
      selectFolder(a.sourceId, cut < 0 ? "" : a.relPath.slice(0, cut));
    } else if (a.kind === "savedSearch") {
      const s = savedSearches.find((x) => x.id === a.id && x.sourceId === a.sourceId);
      if (s) applySaved(s);
      else assistantAction({ kind: "search", text: a.text, tags: a.tags, tagMode: a.tagMode, favoritesOnly: a.favoritesOnly });
    } else {
      inboxOpen = false;
      selected = null;
      query = a.text;
      selectedTags = [...a.tags];
      tagMode = a.tagMode;
      tagsBefore = selectedTags.length;
      favoritesOnly = a.favoritesOnly;
    }
  }

  async function loadIndexInfo() {
    const [t, s, f] = await Promise.all([api.listTags(), api.indexStatus(), api.favoriteFolders()]);
    tags = t;
    favoriteFolders = f;
    stats = s.stats;
    progress ??= s.progress;
  }

  function selectFolder(sourceId: string, relPath: string) {
    lastChange = null;
    inboxOpen = false;
    healthOpen = false;
    comingUpOpen = false;
    copyProblems = null;
    query = "";
    selectedTags = [];
    favoritesOnly = false;
    folder = { sourceId, relPath };
    selected = null;
    entries = [];
    loadEntries();
  }

  onMount(() => {
    api.appInfo().then((i) => (info = i));
    loadSources();
    loadIndexInfo();
    // The App Store updates its own edition.
    const selfUpdating = api.appInfo().then((i) => i.edition !== "app-store");
    api.getSettings().then(async (s) => {
      appSettings = s;
      if (await selfUpdating) checkNowAndThen(s.checkUpdates);
    });
    // A window left open for days still hears about new versions.
    const updateTimer = setInterval(
      async () => checkNowAndThen((await selfUpdating) && (appSettings?.checkUpdates ?? false)),
      6 * 60 * 60 * 1000,
    );
    loadInboxCount();
    senderList.ensure();
    loadHealthBadge();
    loadComingCounts(true);
    loadSavedSearches();
    api.remindersTakeShown().then(showReminder).catch(() => {});

    const unlisten = [
      onRemindersShow(showReminder),
      onDirChanged((batch) => changes.push(batch)),
      onMenuImport(() => (importing = true)),
      onMenuSettings(() => api.openSettings()),
      onMenuCheckUpdates(() => {
        api.openSettings("updates");
        updates.check(true);
      }),
      // Settings live in their own window (#80): follow what changes there.
      onSettingsChanged(async (s) => {
        appSettings = s;
        loadComingCounts(true);
        await loadLanguage();
        // The menu is built again in the new language.
        api.setPreviewMenu(!pagePreview.hidden).catch(() => {});
        loadInboxCount();
        assistant?.refreshInfo();
      }),
      onSourcesChanged(() => {
        loadSources();
        loadSavedSearches();
      }),
      // A Continuity Camera scan (#124) arrived: show it in the inbox.
      onContinuityScan((r) => {
        if (r.error) {
          error = r.error;
          return;
        }
        scanOpen = false;
        openInbox();
        loadInboxCount();
      }),
      onMenuQuickOpen(() => {
        if (!viewer && !reader) quickOpen = true;
      }),
      onMenuTogglePreview(() => setPreviewHidden(!pagePreview.hidden)),
      onMenuImportInbox(() => {
        if (appSettings?.inbox) {
          openInbox();
          importInboxOpen = true;
        } else {
          api.openSettings("inbox");
        }
      }),
      onIntakeChanged(loadInboxCount),
      onIndexProgress((p) => (progress = p)),
      onShareProgress((name) => (sendingName = name)),
      onIndexChanged(() => {
        indexTick++;
        loadIndexInfo();
        senderList.refresh();
        loadHealthBadge();
        loadComingCounts();
      }),
      getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) {
          loadSources(true);
          // Another computer may have saved one (synced with the source).
          loadSavedSearches();
        }
      }),
    ];
    const closeMenu = () => {
      menuFor = null;
      savedMenu = null;
      itemMenu = null;
      bulkMenu = null;
    };
    const shortcuts = (e: KeyboardEvent) => {
      if (viewer || quickOpen || reader) return; // they have the keyboard
      const typing =
        e.target instanceof Element && e.target.closest("input, textarea, select, [contenteditable]");
      const cmd = info?.os === "macos" ? e.metaKey : e.ctrlKey;
      if (e.key === "Escape" && (itemMenu || bulkMenu)) {
        itemMenu = null;
        bulkMenu = null;
      } else if (cmd && !e.shiftKey && !e.altKey && e.key === "k") {
        // ⌘K / Ctrl+K: quick open (#105). On the Mac, File ▸ Quick Open…
        // has the key, and this is for when the menu doesn't see it.
        e.preventDefault();
        itemMenu = null;
        quickOpen = true;
      } else if (
        !typing &&
        selected &&
        !inboxOpen &&
        !renamingAt &&
        e.key === " " &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        !e.shiftKey &&
        // Other buttons keep Space for pressing them; documents' own
        // (search results, tiles) don't.
        !(e.target instanceof Element && e.target.closest("button:not(.result, .tile), a, [role=dialog]"))
      ) {
        // Space: the page viewer (#105).
        e.preventDefault();
        preview(selected);
      } else if (!typing && !inboxOpen && cmd && !e.shiftKey && !e.altKey && e.key === "a" && shownItems().length) {
        // ⌘A / Ctrl+A selects everything shown (#102).
        e.preventDefault();
        pickAll();
      } else if (
        !typing &&
        !inboxOpen &&
        !renamingAt &&
        (searching || folderMode === "list") &&
        (e.key === "ArrowDown" || e.key === "ArrowUp") &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey
      ) {
        e.preventDefault();
        stepPick(e.key === "ArrowDown" ? 1 : -1, e.shiftKey);
      } else if (e.key === "F1") {
        e.preventDefault();
        api.openHelp();
      } else if (
        !typing &&
        selected &&
        !inboxOpen &&
        (info?.os === "macos" ? e.metaKey && e.key === "Backspace" : e.key === "Delete")
      ) {
        e.preventDefault();
        trashPicked();
      } else if (
        !typing &&
        selected &&
        !inboxOpen &&
        !searching &&
        !renamingAt &&
        !several &&
        !e.metaKey &&
        !e.ctrlKey &&
        !e.altKey &&
        (info?.os === "macos" ? e.key === "Enter" : e.key === "F2")
      ) {
        // Return (Mac) or F2 renames, as in the Finder and File Explorer.
        e.preventDefault();
        startRenaming(selected);
      } else if (!typing && selected && !several && info?.os === "macos" && e.metaKey && (e.key === "o" || e.key === "ArrowDown")) {
        // ⌘O or ⌘↓ opens, as in the Finder.
        e.preventDefault();
        activate(selected.sourceId, selected.entry);
      } else if (!typing && info?.os !== "macos" && e.ctrlKey && !e.shiftKey && e.key === "z" && lastChange) {
        // Ctrl+Z undoes renaming documents (the Mac has Edit ▸ Undo).
        e.preventDefault();
        undoLast();
      } else if (info?.os !== "macos" && e.ctrlKey && e.key === ",") {
        // Ctrl+, opens Settings (the menu's ⌘, on the Mac).
        e.preventDefault();
        api.openSettings();
      } else if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        inboxOpen = false;
        searchInput?.focus();
        searchInput?.select();
      }
    };
    // Edit ▸ Undo, or ⌘Z outside a text field (see +layout.svelte).
    const undo = () => undoLast();
    window.addEventListener("rupert-undo", undo);
    window.addEventListener("click", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("keydown", shortcuts);
    return () => {
      unlisten.forEach((p) => p.then((off) => off()));
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("keydown", shortcuts);
      window.removeEventListener("rupert-undo", undo);
      clearInterval(updateTimer);
    };
  });

  $effect(() => {
    changes.tick;
    if (folder && !searching && changes.affects(folder.sourceId, folder.relPath)) loadEntries();
  });

  // Refresh the folder's titles/tags when the index catches up.
  $effect(() => {
    if (indexTick === 0) return;
    untrack(() => {
      if (folder && !searching) loadEntries();
    });
  });

  // MARK: - Search

  let searchSeq = 0;
  /** What the search's matches add up to, per currency (#108). */
  let searchTotals = $state<Total[]>([]);
  $effect(() => {
    const args = {
      text: query,
      tags: [...selectedTags],
      tagMode,
      favoritesOnly,
      ...dateRange(dateFilter),
      kinds: kindFilter ? [kindFilter] : [],
      sourceId: whereFilter?.sourceId ?? null,
      folder: whereFilter?.scope === "folder" ? whereFilter.relPath : null,
      byDate: resultsByDate,
      senders: fromFilter ? [fromFilter] : [],
      ...(dueFilter === "upcoming" ? { dueFrom: todayIso() } : dueFilter === "past" ? { dueTo: dayBefore(todayIso()) } : {}),
    };
    indexTick; // re-run when the index changes
    if (!searching) {
      hits = [];
      searchedOnce = false;
      return;
    }
    const seq = ++searchSeq;
    const timer = setTimeout(async () => {
      const result = await run(() => api.search(args));
      if (seq !== searchSeq) return;
      hits = result ?? [];
      refreshPicked(hits.map((h) => ({ sourceId: h.sourceId, entry: h })));
      searchedOnce = true;
      // The amounts of every match, not only the 300 shown (#108).
      searchTotals = hits.some((h) => h.amount) || hits.length >= 300 ? ((await api.searchTotals(args).catch(() => [])) ?? []) : [];
      if (seq !== searchSeq) searchTotals = [];
    }, 150);
    return () => clearTimeout(timer);
  });

  // Whether the open folder is a favorite (read from the folder itself).
  $effect(() => {
    const f = folder;
    indexTick;
    if (!f) return;
    api
      .folderFavorite(f.sourceId, f.relPath)
      .then((fav) => {
        if (folder === f) folderFav = fav;
      })
      .catch(() => (folderFav = false));
  });

  async function toggleFolderFav() {
    if (!folder) return;
    const f = folder;
    const next = !folderFav;
    folderFav = next;
    try {
      error = null;
      await api.setFolderFavorite(f.sourceId, f.relPath, next);
    } catch (e) {
      error = String(e);
      folderFav = !next;
    }
    favoriteFolders = await api.favoriteFolders();
  }

  // Sidebar filters start a fresh view; typing afterwards searches within
  // them, and further tags narrow or widen the selection.
  let tagsBefore = 0;
  function tagsChanged() {
    inboxOpen = false;
    if (tagsBefore === 0 && selectedTags.length > 0) {
      query = "";
      favoritesOnly = false;
    }
    tagsBefore = selectedTags.length;
    selected = null;
  }

  function removeTag(tag: string) {
    selectedTags = selectedTags.filter((t) => t !== tag);
    tagsBefore = selectedTags.length;
  }

  // MARK: - Library health (#113)

  let healthOpen = $state(false);
  /** Rows with something to fix, for the sidebar. */
  let healthBadge = $state(0);

  function openHealth() {
    lastChange = null;
    inboxOpen = false;
    comingUpOpen = false;
    query = "";
    selectedTags = [];
    favoritesOnly = false;
    selected = null;
    healthOpen = true;
  }

  // MARK: - Saved searches (#110)

  let savedSearches = $state<SavedSearch[]>([]);
  let saveDialog = $state<{ mode: "save" | "rename"; search: SavedSearch } | null>(null);
  let savedMenu = $state<{ x: number; y: number; s: SavedSearch } | null>(null);

  async function loadSavedSearches() {
    savedSearches = await api.savedSearches().catch(() => savedSearches);
  }

  /** The search on screen as a saved search keeps it, with the source it
   * belongs with: the one it looks in, or else the first. */
  function currentSearch(): SavedSearch {
    return {
      sourceId: whereFilter?.sourceId ?? sources[0]?.id ?? "",
      id: "",
      name: "",
      text: query.trim(),
      tags: [...selectedTags],
      tagMode,
      favorites: favoritesOnly,
      date: dateFilter ? { ...dateFilter } : null,
      kind: kindFilter,
      from: fromFilter,
      due: dueFilter,
      scope: whereFilter?.scope ?? null,
      folder: whereFilter?.scope === "folder" ? whereFilter.relPath : null,
      newest: resultsByDate,
    };
  }

  /** What makes two searches find the same documents (not their order). */
  const findsKey = (s: SavedSearch) =>
    JSON.stringify([
      s.text.trim().toLowerCase(),
      s.tags.map((t) => t.toLowerCase()),
      s.tags.length > 1 ? s.tagMode : "any",
      s.favorites,
      s.date ?? null,
      s.kind ?? null,
      s.from ?? null,
      s.due ?? null,
      s.scope ?? null,
      s.scope ? s.sourceId : null,
      s.scope === "folder" ? (s.folder ?? "") : null,
    ]);

  /** The saved search on screen, if the search is one. */
  const activeSaved = $derived.by(() => {
    if (!searching) return null;
    const key = findsKey(currentSearch());
    return savedSearches.find((s) => findsKey(s) === key) ?? null;
  });

  /** Runs a saved search: the search bar and filters take its parts. */
  function applySaved(s: SavedSearch) {
    lastChange = null;
    inboxOpen = false;
    healthOpen = false;
    comingUpOpen = false;
    selected = null;
    query = s.text;
    selectedTags = [...s.tags];
    tagsBefore = selectedTags.length;
    tagMode = s.tagMode;
    favoritesOnly = s.favorites;
    dateFilter = s.date ? { ...s.date } : null;
    if (s.date?.kind === "range") {
      rangeFrom = s.date.from;
      rangeTo = s.date.to;
    }
    kindFilter = s.kind ?? null;
    fromFilter = s.from ?? null;
    dueFilter = s.due ?? null;
    whereFilter = s.scope ? { scope: s.scope, sourceId: s.sourceId, relPath: s.scope === "folder" ? (s.folder ?? "") : "" } : null;
    resultsByDate = s.newest;
  }

  /** What a search finds, in words ("“boat”, tagged Taxes and 2025, dated this year"). */
  function findsText(s: SavedSearch): string {
    const t = m.savedSearches;
    const parts: string[] = [];
    if (s.text.trim()) parts.push(`“${s.text.trim()}”`);
    if (s.tags.length) parts.push(s.tags.length > 1 && s.tagMode === "all" ? t.taggedAll(s.tags) : t.taggedAny(s.tags));
    if (s.favorites) parts.push(t.favorites);
    if (s.date?.kind === "thisYear") parts.push(t.thisYear);
    else if (s.date?.kind === "lastYear") parts.push(t.lastYear);
    else if (s.date?.kind === "range") parts.push(t.between(formatDocDay(s.date.from) || "…", formatDocDay(s.date.to) || "…"));
    if (s.kind && s.kind in m.kinds) parts.push(m.kinds[s.kind as keyof typeof m.kinds]);
    if (s.from) parts.push(t.from(s.from));
    if (s.due === "upcoming") parts.push(m.filters.dueUpcoming.toLowerCase());
    else if (s.due === "past") parts.push(m.filters.duePast.toLowerCase());
    if (s.scope) {
      parts.push(
        s.scope === "folder" && s.folder
          ? m.filters.inFolder(s.folder.slice(s.folder.lastIndexOf("/") + 1))
          : m.filters.inSource(sourceName(s.sourceId)),
      );
    }
    return parts.length ? parts.join(", ") : t.everything;
  }

  function openSaveSearch() {
    const s = currentSearch();
    const name = [s.text, ...s.tags, s.from].filter(Boolean).join(" · ") || m.savedSearches.untitled;
    saveDialog = { mode: "save", search: { ...s, name } };
  }

  async function saveFromDialog(name: string) {
    if (!saveDialog) return;
    await api.saveSearch({ ...saveDialog.search, name });
    await loadSavedSearches();
  }

  /** Right-click ▸ Update to the current search: keeps its name and place
   * in the list; it moves to another source if it now looks there. */
  async function updateSaved(s: SavedSearch) {
    savedMenu = null;
    try {
      await api.saveSearch({ ...currentSearch(), id: s.id, name: s.name }, s.sourceId);
      await loadSavedSearches();
    } catch (e) {
      error = String(e);
    }
  }

  async function removeSaved(s: SavedSearch) {
    savedMenu = null;
    const sure = await ask(m.savedSearches.removeAsk(s.name), { kind: "warning", okLabel: m.savedSearches.removeOk, cancelLabel: m.common.cancel });
    if (!sure) return;
    try {
      await api.removeSavedSearch(s.sourceId, s.id);
      await loadSavedSearches();
    } catch (e) {
      error = String(e);
    }
  }

  // MARK: - Coming up (#109)

  let comingUpOpen = $state(false);
  /** For the sidebar: overdue or due within 7 days. */
  let comingCounts = $state<ReminderCounts | null>(null);
  /** A document a notification asked to show, selected once listed. */
  let comingFocus = $state<string | null>(null);
  const remindersOn = $derived(appSettings?.reminders?.enabled !== false);

  function openComingUp() {
    lastChange = null;
    inboxOpen = false;
    healthOpen = false;
    query = "";
    selectedTags = [];
    favoritesOnly = false;
    selected = null;
    comingUpOpen = true;
  }

  /** A notification was clicked: Coming up, with its document selected. */
  function showReminder(t: { sourceId: string | null; relPath: string | null } | null) {
    if (!t || !remindersOn) return;
    openComingUp();
    comingFocus = t.sourceId && t.relPath ? keyOf(t.sourceId, t.relPath) : null;
  }

  let comingTimer: ReturnType<typeof setTimeout> | undefined;
  function loadComingCounts(soon = false) {
    clearTimeout(comingTimer);
    comingTimer = setTimeout(
      async () => {
        comingCounts = await api.remindersCounts().catch(() => comingCounts);
      },
      soon ? 0 : 1500,
    );
  }

  // Turned off in Settings: leave Coming up.
  $effect(() => {
    if (!remindersOn && comingUpOpen) comingUpOpen = false;
  });

  function filterFavorites() {
    healthOpen = false;
    comingUpOpen = false;
    inboxOpen = false;
    query = "";
    favoritesOnly = !favoritesOnly;
    selectedTags = [];
    tagsBefore = 0;
    selected = null;
  }

  function clearSearch() {
    query = "";
    selectedTags = [];
    tagsBefore = 0;
    favoritesOnly = false;
    dateFilter = null;
    kindFilter = null;
    whereFilter = null;
    fromFilter = null;
    dueFilter = null;
  }

  function openInbox() {
    lastChange = null;
    healthOpen = false;
    comingUpOpen = false;
    inboxOpen = true;
    selected = null;
  }

  /** After filing from the inbox: show the document where it went. */
  function showFiled(filed: Filed) {
    const name = filed.relPath.slice(filed.relPath.lastIndexOf("/") + 1);
    showInFolder({
      sourceId: filed.sourceId,
      entry: { name, relPath: filed.relPath, isDir: false, kind: null, size: 0, modifiedMs: null, cloud: "local" },
    });
  }

  async function loadInboxCount() {
    inboxCount = (await api.inboxItems()).length;
  }

  function showInFolder(sel: Selected) {
    const rel = sel.entry.relPath;
    const dir = rel.includes("/") ? rel.slice(0, rel.lastIndexOf("/")) : "";
    selectFolder(sel.sourceId, dir);
    selected = sel;
  }

  // MARK: - Source actions

  async function addSource() {
    const path = await open({ directory: true, multiple: false, title: m.main.addFolderTitle });
    if (typeof path !== "string") return;
    const added = await run(() => api.addSource(path));
    if (!added) return;
    await loadSources();
    selectFolder(added.id, "");
  }

  /** Picks an unavailable source's folder again: it moved, or (in the Mac
   * App Store edition) access has to be granted once more. */
  async function relocateSource(s: Source) {
    const path = await open({ directory: true, multiple: false, defaultPath: s.root, title: m.main.chooseSource(s.name) });
    if (typeof path !== "string") return;
    const found = await run(() => api.relocateSource(s.id, path));
    if (found) await loadSources(true);
  }

  async function removeSource(s: Source) {
    menuFor = null;
    const ok = await ask(
      m.main.removeAsk(s.name),
      { title: m.main.removeTitle, kind: "warning", okLabel: m.main.remove },
    );
    if (!ok) return;
    await run(() => api.removeSource(s.id));
    if (folder?.sourceId === s.id) {
      folder = null;
      entries = [];
    }
    if (selected?.sourceId === s.id) selected = null;
    await loadSources();
  }

  async function toggleAllTypes(s: Source) {
    menuFor = null;
    await run(() => api.updateSource(s.id, { showAllTypes: !s.showAllTypes }));
    await loadSources();
    if (folder?.sourceId === s.id) loadEntries();
  }

  async function toggleDownload(s: Source) {
    menuFor = null;
    const on = !s.downloadOnlineOnly;
    if (on) {
      const ok = await ask(
        m.main.downloadAsk(s.name),
        { title: m.main.downloadTitle, kind: "info", okLabel: m.main.start },
      );
      if (!ok) return;
    }
    await run(() => api.updateSource(s.id, { downloadOnlineOnly: on }));
    await loadSources();
  }

  function startRename(s: Source) {
    menuFor = null;
    renaming = s.id;
    renameValue = s.name;
  }

  async function finishRename(s: Source) {
    const name = renameValue.trim();
    renaming = null;
    if (!name || name === s.name) return;
    await run(() => api.updateSource(s.id, { name }));
    await loadSources();
  }

  // MARK: - Entry actions

  /** The Markdown document open in the reader. */
  let reader = $state<{ sourceId: string; relPath: string } | null>(null);

  function activate(sourceId: string, e: Entry) {
    if (!e.isDir) rememberDocument(sourceId, e.relPath);
    if (e.isDir) selectFolder(sourceId, e.relPath);
    else if (isMarkdown(e) && e.cloud !== "cloudOnly") reader = { sourceId, relPath: e.relPath };
    else run(() => api.openEntry(sourceId, e.relPath));
  }

  function reveal(sel: Selected) {
    run(() => api.revealEntry(sel.sourceId, sel.entry.relPath));
  }

  // MARK: - Right-click menu and Trash

  /** The Tags… or ⋯ menu under the selection's buttons in Details. */
  let bulkMenu = $state<"tags" | "more" | null>(null);

  /** The right-click menu: where, for what, and whether it's a search result. */
  let itemMenu = $state<{ x: number; y: number; sel: Selected; inSearch: boolean } | null>(null);
  const trashLabel = $derived(info?.os === "windows" ? m.trash.delete : m.trash.moveToTrash);
  const binName = $derived(info?.os === "windows" ? m.trash.recycleBin : m.trash.trash);

  function openItemMenu(sel: Selected, ev: MouseEvent, inSearch = false) {
    ev.preventDefault();
    ev.stopPropagation();
    menuFor = null;
    // On one of several selected: the menu is for all of them (#102).
    if (!isPicked(sel.sourceId, sel.entry.relPath)) pick(sel);
    // Keep it inside the window (the menu is about 230 × 300).
    const x = Math.min(ev.clientX, window.innerWidth - 240);
    const y = Math.min(ev.clientY, window.innerHeight - 310);
    itemMenu = { x, y, sel, inSearch };
  }

  /** Right-click on a folder in the sidebar: a source's own menu for its
   * top folder, the folder menu for the others. */
  function folderMenu(sourceId: string, relPath: string, name: string, ev: MouseEvent) {
    if (relPath === "") {
      ev.preventDefault();
      ev.stopPropagation();
      itemMenu = null;
      menuFor = sourceId;
      return;
    }
    const entry = { name, relPath, isDir: true, kind: null, size: 0, modifiedMs: null, cloud: "local" } as Entry;
    openItemMenu({ sourceId, entry }, ev);
  }

  /** "Index this folder" (#78): re-read it and download its online-only
   * documents to read them. */
  async function indexFolder(sourceId: string, relPath: string) {
    menuFor = null;
    await run(() => api.indexFolder(sourceId, relPath));
  }

  /** Runs a right-click menu command for the item it was opened on, closing
   * the menu first. (The item is read before the menu closes: the menu's
   * markup only knows it while open.) */
  function fromMenu(action: (sel: Selected) => void) {
    const sel = itemMenu?.sel;
    itemMenu = null;
    if (sel) action(sel);
  }

  /** Sends a document (with its sidecar) or a folder to the Trash / Recycle Bin.
   * Files go right away, as in the Finder; folders ask first. */
  async function trashEntry(sel: Selected) {
    itemMenu = null;
    lastChange = null;
    const e = sel.entry;
    if (e.isDir) {
      const ok = await ask(m.main.trashFolderAsk(e.name, binName), {
        title: trashLabel,
        kind: "warning",
        okLabel: trashLabel,
      });
      if (!ok) return;
    }
    const done = await run(async () => {
      await api.trashEntry(sel.sourceId, e.relPath);
      return true;
    });
    if (done) forget(sel);
  }

  /** Takes a document or folder that went away (trashed or moved) out of the
   * view. In the gallery, moves on to its neighbor (handy when sorting out
   * scans). */
  function forget(sel: Selected) {
    const e = sel.entry;
    const at = entries.findIndex((x) => x.relPath === e.relPath);
    const gone = (x: Entry & { sourceId?: string }, sourceId: string) =>
      sourceId === sel.sourceId && (x.relPath === e.relPath || x.relPath.startsWith(e.relPath + "/"));
    hits = hits.filter((h) => !gone(h, h.sourceId));
    picked = picked.filter((p) => !gone(p.entry, p.sourceId));
    if (folder?.sourceId === sel.sourceId) entries = entries.filter((x) => !gone(x, sel.sourceId));
    if (selected && gone(selected.entry, selected.sourceId)) {
      const next = folderMode === "gallery" && at >= 0 ? (entries[at] ?? entries[at - 1]) : undefined;
      selected = next ? { sourceId: sel.sourceId, entry: next } : null;
    }
  }

  // MARK: - Acting on several (#102)

  /** A batch under way: what it does and how far it is. */
  let batch = $state<{ label: (done: number, total: number) => string; done: number; total: number } | null>(null);
  /** What the last batch did ("18 moved · 2 not: …"), shown in the bar at the bottom. */
  let batchNote = $state<string | null>(null);

  /** Runs `act` on each item in turn, showing the progress, then a summary.
   * Resolves to the items it worked for. */
  async function runBatch(
    items: Selected[],
    act: (s: Selected) => Promise<unknown>,
    label: (done: number, total: number) => string,
    summary: (ok: number) => string,
  ): Promise<Selected[]> {
    lastChange = null;
    batchNote = null;
    error = null;
    const ok: Selected[] = [];
    const failed: string[] = [];
    batch = { label, done: 0, total: items.length };
    for (const it of items) {
      try {
        await act(it);
        ok.push(it);
      } catch (e) {
        failed.push(`${it.entry.name}: ${e}`);
      }
      batch = { label, done: batch.done + 1, total: items.length };
    }
    batch = null;
    batchNote = summary(ok.length) + (failed.length ? ` · ${m.several.notDone(failed.length)}: ${failed[0]}` : "");
    return ok;
  }

  /** Move to Trash for what's selected: one question for several. */
  async function trashPicked() {
    const items = picked.slice();
    itemMenu = null;
    if (items.length < 2) {
      if (selected) trashEntry(selected);
      return;
    }
    const ok = await ask(m.several.trashAsk(items.length, binName), { title: trashLabel, kind: "warning", okLabel: trashLabel });
    if (!ok) return;
    const bin = binName;
    const done = await runBatch(
      items,
      (it) => api.trashEntry(it.sourceId, it.entry.relPath),
      (d, t) => m.several.trashing(d, t, bin),
      (n) => m.several.trashed(n, bin),
    );
    done.forEach(forget);
  }

  /** Stars the selected documents, or unstars them if all are starred. */
  async function starPicked() {
    itemMenu = null;
    const docs = picked.filter((p) => !p.entry.isDir) as { sourceId: string; entry: ListedEntry }[];
    const on = docs.some((d) => !d.entry.favorite);
    await runBatch(
      docs,
      async (it) => {
        const meta = await api.setFavorite(it.sourceId, it.entry.relPath, on);
        (it.entry as ListedEntry).favorite = meta.favorite;
      },
      on ? m.several.starring : m.several.unstarring,
      on ? m.several.starred : m.several.unstarred,
    );
    loadIndexInfo();
  }

  /** Add tags… / Remove tags… for the selected documents. */
  let bulkTags = $state<{ mode: "add" | "remove"; docs: { sourceId: string; entry: ListedEntry }[] } | null>(null);

  function openBulkTags(mode: "add" | "remove") {
    itemMenu = null;
    bulkTags = { mode, docs: picked.filter((p) => !p.entry.isDir) as { sourceId: string; entry: ListedEntry }[] };
  }

  function bulkTagsDone(report: TagEditReport) {
    lastChange = null;
    const text =
      m.tags.changed(report.changed) +
      (report.failed.length ? ` · ${m.several.notDone(report.failed.length)}: ${report.failed[0].path}: ${report.failed[0].error}` : "");
    rememberTags(report, text);
    if (!lastChange) batchNote = text;
    if (folder) loadEntries();
    loadIndexInfo();
  }

  /** Share… and Email… for the selected documents: one share, one email. */
  function pickedDocs() {
    return picked.filter((p) => !p.entry.isDir).map((p) => ({ sourceId: p.sourceId, relPath: p.entry.relPath }));
  }
  function sharePicked(at: ShareAt) {
    itemMenu = null;
    lastChange = null;
    run(() => api.shareDocuments(pickedDocs(), at));
  }
  function emailPicked() {
    itemMenu = null;
    lastChange = null;
    run(() => api.emailDocuments(pickedDocs()));
  }

  /** Rename documents… with just the selected documents ticked (they're in
   * the open folder: it isn't offered for search results). */
  function renamePicked() {
    itemMenu = null;
    lastChange = null;
    const docs = picked.filter((p) => !p.entry.isDir);
    if (!docs.length || !folder) return;
    renameDocs = { sourceId: folder.sourceId, relDir: folder.relPath, only: docs.map((d) => d.entry.relPath) };
  }

  /** The selected documents' details at a glance (Details with several). */
  const pickedSummary = $derived.by(() => {
    const docs = picked.filter((p) => !p.entry.isDir);
    const tagLists = docs.map((d) => (d.entry as ListedEntry).tags ?? []);
    const common = tagLists.length ? tagLists.reduce((a, b) => a.filter((t) => b.includes(t))) : [];
    return {
      docs: docs.length,
      folders: picked.length - docs.length,
      size: docs.reduce((n, d) => n + (d.entry.size || 0), 0),
      common,
      allStarred: docs.length > 0 && docs.every((d) => (d.entry as ListedEntry).favorite),
      totals: addUp(docs.map((d) => d.entry as ListedEntry)),
    };
  });

  // MARK: - List columns (#138)

  let documentsWidth = $state(0);
  /** The columns shown, in order. In a narrow middle Kind steps aside
   * first, so names keep their room (as before #138). */
  const shownColumns = $derived(
    listColumns.order.filter(
      (c) => !listColumns.hidden.includes(c) && !(c === "kind" && documentsWidth > 0 && documentsWidth < 640),
    ),
  );
  /** Below this the list scrolls sideways: Name keeps its smallest width. */
  const listMinWidth = $derived(shownColumns.reduce((sum, c) => sum + (columnWidth(c) ?? NAME_MIN), 0));
  const favoriteFolderKeys = $derived(new Set(favoriteFolders.map((f) => keyOf(f.sourceId, f.relPath))));
  const isFavoriteFolder = (sourceId: string, relPath: string) => favoriteFolderKeys.has(keyOf(sourceId, relPath));

  /** A subfolder's star in the list: a favorite folder, written to the
   * folder's own .rupert/.folder.json (as the star by the folder path). */
  async function toggleSubfolderFav(sourceId: string, relPath: string) {
    const next = !isFavoriteFolder(sourceId, relPath);
    try {
      error = null;
      await api.setFolderFavorite(sourceId, relPath, next);
    } catch (e) {
      error = String(e);
    }
    favoriteFolders = await api.favoriteFolders();
  }

  // MARK: - Page preview in Details (#127)

  let detailsH = $state(0);
  let detailsW = $state(0);
  let detailsBodyH = $state(0);
  /** The gallery shows the page large already. */
  const galleryShown = $derived(!searching && !!folder && folderMode === "gallery" && entries.length > 0);
  /** The document whose page Details shows: one PDF or picture selected,
   * outside the gallery. */
  const pagePreviewFor = $derived(
    selected && !several && !selected.entry.isDir && !galleryShown && (selected.entry.kind === "pdf" || selected.entry.kind === "image")
      ? selected
      : null,
  );

  // MARK: - Page viewer (#83)

  /** The document shown large in the page viewer: from the gallery, or
   * Space on the selected document in any view (#105). */
  let viewer = $state<{
    sourceId: string;
    entry: ListedEntry;
    page: number;
    /** Words to mark (#106): the search's, or a cited passage. */
    find?: { text: string; phrase: boolean } | null;
  } | null>(null);

  /** In search results, the viewer marks the search's words (#106). */
  const searchWords = () => (searching && query.trim() ? { text: query.trim(), phrase: false } : null);

  /** Space: the selected document in the page viewer (a Markdown file in
   * the reader). Folders have nothing to show. */
  function preview(sel: Selected, page = 0) {
    if (sel.entry.isDir) return;
    rememberDocument(sel.sourceId, sel.entry.relPath);
    if (isMarkdown(sel.entry) && sel.entry.cloud !== "cloudOnly") reader = { sourceId: sel.sourceId, relPath: sel.entry.relPath };
    else viewer = { sourceId: sel.sourceId, entry: sel.entry as ListedEntry, page, find: searchWords() };
  }

  /** A page an answer cites (#106): the document shown in its folder and
   * open in the page viewer at that page, the passage marked. */
  async function openCitation(c: Citation) {
    const [doc] = await api.documentsInfo([{ sourceId: c.sourceId, relPath: c.relPath }]).catch(() => []);
    if (!doc) return;
    const cut = c.relPath.lastIndexOf("/");
    pendingSelect = c.relPath;
    selectFolder(c.sourceId, cut < 0 ? "" : c.relPath.slice(0, cut));
    rememberDocument(c.sourceId, c.relPath);
    viewer = {
      sourceId: c.sourceId,
      entry: doc,
      // Online-only: only its first page has a kept picture.
      page: doc.cloud === "cloudOnly" ? 0 : c.page,
      find: c.passage ? { text: c.passage, phrase: true } : null,
    };
  }

  /** ↑ ↓ in the page viewer: the neighboring document of the view, which
   * becomes the selection too. Folders are passed over. */
  function previewStep(step: 1 | -1) {
    if (!viewer) return;
    const list = shownItems().filter((x) => !x.entry.isDir);
    const at = list.findIndex((x) => x.sourceId === viewer!.sourceId && x.entry.relPath === viewer!.entry.relPath);
    const next = at < 0 ? undefined : list[at + step];
    if (!next) return;
    pick(next);
    if (isMarkdown(next.entry) && next.entry.cloud !== "cloudOnly") {
      viewer = null;
      reader = { sourceId: next.sourceId, relPath: next.entry.relPath };
    } else {
      viewer = { sourceId: next.sourceId, entry: next.entry as ListedEntry, page: 0, find: viewer.find };
    }
    tick().then(() =>
      document.querySelector(`[data-pick="${CSS.escape(keyOf(next.sourceId, next.entry.relPath))}"]`)?.scrollIntoView({ block: "nearest" }),
    );
  }

  // MARK: - Export (#107)

  /** The documents being exported, and the name for the bundle. */
  let exporting = $state<{ docs: DocPlace[]; title: string } | null>(null);

  /** Export… for documents and folders (a folder brings everything in it). */
  async function startExport(items: Selected[], title: string) {
    exporting = { docs: await collectDocs(items), title };
  }

  /** The documents of a selection, a folder's all below it, once each. */
  async function collectDocs(items: Selected[]): Promise<DocPlace[]> {
    itemMenu = null;
    menuFor = null;
    const seen = new Set<string>();
    const docs: DocPlace[] = [];
    const add = (d: DocPlace) => {
      const k = keyOf(d.sourceId, d.relPath);
      if (!seen.has(k)) {
        seen.add(k);
        docs.push(d);
      }
    };
    for (const it of items) {
      if (it.entry.isDir) {
        const inside = await run(() => api.exportFolderDocs(it.sourceId, it.entry.relPath));
        for (const d of inside ?? []) add(d);
      } else {
        add({ sourceId: it.sourceId, relPath: it.entry.relPath });
      }
    }
    return docs;
  }

  // MARK: - Read details (#108)

  /** The documents whose From, amount and due date are being read. */
  let readingDetails = $state<{ docs: DocPlace[]; title: string } | null>(null);

  async function startReadDetails(items: Selected[], title: string) {
    readingDetails = { docs: await collectDocs(items), title };
  }

  /** A name for what's selected: the folder they're in, or the one folder. */
  function exportTitle(items: Selected[]): string {
    if (items.length === 1) {
      const e = items[0].entry;
      if (e.isDir) return e.relPath ? e.name : sourceName(items[0].sourceId);
      return e.name.replace(/\.[^.]+$/, "");
    }
    if (searching) return searchTitle();
    const here = folder?.relPath ? folder.relPath.slice(folder.relPath.lastIndexOf("/") + 1) : folder ? sourceName(folder.sourceId) : "";
    return here || m.exportDocs.defaultName;
  }

  /** The search's words and tags, to name its export ("Taxes 2025"). */
  function searchTitle(): string {
    return [query.trim(), ...selectedTags].filter(Boolean).join(" ") || m.exportDocs.defaultName;
  }

  // MARK: - Quick open (#105)

  let quickOpen = $state(false);

  /** What quick open offers besides folders, documents and tags. */
  const quickCommands = $derived.by((): QuickCommand[] => {
    const mac = info?.os === "macos";
    const list: QuickCommand[] = [];
    if (appSettings?.inbox) {
      list.push({ id: "inbox", label: m.main.inbox });
      list.push({ id: "import", label: m.common.ellipsis(m.importInbox.title), keys: mac ? "⇧⌘I" : undefined });
      list.push({ id: "scan", label: m.scanPhone.title });
    }
    list.push({ id: "favorites", label: m.quickOpen.starred });
    list.push({ id: "tags", label: m.main.manageTags });
    list.push({ id: "ask", label: m.main.askRupert, keys: mac ? "⌘J" : "Ctrl+J" });
    list.push({ id: "settings", label: m.main.settings, keys: mac ? "⌘," : "Ctrl+," });
    list.push({ id: "help", label: m.help.title, keys: "F1" });
    return list;
  });

  function quickDocument(hit: Hit, open: boolean) {
    rememberDocument(hit.sourceId, hit.relPath);
    if (open) {
      activate(hit.sourceId, hit);
      return;
    }
    const cut = hit.relPath.lastIndexOf("/");
    pendingSelect = hit.relPath;
    selectFolder(hit.sourceId, cut < 0 ? "" : hit.relPath.slice(0, cut));
  }

  function quickTag(tag: string) {
    inboxOpen = false;
    query = "";
    favoritesOnly = false;
    selectedTags = [tag];
    tagsBefore = 1;
    selected = null;
  }

  function quickCommand(id: string) {
    if (id === "inbox") openInbox();
    else if (id === "import") {
      openInbox();
      importInboxOpen = true;
    } else if (id === "scan") scanOpen = true;
    else if (id === "favorites") {
      if (!favoritesOnly) filterFavorites();
    } else if (id === "tags") tagManagerOpen = true;
    else if (id === "ask") assistant?.show();
    else if (id === "settings") api.openSettings();
    else if (id === "help") api.openHelp();
  }

  // MARK: - Renaming (#65)

  /** The document or folder whose name is being edited in place. */
  let renamingAt = $state<{ sourceId: string; relPath: string } | null>(null);
  const isRenaming = (sourceId: string, relPath: string) =>
    renamingAt?.sourceId === sourceId && renamingAt.relPath === relPath;

  /** Starts editing a name: in a folder's listing or the sidebar. */
  function startRenaming(sel: Selected) {
    itemMenu = null;
    lastChange = null;
    renamingAt = { sourceId: sel.sourceId, relPath: sel.entry.relPath };
  }

  /** Renames on disk, then shows the new name (and follows a renamed
   * folder that was open, or holds the open one). */
  async function finishRenaming(sourceId: string, relPath: string, name: string) {
    renamingAt = null;
    const newRel = await run(() => api.renameEntry(sourceId, relPath, name));
    if (!newRel) return;
    // Offered for Undo (#104): back under its old name.
    if (newRel !== relPath) {
      const shown = newRel.slice(newRel.lastIndexOf("/") + 1);
      lastChange = {
        kind: "move",
        moves: [{ nowSourceId: sourceId, nowRelPath: newRel, wasSourceId: sourceId, wasRelPath: relPath }],
        text: m.undo.renamedTo(shown),
        label: m.undo.labelRename(1),
      };
    }
    const moved = (p: string) => (p === relPath || p.startsWith(relPath + "/") ? newRel + p.slice(relPath.length) : p);
    if (folder?.sourceId === sourceId && moved(folder.relPath) !== folder.relPath) {
      selectFolder(sourceId, moved(folder.relPath));
      return;
    }
    if (selected?.sourceId === sourceId && selected.entry.relPath === relPath) pendingSelect = newRel;
    if (folder?.sourceId === sourceId) loadEntries();
  }


  // MARK: - Rename documents (#64)

  /** The folder whose documents are being renamed together. */
  let renameDocs = $state<{ sourceId: string; relDir: string; only: string[] | null } | null>(null);
  // MARK: - Undo (#104)

  /** The last change Rupert made, offered for Undo (the notice at the
   * bottom, Edit ▸ Undo, ⌘Z / Ctrl+Z) until something else is done. One
   * level, not kept across restarts. `text` is the notice, `label` the
   * menu's "Undo …". */
  type Change = { text: string; label: string } & (
    | { kind: "rename"; sourceId: string; relDir: string; renamed: Renamed[] }
    | { kind: "move"; moves: MoveBackItem[] }
    | { kind: "file"; moves: MoveBackItem[] }
    | { kind: "tags"; changes: TagChange[] }
  );
  let lastChange = $state<Change | null>(null);
  /** A short note after Undo. */
  let renameNote = $state<string | null>(null);
  /** Counted up when a filing is undone, so the inbox drops its notice. */
  let filingUndone = $state(0);

  // View ▸ Hide/Show Page Preview says what it would do (#127).
  $effect(() => {
    api.setPreviewMenu(!pagePreview.hidden).catch(() => {});
  });

  $effect(() => {
    api.setUndoLabel(lastChange?.label ?? null).catch(() => {});
  });

  /** Moves made just now (one or several), offered for Undo. */
  function rememberMoves(moves: MoveBackItem[], destSourceId: string, destDir: string, failed = "") {
    if (!moves.length) return;
    const where = destDir ? destDir.slice(destDir.lastIndexOf("/") + 1) : sourceName(destSourceId);
    lastChange = { kind: "move", moves, text: m.undo.moved(moves.length, where) + failed, label: m.undo.labelMove(moves.length) };
  }

  /** A tag edit just made (the tag manager, Add/Remove tags…), offered for Undo. */
  function rememberTags(report: TagEditReport, text: string) {
    lastChange = report.changes.length
      ? { kind: "tags", changes: report.changes, text, label: m.undo.labelTags(report.changes.length) }
      : null;
  }

  async function undoLast() {
    const c = lastChange;
    if (!c) return;
    lastChange = null;
    batchNote = null;
    if (c.kind === "rename") {
      const outcome = await run(() => api.renameUndo(c.sourceId, c.renamed));
      if (!outcome) return;
      renameNote = m.renameDocs.undone(outcome.renamed.length);
      if (outcome.failed.length) error = `${m.renameDocs.failed(outcome.failed.length)}: ${outcome.failed[0].message}`;
      followRenamed(c.sourceId, outcome.renamed.map((r) => [r.from, r.to]));
    } else if (c.kind === "move" || c.kind === "file") {
      const r = await run(() => api.moveBack(c.moves));
      if (!r) return;
      const renaming =
        c.moves.length === 1 &&
        c.moves[0].nowSourceId === c.moves[0].wasSourceId &&
        parentDir(c.moves[0].nowRelPath) === parentDir(c.moves[0].wasRelPath);
      renameNote = c.kind === "file" ? m.undo.backInInbox : renaming ? m.undo.renamedBack : m.undo.movedBack(r.done);
      if (r.failed.length) error = `${m.undo.notBack(r.failed.length)}: ${r.failed[0].error}`;
      if (c.kind === "file") {
        filingUndone++;
        loadInboxCount();
      }
      if (folder) loadEntries();
    } else {
      const r = await run(() => api.tagUndo(c.changes));
      if (!r) return;
      const left = c.changes.length - r.changed - r.failed.length;
      renameNote = m.undo.tagsBack(r.changed) + (left > 0 ? ` · ${m.undo.changedSince(left)}` : "");
      if (r.failed.length) error = `${m.several.notDone(r.failed.length)}: ${r.failed[0].path}: ${r.failed[0].error}`;
      if (folder) loadEntries();
      loadIndexInfo();
    }
    setTimeout(() => (renameNote = null), 6000);
  }

  const parentDir = (relPath: string) => (relPath.includes("/") ? relPath.slice(0, relPath.lastIndexOf("/")) : "");

  /** From a folder: all its documents; from a document: its folder's, only it ticked. */
  function openRenameDocs(sel: Selected) {
    itemMenu = null;
    menuFor = null;
    lastChange = null;
    const e = sel.entry;
    renameDocs = e.isDir
      ? { sourceId: sel.sourceId, relDir: e.relPath, only: null }
      : { sourceId: sel.sourceId, relDir: parentDir(e.relPath), only: [e.relPath] };
  }

  function whereLabel(sourceId: string, relDir: string): string {
    return [sourceName(sourceId), ...(relDir ? relDir.split("/") : [])].join(" › ");
  }

  function renamedDocs(sourceId: string, relDir: string, outcome: RenameOutcome) {
    const folderName = relDir ? relDir.slice(relDir.lastIndexOf("/") + 1) : sourceName(sourceId);
    let text = m.renameDocs.done(outcome.renamed.length, folderName);
    if (outcome.failed.length) text += ` · ${m.renameDocs.failed(outcome.failed.length)}: ${outcome.failed[0].message}`;
    lastChange = outcome.renamed.length
      ? { kind: "rename", sourceId, relDir, renamed: outcome.renamed, text, label: m.undo.labelRename(outcome.renamed.length) }
      : null;
    if (!outcome.renamed.length && outcome.failed.length) error = text;
    followRenamed(sourceId, outcome.renamed.map((r) => [r.from, r.to]));
  }

  /** Keeps the selection on a renamed document and shows the new names. */
  function followRenamed(sourceId: string, moves: string[][]) {
    if (selected?.sourceId === sourceId) {
      const moved = moves.find(([from]) => from === selected!.entry.relPath);
      if (moved) pendingSelect = moved[1];
    }
    if (folder?.sourceId === sourceId) loadEntries();
  }

  // MARK: - Share… and Email… (#69)

  /** The online-only document being downloaded to send it. */
  let sendingName = $state<string | null>(null);

  function shareDoc(sel: Selected, at: ShareAt) {
    lastChange = null;
    run(() => api.shareDocument(sel.sourceId, sel.entry.relPath, null, at));
  }

  function emailDoc(sel: Selected) {
    lastChange = null;
    run(() => api.emailDocument(sel.sourceId, sel.entry.relPath, null));
  }
  // MARK: - Moving

  /** What "Move to…" is choosing a place for: one item, or the selection. */
  let moving = $state<Selected[] | null>(null);

  /** Moves a document (with its details) or a folder into another folder,
   * in this source or another. Resolves to whether it worked. */
  // The star and the cloud on a document in a list.
  function starDoc(sourceId: string, e: Entry & { favorite?: boolean }) {
    run(() => docActions.toggleFavorite(sourceId, e));
  }
  function downloadDoc(sourceId: string, e: Entry) {
    run(() => docActions.download(sourceId, e));
  }

  // Dragging several onto a folder moves them as one batch.
  onMoveMany((items, sourceId, relPath) => movePicked(items, sourceId, relPath));

  /** Moves several items into a folder, one by one, with progress and a
   * summary (#102). */
  async function movePicked(items: Selected[], destSourceId: string, destDir: string) {
    const moves: MoveBackItem[] = [];
    const done = await runBatch(
      items,
      async (it) => {
        const now = await api.moveEntry(it.sourceId, it.entry.relPath, destSourceId, destDir);
        moves.push({ nowSourceId: destSourceId, nowRelPath: now, wasSourceId: it.sourceId, wasRelPath: it.entry.relPath });
      },
      m.several.moving,
      m.several.moved,
    );
    done.forEach(forget);
    if (folder?.sourceId === destSourceId && folder.relPath === destDir) loadEntries();
    // The notice says where they went, with Undo (#104), and any that didn't move.
    const note = batchNote ?? "";
    batchNote = null;
    rememberMoves(moves, destSourceId, destDir, note.includes(" · ") ? note.slice(note.indexOf(" · ")) : "");
  }

  async function moveItem(item: MoveItem, destSourceId: string, destDir: string): Promise<boolean> {
    const now = await run(() => api.moveEntry(item.sourceId, item.entry.relPath, destSourceId, destDir));
    if (!now) return false;
    forget(item);
    if (folder?.sourceId === destSourceId && folder.relPath === destDir) loadEntries();
    if (now !== item.entry.relPath || item.sourceId !== destSourceId) {
      rememberMoves(
        [{ nowSourceId: destSourceId, nowRelPath: now, wasSourceId: item.sourceId, wasRelPath: item.entry.relPath }],
        destSourceId,
        destDir,
      );
    }
    return true;
  }

  // MARK: Files dropped from other apps

  let copying = $state<{ done: number; total: number } | null>(null);
  /** What couldn't be copied. Kept apart from `error`, which every reload
   * of the folder clears. */
  let copyProblems = $state<string | null>(null);

  /** Copies files dropped from another app into a folder (or the inbox),
   * keeping their names, then shows where they went. */
  async function copyFiles(dropped: Dropped, sourceId: string, relPath: string) {
    copyProblems = null;
    const problems = dropped.folders.map((name) => m.main.folderNotCopied(name));
    let copied = 0;
    copying = { done: 0, total: dropped.files.length };
    for (const file of dropped.files) {
      try {
        await api.copyIn(sourceId, relPath, file);
        copied++;
      } catch (e) {
        problems.push(m.main.copyFailed(file.name, String(e)));
      }
      copying = { done: copying.done + 1, total: dropped.files.length };
    }
    copying = null;
    const inbox = appSettings?.inbox;
    if (!copied) {
      // Nothing to show.
    } else if (inbox?.sourceId === sourceId && inbox.relPath === relPath) {
      loadInboxCount();
    } else if (!inboxOpen && folder?.sourceId === sourceId && folder.relPath === relPath) {
      loadEntries();
    } else {
      selectFolder(sourceId, relPath);
    }
    copyProblems = problems.length ? problems.join(" ") : null;
  }

  /** Drops onto the inbox (the sidebar item, or the inbox itself). */
  const inboxDrop = $derived(
    appSettings?.inbox
      ? dropTarget(appSettings.inbox.sourceId, appSettings.inbox.relPath, {
          files: (d) => copyFiles(d, appSettings!.inbox!.sourceId, appSettings!.inbox!.relPath),
        })
      : {},
  );
  const inboxDropOver = $derived(!!appSettings?.inbox && isDropOver(appSettings.inbox.sourceId, appSettings.inbox.relPath));

  function dirOf(relPath: string) {
    return relPath.includes("/") ? relPath.slice(0, relPath.lastIndexOf("/")) : "";
  }

  /** Indexing or downloading that Stop can halt (#78). */
  const stoppable = $derived(
    !!progress &&
      !progress.stopped &&
      ((progress.active && progress.total > 0) || progress.downloadTotal > 0 || progress.pictureTotal > 0),
  );

  function statusText(): string {
    if (batch) return batch.label(batch.done, batch.total);
    if (copying && copying.total > 0) return m.main.copying(copying.done, copying.total);
    if (progress?.stopped) return m.main.indexingStopped;
    if (progress?.active && progress.total > 0) {
      return m.main.indexing(progress.done, progress.total);
    }
    if (progress?.active) return m.main.checkingChanges;
    if (progress && progress.downloadTotal > 0) {
      return m.main.downloadingToRead(progress.downloadDone, progress.downloadTotal);
    }
    if (progress && progress.pictureTotal > 0) {
      return m.main.keepingPictures(progress.pictureDone, progress.pictureTotal);
    }
    return countsText();
  }

  /** The library's counts, when the status line has nothing else to say:
   * a click opens Library health (#113). */
  function countsText(): string {
    if (!stats || stats.documents === 0) return "";
    const parts = [m.main.documents(stats.documents)];
    parts.push(m.main.searchable(stats.withText));
    if (stats.cloudOnlyUnindexed) parts.push(m.main.onlineOnlyCount(stats.cloudOnlyUnindexed));
    return parts.join(" · ");
  }

  /** The sidebar's Library health badge: rows with something to fix. Read
   * at most every few seconds while the index changes. */
  let healthTimer: ReturnType<typeof setTimeout> | undefined;
  function loadHealthBadge() {
    clearTimeout(healthTimer);
    healthTimer = setTimeout(async () => {
      const c = await api.healthCounts(null).catch(() => null);
      if (c) healthBadge = Object.values(c).filter((n) => n > 0).length;
    }, 3000);
  }
</script>

<!-- Files dropped where no folder takes them are refused, rather than
     opened by the web view. -->
<svelte:window
  bind:innerWidth={windowWidth}
  ondragover={(ev) => {
    if (ev.dataTransfer?.types.includes("Files")) {
      ev.preventDefault();
      ev.dataTransfer.dropEffect = "none";
    }
  }}
  ondrop={(ev) => {
    if (ev.dataTransfer?.types.includes("Files")) ev.preventDefault();
  }}
/>

<!-- What can be done to several at once (#102): in Details and the
     right-click menu. -->
{#snippet bulkActions(kind: "buttons" | "menu")}
  {@const sm = pickedSummary}
  {#if kind === "menu"}
    <!-- The right-click menu: every action as text. -->
    <button role="menuitem" onclick={() => ((itemMenu = null), (moving = picked.slice()))}>{m.main.moveTo}</button>
    {#if sm.docs}
      <button role="menuitem" onclick={() => openBulkTags("add")}>{m.several.addTags}</button>
      <button role="menuitem" onclick={() => openBulkTags("remove")}>{m.several.removeTags}</button>
      <button role="menuitem" onclick={starPicked}>{sm.allStarred ? "☆ " + m.several.unstar : "★ " + m.several.star}</button>
      <hr />
      {#if info?.os !== "linux"}
        <button role="menuitem" onclick={() => sharePicked(itemMenu ? { x: itemMenu.x, y: itemMenu.y } : { x: 0, y: 0 })}>{m.share.share}</button>
      {/if}
      <button role="menuitem" onclick={emailPicked}>{m.share.email}</button>
      {#if !searching}<button role="menuitem" onclick={renamePicked}>{m.renameDocs.menu}</button>{/if}
    {/if}
    <button role="menuitem" onclick={() => startExport(picked.slice(), exportTitle(picked))}>{m.common.ellipsis(m.exportDocs.export)}</button>
    <button role="menuitem" onclick={() => startReadDetails(picked.slice(), exportTitle(picked))}>{m.common.ellipsis(m.readDetails.menu)}</button>
    <hr />
    <button role="menuitem" class="danger" onclick={trashPicked}>{trashLabel}</button>
  {:else}
    <!-- Details: icons with tooltips on top (the trash can on its own at
         the end, the rarer actions under ⋯), then the two main actions as
         words. -->
    <div class="bulk-row icons">
      {#if sm.docs}
        <button
          class="icon-action star-action"
          class:on={sm.allStarred}
          onclick={starPicked}
          title={sm.allStarred ? m.several.unstar : m.several.star}
          aria-label={sm.allStarred ? m.several.unstar : m.several.star}>{sm.allStarred ? "★" : "☆"}</button
        >
        {#if info?.os !== "linux"}
          <button
            class="icon-action"
            onclick={(ev) => sharePicked(atElement(ev.currentTarget))}
            title={m.common.ellipsis(m.share.share)}
            aria-label={m.share.share}><Icon name="share" size={16} /></button
          >
        {/if}
        <button class="icon-action" onclick={emailPicked} title={m.common.ellipsis(m.share.email)} aria-label={m.share.email}
          ><Icon name="mail" size={16} /></button
        >
      {/if}
      <button
        class="icon-action"
        onclick={() => startExport(picked.slice(), exportTitle(picked))}
        title={m.common.ellipsis(m.exportDocs.export)}
        aria-label={m.exportDocs.export}><Icon name="export" size={16} /></button
      >
      <span class="drop">
        <button
          class="icon-action"
          onclick={(e) => (e.stopPropagation(), (bulkMenu = bulkMenu === "more" ? null : "more"))}
          title={m.several.more}
          aria-label={m.several.more}
          aria-haspopup="menu"
          aria-expanded={bulkMenu === "more"}><Icon name="more" size={16} /></button
        >
        {#if bulkMenu === "more"}
          <div class="drop-menu" role="menu">
            {#if sm.docs && !searching}
              <button role="menuitem" onclick={() => ((bulkMenu = null), renamePicked())}>{m.renameDocs.menu}</button>
            {/if}
            <button role="menuitem" onclick={() => ((bulkMenu = null), startReadDetails(picked.slice(), exportTitle(picked)))}
              >{m.common.ellipsis(m.readDetails.menu)}</button
            >
          </div>
        {/if}
      </span>
      <span class="grow"></span>
      <button class="icon-action danger" onclick={trashPicked} title={m.common.ellipsis(trashLabel)} aria-label={trashLabel}
        ><Icon name="trash" size={16} /></button
      >
    </div>
    <div class="bulk-row">
      <button class="primary" onclick={() => (moving = picked.slice())}>{m.main.moveTo}</button>
      {#if sm.docs}
        <span class="drop">
          <button
            onclick={(e) => (e.stopPropagation(), (bulkMenu = bulkMenu === "tags" ? null : "tags"))}
            aria-haspopup="menu"
            aria-expanded={bulkMenu === "tags"}>{m.common.ellipsis(m.several.tags)} ▾</button
          >
          {#if bulkMenu === "tags"}
            <div class="drop-menu" role="menu">
              <button role="menuitem" onclick={() => ((bulkMenu = null), openBulkTags("add"))}>{m.several.addTags}</button>
              <button role="menuitem" onclick={() => ((bulkMenu = null), openBulkTags("remove"))}>{m.several.removeTags}</button>
            </div>
          {/if}
        </span>
      {/if}
    </div>
  {/if}
{/snippet}

<div
  class="shell"
  class:inbox-mode={inboxOpen}
  style:--side-width="{sideWidth}px"
  style:--details-width="{rightWidth}px"
  style:--inbox-side="{rightWidth}px"
>
  <Splitter column="side" width={sideWidth} max={windowWidth - rightWidth - MIDDLE} at="left" label={m.main.resizeLibrary} />
  <Splitter
    column={rightColumn}
    width={rightWidth}
    max={windowWidth - sideWidth - MIDDLE}
    at="right"
    inset={rightColumn === "inbox" ? INBOX_PADDING : 0}
    label={rightColumn === "inbox" ? m.main.resizeFileIt : m.main.resizeDetails}
  />
  <!-- MARK: Sidebar -->
  <aside class="sources">
    <header>
      <h2>{m.main.library}</h2>
    </header>
    <button class="lib-item" class:active={inboxOpen} class:drop={inboxDropOver} onclick={openInbox} {...inboxDrop}>
      <span class="lib-icon inbox-icon"><Icon name="inbox" size={14} /></span>
      <span class="label">{m.main.inbox}</span>
      {#if !appSettings?.inbox}<span class="count">{m.main.setUp}</span>{:else if inboxCount}<span class="count">{shortCount(inboxCount)}</span>{/if}
    </button>
    <!-- Coming up (#109): documents with a due date soon. -->
    {#if remindersOn}
      <button class="lib-item" class:active={comingUpOpen} onclick={openComingUp}>
        <span class="lib-icon coming-icon"><Icon name="calendar" size={14} /></span>
        <span class="label">{m.reminders.title}</span>
        {#if comingCounts?.soon}<span class="count" class:alert={comingCounts.overdue > 0}>{shortCount(comingCounts.soon)}</span>{/if}
      </button>
    {/if}
    <!-- Library health (#113): what could be better, and the fixes. -->
    <button class="lib-item" class:active={healthOpen} onclick={openHealth}>
      <span class="lib-icon health-icon">♥</span>
      <span class="label">{m.health.title}</span>
      {#if healthBadge}<span class="count">{shortCount(healthBadge)}</span>{/if}
    </button>
    <button class="lib-item" class:active={favoritesOnly} onclick={filterFavorites}>
      <span class="lib-icon star">★</span>
      <span class="label">{m.main.favorites}</span>
      {#if stats?.favorites}<span class="count">{shortCount(stats.favorites)}</span>{/if}
    </button>
    {#each favoriteFolders as f (f.sourceId + "/" + f.relPath)}
      <button
        class="lib-item fav-folder"
        class:active={shownFolder?.sourceId === f.sourceId && shownFolder?.relPath === f.relPath}
        onclick={() => selectFolder(f.sourceId, f.relPath)}
        title="{f.sourceName}{f.relPath ? ' / ' + f.relPath : ''}"
      >
        <span class="lib-icon folder-icon"><Icon name="folder" size={14} /></span>
        <span class="label">{f.name}</span>
      </button>
    {/each}
    <!-- Saved searches (#110): a section of their own. -->
    {#if savedSearches.length}
      <h2 class="side-section">{m.savedSearches.title}</h2>
      {#each savedSearches as s (s.sourceId + "\n" + s.id)}
        <button
          class="lib-item"
          class:active={activeSaved?.id === s.id && activeSaved?.sourceId === s.sourceId}
          title={findsText(s)}
          onclick={() => applySaved(s)}
          oncontextmenu={(e) => {
            e.preventDefault();
            e.stopPropagation();
            savedMenu = { x: e.clientX, y: e.clientY, s };
          }}
        >
          <span class="lib-icon saved-icon"><Icon name="search" size={14} /></span>
          <span class="label">{s.name}</span>
        </button>
      {/each}
    {/if}
    <div class="view-switch" role="tablist" aria-label={m.main.showInSidebar}>
      <button
        role="tab"
        aria-selected={sidebarView === "folders"}
        aria-label={m.main.folders}
        title={m.main.folders}
        class:on={sidebarView === "folders"}
        onclick={() => setSidebarView("folders")}
      >
        <Icon name="folder" size={16} />
      </button>
      <button
        role="tab"
        aria-selected={sidebarView === "tags"}
        aria-label={selectedTags.length ? m.main.tagsSelected(selectedTags.length) : m.main.tags}
        title={m.main.tags}
        class:on={sidebarView === "tags"}
        onclick={() => setSidebarView("tags")}
      >
        <Icon name="tag" size={16} />
        {#if selectedTags.length}<span class="badge">{shortCount(selectedTags.length)}</span>{/if}
      </button>
    </div>

    {#if sidebarView === "tags"}
      <div class="tag-pane" role="tabpanel">
        <TagFilter
          bind:selected={selectedTags}
          bind:mode={tagMode}
          refresh={indexTick}
          limit={25}
          onchange={tagsChanged}
        />
        <button class="manage-tags" onclick={() => (tagManagerOpen = true)}>
          {m.main.manageTags}{#if tags.length}&nbsp;<span class="count">{m.main.inAll(tags.length)}</span>{/if}
        </button>
      </div>
    {:else}
    <header class="sources-header">
      <h2>{m.main.sources}</h2>
      <button class="icon-btn" onclick={addSource} title={m.main.addFolder} aria-label={m.main.addFolder}>
        <Icon name="plus" />
      </button>
    </header>

    <div role="tree" aria-label={m.main.sources}>
      {#each sources as s (s.id)}
        {#if renaming === s.id}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="rename"
            bind:value={renameValue}
            autofocus
            onblur={() => finishRename(s)}
            onkeydown={(e) => {
              if (e.key === "Enter") finishRename(s);
              if (e.key === "Escape") renaming = null;
            }}
          />
        {:else if s.available}
          <div class="source">
            <FolderNode
              sourceId={s.id}
              relPath=""
              name={s.name}
              depth={0}
              selected={shownFolder}
              onselect={selectFolder}
              onmove={moveItem}
              oncopy={copyFiles}
              onmenu={folderMenu}
              renaming={renamingAt}
              onrename={finishRenaming}
              onrenamecancel={() => (renamingAt = null)}
              startExpanded
            >
              {#snippet trailing()}
                <span class="provider">{providerLabel(s.provider)}</span>
                <button
                  class="icon-btn more"
                  aria-label={m.main.sourceOptions}
                  onclick={(e) => {
                    e.stopPropagation();
                    menuFor = menuFor === s.id ? null : s.id;
                  }}
                >
                  <Icon name="more" size={14} />
                </button>
              {/snippet}
            </FolderNode>
            {#if menuFor === s.id}
              {@render menu(s)}
            {/if}
          </div>
        {:else}
          <div class="source unavailable" title={s.root}>
            <div class="row">
              <Icon name="warning" size={14} />
              <span class="label">{s.name}</span>
              <button class="link-btn" onclick={() => relocateSource(s)} title={m.main.chooseAgain(s.root)}>
                {info?.os === "macos" ? m.main.allowAccess : m.main.locate}
              </button>
              <button
                class="icon-btn more"
                aria-label={m.main.sourceOptions}
                onclick={(e) => {
                  e.stopPropagation();
                  menuFor = menuFor === s.id ? null : s.id;
                }}
              >
                <Icon name="more" size={14} />
              </button>
            </div>
            {#if menuFor === s.id}
              {@render menu(s)}
            {/if}
          </div>
        {/if}
      {/each}
    </div>

    {#if sources.length === 0}
      <p class="empty small">{m.main.noSources}</p>
    {/if}

    <button class="lib-item import" onclick={() => (importing = true)}>
      <span class="label">{m.main.importScanSnap}</span>
    </button>
    {/if}
  </aside>

  {#snippet menu(s: Source)}
    <div class="menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="menu-path" title={s.root}>{s.root}</div>
      <button role="menuitemcheckbox" aria-checked={s.showAllTypes} onclick={() => toggleAllTypes(s)}>
        <span class="check">{s.showAllTypes ? "✓" : ""}</span>{m.main.showAllTypes}
      </button>
      {#if s.provider !== "local"}
        <button
          role="menuitemcheckbox"
          aria-checked={s.downloadOnlineOnly}
          onclick={() => toggleDownload(s)}
          title={m.main.downloadTip}
        >
          <span class="check">{s.downloadOnlineOnly ? "✓" : ""}</span>{m.main.downloadToIndex}
        </button>
      {/if}
      <button role="menuitem" onclick={() => indexFolder(s.id, "")} title={m.main.indexFolderTip}>
        <span class="check"></span>{m.main.indexFolder}
      </button>
      <button role="menuitem" onclick={() => startRename(s)}>
        <span class="check"></span>{m.main.rename}
      </button>
      <button
        role="menuitem"
        onclick={() =>
          startExport(
            [{ sourceId: s.id, entry: { name: s.name, relPath: "", isDir: true, kind: null, size: 0, modifiedMs: null, cloud: "local" } }],
            s.name,
          )}
      >
        <span class="check"></span>{m.common.ellipsis(m.exportDocs.export)}
      </button>
      <button
        role="menuitem"
        onclick={() =>
          startReadDetails(
            [{ sourceId: s.id, entry: { name: s.name, relPath: "", isDir: true, kind: null, size: 0, modifiedMs: null, cloud: "local" } }],
            s.name,
          )}
      >
        <span class="check"></span>{m.common.ellipsis(m.readDetails.menu)}
      </button>
      <button
        role="menuitem"
        onclick={() =>
          openRenameDocs({
            sourceId: s.id,
            entry: { name: s.name, relPath: "", isDir: true, kind: null, size: 0, modifiedMs: null, cloud: "local" },
          })}
      >
        <span class="check"></span>{m.renameDocs.menu}
      </button>
      <button role="menuitem" class="danger" onclick={() => removeSource(s)}>
        <span class="check"></span>{m.main.removeFromRupert}
      </button>
    </div>
  {/snippet}

  {#snippet errorBanner()}
    {#if error}
      <div class="error" role="alert">
        <Icon name="warning" size={14} />
        <span>{error}</span>
        <button class="icon-btn" onclick={() => (error = null)} aria-label={m.main.dismiss}>×</button>
      </div>
    {/if}
    {#if copyProblems}
      <div class="error" role="alert">
        <Icon name="warning" size={14} />
        <span>{copyProblems}</span>
        <button class="icon-btn" onclick={() => (copyProblems = null)} aria-label={m.main.dismiss}>×</button>
      </div>
    {/if}
  {/snippet}


  {#snippet tagChips(list: string[] | undefined)}
    <!-- Each tag once: a key twice would stop the whole window. -->
    {#each [...new Set(list ?? [])] as t (t)}<span class="chip">{t}</span>{/each}
  {/snippet}

  {#if inboxOpen && appSettings}
    <main class="documents inbox-pane" class:drop={inboxDropOver} {...inboxDrop}>
      {@render errorBanner()}
      <Inbox
        {sources}
        settings={appSettings}
        onfiled={showFiled}
        onfiling={(sourceId, relPath, filed, label) =>
          (lastChange = {
            kind: "file",
            moves: [{ nowSourceId: filed.sourceId, nowRelPath: filed.relPath, wasSourceId: sourceId, wasRelPath: relPath }],
            text: m.inbox.filedIn(label),
            label: m.undo.labelFiling,
          })}
        onundo={undoLast}
        canUndo={lastChange?.kind === "file"}
        {filingUndone}
        onsettings={() => api.openSettings("inbox")}
        onscan={() => (scanOpen = true)}
        onimport={() => (importInboxOpen = true)}
        onshow={(sourceId, relPath) =>
          showInFolder({
            sourceId,
            entry: {
              name: relPath.slice(relPath.lastIndexOf("/") + 1),
              relPath,
              isDir: false,
              kind: null,
              size: 0,
              modifiedMs: null,
              cloud: "local",
            },
          })}
      />
    </main>
  {:else}
  <!-- MARK: Documents -->
  <main
    class="documents"
    bind:clientWidth={documentsWidth}
    class:gallery-mode={galleryShown}
    class:drop={!searching && !!folder && isDropOver(folder.sourceId, folder.relPath)}
    {...!searching && folder ? dropTarget(folder.sourceId, folder.relPath, { files: (d) => copyFiles(d, folder!.sourceId, folder!.relPath) }) : {}}
  >
    <div class="searchbar">
      <input
        bind:this={searchInput}
        bind:value={query}
        type="search"
        placeholder={m.main.searchPlaceholder}
        onkeydown={(e) => {
          if (e.key === "Escape") clearSearch();
        }}
      />
      {#each selectedTags as t, i (t)}
        {#if i > 0}<span class="tag-join">{tagMode === "all" ? m.main.and : m.main.or}</span>{/if}
        <button class="filter-chip" onclick={() => removeTag(t)} title={m.main.removeFilter}># {t} ×</button>
      {/each}
      {#if selectedTags.length > 1}
        <button
          class="filter-chip mode-chip"
          onclick={() => (tagMode = tagMode === "all" ? "any" : "all")}
          title={m.main.switchMode}
        >
          {tagMode === "all" ? m.main.allTags : m.main.anyTag}
        </button>
      {/if}
      {#if favoritesOnly}
        <button class="filter-chip" onclick={() => (favoritesOnly = false)} title={m.main.removeFilter}>{m.main.favoritesChip}</button>
      {/if}
    </div>

    <!-- Search filters (#103): always there, so a date, kind or place
         alone can search too. -->
    <div class="filter-row">
      <select
        value={dateFilter ? dateFilter.kind : ""}
        onchange={(e) => {
          const v = e.currentTarget.value;
          dateFilter = v === "thisYear" || v === "lastYear" ? { kind: v } : v === "range" ? { kind: "range", from: rangeFrom, to: rangeTo } : null;
        }}
      >
        <option value="">{m.filters.anyDate}</option>
        <option value="thisYear">{m.filters.thisYear(new Date().getFullYear())}</option>
        <option value="lastYear">{m.filters.lastYear(new Date().getFullYear() - 1)}</option>
        <option value="range">{m.filters.chooseDates}</option>
      </select>
      {#if dateFilter?.kind === "range"}
        <input type="date" bind:value={rangeFrom} onchange={() => (dateFilter = { kind: "range", from: rangeFrom, to: rangeTo })} aria-label={m.filters.from} />
        <span>–</span>
        <input type="date" bind:value={rangeTo} onchange={() => (dateFilter = { kind: "range", from: rangeFrom, to: rangeTo })} aria-label={m.filters.to} />
      {/if}
      <select value={kindFilter ?? ""} onchange={(e) => (kindFilter = e.currentTarget.value || null)}>
        <option value="">{m.filters.anyKind}</option>
        {#each KINDS as k (k)}<option value={k}>{m.kinds[k]}</option>{/each}
      </select>
      <select value={fromFilter ?? ""} onchange={(e) => (fromFilter = e.currentTarget.value || null)} title={m.filters.fromTip}>
        <option value="">{m.filters.anySender}</option>
        {#if fromFilter && !senderList.names.some((n) => n.toLowerCase() === fromFilter!.toLowerCase())}
          <option value={fromFilter}>{fromFilter}</option>
        {/if}
        {#each senderList.list as s (s.tag)}<option value={s.tag}>{s.tag} ({shortCount(s.count)})</option>{/each}
      </select>
      <select value={dueFilter ?? ""} onchange={(e) => (dueFilter = (e.currentTarget.value || null) as typeof dueFilter)}>
        <option value="">{m.filters.anyDue}</option>
        <option value="upcoming">{m.filters.dueUpcoming}</option>
        <option value="past">{m.filters.duePast}</option>
      </select>
      <select value={whereFilter?.scope ?? "everywhere"} onchange={(e) => setWhere(e.currentTarget.value as "everywhere" | "source" | "folder")}>
        <option value="everywhere">{m.filters.everywhere}</option>
        {#if folder || whereFilter}
          <option value="source">{whereFilter && whereFilter.scope === "source" ? whereLabelOf(whereFilter) : m.filters.thisSource}</option>
          <option value="folder">{whereFilter && whereFilter.scope === "folder" ? whereLabelOf(whereFilter) : m.filters.thisFolder}</option>
        {/if}
      </select>
    </div>

    {@render errorBanner()}

    {#if searching}
      <div class="results-head">
        <p class="result-count muted">
          {#if !searchedOnce}{m.main.searching}{:else if hits.length === 0}{m.main.noMatches}{:else}{m.main.hits(hits.length, hits.length >= 300)}{/if}
          {#if searchedOnce && hits.length && searchTotals.length}
            <span class="totals" title={m.several.totalTip}>· {m.several.total}: {formatTotals(searchTotals)}</span>
          {/if}
        </p>
        <span class="view-switch order-switch" role="group" aria-label={m.filters.sort}>
          <button class:on={!resultsByDate} aria-pressed={!resultsByDate} onclick={() => (resultsByDate = false)}>{m.filters.bestMatch}</button>
          <button class:on={resultsByDate} aria-pressed={resultsByDate} onclick={() => (resultsByDate = true)}>{m.filters.newest}</button>
        </span>
        {#if activeSaved}
          <span class="saved-as" title={m.savedSearches.savedAsTip}><Icon name="search" size={13} />{activeSaved.name}</span>
        {:else}
          <button class="results-export" title={m.savedSearches.saveTip} onclick={openSaveSearch}>{m.savedSearches.save}</button>
        {/if}
        {#if hits.length}
          <button
            class="results-export"
            title={m.exportDocs.resultsTip}
            onclick={() => startExport(hits.map((h) => ({ sourceId: h.sourceId, entry: h })), searchTitle())}
            >{m.common.ellipsis(m.exportDocs.export)}</button
          >
        {/if}
      </div>
      <ul class="results">
        {#each hits as h (h.sourceId + "\n" + h.relPath)}
          <li class="doc-host" class:selected={isPicked(h.sourceId, h.relPath)}>
            <button
              class="result"
              class:selected={isPicked(h.sourceId, h.relPath)}
              data-pick={keyOf(h.sourceId, h.relPath)}
              onclick={(ev) => pick({ sourceId: h.sourceId, entry: h }, ev)}
              ondblclick={() => activate(h.sourceId, h)}
              oncontextmenu={(ev) => openItemMenu({ sourceId: h.sourceId, entry: h }, ev, true)}
              {...dragSource({ sourceId: h.sourceId, entry: h }, () => dragGroup(h.sourceId, h.relPath))}
            >
              <div class="r-title">
                <span class="file-icon"><Icon name={h.kind === "pdf" ? "pdf" : "file"} size={15} /></span>
                <span class="star-space"></span>
                <strong>{h.title ?? h.name}</strong>
                {#if h.title}<span class="muted r-name">{h.name}</span>{/if}
              </div>
              {#if h.snippet}
                <div class="snippet">
                  {#each snippetParts(h.snippet) as p, i (i)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}
                </div>
              {/if}
              <div class="r-meta muted">
                <span>{sourceName(h.sourceId)}{dirOf(h.relPath) ? " / " + dirOf(h.relPath) : ""}</span>
                {#if h.documentDate}<span>· {h.documentDate}</span>{/if}
                {#if h.sender}<span>· {h.sender}</span>{/if}
                {#if h.amount}<span>· {formatMoney(h.amount, h.currency)}</span>{/if}
                {#if h.dueDate}<span class:overdue={!h.done && h.dueDate < todayIso()}>· {m.details.dueOn(formatDocDay(h.dueDate))}</span>{/if}
                {@render tagChips(h.tags)}
              </div>
            </button>
            <!-- Beside the result's button (buttons can't hold buttons), placed
                 as in the folder list: the star before the title, the cloud
                 at the end. -->
            <span class="r-star"><DocStar sourceId={h.sourceId} doc={h} onclick={() => starDoc(h.sourceId, h)} /></span>
            {#if h.cloud === "cloudOnly"}
              <span class="r-marks"><CloudBadge sourceId={h.sourceId} doc={h} size={13} onclick={() => downloadDoc(h.sourceId, h)} /></span>
            {/if}
          </li>
        {/each}
      </ul>
    {:else if comingUpOpen}
      <ComingUp
        tick={indexTick}
        selectedKey={selected ? keyOf(selected.sourceId, selected.entry.relPath) : null}
        focus={comingFocus}
        onfocused={() => (comingFocus = null)}
        onselect={(h) => (selected = { sourceId: h.sourceId, entry: h })}
        onopen={(h) => activate(h.sourceId, h)}
        onshowinfolder={(h) => showInFolder({ sourceId: h.sourceId, entry: h })}
        oncounts={(c) => (comingCounts = c)}
      />
    {:else if healthOpen}
      <Health
        {sources}
        os={info?.os}
        tick={indexTick}
        selectedKey={selected ? keyOf(selected.sourceId, selected.entry.relPath) : null}
        onselect={(h) => (selected = { sourceId: h.sourceId, entry: h })}
        onopen={(h) => activate(h.sourceId, h)}
        onrename={(sourceId, relDir, only) => (renameDocs = { sourceId, relDir, only })}
        onindex={(sourceId) => indexFolder(sourceId, "")}
        oncounts={(n) => (healthBadge = n)}
      />
    {:else if folder && source}
      <nav class="crumbs" aria-label={m.main.folderPath}>
        {#each crumbs as c, i (c.relPath)}
          {#if i > 0}<span class="sep">/</span>{/if}
          <button class:current={i === crumbs.length - 1} onclick={() => selectFolder(source.id, c.relPath)}
            >{c.name}</button
          >
        {/each}
        <button
          class="folder-star"
          class:on={folderFav}
          onclick={toggleFolderFav}
          title={folderFav ? m.main.unfavoriteFolder : m.main.favoriteFolder}
          aria-pressed={folderFav}>{folderFav ? "★" : "☆"}</button
        >
        {#if folderMode !== "list"}
          <label class="sort-pick" title={m.filters.sort}>
            <span class="muted">{m.filters.sort}</span>
            <select
              value={sort.key + (sort.desc ? ":desc" : ":asc")}
              onchange={(e) => {
                const [k, d] = e.currentTarget.value.split(":");
                sortBy(k as SortKey, d === "desc");
              }}
            >
              <option value="name:asc">{m.main.name}</option>
              <option value="date:desc">{m.filters.dateNewest}</option>
              <option value="date:asc">{m.filters.dateOldest}</option>
              <option value="modified:desc">{m.filters.modifiedNewest}</option>
              <option value="size:desc">{m.filters.sizeBiggest}</option>
              <option value="kind:asc">{m.main.kind}</option>
            </select>
          </label>
        {/if}
        <span class="view-switch" role="group" aria-label={m.main.view}>
          {#each [["list", "view-list", m.main.list], ["icons", "view-grid", m.main.largeIcons], ["gallery", "view-gallery", m.main.gallery]] as const as [mode, icon, label] (mode)}
            <button class:on={folderMode === mode} onclick={() => setFolderMode(mode)} title={label} aria-label={label} aria-pressed={folderMode === mode}>
              <Icon name={icon} size={16} />
            </button>
          {/each}
        </span>
      </nav>

      {#if entries.length === 0 && loading}
        <p class="empty center">{m.common.loading}</p>
      {:else if entries.length === 0}
        <p class="empty center">
          {source.showAllTypes ? m.main.emptyFolder : m.main.noDocuments}
        </p>
      {:else if folderMode !== "list"}
        <PictureView
          mode={folderMode}
          sourceId={source.id}
          entries={sortedEntries}
          selectedRelPath={selected?.sourceId === source.id ? selected.entry.relPath : null}
          picked={pickedIn(source.id)}
          dragGroup={(e) => dragGroup(source.id, e.relPath)}
          onselect={(e, ev) => pick({ sourceId: source.id, entry: e }, ev)}
          onactivate={(e) => activate(source.id, e)}
          onmenu={(e, ev) => openItemMenu({ sourceId: source.id, entry: e }, ev)}
          onmove={moveItem}
          oncopy={copyFiles}
          onstar={(e) => starDoc(source.id, e)}
          ondownload={(e) => downloadDoc(source.id, e)}
          renaming={renamingAt?.sourceId === source.id ? renamingAt.relPath : null}
          onrename={(e, name) => finishRenaming(source.id, e.relPath, name)}
          onrenamecancel={() => (renamingAt = null)}
          enterOpens={info?.os !== "macos"}
          onview={(e, page) => preview({ sourceId: source.id, entry: e }, page)}
          paused={!!viewer}
        />
      {:else}
        <table style:min-width="{listMinWidth}px">
          <ListHeader columns={shownColumns} {sort} onsort={(k) => sortBy(k)} />
          <tbody>
            {#each sortedEntries as e (e.relPath)}
              <tr
                class="doc-host"
                class:selected={isPicked(source.id, e.relPath)}
                class:drop={e.isDir && isDropOver(source.id, e.relPath)}
                data-pick={keyOf(source.id, e.relPath)}
                onclick={(ev) => pick({ sourceId: source.id, entry: e }, ev)}
                ondblclick={() => activate(source.id, e)}
                oncontextmenu={(ev) => openItemMenu({ sourceId: source.id, entry: e }, ev)}
                {...isRenaming(source.id, e.relPath) ? {} : dragSource({ sourceId: source.id, entry: e }, () => dragGroup(source.id, e.relPath))}
                {...e.isDir
                  ? dropTarget(source.id, e.relPath, {
                      move: (item) => moveItem(item, source.id, e.relPath),
                      files: (d) => copyFiles(d, source.id, e.relPath),
                    })
                  : {}}
              >
                {#each shownColumns as c (c)}
                  {#if c === "star"}
                    <td class="star">
                      {#if e.isDir}
                        {@const fav = isFavoriteFolder(source.id, e.relPath)}
                        <button
                          type="button"
                          class="folder-star"
                          class:on={fav}
                          title={fav ? m.main.unfavoriteFolder : m.main.favoriteFolder}
                          aria-label={fav ? m.main.unfavoriteFolder : m.main.favoriteFolder}
                          aria-pressed={fav}
                          draggable="false"
                          onclick={(ev) => (ev.stopPropagation(), toggleSubfolderFav(source.id, e.relPath))}
                          ondblclick={(ev) => ev.stopPropagation()}>{fav ? "★" : "☆"}</button
                        >
                      {:else}
                        <DocStar sourceId={source.id} doc={e} onclick={() => starDoc(source.id, e)} />
                      {/if}
                    </td>
                  {:else if c === "name"}
                    <td class="name">
                      <span class="name-cell">
                        <span class="file-icon" class:folder={e.isDir}>
                          <Icon name={e.isDir ? "folder" : e.kind === "pdf" ? "pdf" : "file"} size={15} />
                        </span>
                        {#if isRenaming(source.id, e.relPath)}
                          <RenameField
                            name={e.name}
                            isDir={e.isDir}
                            oncommit={(name) => finishRenaming(source.id, e.relPath, name)}
                            oncancel={() => (renamingAt = null)}
                          />
                        {:else}
                          <span class="file-name" title={e.title ?? undefined}>{e.name}</span>
                        {/if}
                        {#if e.tags?.length}<span class="row-tags">{@render tagChips(e.tags)}</span>{/if}
                        {#if e.cloud === "cloudOnly"}
                          <CloudBadge sourceId={source.id} doc={e} onclick={() => downloadDoc(source.id, e)} />
                        {/if}
                      </span>
                    </td>
                  {:else if c === "kind"}
                    <td class="muted kind">{kindLabel(e.isDir ? "folder" : e.kind)}</td>
                  {:else if c === "size"}
                    <td class="num muted">{e.isDir || (e.cloud === "cloudOnly" && !e.size) ? "" : formatSize(e.size)}</td>
                  {:else if c === "date"}
                    <td class="muted">{e.isDir ? "" : formatDocDay(e.documentDate)}</td>
                  {:else if c === "sender"}
                    <td class="muted" title={e.sender ?? undefined}>{e.sender ?? ""}</td>
                  {:else if c === "amount"}
                    <td class="num muted">{formatMoney(e.amount, e.currency)}</td>
                  {:else if c === "due"}
                    <td class="muted" class:overdue={!!e.dueDate && !e.done && e.dueDate < todayIso()}>{formatDocDay(e.dueDate)}</td>
                  {:else}
                    <td class="muted" title={formatDate(e.modifiedMs)}>{formatDay(e.modifiedMs)}</td>
                  {/if}
                {/each}
              </tr>
            {/each}
          </tbody>
        </table>
      {/if}
    {:else}
      <div class="welcome">
        <h1 class="visually-hidden">Rupert</h1>
        <button class="welcome-owl" onclick={() => assistant?.show()} title={m.main.askRupertKey}>
          <RupertOwl size={132} />
          <span>{m.main.askRupert}</span>
        </button>
        <p class="muted">{m.main.tagline}</p>
        {#if sources.length === 0}
          <p>{m.main.getStarted}</p>
          <button class="primary" onclick={addSource}>{m.main.addFolderButton}</button>
        {:else}
          <p class="muted">{m.main.chooseOrSearch}</p>
        {/if}
      </div>
    {/if}
  </main>

  <!-- MARK: Details -->
  <section class="details" bind:clientHeight={detailsH} bind:clientWidth={detailsW}>
    {#if pagePreviewFor}
      {@const pv = pagePreviewFor}
      {#key `${pv.sourceId}\n${pv.entry.relPath}`}
        <DocPreview
          sourceId={pv.sourceId}
          entry={pv.entry}
          room={detailsH - detailsBodyH}
          width={detailsW}
          onopen={(page) => preview(pv, page)}
        />
      {/key}
    {/if}
    <div class="details-body" bind:offsetHeight={detailsBodyH}>
    <h2>{m.main.details}</h2>
    {#if several}
      {@const sm = pickedSummary}
      <div class="detail-name">{m.several.selectedCount(picked.length)}</div>
      <dl>
        {#if sm.docs}
          <dt>{m.several.documents}</dt>
          <dd>{sm.docs}{sm.size ? ` · ${formatSize(sm.size)}` : ""}</dd>
        {/if}
        {#if sm.folders}
          <dt>{m.several.folders}</dt>
          <dd>{sm.folders}</dd>
        {/if}
        {#if sm.docs}
          <dt>{m.several.tagsInCommon}</dt>
          <dd class="common-tags">
            {#if sm.common.length}{@render tagChips(sm.common)}{:else}<span class="muted">{m.several.noCommonTags}</span>{/if}
          </dd>
        {/if}
        {#if sm.totals.length}
          <dt>{m.several.total}</dt>
          <dd>
            {formatTotals(sm.totals)}
            {#if sm.totals.reduce((n, t) => n + t.count, 0) < sm.docs}<span class="muted small">{m.several.withAmount(sm.totals.reduce((n, t) => n + t.count, 0))}</span>{/if}
          </dd>
        {/if}
      </dl>
      <div class="actions several-actions">
        {@render bulkActions("buttons")}
      </div>
    {:else if selected}
      {@const e = selected.entry}
      <div class="detail-name">{e.name}</div>
      {#if !e.isDir}
        <MetaEditor sourceId={selected.sourceId} entry={e} />
        <hr />
      {/if}
      <dl>
        <dt>{m.main.kind}</dt>
        <dd>{kindLabel(e.isDir ? "folder" : e.kind)}</dd>
        {#if !e.isDir && e.size}
          <dt>{m.main.size}</dt>
          <dd>{formatSize(e.size)}</dd>
        {/if}
        <dt>{m.main.modified}</dt>
        <dd>{formatDate(e.modifiedMs)}</dd>
        <dt>{m.main.location}</dt>
        <dd>{sourceName(selected.sourceId)}/{e.relPath}</dd>
        {#if e.cloud === "cloudOnly"}
          <dt>{m.main.storage}</dt>
          <dd class="cloud-note"><Icon name="cloud" size={14} /> {m.main.onlineOnlyNote}</dd>
        {/if}
      </dl>
      <div class="actions">
        <button class="primary" onclick={() => activate(selected!.sourceId, e)}>{m.main.open}</button>
        <button onclick={() => reveal(selected!)}>{revealLabel(info?.os)}</button>
        {#if !e.isDir}
          {#if info?.os !== "linux"}
            <button
              class="icon-action"
              onclick={(ev) => shareDoc(selected!, atElement(ev.currentTarget))}
              title={m.common.ellipsis(info?.os === "windows" ? m.share.shareTipWindows : m.share.shareTip)}
              aria-label={m.share.share}><Icon name="share" size={16} /></button
            >
          {/if}
          <button class="icon-action" onclick={() => emailDoc(selected!)} title={m.share.emailTip} aria-label={m.share.email}
            ><Icon name="mail" size={16} /></button
          >
        {/if}
        {#if searching}
          <button onclick={() => showInFolder(selected!)}>{m.main.showInRupert}</button>
        {/if}
      </div>
    {:else}
      <p class="empty">{m.main.selectDocument}</p>
    {/if}
    <!-- Room for the Ask Rupert button at the end of what scrolls. -->
    <div class="fab-room" aria-hidden="true"></div>
    </div>
  </section>
  {/if}
</div>

{#if viewer}
  {#key `${viewer.sourceId}/${viewer.entry.relPath}`}
    <PageViewer
      sourceId={viewer.sourceId}
      entry={viewer.entry}
      title={viewer.entry.title ?? null}
      startPage={viewer.page}
      find={viewer.find ?? null}
      onclose={() => (viewer = null)}
      onstep={previewStep}
    />
  {/key}
{/if}

{#if exporting}
  <ExportDialog docs={exporting.docs} title={exporting.title} os={info?.os} onclose={() => (exporting = null)} />
{/if}

{#if readingDetails}
  <ReadDetailsDialog
    docs={readingDetails.docs}
    title={readingDetails.title}
    onclose={() => (readingDetails = null)}
    onsettings={() => api.openSettings("local-ai")}
  />
{/if}

{#if quickOpen}
  <QuickOpen
    {sources}
    commands={quickCommands}
    mac={info?.os === "macos"}
    onfolder={selectFolder}
    ondocument={quickDocument}
    ontag={quickTag}
    oncommand={quickCommand}
    onclose={() => (quickOpen = false)}
  />
{/if}

{#if reader}
  <MarkdownReader sourceId={reader.sourceId} relPath={reader.relPath} onclose={() => (reader = null)} />
{/if}

{#if savedMenu}
  {@const sm = savedMenu}
  <div
    class="menu item-menu"
    role="menu"
    tabindex="-1"
    style:left="{sm.x}px"
    style:top="{sm.y}px"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <button
      role="menuitem"
      onclick={() => {
        savedMenu = null;
        saveDialog = { mode: "rename", search: sm.s };
      }}>{m.savedSearches.renameMenu}</button
    >
    <button role="menuitem" disabled={!searching || activeSaved === sm.s} title={m.savedSearches.updateTip} onclick={() => updateSaved(sm.s)}
      >{m.savedSearches.update}</button
    >
    <hr />
    <button role="menuitem" class="danger" onclick={() => removeSaved(sm.s)}>{m.savedSearches.remove}</button>
  </div>
{/if}

{#if saveDialog}
  {@const sd = saveDialog}
  <SaveSearchDialog
    mode={sd.mode}
    name={sd.search.name}
    finds={findsText(sd.search)}
    keptWith={sourceName(sd.search.sourceId)}
    onsave={saveFromDialog}
    onclose={() => (saveDialog = null)}
  />
{/if}

{#if itemMenu && several && isPicked(itemMenu.sel.sourceId, itemMenu.sel.entry.relPath)}
  {@const mn = itemMenu}
  <div
    class="menu item-menu"
    role="menu"
    tabindex="-1"
    style:left="{mn.x}px"
    style:top="{mn.y}px"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <div class="menu-head">{m.several.selectedCount(picked.length)}</div>
    {@render bulkActions("menu")}
  </div>
{:else if itemMenu}
  {@const mn = itemMenu}
  <div
    class="menu item-menu"
    role="menu"
    tabindex="-1"
    style:left="{mn.x}px"
    style:top="{mn.y}px"
    onclick={(e) => e.stopPropagation()}
    onkeydown={() => {}}
  >
    <button role="menuitem" onclick={() => fromMenu((sel) => activate(sel.sourceId, sel.entry))}>{m.main.open}</button>
    <button role="menuitem" onclick={() => fromMenu(reveal)}>{revealLabel(info?.os)}</button>
    {#if mn.inSearch}
      <button role="menuitem" onclick={() => fromMenu(showInFolder)}>{m.main.showInRupert}</button>
    {/if}
    {#if !mn.inSearch}
      <button role="menuitem" onclick={() => fromMenu(startRenaming)}>{m.main.rename}</button>
      <button role="menuitem" onclick={() => fromMenu(openRenameDocs)}>{m.renameDocs.menu}</button>
    {/if}
    <button role="menuitem" onclick={() => fromMenu((sel) => (moving = [sel]))}>{m.main.moveTo}</button>
    {#if !mn.sel.entry.isDir}
      <hr />
      {#if info?.os !== "linux"}
        <button
          role="menuitem"
          onclick={() => {
            // Where the menu was, read before it closes.
            const at = { x: mn.x, y: mn.y };
            fromMenu((sel) => shareDoc(sel, at));
          }}>{m.share.share}</button
        >
      {/if}
      <button role="menuitem" onclick={() => fromMenu(emailDoc)}>{m.share.email}</button>
    {/if}
    {#if mn.sel.entry.isDir}
      <button role="menuitem" title={m.main.indexFolderTip} onclick={() => fromMenu((sel) => indexFolder(sel.sourceId, sel.entry.relPath))}>
        {m.main.indexFolder}
      </button>
    {/if}
    <button role="menuitem" onclick={() => fromMenu((sel) => startExport([sel], exportTitle([sel])))}>{m.common.ellipsis(m.exportDocs.export)}</button>
    <button role="menuitem" onclick={() => fromMenu((sel) => startReadDetails([sel], exportTitle([sel])))}>{m.common.ellipsis(m.readDetails.menu)}</button>
    <hr />
    <button role="menuitem" class="danger" onclick={() => trashEntry(mn.sel)}>{trashLabel}</button>
  </div>
{/if}

<Assistant
  bind:this={assistant}
  onaction={assistantAction}
  oncite={openCitation}
  onsettings={() => api.openSettings("assistant")}
/>

{#if tagManagerOpen}
  <TagManager
    onclose={() => (tagManagerOpen = false)}
    onchanged={(report) => {
      selectedTags = [];
      tagsBefore = 0;
      loadIndexInfo();
      // Offered for Undo (#104) once the tag manager closes.
      rememberTags(report, m.tags.changed(report.changed));
    }}
  />
{/if}

{#if scanOpen}
  <ScanPhonePanel
    inbox={appSettings?.inbox ?? null}
    {sources}
    onclose={() => (scanOpen = false)}
    onsettings={() => {
      scanOpen = false;
      api.openSettings("inbox");
    }}
  />
{/if}

{#if bulkTags}
  <BulkTagsDialog mode={bulkTags.mode} docs={bulkTags.docs} onclose={() => (bulkTags = null)} ondone={bulkTagsDone} />
{/if}

{#if moving}
  <MoveDialog
    items={moving}
    {sources}
    onmove={async (sourceId, relPath) => {
      const items = moving!.filter((it) => canMoveInto(it, sourceId, relPath));
      if (items.length > 1) {
        moving = null;
        movePicked(items, sourceId, relPath);
        return null;
      }
      if (await moveItem(items[0], sourceId, relPath)) return null;
      const message = error; // shown in the dialog instead of behind it
      error = null;
      return message ?? m.main.moveFailed;
    }}
    onclose={() => (moving = null)}
  />
{/if}

{#if importing}
  <ImportDialog {sources} onclose={() => (importing = false)} onsourceadded={() => loadSources()} />
{/if}

{#if importInboxOpen && appSettings?.inbox}
  {@const inbox = appSettings.inbox}
  <InboxImportDialog
    inboxName={[sourceName(inbox.sourceId), inbox.relPath].filter(Boolean).join("/")}
    onclose={() => (importInboxOpen = false)}
  />
{/if}

{#if renameDocs}
  {@const r = renameDocs}
  <RenameDialog
    sourceId={r.sourceId}
    relDir={r.relDir}
    only={r.only}
    where={whereLabel(r.sourceId, r.relDir)}
    pattern={appSettings?.namePattern ?? "{date} {title}"}
    ondone={(outcome) => renamedDocs(r.sourceId, r.relDir, outcome)}
    onclose={() => (renameDocs = null)}
  />
{/if}

{#if sendingName}
  <div class="undo-bar" role="status">
    <span>{m.share.downloading(sendingName)}</span>
    <button class="undo-link" onclick={() => api.shareCancel()}>{m.common.cancel}</button>
  </div>
{:else if (lastChange && lastChange.kind !== "file") || renameNote || batchNote}
  <!-- Filing has its Undo in the inbox's own notice. -->
  {@const shown = lastChange && lastChange.kind !== "file" ? lastChange : null}
  <div class="undo-bar" role="status">
    <span>{shown?.text ?? renameNote ?? batchNote}</span>
    {#if shown}
      <button class="undo-link" onclick={undoLast} title={info?.os === "macos" ? "⌘Z" : "Ctrl+Z"}>{m.renameDocs.undo}</button>
    {/if}
    <button
      class="undo-close"
      onclick={() => ((lastChange = null), (renameNote = null), (batchNote = null))}
      aria-label={m.renameDocs.dismiss}>×</button
    >
  </div>
{/if}

<footer>
  <span>
    {#if info}
      <span title={m.main.versionTip}>{info.name} {info.version} · {editionLabel(info)}</span>
    {/if}
    <button class="footer-btn" onclick={() => api.openSettings()} title={m.main.settings}>{m.main.settings}</button>
    <button class="footer-btn" onclick={() => api.openHelp()} title={m.main.helpTip}>{m.main.help}</button>
  </span>
  <span class="status">
    {#if updates.status === "available"}
      <button class="footer-btn update" onclick={() => updates.install()} title={m.main.updateTip}>
        {m.main.updateTo(updates.version ?? "")}
      </button> ·
    {:else if updates.status === "downloading"}
      {m.main.downloadingUpdate(updates.progress !== null ? Math.round(updates.progress * 100) : null)} ·
    {:else if updates.status === "restarting"}
      {m.main.restarting} ·
    {/if}
    {#if countsText() && statusText() === countsText()}
      <button class="footer-link" onclick={openHealth} title={m.health.title}>{countsText()}</button>
    {:else}
      {statusText()}
    {/if}
    {#if stoppable}
      <button class="footer-btn" onclick={() => api.stopIndexing()} title={m.main.stopIndexingTip}>{m.main.stopIndexing}</button>
    {:else if progress?.stopped}
      <button class="footer-btn" onclick={() => api.resumeIndexing()} title={m.main.resumeIndexingTip}>{m.main.resumeIndexing}</button>
    {/if}
    {#if progress && !progress.pdfText}· {m.main.noPdfText}{/if}
  </span>
</footer>

<style>
  .shell.inbox-mode {
    grid-template-columns: var(--side-width) minmax(0, 1fr);
  }
  .inbox-pane {
    padding: 0;
    overflow: hidden;
  }
  .inbox-pane > .error {
    margin: 12px 16px 0;
  }
  /* Files from another app are being dragged over the folder or inbox. */
  .documents.drop {
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .footer-btn.update {
    padding: 0;
    color: var(--accent);
    font-weight: 600;
  }
  .footer-btn {
    border: 0;
    background: none;
    color: inherit;
    font-size: inherit;
    padding: 0 0 0 10px;
    text-decoration: underline;
  }
  .shell {
    height: calc(100vh - 22px);
    display: grid;
    grid-template-columns: var(--side-width) minmax(0, 1fr) var(--details-width);
    position: relative;
  }

  h2 {
    margin: 0;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .muted,
  .empty {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
    padding: 0 8px;
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
  .icon-btn {
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    padding: 2px;
    color: var(--muted);
    border-radius: 4px;
  }
  .icon-btn:hover {
    background: var(--hover);
    color: var(--text);
  }

  /* Sidebar */
  .sources {
    background: var(--panel);
    border-right: 1px solid var(--border);
    padding: 10px 8px;
    overflow: auto;
  }
  .sources header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 4px 6px 8px;
  }
  .sources-header {
    margin-top: 2px;
  }
  .lib-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 26px;
    border: 0;
    background: none;
    padding: 0 8px;
    text-align: left;
    border-radius: 6px;
  }
  .lib-item:hover {
    background: var(--hover);
  }
  .footer-link {
    border: 0;
    background: none;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .footer-link:hover {
    color: var(--text);
    text-decoration: underline;
  }
  .health-icon,
  .coming-icon,
  .saved-icon {
    color: var(--accent-strong);
  }
  .coming-icon,
  .saved-icon {
    display: inline-flex;
  }
  /* Saved searches (#110): a sidebar section under Favorites. */
  .side-section {
    padding: 14px 8px 6px;
  }
  .saved-as {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--accent-strong);
    font-weight: 600;
    font-size: 12px;
  }
  /* Something in Coming up is overdue. */
  .count.alert {
    color: white;
    background: var(--danger);
    border-radius: 8px;
    padding: 0 6px;
    /* The digit lines up with the plain counts above and below; the pill
       reaches into the row's padding. */
    margin-right: -6px;
  }
  .lib-item.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  /* Files from another app are being dragged over it. */
  .lib-item.drop {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .lib-item .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lib-icon {
    width: 16px;
    display: grid;
    place-items: center;
    color: var(--muted);
  }
  .fav-folder {
    padding-left: 22px;
  }
  .folder-icon {
    color: var(--folder);
  }
  .folder-star {
    margin-left: 6px;
    color: var(--muted);
    font-size: 15px;
  }
  .folder-star.on {
    color: #d9a520;
  }
  .lib-icon.star {
    color: #d9a520;
  }
  .lib-item.import {
    margin-top: 14px;
    color: var(--muted);
    font-size: 12px;
  }
  .view-switch {
    display: flex;
    margin: 12px 4px 8px;
    padding: 2px;
    border-radius: 7px;
    background: var(--hover);
  }
  .view-switch button {
    flex: 1;
    position: relative;
    border: 0;
    background: none;
    border-radius: 5px;
    padding: 4px 0;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .view-switch button:hover:not(.on) {
    color: var(--text);
  }
  .view-switch .badge {
    position: absolute;
    top: 1px;
    left: calc(50% + 6px);
  }
  .view-switch button.on {
    background: var(--bg);
    color: var(--accent);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.15);
  }
  .badge {
    font-size: 10px;
    font-weight: 600;
    color: #fff;
    background: var(--accent);
    border-radius: 8px;
    padding: 0 5px;
  }
  .tag-pane {
    padding: 0 0 6px 4px;
  }
  .manage-tags {
    border: 0;
    background: none;
    color: var(--muted);
    font-size: 12px;
    padding: 2px 8px;
  }
  .manage-tags:hover {
    color: var(--accent);
  }
  .tag-join {
    color: var(--muted);
    font-size: 12px;
  }
  .mode-chip {
    font-style: italic;
  }
  .count {
    font-size: 11px;
    color: var(--muted);
  }
  .source {
    position: relative;
    margin-bottom: 6px;
  }
  .source :global(.row .label) {
    font-weight: 500;
  }
  .provider {
    font-size: 10px;
    color: var(--muted);
    white-space: nowrap;
  }
  .more {
    opacity: 0;
  }
  .source:hover .more,
  .source:focus-within .more {
    opacity: 1;
  }
  .unavailable .row {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 8px;
    color: var(--muted);
  }
  .link-btn {
    border: 0;
    background: none;
    padding: 0 4px;
    font-size: 11px;
    color: var(--accent);
    text-decoration: underline;
    white-space: nowrap;
  }
  .unavailable .label {
    flex: 1;
  }
  .rename {
    width: 100%;
    font: inherit;
    padding: 3px 8px;
    margin-bottom: 6px;
    border: 1px solid var(--accent);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
  }
  .menu {
    position: absolute;
    right: 4px;
    top: 28px;
    z-index: 10;
    min-width: 210px;
    /* Within the folder's row: a long path wraps instead of widening the
       menu past the window's edge. */
    max-width: calc(100% - 8px);
    padding: 4px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
  }
  .menu button {
    display: flex;
    width: 100%;
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 8px;
  }
  .menu button:hover {
    background: var(--hover);
  }
  .menu .danger {
    color: var(--danger);
  }
  .menu button:disabled {
    opacity: 0.45;
  }
  .menu .check {
    width: 18px;
  }
  .item-menu {
    position: fixed;
    right: auto;
    z-index: 30;
  }
  .item-menu hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
  .menu-head {
    padding: 4px 8px 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--muted);
  }
  .several-actions button.danger {
    color: var(--danger);
  }
  /* The selection's actions (#102): Move to… and Tags… as words, then a
     row of icons with the trash can on its own at the end. */
  .several-actions {
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
  }
  .bulk-row {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .bulk-row .grow {
    flex: 1;
  }
  .star-action {
    font-size: 16px;
    line-height: 1;
  }
  .star-action.on {
    color: #d9a520;
  }
  .drop {
    position: relative;
  }
  .drop-menu {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 15;
    min-width: 180px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.15);
  }
  .drop-menu button {
    border: 0;
    background: none;
    text-align: left;
    padding: 5px 8px;
    border-radius: 5px;
  }
  .drop-menu button:hover {
    background: var(--hover);
  }
  .fab-room {
    height: 52px;
  }
  .common-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .view-switch {
    margin-left: auto;
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 7px;
    background: var(--panel);
  }
  .crumbs .view-switch button {
    display: grid;
    place-items: center;
    padding: 3px 6px;
    border-radius: 5px;
    color: var(--muted);
  }
  .crumbs .view-switch button.on {
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 1px 2px rgb(0 0 0 / 0.15);
  }
  .documents.gallery-mode {
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .menu-path {
    padding: 4px 8px 6px;
    font-size: 11px;
    color: var(--muted);
    overflow-wrap: anywhere;
    border-bottom: 1px solid var(--border);
    margin-bottom: 4px;
  }

  /* Documents */
  .documents {
    overflow: auto;
    padding: 12px 16px;
    min-width: 0;
    /* Not a CSS container: that would make it the box fixed menus and
       dialogs inside it (the inbox's right-click menu, the column menu,
       the folder picker, the page viewer) are placed in and clipped by.
       The list's columns follow its width from `documentsWidth` (#138). */
  }
  .searchbar {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
    margin-bottom: 12px;
  }
  .searchbar input {
    flex: 1 1 200px;
    min-width: 0;
    font: inherit;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg);
    color: var(--text);
  }
  .searchbar input:focus {
    outline: 2px solid var(--accent-soft);
    border-color: var(--accent);
  }
  .filter-chip {
    border: 0;
    background: var(--accent-soft);
    color: var(--accent-strong);
    border-radius: 12px;
    padding: 3px 10px;
    white-space: nowrap;
  }
  /* Search filters (#103). */
  /* #108: a due date that's passed, and a search's totals. */
  .overdue {
    color: var(--danger, #c0392b);
  }
  .totals {
    white-space: nowrap;
    color: var(--text);
  }
  .filter-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: -4px 0 12px;
    font-size: 12px;
  }
  .filter-row select,
  .filter-row input {
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    padding: 3px 6px;
  }
  .results-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .results-export {
    font: inherit;
    font-size: 12px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    padding: 2px 10px;
  }
  .results-head .result-count {
    flex: 1;
  }
  .order-switch button {
    white-space: nowrap;
    font-size: 12px;
    padding: 2px 8px;
    border: 0;
    background: none;
    color: var(--muted);
    border-radius: 5px;
  }
  .order-switch button.on {
    background: var(--bg);
    color: var(--text);
  }
  .error {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    margin-bottom: 10px;
    border-radius: 6px;
    background: var(--danger-soft);
    color: var(--danger);
  }
  .error span {
    flex: 1;
  }
  .crumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 2px;
    margin-bottom: 10px;
  }
  .crumbs button {
    border: 0;
    background: none;
    padding: 2px 4px;
    color: var(--muted);
  }
  .crumbs button:hover {
    color: var(--text);
  }
  .crumbs .current {
    color: var(--text);
    font-weight: 600;
  }
  .sep {
    color: var(--border);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  .sort-pick {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: auto;
    font-size: 12px;
  }
  .sort-pick select {
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    padding: 2px 6px;
  }
  .sort-pick + .view-switch {
    margin-left: 8px;
  }
  td {
    padding: 4px 8px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  tbody tr {
    cursor: default;
    user-select: none;
  }
  tbody tr:hover {
    background: var(--hover);
  }
  tbody tr.selected {
    background: var(--accent-soft);
  }
  tbody tr.drop {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  /* The name, its tags and badge in a row; the cell stays a table cell. */
  .name-cell {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  td.star {
    padding: 4px 0;
    text-align: center;
  }
  /* A folder's star in the list (#138), as DocStar shows a document's. */
  .folder-star {
    border: 0;
    background: none;
    padding: 0 1px;
    line-height: 1;
    font-size: 14px;
    color: var(--muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .folder-star.on {
    color: #d9a520;
  }
  .folder-star.on,
  .folder-star:focus-visible,
  .doc-host:hover .folder-star,
  .doc-host.selected .folder-star {
    opacity: 1;
  }
  .folder-star:hover {
    color: #d9a520;
  }
  .file-name {
    flex: 0 1 auto;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* A document's tags in the list give way first, so its name stays
     readable; the name is cut only once they're out of room. */
  .row-tags {
    flex: 0 1000 auto;
    min-width: 0;
    display: flex;
    gap: 4px;
    overflow: hidden;
  }
  .file-icon {
    display: grid;
    color: var(--muted);
    flex: none;
  }
  .file-icon.folder {
    color: var(--folder);
  }
  .chip {
    flex: none;
    font-size: 11px;
    padding: 0 6px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .num {
    text-align: right;
  }
  .center {
    text-align: center;
    margin-top: 40px;
  }
  .welcome {
    max-width: 420px;
    margin: 80px auto 0;
    text-align: center;
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .welcome-owl {
    display: inline-flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    margin: 0 0 10px;
    padding: 10px 18px 8px;
    border: 0;
    border-radius: 14px;
    background: none;
    color: var(--accent);
    font-weight: 600;
    transition:
      background 0.2s,
      transform 0.2s;
  }
  .welcome-owl:hover {
    background: var(--hover);
    transform: translateY(-2px);
  }
  .welcome-owl:focus-visible {
    outline: 2px solid var(--accent);
  }

  /* Results */
  .result-count {
    margin: 0 0 8px;
    font-size: 12px;
  }
  .results {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .result {
    display: block;
    width: 100%;
    text-align: left;
    border: 0;
    border-radius: 8px;
    background: none;
    padding: 8px 10px;
    margin-bottom: 2px;
  }
  .results li {
    position: relative;
  }
  .result {
    padding-right: 32px;
  }
  /* A result's star, over the space kept for it after the file icon
     (10px padding + 15px icon + 6px gap), as in the folder list. */
  .r-star {
    position: absolute;
    top: 9px;
    left: 31px;
    display: flex;
  }
  /* The cloud, at the end of a result's title line. */
  .r-marks {
    position: absolute;
    top: 9px;
    right: 10px;
    display: flex;
    align-items: center;
    gap: 4px;
  }
  /* A folder row's place for the star documents have. */
  .star-space {
    flex: none;
    width: 16px;
  }
  .result:hover {
    background: var(--hover);
  }
  .result.selected {
    background: var(--accent-soft);
  }
  .r-title {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .r-title strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .r-name {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .snippet {
    margin: 3px 0 0 21px;
    font-size: 12px;
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .snippet mark {
    background: rgba(217, 165, 32, 0.35);
    color: inherit;
    border-radius: 2px;
  }
  .r-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    margin: 3px 0 0 21px;
    font-size: 11px;
  }

  /* Details */
  .details {
    background: var(--panel);
    border-left: 1px solid var(--border);
    overflow: auto;
    /* Room for the scroll bar always: where it takes space (Windows,
       Linux) it would otherwise narrow the column, shrink the page preview
       (sized by width), no longer be needed, and come back: a flicker. */
    scrollbar-gutter: stable;
    display: flex;
    flex-direction: column;
  }
  .details-body {
    flex: none;
    padding: 10px 14px;
  }
  .details h2 {
    margin-bottom: 10px;
  }
  .detail-name {
    font-weight: 600;
    font-size: 14px;
    word-break: break-word;
    margin-bottom: 10px;
  }
  hr {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 14px 0;
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 10px;
    margin: 0 0 14px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    word-break: break-word;
  }
  .cloud-note {
    display: flex;
    gap: 4px;
    align-items: center;
    color: var(--cloud);
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  /* As tall as the text buttons beside them (one control size per group,
     as Apple's guidelines ask): 13px text + 2 × 4px padding + border. */
  .actions .icon-action {
    width: 32px;
    min-height: 26px;
    padding: 0;
    display: grid;
    place-items: center;
    color: var(--accent-strong);
  }

  .undo-bar {
    position: fixed;
    left: 50%;
    bottom: 36px;
    transform: translateX(-50%);
    z-index: 25; /* above the chat button, below dialogs */
    display: flex;
    align-items: center;
    gap: 14px;
    max-width: calc(100vw - 160px);
    padding: 8px 10px 8px 14px;
    border-radius: 10px;
    background: #2b2723;
    color: #fff;
    font-size: 13px;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.25);
  }
  .undo-bar span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .undo-link,
  .undo-close {
    border: 0;
    background: none;
    color: #f0c896;
    font: inherit;
    padding: 0 2px;
  }
  .undo-link {
    text-decoration: underline;
    font-weight: 600;
  }
  .undo-close {
    color: #cfc6bb;
    font-size: 16px;
    line-height: 1;
  }
  footer {
    height: 22px;
    padding: 3px 12px;
    font-size: 11px;
    color: var(--muted);
    border-top: 1px solid var(--border);
    display: flex;
    justify-content: space-between;
    gap: 12px;
    white-space: nowrap;
    overflow: hidden;
  }
  .status {
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
