<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ask, open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    api,
    onDirChanged,
    onIndexChanged,
    onIndexProgress,
    onIntakeChanged,
    onMenuImport,
    onMenuScan,
    onMenuSettings,
    onMenuCheckUpdates,
    providerLabel,
    revealLabel,
    editionLabel,
    snippetParts,
    type AppInfo,
    type Filed,
    type Settings,
    type Entry,
    type Hit,
    type IndexStats,
    type ListedEntry,
    type Progress,
    type Source,
    type TagCount,
    type TagMode,
    type FavoriteFolder,
    type AssistantAction,
  } from "#lib/api.ts";
  import { changes } from "#lib/changes.svelte.ts";
  import { formatDate, formatSize, kindLabel } from "#lib/format.ts";
  import FolderNode from "#lib/FolderNode.svelte";
  import MoveDialog from "#lib/MoveDialog.svelte";
  import { dragSource, dropTarget, isDropOver, type MoveItem } from "#lib/drag.svelte.ts";
  import Icon from "#lib/Icon.svelte";
  import ImportDialog from "#lib/ImportDialog.svelte";
  import Inbox from "#lib/Inbox.svelte";
  import TagFilter from "#lib/TagFilter.svelte";
  import TagManager from "#lib/TagManager.svelte";
  import Assistant from "#lib/Assistant.svelte";
  import ScanDialog from "#lib/ScanDialog.svelte";
  import SettingsDialog from "#lib/SettingsDialog.svelte";
  import MetaEditor from "#lib/MetaEditor.svelte";
  import PictureView from "#lib/PictureView.svelte";
  import DocStar from "#lib/DocStar.svelte";
  import CloudBadge from "#lib/CloudBadge.svelte";
  import * as docActions from "#lib/doc-actions.svelte.ts";
  import MarkdownReader from "#lib/MarkdownReader.svelte";
  import { isMarkdown } from "#lib/markdown.ts";
  import { checkNowAndThen, updates } from "#lib/updates.svelte.ts";
  import RupertOwl from "#lib/RupertOwl.svelte";
  import { m } from "#lib/i18n/index.ts";

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
  let settingsOpen = $state(false);
  /** A section of Settings to show first (e.g. from the chat's gear). */
  let settingsSection = $state<"assistant" | "updates" | null>(null);
  let scanOpen = $state(false);
  let appSettings = $state<Settings | null>(null);
  let inboxOpen = $state(false);
  let inboxCount = $state(0);
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

  const searching = $derived(query.trim() !== "" || selectedTags.length > 0 || favoritesOnly);
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
      if (entry) selected = { sourceId: current.sourceId, entry };
    } else if (selected && !searching) {
      const rel = selected.entry.relPath;
      const still = entries.find((e) => e.relPath === rel);
      selected = still ? { sourceId: current.sourceId, entry: still } : null;
    }
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
    inboxOpen = false;
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

    const unlisten = [
      onDirChanged((batch) => changes.push(batch)),
      onMenuImport(() => (importing = true)),
      onMenuSettings(() => (settingsOpen = true)),
      onMenuCheckUpdates(() => {
        settingsSection = "updates";
        settingsOpen = true;
        updates.check(true);
      }),
      onMenuScan(() => (scanOpen = true)),
      onIntakeChanged(loadInboxCount),
      onIndexProgress((p) => (progress = p)),
      onIndexChanged(() => {
        indexTick++;
        loadIndexInfo();
      }),
      getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) loadSources(true);
      }),
    ];
    const closeMenu = () => {
      menuFor = null;
      itemMenu = null;
    };
    const shortcuts = (e: KeyboardEvent) => {
      const typing =
        e.target instanceof Element && e.target.closest("input, textarea, select, [contenteditable]");
      if (e.key === "Escape" && itemMenu) {
        itemMenu = null;
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
        trashEntry(selected);
      } else if ((e.metaKey || e.ctrlKey) && e.key === "f") {
        e.preventDefault();
        inboxOpen = false;
        searchInput?.focus();
        searchInput?.select();
      }
    };
    window.addEventListener("click", closeMenu);
    window.addEventListener("scroll", closeMenu, true);
    window.addEventListener("keydown", shortcuts);
    return () => {
      unlisten.forEach((p) => p.then((off) => off()));
      window.removeEventListener("click", closeMenu);
      window.removeEventListener("scroll", closeMenu, true);
      window.removeEventListener("keydown", shortcuts);
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
  $effect(() => {
    const args = { text: query, tags: [...selectedTags], tagMode, favoritesOnly };
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
      searchedOnce = true;
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

  function filterFavorites() {
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
  }

  function openInbox() {
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
    if (e.isDir) selectFolder(sourceId, e.relPath);
    else if (isMarkdown(e) && e.cloud !== "cloudOnly") reader = { sourceId, relPath: e.relPath };
    else run(() => api.openEntry(sourceId, e.relPath));
  }

  function reveal(sel: Selected) {
    run(() => api.revealEntry(sel.sourceId, sel.entry.relPath));
  }

  // MARK: - Right-click menu and Trash

  /** The right-click menu: where, for what, and whether it's a search result. */
  let itemMenu = $state<{ x: number; y: number; sel: Selected; inSearch: boolean } | null>(null);
  const trashLabel = $derived(info?.os === "windows" ? m.trash.delete : m.trash.moveToTrash);
  const binName = $derived(info?.os === "windows" ? m.trash.recycleBin : m.trash.trash);

  function openItemMenu(sel: Selected, ev: MouseEvent, inSearch = false) {
    ev.preventDefault();
    ev.stopPropagation();
    menuFor = null;
    selected = sel;
    // Keep it inside the window (the menu is about 220 × 150).
    const x = Math.min(ev.clientX, window.innerWidth - 230);
    const y = Math.min(ev.clientY, window.innerHeight - 160);
    itemMenu = { x, y, sel, inSearch };
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
    if (folder?.sourceId === sel.sourceId) entries = entries.filter((x) => !gone(x, sel.sourceId));
    if (selected && gone(selected.entry, selected.sourceId)) {
      const next = folderMode === "gallery" && at >= 0 ? (entries[at] ?? entries[at - 1]) : undefined;
      selected = next ? { sourceId: sel.sourceId, entry: next } : null;
    }
  }

  // MARK: - Moving

  /** The document or folder "Move to…" is choosing a place for. */
  let moving = $state<Selected | null>(null);

  /** Moves a document (with its details) or a folder into another folder,
   * in this source or another. Resolves to whether it worked. */
  // The star and the cloud on a document in a list.
  function starDoc(sourceId: string, e: Entry & { favorite?: boolean }) {
    run(() => docActions.toggleFavorite(sourceId, e));
  }
  function downloadDoc(sourceId: string, e: Entry) {
    run(() => docActions.download(sourceId, e));
  }

  async function moveItem(item: MoveItem, destSourceId: string, destDir: string): Promise<boolean> {
    const done = await run(async () => {
      await api.moveEntry(item.sourceId, item.entry.relPath, destSourceId, destDir);
      return true;
    });
    if (!done) return false;
    forget(item);
    if (folder?.sourceId === destSourceId && folder.relPath === destDir) loadEntries();
    return true;
  }

  function dirOf(relPath: string) {
    return relPath.includes("/") ? relPath.slice(0, relPath.lastIndexOf("/")) : "";
  }

  function statusText(): string {
    if (progress?.active && progress.total > 0) {
      return m.main.indexing(progress.done, progress.total);
    }
    if (progress?.active) return m.main.checkingChanges;
    if (progress && progress.downloadTotal > 0) {
      return m.main.downloadingToRead(progress.downloadDone, progress.downloadTotal);
    }
    if (!stats || stats.documents === 0) return "";
    const parts = [m.main.documents(stats.documents)];
    parts.push(m.main.searchable(stats.withText));
    if (stats.cloudOnlyUnindexed) parts.push(m.main.onlineOnlyCount(stats.cloudOnlyUnindexed));
    return parts.join(" · ");
  }
</script>

<div class="shell" class:inbox-mode={inboxOpen}>
  <!-- MARK: Sidebar -->
  <aside class="sources">
    <header>
      <h2>{m.main.library}</h2>
    </header>
    <button class="lib-item" class:active={inboxOpen} onclick={openInbox}>
      <span class="lib-icon inbox-icon"><Icon name="inbox" size={14} /></span>
      <span class="label">{m.main.inbox}</span>
      {#if !appSettings?.inbox}<span class="count">{m.main.setUp}</span>{:else if inboxCount}<span class="count">{inboxCount}</span>{/if}
    </button>
    <button class="lib-item" class:active={favoritesOnly} onclick={filterFavorites}>
      <span class="lib-icon star">★</span>
      <span class="label">{m.main.favorites}</span>
      {#if stats?.favorites}<span class="count">{stats.favorites}</span>{/if}
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
        {#if selectedTags.length}<span class="badge">{selectedTags.length}</span>{/if}
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
      <button role="menuitem" onclick={() => startRename(s)}>
        <span class="check"></span>{m.main.rename}
      </button>
      <button role="menuitem" class="danger" onclick={() => removeSource(s)}>
        <span class="check"></span>{m.main.removeFromRupert}
      </button>
    </div>
  {/snippet}

  {#snippet tagChips(list: string[] | undefined)}
    {#each list ?? [] as t (t)}<span class="chip">{t}</span>{/each}
  {/snippet}

  {#if inboxOpen && appSettings}
    <main class="documents inbox-pane">
      <Inbox
        {sources}
        settings={appSettings}
        onfiled={showFiled}
        onsettings={() => (settingsOpen = true)}
        onscan={() => (scanOpen = true)}
      />
    </main>
  {:else}
  <!-- MARK: Documents -->
  <main class="documents" class:gallery-mode={!searching && folder && folderMode === "gallery" && entries.length > 0}>
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

    {#if error}
      <div class="error" role="alert">
        <Icon name="warning" size={14} />
        <span>{error}</span>
        <button class="icon-btn" onclick={() => (error = null)} aria-label={m.main.dismiss}>×</button>
      </div>
    {/if}

    {#if searching}
      <p class="result-count muted">
        {#if !searchedOnce}{m.main.searching}{:else if hits.length === 0}{m.main.noMatches}{:else}{m.main.hits(hits.length, hits.length >= 300)}{/if}
      </p>
      <ul class="results">
        {#each hits as h (h.sourceId + "\n" + h.relPath)}
          <li class="doc-host" class:selected={selected?.sourceId === h.sourceId && selected?.entry.relPath === h.relPath}>
            <button
              class="result"
              class:selected={selected?.sourceId === h.sourceId && selected?.entry.relPath === h.relPath}
              onclick={() => (selected = { sourceId: h.sourceId, entry: h })}
              ondblclick={() => activate(h.sourceId, h)}
              oncontextmenu={(ev) => openItemMenu({ sourceId: h.sourceId, entry: h }, ev, true)}
              {...dragSource({ sourceId: h.sourceId, entry: h })}
            >
              <div class="r-title">
                <span class="file-icon"><Icon name={h.kind === "pdf" ? "pdf" : "file"} size={15} /></span>
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
                {@render tagChips(h.tags)}
              </div>
            </button>
            <!-- Beside the result's button (buttons can't hold buttons). -->
            <span class="r-marks">
              {#if h.cloud === "cloudOnly"}<CloudBadge sourceId={h.sourceId} doc={h} size={13} onclick={() => downloadDoc(h.sourceId, h)} />{/if}
              <DocStar sourceId={h.sourceId} doc={h} onclick={() => starDoc(h.sourceId, h)} />
            </span>
          </li>
        {/each}
      </ul>
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
          {entries}
          selectedRelPath={selected?.sourceId === source.id ? selected.entry.relPath : null}
          onselect={(e) => (selected = { sourceId: source.id, entry: e })}
          onactivate={(e) => activate(source.id, e)}
          onmenu={(e, ev) => openItemMenu({ sourceId: source.id, entry: e }, ev)}
          onmove={moveItem}
          onstar={(e) => starDoc(source.id, e)}
          ondownload={(e) => downloadDoc(source.id, e)}
        />
      {:else}
        <table>
          <thead>
            <tr>
              <th class="name">{m.main.name}</th>
              <th>{m.main.kind}</th>
              <th class="num">{m.main.size}</th>
              <th>{m.main.modified}</th>
            </tr>
          </thead>
          <tbody>
            {#each entries as e (e.relPath)}
              <tr
                class="doc-host"
                class:selected={selected?.entry.relPath === e.relPath}
                class:drop={e.isDir && isDropOver(source.id, e.relPath)}
                onclick={() => (selected = { sourceId: source.id, entry: e })}
                ondblclick={() => activate(source.id, e)}
                oncontextmenu={(ev) => openItemMenu({ sourceId: source.id, entry: e }, ev)}
                {...dragSource({ sourceId: source.id, entry: e })}
                {...e.isDir ? dropTarget(source.id, e.relPath, (item) => moveItem(item, source.id, e.relPath)) : {}}
              >
                <td class="name">
                  <span class="file-icon" class:folder={e.isDir}>
                    <Icon name={e.isDir ? "folder" : e.kind === "pdf" ? "pdf" : "file"} size={15} />
                  </span>
                  {#if e.isDir}<span class="star-space"></span>{:else}<DocStar sourceId={source.id} doc={e} onclick={() => starDoc(source.id, e)} />{/if}
                  <span class="file-name" title={e.title ?? undefined}>{e.name}</span>
                  {@render tagChips(e.tags)}
                  {#if e.cloud === "cloudOnly"}
                    <CloudBadge sourceId={source.id} doc={e} onclick={() => downloadDoc(source.id, e)} />
                  {/if}
                </td>
                <td class="muted">{kindLabel(e.isDir ? "folder" : e.kind)}</td>
                <td class="num muted">{e.isDir || (e.cloud === "cloudOnly" && !e.size) ? "" : formatSize(e.size)}</td>
                <td class="muted">{formatDate(e.modifiedMs)}</td>
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
  <section class="details">
    <h2>{m.main.details}</h2>
    {#if selected}
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
        {#if searching}
          <button onclick={() => showInFolder(selected!)}>{m.main.showInRupert}</button>
        {/if}
      </div>
    {:else}
      <p class="empty">{m.main.selectDocument}</p>
    {/if}
  </section>
  {/if}
</div>

{#if reader}
  <MarkdownReader sourceId={reader.sourceId} relPath={reader.relPath} onclose={() => (reader = null)} />
{/if}

{#if itemMenu}
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
    <button role="menuitem" onclick={() => fromMenu((sel) => (moving = sel))}>{m.main.moveTo}</button>
    <hr />
    <button role="menuitem" class="danger" onclick={() => trashEntry(mn.sel)}>{trashLabel}</button>
  </div>
{/if}

<Assistant
  bind:this={assistant}
  onaction={assistantAction}
  onsettings={() => {
    settingsSection = "assistant";
    settingsOpen = true;
  }}
/>

{#if tagManagerOpen}
  <TagManager
    onclose={() => (tagManagerOpen = false)}
    onchanged={() => {
      selectedTags = [];
      tagsBefore = 0;
      loadIndexInfo();
    }}
  />
{/if}

{#if scanOpen}
  <ScanDialog
    hasInbox={!!appSettings?.inbox}
    onclose={() => (scanOpen = false)}
    onscanned={() => {
      openInbox();
      loadInboxCount();
    }}
    onsettings={() => {
      scanOpen = false;
      settingsOpen = true;
    }}
  />
{/if}

{#if moving}
  <MoveDialog
    item={moving}
    {sources}
    onmove={async (sourceId, relPath) => {
      if (await moveItem(moving!, sourceId, relPath)) return null;
      const message = error; // shown in the dialog instead of behind it
      error = null;
      return message ?? m.main.moveFailed;
    }}
    onclose={() => (moving = null)}
  />
{/if}

{#if settingsOpen}
  <SettingsDialog
    {sources}
    section={settingsSection}
    onclose={() => {
      settingsOpen = false;
      settingsSection = null;
    }}
    onsaved={(s) => {
      appSettings = s;
      loadInboxCount();
      assistant?.refreshInfo();
    }}
  />
{/if}

{#if importing}
  <ImportDialog {sources} onclose={() => (importing = false)} onsourceadded={() => loadSources()} />
{/if}

<footer>
  <span>
    {#if info}
      <span title={m.main.versionTip}>{info.name} {info.version} · {editionLabel(info)}</span>
    {/if}
    <button class="footer-btn" onclick={() => (settingsOpen = true)} title={m.main.settings}>{m.main.settings}</button>
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
    {statusText()}
    {#if progress && !progress.pdfText}· {m.main.noPdfText}{/if}
  </span>
</footer>

<style>
  .shell.inbox-mode {
    grid-template-columns: 260px 1fr;
  }
  .inbox-pane {
    padding: 0;
    overflow: hidden;
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
    grid-template-columns: 260px 1fr 280px;
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
  .lib-item.active {
    background: var(--accent-soft);
    color: var(--accent-strong);
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
  }
  .searchbar {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-bottom: 12px;
  }
  .searchbar input {
    flex: 1;
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
  th {
    text-align: left;
    font-weight: 500;
    font-size: 11px;
    color: var(--muted);
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
  }
  th:nth-child(2) {
    width: 70px;
  }
  th:nth-child(3) {
    width: 80px;
  }
  th:nth-child(4) {
    width: 170px;
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
  td.name {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
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
    padding-right: 56px;
  }
  /* The star and cloud, at the end of a result's title line. */
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
    padding: 10px 14px;
    overflow: auto;
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
