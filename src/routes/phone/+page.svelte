<script lang="ts">
  // Rupert for iPhone: browse and search the library, open documents, and
  // scan paper into the inbox folder. Read-only for the library itself.
  import { onMount } from "svelte";
  import { onBackButtonPress } from "@tauri-apps/api/app";
  import type { PluginListener } from "@tauri-apps/api/core";
  import Icon from "#lib/Icon.svelte";
  import RupertOwl, { type OwlMood } from "#lib/RupertOwl.svelte";
  import TagFilter from "#lib/TagFilter.svelte";
  import Ask from "#lib/phone/Ask.svelte";
  import FolderPictures from "#lib/phone/FolderPictures.svelte";
  import { snippetParts, type TagMode } from "#lib/api.ts";
  import { formatSize } from "#lib/format.ts";
  import {
    formatDocDate,
    onFoldersReady,
    onIndexChanged,
    onProgress,
    onRemoteProgress,
    phone,
    scanName,
    type DocInfo,
    type FavoriteFolder,
    type Hit,
    type Listed,
    type OneDriveFolder,
    type OneDriveStatus,
    type CloudService,
    type CloudStatus,
    type AppVersion,
    type AssistantAction,
    type AssistantStatus,
    type Overview,
    type Progress,
    type ScanSettings,
    type SharedSent,
    type TagCount,
  } from "#lib/phone/api.ts";
  import { LANGS, languageNames, loadLanguage, m, useLanguage, type Lang } from "#lib/i18n/index.ts";

  type Tab = "library" | "search" | "scan" | "ask" | "settings";
  /** Android: Google Drive and OneDrive only (no iCloud Drive or Files), the
   * back gesture, its own wording. Known at once from the web view. */
  const android = /Android/i.test(navigator.userAgent);
  const tx = $derived(android ? m.phone.android : null);
  /** `inbox`: shown as "Inbox" in the current language, whatever `name` says. */
  type Crumb = { sourceId: string; relPath: string; name: string; inbox?: boolean };
  /** What the document sheet shows: indexed details, or just the file. */
  type Sheet = { sourceId: string; relPath: string; name: string; cloudOnly: boolean; size: number; info: DocInfo | null };

  let tab = $state<Tab>("library");
  /** Ask Rupert (Apple Intelligence); the tab is offered when the iPhone can run it. */
  let askStatus = $state<AssistantStatus | null>(null);
  let ask: Ask | undefined = $state();
  let overview = $state<Overview | null>(null);
  let favFolders = $state<FavoriteFolder[]>([]);

  let stack = $state<Crumb[]>([]);
  let entries = $state<Listed[]>([]);
  let listing = $state(false);
  let listError = $state<string | null>(null);

  let query = $state("");
  let selectedTags = $state<string[]>([]);
  let tagMode = $state<TagMode>("any");
  let tagSheet = $state(false);
  let indexTick = $state(0);
  let favoritesOnly = $state(false);
  let hits = $state<Hit[]>([]);
  let tags = $state<TagCount[]>([]);

  let sheet = $state<Sheet | null>(null);
  let opening = $state(false);
  let sharing = $state(false);
  /** Settings ▸ Sharing: a PDF's notes go along. */
  let sendNotes = $state(false);
  let openError = $state<string | null>(null);

  let scanning = $state(false);
  const SCAN_KEY = "rupert.phoneScan";
  let scanSettings = $state<ScanSettings>(loadScanSettings());

  function loadScanSettings(): ScanSettings {
    try {
      const saved = JSON.parse(localStorage.getItem(SCAN_KEY) ?? "null");
      if (saved?.mode && saved?.dpi) return saved;
    } catch {
      /* first time */
    }
    return { mode: "gray", dpi: 300 };
  }

  function saveScanSettings() {
    try {
      localStorage.setItem(SCAN_KEY, JSON.stringify($state.snapshot(scanSettings)));
    } catch {
      /* not essential */
    }
  }
  let lastScan = $state<string | null>(null);
  let scanError = $state<string | null>(null);
  /** Rupert, the scan button: happy for a moment after a scan, puzzled after an error. */
  let afterScan = $state<"ready" | "saved" | "error">("ready");
  let afterScanTimer: ReturnType<typeof setTimeout> | undefined;
  const scanState = $derived(scanning ? "saving" : afterScan);
  const scanMood: Record<typeof scanState, OwlMood> = {
    ready: "idle",
    saving: "opening", // the folder badge: he's filing it
    saved: "happy",
    error: "puzzled",
  };
  const scanSays = $derived<Record<typeof scanState, string>>({
    ready: m.phone.scanReady,
    saving: m.phone.scanSaving,
    saved: m.phone.scanSaved,
    error: m.phone.scanError,
  });

  let confirmRemove = $state<string | null>(null);
  let od = $state<OneDriveStatus | null>(null);
  let gd = $state<CloudStatus | null>(null);
  let remoteProgress = $state<Progress | null>(null);
  /** The cloud folder chooser: which service, what it's for, and where it is. */
  type Chooser = {
    service: CloudService;
    purpose: "library" | "inbox";
    path: OneDriveFolder[];
    folders: OneDriveFolder[];
    loading: boolean;
    error: string | null;
  };
  let chooser = $state<Chooser | null>(null);
  /** Settings ▸ Language: the choice (null: automatic) and the iPhone's language. */
  let languageSetting = $state<string | null>(null);
  let systemLang = $state<Lang>("en");

  async function loadLanguageInfo() {
    const info = await loadLanguage();
    if (info) {
      languageSetting = info.setting ?? null;
      systemLang = info.system;
    }
  }

  async function chooseLanguage(language: string | null) {
    const info = await phone.setLanguage(language).catch(() => null);
    if (!info) return;
    languageSetting = info.setting ?? null;
    systemLang = info.system;
    useLanguage(info.current);
  }
  let busy = $state(false);
  let settingsError = $state<string | null>(null);

  const here = $derived(stack.at(-1) ?? null);
  const hereFav = $derived(
    !!here && favFolders.some((f) => f.sourceId === here.sourceId && f.relPath === here.relPath),
  );
  let favBusy = $state(false);
  let favError = $state<string | null>(null);
  const progress = $derived(overview?.progress);
  const busyIndexing = $derived(Boolean(progress?.active || remoteProgress?.active));
  const shownProgress = $derived(remoteProgress?.active ? remoteProgress : progress);
  const title = $derived(
    tab === "library"
      ? here?.inbox
        ? m.phone.inbox
        : (here?.name ?? m.phone.library)
      : tab === "search"
        ? m.phone.search
        : tab === "scan"
          ? m.phone.scan
          : tab === "ask"
            ? m.assistant.title
            : m.phone.settings,
  );

  // MARK: Loading

  async function loadOverview() {
    overview = await phone.overview();
    od = await phone.onedriveStatus().catch(() => null);
    gd = await phone.gdriveStatus().catch(() => null);
    favFolders = await phone.favoriteFolders().catch(() => []);
    sendNotes = await phone.sendNotes().catch(() => false);
  }

  async function loadFolder() {
    if (!here) return;
    const { sourceId, relPath } = here;
    listing = true;
    listError = null;
    try {
      const list = await phone.listDir(sourceId, relPath);
      if (here?.sourceId === sourceId && here?.relPath === relPath) entries = list;
    } catch (e) {
      listError = String(e);
      entries = [];
    } finally {
      listing = false;
    }
  }

  let searchTimer: ReturnType<typeof setTimeout> | undefined;
  function runSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(async () => {
      hits = await phone.search(query, [...selectedTags], tagMode, favoritesOnly).catch(() => []);
    }, 150);
  }

  let refreshTimer: ReturnType<typeof setTimeout> | undefined;
  function indexChanged() {
    clearTimeout(refreshTimer);
    refreshTimer = setTimeout(() => {
      loadOverview();
      if (here) loadFolder();
      if (tab === "search") runSearch();
      phone.tags().then((t) => (tags = t));
      indexTick++;
    }, 300);
  }

  /** Shown in Settings ▸ About. */
  let version = $state<AppVersion | null>(null);

  /** Settings ▸ About ▸ Privacy policy: the web page, in the phone's browser. */
  async function openPrivacyPolicy() {
    settingsError = null;
    try {
      await phone.openPrivacyPolicy();
    } catch (e) {
      settingsError = String(e);
    }
  }

  // MARK: Folder views

  /** How each folder is shown (list, large icons or gallery), kept on this
   * phone by "<source id>\n<folder>". Folders not set are lists. */
  type FolderView = "list" | "icons" | "gallery";
  const VIEWS_KEY = "rupert.phone.folderViews";
  let folderViews = $state<Record<string, FolderView>>(loadFolderViews());

  function loadFolderViews(): Record<string, FolderView> {
    try {
      return JSON.parse(localStorage.getItem(VIEWS_KEY) ?? "{}") ?? {};
    } catch {
      return {};
    }
  }

  const hereView = $derived<FolderView>(here ? (folderViews[`${here.sourceId}\n${here.relPath}`] ?? "list") : "list");

  function setView(view: FolderView) {
    if (!here) return;
    const key = `${here.sourceId}\n${here.relPath}`;
    if (view === "list") delete folderViews[key];
    else folderViews[key] = view;
    try {
      localStorage.setItem(VIEWS_KEY, JSON.stringify(folderViews));
    } catch {
      // Not essential.
    }
  }

  // MARK: Documents shared from other apps

  /** What was last sent from the share sheet (or why it waits). */
  let shared = $state<SharedSent | null>(null);
  let sharedTimer: ReturnType<typeof setTimeout> | undefined;

  /** Sends documents shared with Rupert that wait on the phone to the inbox:
   * on Android all of them, on iPhone those the share sheet couldn't save. */
  async function sendShared() {
    const result = await phone
      .sendShared()
      .catch((e): SharedSent => ({ sent: [], problems: [String(e)], refused: [] }));
    if (!result.sent.length && !result.problems.length && !result.refused.length) return;
    shared = result;
    clearTimeout(sharedTimer);
    if (!result.problems.length && !result.refused.length) sharedTimer = setTimeout(() => (shared = null), 6000);
  }

  // When the inbox is chosen or changed, documents waiting for it can go.
  let inboxSeen: string | null | undefined;
  $effect(() => {
    const inbox = overview ? (overview.inbox ?? null) : undefined;
    if (inbox === undefined || inbox === inboxSeen) return;
    const first = inboxSeen === undefined;
    inboxSeen = inbox;
    if (!first && inbox) sendShared();
  });

  onMount(() => {
    // Sharing on Android opens Rupert; on iPhone, anything the share sheet
    // couldn't save waits for the app.
    sendShared();
    const onVisible = () => {
      if (document.visibilityState === "visible") sendShared();
    };
    document.addEventListener("visibilitychange", onVisible);
    return () => document.removeEventListener("visibilitychange", onVisible);
  });

  onMount(() => {
    loadOverview();
    phone.appVersion().then((v) => (version = v));
    loadLanguageInfo();
    loadAskStatus();
    phone.tags().then((t) => (tags = t));
    const unlisten = [
      onProgress((p) => {
        if (overview) overview.progress = p;
      }),
      onIndexChanged(indexChanged),
      onRemoteProgress((p) => {
        remoteProgress = p;
        if (!p.active) loadOverview();
      }),
      onFoldersReady(loadOverview),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  // MARK: Back gesture (Android)

  /** Something the back gesture can close or leave, innermost first. */
  function goBack(): boolean {
    if (chooser) chooser = null;
    else if (sheet) sheet = null;
    else if (tagSheet) tagSheet = false;
    else if (confirmRemove) confirmRemove = null;
    else if (tab === "library" && stack.length) back();
    else if (tab !== "library") switchTab("library");
    else return false;
    return true;
  }
  const canGoBack = $derived(
    Boolean(chooser || sheet || tagSheet || confirmRemove || stack.length || tab !== "library"),
  );
  // Listen only while there's something to go back from; otherwise Android's
  // own back gesture leaves the app.
  let backListener: Promise<PluginListener> | null = null;
  $effect(() => {
    if (!android) return;
    if (canGoBack && !backListener) {
      backListener = onBackButtonPress(() => goBack());
    } else if (!canGoBack && backListener) {
      const listener = backListener;
      backListener = null;
      listener.then((l) => l.unregister());
    }
  });

  // MARK: Navigation

  /** The library folders (without the source added only for the inbox). */
  const library = $derived(overview?.sources.filter((s) => !s.inbox) ?? []);

  /** Shows the inbox, wherever it is in the library. */
  function openInbox() {
    const place = overview?.inboxPlace;
    if (!place) return;
    tab = "library";
    stack = [{ sourceId: place.sourceId, relPath: place.relPath, name: m.phone.inbox, inbox: true }];
    entries = [];
    loadFolder();
  }

  function openSource(id: string, name: string) {
    stack = [{ sourceId: id, relPath: "", name }];
    entries = [];
    loadFolder();
  }

  function openFavorite(f: FavoriteFolder) {
    stack = [{ sourceId: f.sourceId, relPath: f.relPath, name: f.name }];
    entries = [];
    loadFolder();
  }

  async function toggleFolderFav() {
    if (!here || favBusy) return;
    const { sourceId, relPath, name } = here;
    const next = !hereFav;
    favBusy = true;
    favError = null;
    // Show it right away; undo if saving fails.
    favFolders = next
      ? [...favFolders, { sourceId, relPath, name, sourceName: "" }]
      : favFolders.filter((f) => !(f.sourceId === sourceId && f.relPath === relPath));
    try {
      await phone.setFolderFavorite(sourceId, relPath, next);
    } catch (e) {
      favError = String(e);
    } finally {
      favFolders = await phone.favoriteFolders().catch(() => favFolders);
      favBusy = false;
    }
  }

  function showFavoriteDocuments() {
    favoritesOnly = true;
    selectedTags = [];
    query = "";
    switchTab("search");
  }

  function openEntry(e: Listed) {
    if (!here) return;
    if (e.isDir) {
      stack = [...stack, { sourceId: here.sourceId, relPath: e.relPath, name: e.name }];
      entries = [];
      loadFolder();
    } else {
      showDoc(here.sourceId, e.relPath, e.name, e.cloud === "cloudOnly", e.size);
    }
  }

  function back() {
    stack = stack.slice(0, -1);
    entries = [];
    if (here) loadFolder();
  }

  async function loadAskStatus() {
    askStatus = await phone.assistantStatus().catch(() => null);
  }

  /** Shows what Rupert's answer points to (its buttons under the answer). */
  async function askAction(a: AssistantAction) {
    const last = (p: string) => p.split("/").pop() ?? p;
    switch (a.kind) {
      case "openFolder":
        tab = "library";
        stack = [{ sourceId: a.sourceId, relPath: a.relPath, name: a.relPath ? last(a.relPath) : sourceName(a.sourceId) }];
        entries = [];
        loadFolder();
        break;
      case "showDocument": {
        const info = await phone.document(a.sourceId, a.relPath).catch(() => null);
        showDoc(a.sourceId, a.relPath, last(a.relPath), info?.cloud === "cloudOnly", info?.size ?? 0);
        break;
      }
      case "search":
        query = a.text;
        selectedTags = [...a.tags];
        tagMode = a.tagMode;
        favoritesOnly = a.favoritesOnly;
        switchTab("search");
        break;
    }
  }

  function switchTab(t: Tab) {
    if (t === tab && t === "library") stack = [];
    if (t === "ask") loadAskStatus();
    tab = t;
    if (t === "search") runSearch();
    if (t === "settings" || t === "scan") loadOverview();
  }

  async function showDoc(sourceId: string, relPath: string, name: string, cloudOnly: boolean, size: number) {
    openError = null;
    sheet = { sourceId, relPath, name, cloudOnly, size, info: null };
    const info = await phone.document(sourceId, relPath).catch(() => null);
    if (sheet?.relPath === relPath) sheet.info = info;
  }

  async function openDoc() {
    if (!sheet) return;
    opening = true;
    openError = null;
    try {
      await phone.open(sheet.sourceId, sheet.relPath);
    } catch (e) {
      openError = String(e);
    } finally {
      opening = false;
    }
  }

  /** Share: the phone's share sheet (downloads the document first if needed). */
  async function shareDoc(button: HTMLElement) {
    if (!sheet) return;
    const r = button.getBoundingClientRect();
    sharing = true;
    openError = null;
    try {
      await phone.share(sheet.sourceId, sheet.relPath, [r.left, r.top, r.width, r.height]);
    } catch (e) {
      openError = String(e);
    } finally {
      sharing = false;
    }
  }

  // MARK: Actions

  async function scan() {
    scanning = true;
    scanError = null;
    clearTimeout(afterScanTimer);
    afterScan = "ready";
    try {
      saveScanSettings();
      const saved = await phone.scan(scanName(), $state.snapshot(scanSettings));
      if (saved) {
        lastScan = saved;
        afterScan = "saved";
        afterScanTimer = setTimeout(() => (afterScan = "ready"), 2500);
      }
    } catch (e) {
      scanError = String(e);
      afterScan = "error";
    } finally {
      scanning = false;
    }
  }

  async function act(f: () => Promise<unknown>) {
    busy = true;
    settingsError = null;
    try {
      await f();
      await loadOverview();
    } catch (e) {
      settingsError = String(e);
    } finally {
      busy = false;
    }
  }

  // MARK: OneDrive and Google Drive

  const serviceName: Record<CloudService, string> = { onedrive: "OneDrive", googledrive: "Google Drive" };

  async function openChooser(service: CloudService, purpose: "library" | "inbox") {
    chooser = { service, purpose, path: [], folders: [], loading: true, error: null };
    await loadChooser();
  }

  async function loadChooser() {
    if (!chooser) return;
    chooser.loading = true;
    chooser.error = null;
    try {
      const parent = chooser.path.at(-1)?.id ?? null;
      chooser.folders = await (chooser.service === "googledrive"
        ? phone.gdriveFolders(parent)
        : phone.onedriveFolders(parent));
    } catch (e) {
      chooser.error = String(e);
      chooser.folders = [];
    } finally {
      if (chooser) chooser.loading = false;
    }
  }

  function chooserInto(f: OneDriveFolder) {
    if (!chooser) return;
    chooser.path = [...chooser.path, f];
    loadChooser();
  }

  function chooserBack() {
    if (!chooser) return;
    chooser.path = chooser.path.slice(0, -1);
    loadChooser();
  }

  async function chooserUse() {
    const c = chooser;
    const here = c?.path.at(-1);
    if (!c || !here) return;
    chooser = null;
    const path = $state.snapshot(c.path);
    await act(() =>
      c.service === "googledrive"
        ? c.purpose === "library"
          ? phone.addGdriveFolder(here.id, here.name)
          : phone.setGdriveInbox(here.id, here.name, path)
        : c.purpose === "library"
          ? phone.addOnedriveFolder(here.id, here.name)
          : phone.setOnedriveInbox(here.id, here.name, path),
    );
  }

  const providerName = $derived<Record<string, string>>({
    icloud: "iCloud Drive",
    onedrive: "OneDrive",
    googledrive: "Google Drive",
    dropbox: "Dropbox",
    local: m.phone.folder,
  });

  function folderOf(relPath: string): string {
    const i = relPath.lastIndexOf("/");
    return i < 0 ? "" : relPath.slice(0, i);
  }

  function sourceName(id: string): string {
    return overview?.sources.find((s) => s.id === id)?.name ?? "";
  }
</script>

<div class="app">
  <header>
    {#if tab === "library" && here}
      <button class="back" onclick={back} aria-label={m.phone.back}><Icon name="back" size={22} /></button>
    {/if}
    <h1>{title}</h1>
    {#if tab === "library" && here}
      <button
        class="star"
        class:on={hereFav}
        onclick={toggleFolderFav}
        disabled={favBusy}
        aria-pressed={hereFav}
        aria-label={hereFav ? m.phone.unfavoriteFolder : m.phone.favoriteFolder}
        >{hereFav ? "★" : "☆"}</button
      >
    {/if}
    {#if tab === "ask"}
      <button class="star" onclick={() => ask?.reset()} aria-label={m.assistant.newConversation}>
        <Icon name="new-chat" size={22} />
      </button>
    {/if}
    {#if busyIndexing && shownProgress}
      <span class="indexing" title={shownProgress.current ?? ""}>
        <span class="spinner"></span>{shownProgress.total ? `${shownProgress.done} / ${shownProgress.total}` : ""}
      </span>
    {/if}
  </header>

  {#if shared}
    <div class="shared" class:problem={shared.problems.length > 0 || shared.refused.length > 0} role="status">
      <p>
        {#if shared.sent.length === 1}{m.phone.sharedSentOne(shared.sent[0])}{:else if shared.sent.length > 1}{m.phone.sharedSentMany(shared.sent.length)}{/if}
        {#if shared.problems.length}{m.phone.sharedWaiting(shared.problems.join(" "))}{/if}
        {#if shared.refused.length}{m.phone.sharedRefused(shared.refused.join(", "))}{/if}
      </p>
      {#if shared.sent.length && overview?.inboxPlace}
        <button class="plain" onclick={() => ((shared = null), openInbox())}>{m.phone.openInbox}</button>
      {/if}
      <button class="plain close" onclick={() => (shared = null)} aria-label={m.main.dismiss}>×</button>
    </div>
  {/if}

  <main hidden={tab === "ask"}>
    {#if tab === "library"}
      {#if !here}
        {#if overview && library.length === 0 && !overview.inboxPlace}
          <div class="empty">
            <p>{tx?.pickFolders ?? m.phone.pickFolders}</p>
            {#if !android}
              <button class="primary" disabled={busy} onclick={() => act(phone.addFolder)}>{m.phone.addFolder}</button>
            {/if}
            {#if od?.signedIn}
              <button class="primary" disabled={busy} onclick={() => openChooser("onedrive", "library")}>{m.phone.addOneDrive}</button>
            {:else if od?.configured}
              <p class="note small">{m.phone.oneDriveSignInFirst}</p>
            {/if}
            {#if gd?.signedIn}
              <button class="primary" disabled={busy} onclick={() => openChooser("googledrive", "library")}>
                {m.phone.addGoogleDrive}
              </button>
            {:else if gd?.configured}
              <p class="note small">{m.phone.googleDriveSignInFirst}</p>
            {/if}
            {#if settingsError}<p class="error">{settingsError}</p>{/if}
          </div>
        {:else if overview}
          {#if favFolders.length || overview.stats.favorites}
            <h2 class="group">{m.phone.favorites}</h2>
            <ul class="list">
              {#each favFolders as f (f.sourceId + "/" + f.relPath)}
                <li>
                  <button class="row" onclick={() => openFavorite(f)}>
                    <span class="icon folder"><Icon name="folder" size={22} /></span>
                    <span class="main">
                      <span class="name">{f.name}</span>
                      <span class="sub">{f.relPath ? `${f.sourceName} / ${f.relPath}` : f.sourceName}</span>
                    </span>
                    <Icon name="chevron" />
                  </button>
                </li>
              {/each}
              {#if overview.stats.favorites}
                <li>
                  <button class="row" onclick={showFavoriteDocuments}>
                    <span class="icon fav"><Icon name="star" size={22} /></span>
                    <span class="main">
                      <span class="name">{m.phone.favoriteDocuments}</span>
                      <span class="sub">{overview.stats.favorites}</span>
                    </span>
                    <Icon name="chevron" />
                  </button>
                </li>
              {/if}
            </ul>
            <h2 class="group">{m.phone.library}</h2>
          {/if}
          <ul class="list">
            {#if overview.inboxPlace}
              <li>
                <button class="row" onclick={openInbox}>
                  <span class="icon folder"><Icon name="inbox" size={22} /></span>
                  <span class="main">
                    <span class="name">{m.phone.inbox}</span>
                    <span class="sub">{m.phone.newScans(overview.inbox ?? "")}</span>
                  </span>
                  <Icon name="chevron" />
                </button>
              </li>
            {/if}
            {#each library as s (s.id)}
              <li>
                <button class="row" onclick={() => openSource(s.id, s.name)} disabled={!s.available}>
                  <span class="icon folder"><Icon name="library" size={22} /></span>
                  <span class="main">
                    <span class="name">{s.name}</span>
                    <span class="sub">{s.available ? providerName[s.provider] : (tx?.notAvailable ?? m.phone.notAvailable)}</span>
                  </span>
                  <Icon name="chevron" />
                </button>
              </li>
            {/each}
          </ul>
          {#if library.length === 0}
            <p class="note">{m.phone.addInSettings}</p>
          {/if}
          <p class="note">{m.phone.stats(overview.stats.documents, overview.stats.withText)}</p>
        {/if}
      {:else}
        {#if listError}<p class="error pad">{listError}</p>{/if}
        {#if favError}<p class="error pad">{favError}</p>{/if}
        {#if !listing && entries.length === 0 && !listError}
          <p class="note">{m.phone.noDocuments}</p>
        {/if}
        {#if entries.length > 0}
          <div class="viewbar" role="group" aria-label={m.main.view}>
            {#each [["list", "view-list", m.main.list], ["icons", "view-grid", m.main.largeIcons], ["gallery", "view-gallery", m.main.gallery]] as const as [view, icon, label] (view)}
              <button class:on={hereView === view} onclick={() => setView(view)} aria-label={label} aria-pressed={hereView === view}>
                <Icon name={icon} size={18} />
              </button>
            {/each}
          </div>
        {/if}
        {#if hereView !== "list" && here && entries.length > 0}
          <FolderPictures mode={hereView} sourceId={here.sourceId} {entries} onopen={openEntry} />
        {:else}
        <ul class="list">
          {#each entries as e (e.relPath)}
            <li>
              <button class="row" onclick={() => openEntry(e)}>
                <span class="icon" class:folder={e.isDir}><Icon name={e.isDir ? "folder" : e.kind === "pdf" ? "pdf" : "file"} size={22} /></span>
                <span class="main">
                  <span class="name">{e.meta?.title || e.name}</span>
                  {#if !e.isDir}
                    <span class="sub">
                      {#if e.meta?.favorite}<span class="fav"><Icon name="star" size={11} /></span>{/if}
                      {[formatDocDate(e.meta?.documentDate ?? null), ...(e.meta?.tags ?? [])]
                        .filter(Boolean)
                        .join(" · ") || formatSize(e.size)}
                    </span>
                  {/if}
                </span>
                {#if e.cloud === "cloudOnly"}<span class="cloud" title={m.phone.inTheCloud}><Icon name="cloud" /></span>{/if}
                {#if e.isDir}<Icon name="chevron" />{/if}
              </button>
            </li>
          {/each}
        </ul>
        {/if}
      {/if}
    {:else if tab === "search"}
      <div class="searchbar">
        <Icon name="search" />
        <input
          type="search"
          placeholder={m.phone.searchPlaceholder}
          bind:value={query}
          oninput={runSearch}
          autocapitalize="off"
          autocomplete="off"
        />
      </div>
      <div class="chips">
        <button class="chip" class:on={favoritesOnly} onclick={() => ((favoritesOnly = !favoritesOnly), runSearch())}>
          <Icon name="star" size={12} /> {m.phone.favoritesChip}
        </button>
        {#if tags.length}
          <button class="chip" class:on={selectedTags.length > 0} onclick={() => (tagSheet = true)}>
            {m.phone.tagsChip(selectedTags.length)}
          </button>
        {/if}
        {#each selectedTags as t, i (t)}
          {#if i > 0}<span class="join">{tagMode === "all" ? m.phone.and : m.phone.or}</span>{/if}
          <button
            class="chip on"
            onclick={() => ((selectedTags = selectedTags.filter((x) => x !== t)), runSearch())}
            aria-label={m.phone.removeTag(t)}
          >
            {t} ✕
          </button>
        {/each}
      </div>
      {#if hits.length === 0 && (query || selectedTags.length || favoritesOnly)}
        <p class="note">{m.phone.nothingFound}</p>
      {/if}
      <ul class="list">
        {#each hits as h (h.sourceId + h.relPath)}
          <li>
            <button class="row" onclick={() => showDoc(h.sourceId, h.relPath, h.name, h.cloud === "cloudOnly", h.size)}>
              <span class="icon"><Icon name={h.kind === "pdf" ? "pdf" : "file"} size={22} /></span>
              <span class="main">
                <span class="name">{h.title || h.name}</span>
                <span class="sub">
                  {[formatDocDate(h.documentDate), folderOf(h.relPath) || sourceName(h.sourceId)].filter(Boolean).join(" · ")}
                </span>
                {#if h.snippet}
                  <span class="snippet">
                    {#each snippetParts(h.snippet) as p, i (i)}{#if p.hit}<mark>{p.text}</mark>{:else}{p.text}{/if}{/each}
                  </span>
                {/if}
              </span>
              {#if h.cloud === "cloudOnly"}<span class="cloud"><Icon name="cloud" /></span>{/if}
            </button>
          </li>
        {/each}
      </ul>
    {:else if tab === "scan"}
      <div class="scan">
        {#if overview && !overview.inbox}
          <p>{tx?.scanIntro ?? m.phone.scanIntro}</p>
          {#if !android}
            <button class="primary" disabled={busy} onclick={() => act(phone.chooseInbox)}>{m.phone.chooseInFiles}</button>
          {/if}
          {#if od?.signedIn}
            <button class="primary" disabled={busy} onclick={() => openChooser("onedrive", "inbox")}>{m.phone.chooseInOneDrive}</button>
          {/if}
          {#if gd?.signedIn}
            <button class="primary" disabled={busy} onclick={() => openChooser("googledrive", "inbox")}>
              {m.phone.chooseInGoogleDrive}
            </button>
          {/if}
          {#if settingsError}<p class="error">{settingsError}</p>{/if}
        {:else if overview}
          <div class="scan-options">
            <div class="segmented" role="radiogroup" aria-label={m.phone.color}>
              {#each [["color", m.phone.color], ["gray", m.phone.grayscale], ["bw", m.phone.blackWhite]] as [value, label] (value)}
                <button
                  role="radio"
                  aria-checked={scanSettings.mode === value}
                  class:on={scanSettings.mode === value}
                  onclick={() => ((scanSettings.mode = value as ScanSettings["mode"]), saveScanSettings())}
                >
                  {label}
                </button>
              {/each}
            </div>
            <div class="segmented" role="radiogroup" aria-label={m.phone.resolution}>
              {#each [150, 200, 300, 400] as dpi (dpi)}
                <button
                  role="radio"
                  aria-checked={scanSettings.dpi === dpi}
                  class:on={scanSettings.dpi === dpi}
                  onclick={() => ((scanSettings.dpi = dpi), saveScanSettings())}
                >
                  {m.phone.dpi(dpi)}
                </button>
              {/each}
            </div>
            <p class="note small">
              {scanSettings.mode === "bw"
                ? m.phone.bwNote
                : scanSettings.mode === "gray"
                  ? m.phone.grayNote
                  : m.phone.colorNote}
              {m.phone.dpiNote}
            </p>
          </div>
          <!-- Rupert is the scan button, and says what's happening. -->
          <button class="owl-scan" onclick={scan} disabled={scanning} aria-label={scanning ? m.phone.savingScan : m.phone.scanDocument}>
            <span class="bubble" aria-hidden="true">
              {#if scanState === "ready"}<Icon name="camera" size={16} />{/if}
              {scanSays[scanState]}
            </span>
            <RupertOwl mood={scanMood[scanState]} size={150} />
          </button>
          <p class="note">
            {m.phone.savedTo[0]} <strong>{overview.inboxService ? `${serviceName[overview.inboxService]} ▸ ` : ""}{overview.inbox}</strong> {m.phone.savedTo[1]}
          </p>
          {#if lastScan}
            <p class="ok">
              {m.phone.savedScan(lastScan)}
              {#if overview?.inboxPlace}<button class="plain" onclick={openInbox}>{m.phone.openInbox}</button>{/if}
            </p>
          {/if}
          {#if scanError}<p class="error">{scanError}</p>{/if}
        {/if}
      </div>
    {:else if overview}
      <section>
        <h2>{m.phone.libraryFolders}</h2>
        <ul class="list boxed">
          {#each library as s (s.id)}
            <li class="setting">
              <span class="main">
                <span class="name">{s.name}</span>
                <span class="sub">{providerName[s.provider]}{s.available ? "" : m.phone.notAvailableShort}</span>
              </span>
              {#if confirmRemove === s.id}
                <button class="danger" onclick={() => act(() => phone.removeFolder(s.id)).then(() => (confirmRemove = null))}>
                  {m.phone.remove}
                </button>
                <button class="plain" onclick={() => (confirmRemove = null)}>{m.phone.keep}</button>
              {:else}
                <button class="plain" onclick={() => (confirmRemove = s.id)}>{m.phone.removeEllipsis}</button>
              {/if}
            </li>
          {/each}
          {#if !android}
            <li class="setting">
              <button class="plain add" disabled={busy} onclick={() => act(phone.addFolder)}>
                <Icon name="plus" /> {m.phone.addFolder}
              </button>
            </li>
          {/if}
          {#if od?.signedIn}
            <li class="setting">
              <button class="plain add" disabled={busy} onclick={() => openChooser("onedrive", "library")}>
                <Icon name="plus" /> {m.phone.addOneDrive}
              </button>
            </li>
          {/if}
          {#if gd?.signedIn}
            <li class="setting">
              <button class="plain add" disabled={busy} onclick={() => openChooser("googledrive", "library")}>
                <Icon name="plus" /> {m.phone.addGoogleDrive}
              </button>
            </li>
          {/if}
        </ul>
        <p class="note small">{tx?.removingNote ?? m.phone.removingNote}</p>
      </section>
      <section>
        <h2>{m.phone.inboxForScans}</h2>
        <ul class="list boxed">
          <li class="setting">
            <span class="main">
              <span class="name">{overview.inbox ?? m.phone.notChosen}</span>
              {#if overview.inbox}<span class="sub">{overview.inboxService ? serviceName[overview.inboxService] : m.phone.files}</span>{/if}
            </span>
            {#if !android}
              <button class="plain" disabled={busy} onclick={() => act(phone.chooseInbox)}>{m.phone.inFiles}</button>
            {/if}
            {#if od?.signedIn}
              <button class="plain" disabled={busy} onclick={() => openChooser("onedrive", "inbox")}>{m.phone.inOneDrive}</button>
            {/if}
            {#if gd?.signedIn}
              <button class="plain" disabled={busy} onclick={() => openChooser("googledrive", "inbox")}>{m.phone.inGoogleDrive}</button>
            {/if}
          </li>
        </ul>
      </section>
      <section>
        <h2>OneDrive</h2>
        <ul class="list boxed">
          <li class="setting">
            {#if !od?.configured}
              <span class="main"><span class="sub wrap">{m.phone.notRegistered}</span></span>
            {:else if od.signedIn}
              <span class="main">
                <span class="name">{od.account ?? m.phone.signedIn}</span>
                {#if remoteProgress?.active}
                  <span class="sub">{m.phone.readingFrom(remoteProgress.current ?? "OneDrive")}</span>
                {:else if od.error}
                  <span class="sub error">{od.error}</span>
                {/if}
              </span>
              <button class="plain" disabled={busy} onclick={() => act(phone.onedriveSignOut)}>{m.phone.signOut}</button>
            {:else}
              <span class="main"><span class="name">{m.phone.notSignedIn}</span></span>
              <button class="plain" disabled={busy} onclick={() => act(phone.onedriveSignIn)}>{m.phone.signIn}</button>
            {/if}
          </li>
        </ul>
        <p class="note small left">{tx?.oneDriveNote ?? m.phone.oneDriveNote}</p>
      </section>
      {#if gd?.configured}
        <section>
          <h2>Google Drive <span class="preview">{m.phone.preview}</span></h2>
          <ul class="list boxed">
            <li class="setting">
              {#if gd.signedIn}
                <span class="main">
                  <span class="name">{gd.account ?? m.phone.signedIn}</span>
                  {#if remoteProgress?.active}
                    <span class="sub">{m.phone.readingFrom(remoteProgress.current ?? "Google Drive")}</span>
                  {:else if gd.error}
                    <span class="sub error">{gd.error}</span>
                  {/if}
                </span>
                <button class="plain" disabled={busy} onclick={() => act(phone.gdriveSignOut)}>{m.phone.signOut}</button>
              {:else}
                <span class="main"><span class="name">{m.phone.notSignedIn}</span></span>
                <button class="plain" disabled={busy} onclick={() => act(phone.gdriveSignIn)}>{m.phone.signIn}</button>
              {/if}
            </li>
          </ul>
          <p class="note small left">
            {#if tx}{tx.googleNote}{:else}
            {m.phone.googleNote[0]} <b>{m.phone.googleNote[1]}</b>{m.phone.googleNote[2]} <b>{m.phone.googleNote[3]}</b>{m.phone.googleNote[4]}
            {/if}
          </p>
        </section>
      {/if}
      <section>
        <h2>{m.phone.index}</h2>
        <p class="note small left">{m.phone.indexNote(overview.stats.documents, overview.stats.withText)}</p>
        <button class="plain add" onclick={() => phone.refresh()} disabled={busyIndexing}>{m.phone.checkAgain}</button>
      </section>
      <section>
        <h2>{m.phone.sharing}</h2>
        <ul class="list boxed">
          <li class="setting">
            <label class="toggle">
              <span class="main"><span class="name">{m.phone.sendNotes}</span></span>
              <input
                type="checkbox"
                role="switch"
                checked={sendNotes}
                onchange={(e) => {
                  sendNotes = e.currentTarget.checked;
                  phone.setSendNotes(sendNotes);
                }}
              />
            </label>
          </li>
        </ul>
        <p class="note small left">{m.phone.sendNotesNote}</p>
      </section>
      <section>
        <h2>{m.phone.language}</h2>
        <ul class="list boxed">
          <li class="setting">
            <select
              class="language"
              aria-label={m.phone.language}
              value={languageSetting ?? ""}
              onchange={(e) => chooseLanguage(e.currentTarget.value || null)}
            >
              <option value="">{m.settings.automatic(languageNames[systemLang])}</option>
              {#each LANGS as lang (lang)}<option value={lang}>{languageNames[lang]}</option>{/each}
            </select>
          </li>
        </ul>
        <p class="note small left">{tx?.languageNote ?? m.phone.languageNote}</p>
      </section>
      <section>
        <h2>{m.phone.about}</h2>
        <ul class="list boxed">
          <li class="setting">
            <span class="main"><span class="name">{m.phone.version}</span></span>
            <span class="value selectable">
              {#if version}{version.version}{#if version.build}&nbsp;({version.build}){/if}{#if version.development}&nbsp;· {m.phone.developmentBuild}{/if}{/if}
            </span>
          </li>
          <li>
            <button class="row" onclick={openPrivacyPolicy}>
              <span class="main"><span class="name">{m.phone.privacyPolicy}</span></span>
              <Icon name="chevron" />
            </button>
          </li>
        </ul>
        <p class="note small left">
          {m.phone.aboutNote[0]} <span class="nowrap">github.com/micki-b/Rupert</span>{m.phone.aboutNote[1]}
        </p>
      </section>
      {#if settingsError}<p class="error pad">{settingsError}</p>{/if}
    {/if}
  </main>
  {#if askStatus?.capable}
    <Ask bind:this={ask} status={askStatus} hidden={tab !== "ask"} onaction={askAction} />
  {/if}

  <nav>
    {#each [["library", m.phone.library], ["search", m.phone.search], ["scan", m.phone.scan], ...(askStatus?.capable ? [["ask", m.phone.ask]] : []), ["settings", m.phone.settings]] as [t, label] (t)}
      <button class:on={tab === t} onclick={() => switchTab(t as Tab)} data-tab={t}>
        <Icon name={t === "scan" ? "camera" : t === "ask" ? "chat" : (t as "library" | "search" | "settings")} size={24} />
        <span>{label}</span>
      </button>
    {/each}
  </nav>

  {#if tagSheet}
    <div class="backdrop" role="presentation" onclick={() => ((tagSheet = false), runSearch())}></div>
    <div class="sheet" role="dialog" aria-modal="true" aria-label={m.phone.tags}>
      <div class="grabber"></div>
      <div class="chooser-head">
        <h2 class="doc-title">{m.phone.tags}</h2>
      </div>
      <div class="tag-sheet-body">
        <TagFilter
          bind:selected={selectedTags}
          bind:mode={tagMode}
          refresh={indexTick}
          limit={15}
          variant="sheet"
          onchange={runSearch}
        />
      </div>
      <div class="sheet-actions">
        <span></span>
        <button class="primary" onclick={() => ((tagSheet = false), runSearch())}>
          {selectedTags.length ? m.phone.showDocuments : m.common.done}
        </button>
      </div>
    </div>
  {/if}

  {#if chooser}
    <div class="backdrop" role="presentation" onclick={() => (chooser = null)}></div>
    <div class="sheet chooser" role="dialog" aria-modal="true" aria-label={m.phone.chooseServiceFolder(serviceName[chooser.service])}>
      <div class="grabber"></div>
      <div class="chooser-head">
        {#if chooser.path.length}
          <button class="back" onclick={chooserBack} aria-label={m.phone.back}><Icon name="back" size={22} /></button>
        {/if}
        <h2 class="doc-title">{chooser.path.at(-1)?.name ?? serviceName[chooser.service]}</h2>
      </div>
      <p class="sub">
        {chooser.purpose === "library" ? m.phone.chooseForLibrary : m.phone.chooseForInbox}
      </p>
      {#if chooser.error}<p class="error">{chooser.error}</p>{/if}
      <ul class="list chooser-list">
        {#each chooser.folders as f (f.id)}
          <li>
            <button class="row" onclick={() => chooserInto(f)}>
              <span class="icon folder"><Icon name="folder" size={22} /></span>
              <span class="main"><span class="name">{f.name}</span></span>
              <Icon name="chevron" />
            </button>
          </li>
        {/each}
        {#if !chooser.loading && chooser.folders.length === 0 && !chooser.error}
          <li class="note">{m.phone.noFolders}</li>
        {/if}
      </ul>
      <div class="sheet-actions">
        <button class="plain" onclick={() => (chooser = null)}>{m.common.cancel}</button>
        <button class="primary" onclick={chooserUse} disabled={!chooser.path.length}>
          {chooser.path.length ? m.phone.use(chooser.path.at(-1)?.name ?? "") : m.phone.openAFolder}
        </button>
      </div>
    </div>
  {/if}

  {#if sheet}
    <div class="backdrop" role="presentation" onclick={() => (sheet = null)}></div>
    <div class="sheet" role="dialog" aria-modal="true" aria-label={m.phone.document}>
      <div class="grabber"></div>
      <h2 class="doc-title">{sheet.info?.title || sheet.name}</h2>
      {#if sheet.info?.title}<p class="sub">{sheet.name}</p>{/if}
      <dl>
        {#if sheet.info?.documentDate}<dt>{m.phone.date}</dt><dd>{formatDocDate(sheet.info.documentDate)}</dd>{/if}
        {#if sheet.info?.tags.length}
          <dt>{m.phone.tags}</dt>
          <dd class="tags">{#each sheet.info.tags as t (t)}<span class="chip on">{t}</span>{/each}</dd>
        {/if}
        {#if sheet.info?.notes}<dt>{m.phone.notes}</dt><dd class="notes">{sheet.info.notes}</dd>{/if}
        <dt>{m.phone.folder}</dt>
        <dd>{[sourceName(sheet.sourceId), folderOf(sheet.relPath)].filter(Boolean).join(" / ")}</dd>
        <dt>{m.phone.size}</dt>
        <dd>{formatSize(sheet.size)}{sheet.cloudOnly ? m.phone.inTheCloudSuffix : ""}</dd>
      </dl>
      {#if openError}<p class="error">{openError}</p>{/if}
      <div class="sheet-actions">
        <button class="plain" onclick={() => (sheet = null)}>{m.common.close}</button>
        <button class="plain share" onclick={(e) => shareDoc(e.currentTarget)} disabled={sharing || opening}>
          <Icon name="share" />
          {sharing ? (sheet.cloudOnly ? m.phone.downloading : m.phone.preparing) : m.phone.share}
        </button>
        <button class="primary" onclick={openDoc} disabled={opening || sharing}>
          {opening ? (sheet.cloudOnly ? m.phone.downloading : m.phone.opening) : m.phone.open}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .app {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    font-size: 16px;
    -webkit-user-select: none;
    user-select: none;
    -webkit-tap-highlight-color: transparent;
  }
  header {
    display: flex;
    align-items: center;
    gap: 6px;
    /* Left/right insets: the Dynamic Island in landscape, and the iPhone
       Duo's outer screen, whose status cluster sits at the right. */
    padding: calc(env(safe-area-inset-top) + 8px) calc(env(safe-area-inset-right) + 16px) 8px
      calc(env(safe-area-inset-left) + 16px);
    border-bottom: 1px solid var(--border);
    background: var(--panel);
    min-height: 52px;
  }
  header h1 {
    flex: 1;
    font-size: 20px;
    margin: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .back {
    border: 0;
    background: none;
    color: var(--accent);
    padding: 4px 4px 4px 0;
    display: flex;
  }
  .star {
    border: 0;
    background: none;
    font-size: 22px;
    line-height: 1;
    padding: 4px;
    color: var(--muted);
  }
  .star.on {
    color: #d9a520;
  }
  .indexing {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }
  main[hidden] {
    display: none;
  }
  main {
    flex: 1;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    padding: 0 env(safe-area-inset-right) 12px env(safe-area-inset-left);
  }
  nav {
    display: flex;
    border-top: 1px solid var(--border);
    background: var(--panel);
    /* --nav-inset: Android's navigation bar, which its web view doesn't report (set by the plugin). */
    padding: 0 env(safe-area-inset-right) max(env(safe-area-inset-bottom), var(--nav-inset, 0px)) env(safe-area-inset-left);
  }
  nav button {
    flex: 1;
    border: 0;
    background: none;
    color: var(--muted);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    padding: 6px 0 4px;
    font-size: 11px;
  }
  nav button.on {
    color: var(--accent);
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border: 0;
    border-bottom: 1px solid var(--border);
    background: none;
    text-align: left;
    color: var(--text);
  }
  .row:active {
    background: var(--hover);
  }
  .row:disabled {
    opacity: 0.5;
  }
  .icon {
    color: var(--muted);
    display: flex;
  }
  .icon.folder {
    color: var(--folder);
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    font-size: 13px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
  }
  .group {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
    margin: 16px 16px 6px;
  }
  .fav {
    color: var(--accent);
    vertical-align: -1px;
  }
  .cloud {
    color: var(--cloud);
    display: flex;
  }
  .snippet {
    font-size: 13px;
    color: var(--muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  mark {
    background: var(--accent-soft);
    color: var(--text);
    border-radius: 2px;
  }
  h2 .preview {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 6px;
    background: var(--accent-soft, rgba(0, 0, 0, 0.06));
    color: var(--accent);
    font-size: 11px;
    font-weight: 600;
    text-transform: none;
    letter-spacing: 0;
  }
  .value {
    color: var(--muted);
    font-size: 15px;
  }
  .nowrap {
    white-space: nowrap;
  }
  .selectable {
    user-select: text;
    -webkit-user-select: text;
  }
  .note {
    color: var(--muted);
    font-size: 14px;
    text-align: center;
    padding: 12px 24px;
    margin: 0;
  }
  .note.small {
    font-size: 13px;
  }
  .note.left {
    text-align: left;
    padding: 0 16px 8px;
  }
  .pad {
    padding: 12px 16px;
  }
  .empty,
  .scan {
    padding: 32px 24px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
    line-height: 1.45;
  }
  .empty p,
  .scan p {
    margin: 0;
  }
  .scan-options {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    max-width: 420px;
  }
  .scan-options .note {
    padding: 0 4px;
  }
  .segmented {
    display: flex;
    background: var(--panel);
    border-radius: 9px;
    padding: 2px;
  }
  .segmented button {
    flex: 1;
    border: 0;
    background: none;
    padding: 7px 4px;
    border-radius: 7px;
    font-size: 14px;
    color: var(--text);
  }
  .segmented button.on {
    background: var(--bg);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.15);
    font-weight: 600;
  }
  .searchbar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 12px 16px 6px;
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--panel);
    color: var(--muted);
  }
  .searchbar input {
    flex: 1;
    border: 0;
    background: none;
    font: inherit;
    color: var(--text);
    outline: none;
    -webkit-user-select: text;
    user-select: text;
  }
  .chips {
    display: flex;
    gap: 6px;
    overflow-x: auto;
    padding: 4px 16px 10px;
    border-bottom: 1px solid var(--border);
    scrollbar-width: none;
  }
  .chip {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 4px 10px;
    font-size: 13px;
    background: none;
    color: var(--text);
  }
  .chip.on {
    background: var(--accent-soft);
    border-color: transparent;
    color: var(--accent-strong);
  }
  button.primary {
    background: var(--accent);
    color: #fff;
    border: 0;
    border-radius: 12px;
    padding: 12px 20px;
    font-size: 16px;
    font-weight: 600;
  }
  button:disabled {
    opacity: 0.55;
  }
  /* MARK: Rupert, the scan button */
  button.owl-scan {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    margin: 6px auto 0;
    padding: 4px 24px 8px;
    border: 0;
    background: none;
    -webkit-tap-highlight-color: transparent;
    transition: transform 0.15s;
  }
  button.owl-scan:active:not(:disabled) {
    transform: scale(0.95);
  }
  button.owl-scan:disabled {
    opacity: 1; /* keep him visible while the scan is saved */
  }
  .bubble {
    position: relative;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 7px 14px;
    border: 1.5px solid var(--accent);
    border-radius: 16px;
    background: var(--bg);
    color: var(--text);
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    box-shadow: 0 2px 6px rgb(0 0 0 / 0.1);
  }
  .bubble :global(svg) {
    color: var(--accent);
  }
  /* The tail, pointing down at Rupert. */
  .bubble::after {
    content: "";
    position: absolute;
    left: calc(50% - 6px);
    bottom: -7px;
    width: 11px;
    height: 11px;
    background: var(--bg);
    border-right: 1.5px solid var(--accent);
    border-bottom: 1.5px solid var(--accent);
    transform: rotate(45deg);
  }
  button.plain {
    border: 0;
    background: none;
    color: var(--accent);
    font-size: 15px;
    padding: 6px 4px;
  }
  button.plain.add {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 16px;
  }
  button.danger {
    border: 0;
    border-radius: 8px;
    background: var(--danger);
    color: #fff;
    padding: 6px 10px;
    font-size: 14px;
  }
  section h2,
  .sheet dt {
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--muted);
  }
  section h2 {
    margin: 20px 16px 6px;
  }
  .boxed {
    border-top: 1px solid var(--border);
  }
  .setting select.language {
    flex: 1;
    min-width: 0;
    font: inherit;
    color: var(--text);
    background: none;
    border: 0;
    padding: 4px 0;
  }
  .setting {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    min-height: 52px;
  }
  .error {
    color: var(--danger);
    font-size: 14px;
  }
  .ok {
    color: var(--accent-strong);
  }
  /* Folder views: list, large icons, gallery. */
  .viewbar {
    display: flex;
    justify-content: flex-end;
    gap: 4px;
    padding: 8px 16px 0;
  }
  .viewbar button {
    display: flex;
    padding: 6px 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
  }
  .viewbar button.on {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  /* Documents shared from other apps: sent, or waiting. */
  .shared {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 8px 16px 0;
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent-strong);
    font-size: 14px;
  }
  .shared.problem {
    background: var(--danger-soft);
    color: var(--danger);
  }
  .shared p {
    flex: 1;
    margin: 0;
  }
  .shared .close {
    font-size: 20px;
    line-height: 1;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.3);
  }
  .sheet {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    max-height: 85%;
    overflow-y: auto;
    background: var(--bg);
    border-radius: 16px 16px 0 0;
    padding: 8px calc(env(safe-area-inset-right) + 20px) calc(max(env(safe-area-inset-bottom), var(--nav-inset, 0px)) + 16px)
      calc(env(safe-area-inset-left) + 20px);
    box-shadow: 0 -8px 30px rgb(0 0 0 / 0.2);
  }
  .tag-sheet-body {
    margin: 8px -8px 12px;
    max-height: 60vh;
    overflow-y: auto;
  }
  .join {
    flex: none;
    align-self: center;
    color: var(--muted);
    font-size: 12px;
  }
  .chooser-head {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .chooser-list {
    margin: 8px -20px 12px;
    border-top: 1px solid var(--border);
    max-height: 50vh;
    overflow-y: auto;
  }
  .sub.wrap {
    white-space: normal;
  }
  .grabber {
    width: 36px;
    height: 5px;
    border-radius: 3px;
    background: var(--border);
    margin: 0 auto 12px;
  }
  .doc-title {
    font-size: 20px;
    margin: 0 0 4px;
    -webkit-user-select: text;
    user-select: text;
  }
  dl {
    display: grid;
    grid-template-columns: 70px 1fr;
    gap: 10px 12px;
    margin: 16px 0;
    align-items: baseline;
  }
  dt {
    margin: 0;
  }
  dd {
    margin: 0;
    font-size: 15px;
    overflow-wrap: anywhere;
  }
  dd.tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  dd.notes {
    white-space: pre-wrap;
    -webkit-user-select: text;
    user-select: text;
  }
  .sheet-actions {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .sheet-actions .share {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }
  .toggle input {
    accent-color: var(--accent);
    width: 20px;
    height: 20px;
    flex: none;
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
</style>
