<script lang="ts">
  // Rupert for iPhone: browse and search the library, open documents, and
  // scan paper into the inbox folder. Read-only for the library itself.
  import { onMount } from "svelte";
  import Icon from "$lib/Icon.svelte";
  import RupertOwl, { type OwlMood } from "$lib/RupertOwl.svelte";
  import TagFilter from "$lib/TagFilter.svelte";
  import { snippetParts, type TagMode } from "$lib/api";
  import { formatSize } from "$lib/format";
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
    type AppVersion,
    type Overview,
    type Progress,
    type ScanSettings,
    type TagCount,
  } from "$lib/phone/api";

  type Tab = "library" | "search" | "scan" | "settings";
  type Crumb = { sourceId: string; relPath: string; name: string };
  /** What the document sheet shows: indexed details, or just the file. */
  type Sheet = { sourceId: string; relPath: string; name: string; cloudOnly: boolean; size: number; info: DocInfo | null };

  let tab = $state<Tab>("library");
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
  const scanSays: Record<typeof scanState, string> = {
    ready: "Scan a document?",
    saving: "Filing it…",
    saved: "Got it!",
    error: "That didn't work",
  };

  let confirmRemove = $state<string | null>(null);
  let od = $state<OneDriveStatus | null>(null);
  let remoteProgress = $state<Progress | null>(null);
  /** The OneDrive folder chooser: what it's for, and where it is. */
  type Chooser = {
    purpose: "library" | "inbox";
    path: OneDriveFolder[];
    folders: OneDriveFolder[];
    loading: boolean;
    error: string | null;
  };
  let chooser = $state<Chooser | null>(null);
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
    tab === "library" ? (here?.name ?? "Library") : tab === "search" ? "Search" : tab === "scan" ? "Scan" : "Settings",
  );

  // MARK: Loading

  async function loadOverview() {
    overview = await phone.overview();
    od = await phone.onedriveStatus().catch(() => null);
    favFolders = await phone.favoriteFolders().catch(() => []);
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

  onMount(() => {
    loadOverview();
    phone.appVersion().then((v) => (version = v));
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

  // MARK: Navigation

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

  function switchTab(t: Tab) {
    if (t === tab && t === "library") stack = [];
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

  // MARK: OneDrive

  async function openChooser(purpose: "library" | "inbox") {
    chooser = { purpose, path: [], folders: [], loading: true, error: null };
    await loadChooser();
  }

  async function loadChooser() {
    if (!chooser) return;
    chooser.loading = true;
    chooser.error = null;
    try {
      chooser.folders = await phone.onedriveFolders(chooser.path.at(-1)?.id ?? null);
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
    await act(() =>
      c.purpose === "library" ? phone.addOnedriveFolder(here.id, here.name) : phone.setOnedriveInbox(here.id, here.name),
    );
  }

  const providerName: Record<string, string> = {
    icloud: "iCloud Drive",
    onedrive: "OneDrive",
    googledrive: "Google Drive",
    dropbox: "Dropbox",
    local: "Folder",
  };

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
      <button class="back" onclick={back} aria-label="Back"><Icon name="back" size={22} /></button>
    {/if}
    <h1>{title}</h1>
    {#if tab === "library" && here}
      <button
        class="star"
        class:on={hereFav}
        onclick={toggleFolderFav}
        disabled={favBusy}
        aria-pressed={hereFav}
        aria-label={hereFav ? "Remove this folder from Favorites" : "Add this folder to Favorites"}
        >{hereFav ? "★" : "☆"}</button
      >
    {/if}
    {#if busyIndexing && shownProgress}
      <span class="indexing" title={shownProgress.current ?? ""}>
        <span class="spinner"></span>{shownProgress.total ? `${shownProgress.done} / ${shownProgress.total}` : ""}
      </span>
    {/if}
  </header>

  <main>
    {#if tab === "library"}
      {#if !here}
        {#if overview && overview.sources.length === 0}
          <div class="empty">
            <p>
              Pick the folders that hold your documents, the same ones Rupert uses on your computer: in iCloud Drive,
              OneDrive or on this iPhone.
            </p>
            <button class="primary" disabled={busy} onclick={() => act(phone.addFolder)}>Add a folder…</button>
            {#if od?.signedIn}
              <button class="primary" disabled={busy} onclick={() => openChooser("library")}>Add a OneDrive folder…</button>
            {:else if od?.configured}
              <p class="note small">OneDrive folders need a sign-in first: Settings ▸ OneDrive.</p>
            {/if}
            {#if settingsError}<p class="error">{settingsError}</p>{/if}
          </div>
        {:else if overview}
          {#if favFolders.length || overview.stats.favorites}
            <h2 class="group">Favorites</h2>
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
                      <span class="name">Favorite documents</span>
                      <span class="sub">{overview.stats.favorites}</span>
                    </span>
                    <Icon name="chevron" />
                  </button>
                </li>
              {/if}
            </ul>
            <h2 class="group">Library</h2>
          {/if}
          <ul class="list">
            {#each overview.sources as s (s.id)}
              <li>
                <button class="row" onclick={() => openSource(s.id, s.name)} disabled={!s.available}>
                  <span class="icon folder"><Icon name="library" size={22} /></span>
                  <span class="main">
                    <span class="name">{s.name}</span>
                    <span class="sub">{s.available ? providerName[s.provider] : "Not available on this iPhone"}</span>
                  </span>
                  <Icon name="chevron" />
                </button>
              </li>
            {/each}
          </ul>
          <p class="note">
            {overview.stats.documents.toLocaleString()} documents, {overview.stats.withText.toLocaleString()} searchable.
          </p>
        {/if}
      {:else}
        {#if listError}<p class="error pad">{listError}</p>{/if}
        {#if favError}<p class="error pad">{favError}</p>{/if}
        {#if !listing && entries.length === 0 && !listError}
          <p class="note">This folder has no documents.</p>
        {/if}
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
                {#if e.cloud === "cloudOnly"}<span class="cloud" title="In the cloud"><Icon name="cloud" /></span>{/if}
                {#if e.isDir}<Icon name="chevron" />{/if}
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    {:else if tab === "search"}
      <div class="searchbar">
        <Icon name="search" />
        <input
          type="search"
          placeholder="Words, titles, tags"
          bind:value={query}
          oninput={runSearch}
          autocapitalize="off"
          autocomplete="off"
        />
      </div>
      <div class="chips">
        <button class="chip" class:on={favoritesOnly} onclick={() => ((favoritesOnly = !favoritesOnly), runSearch())}>
          <Icon name="star" size={12} /> Favorites
        </button>
        {#if tags.length}
          <button class="chip" class:on={selectedTags.length > 0} onclick={() => (tagSheet = true)}>
            # Tags{selectedTags.length ? ` · ${selectedTags.length}` : ""}
          </button>
        {/if}
        {#each selectedTags as t, i (t)}
          {#if i > 0}<span class="join">{tagMode === "all" ? "and" : "or"}</span>{/if}
          <button
            class="chip on"
            onclick={() => ((selectedTags = selectedTags.filter((x) => x !== t)), runSearch())}
            aria-label="Remove tag {t}"
          >
            {t} ✕
          </button>
        {/each}
      </div>
      {#if hits.length === 0 && (query || selectedTags.length || favoritesOnly)}
        <p class="note">Nothing found.</p>
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
          <p>
            Scans are saved as PDFs to your Rupert inbox folder. Rupert on your computer then reads them and suggests
            names and folders. Choose the same inbox folder here, in iCloud Drive or OneDrive.
          </p>
          <button class="primary" disabled={busy} onclick={() => act(phone.chooseInbox)}>Choose in Files…</button>
          {#if od?.signedIn}
            <button class="primary" disabled={busy} onclick={() => openChooser("inbox")}>Choose in OneDrive…</button>
          {/if}
          {#if settingsError}<p class="error">{settingsError}</p>{/if}
        {:else if overview}
          <div class="scan-options">
            <div class="segmented" role="radiogroup" aria-label="Color">
              {#each [["color", "Color"], ["gray", "Grayscale"], ["bw", "Black & white"]] as [value, label] (value)}
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
            <div class="segmented" role="radiogroup" aria-label="Resolution">
              {#each [150, 200, 300, 400] as dpi (dpi)}
                <button
                  role="radio"
                  aria-checked={scanSettings.dpi === dpi}
                  class:on={scanSettings.dpi === dpi}
                  onclick={() => ((scanSettings.dpi = dpi), saveScanSettings())}
                >
                  {dpi} dpi
                </button>
              {/each}
            </div>
            <p class="note small">
              {scanSettings.mode === "bw"
                ? "Black & white: sharp, small files; shadows are evened out. Best for text."
                : scanSettings.mode === "gray"
                  ? "Grayscale suits most paperwork."
                  : "Color for receipts with stamps, photos and forms in color."}
              300 dpi gives the best text recognition.
            </p>
          </div>
          <!-- Rupert is the scan button, and says what's happening. -->
          <button class="owl-scan" onclick={scan} disabled={scanning} aria-label={scanning ? "Saving the scan" : "Scan a document"}>
            <span class="bubble" aria-hidden="true">
              {#if scanState === "ready"}<Icon name="camera" size={16} />{/if}
              {scanSays[scanState]}
            </span>
            <RupertOwl mood={scanMood[scanState]} size={150} />
          </button>
          <p class="note">
            Pages are saved to <strong>{overview.inboxOnOnedrive ? "OneDrive ▸ " : ""}{overview.inbox}</strong> as one PDF.
          </p>
          {#if lastScan}<p class="ok">Saved “{lastScan}”.</p>{/if}
          {#if scanError}<p class="error">{scanError}</p>{/if}
        {/if}
      </div>
    {:else if overview}
      <section>
        <h2>Library folders</h2>
        <ul class="list boxed">
          {#each overview.sources as s (s.id)}
            <li class="setting">
              <span class="main">
                <span class="name">{s.name}</span>
                <span class="sub">{providerName[s.provider]}{s.available ? "" : " · not available"}</span>
              </span>
              {#if confirmRemove === s.id}
                <button class="danger" onclick={() => act(() => phone.removeFolder(s.id)).then(() => (confirmRemove = null))}>
                  Remove
                </button>
                <button class="plain" onclick={() => (confirmRemove = null)}>Keep</button>
              {:else}
                <button class="plain" onclick={() => (confirmRemove = s.id)}>Remove…</button>
              {/if}
            </li>
          {/each}
          <li class="setting">
            <button class="plain add" disabled={busy} onclick={() => act(phone.addFolder)}>
              <Icon name="plus" /> Add a folder…
            </button>
          </li>
          {#if od?.signedIn}
            <li class="setting">
              <button class="plain add" disabled={busy} onclick={() => openChooser("library")}>
                <Icon name="plus" /> Add a OneDrive folder…
              </button>
            </li>
          {/if}
        </ul>
        <p class="note small">Removing a folder only removes it from Rupert on this iPhone. Nothing is deleted.</p>
      </section>
      <section>
        <h2>Inbox for scans</h2>
        <ul class="list boxed">
          <li class="setting">
            <span class="main">
              <span class="name">{overview.inbox ?? "Not chosen"}</span>
              {#if overview.inbox}<span class="sub">{overview.inboxOnOnedrive ? "OneDrive" : "Files"}</span>{/if}
            </span>
            <button class="plain" disabled={busy} onclick={() => act(phone.chooseInbox)}>In Files…</button>
            {#if od?.signedIn}
              <button class="plain" disabled={busy} onclick={() => openChooser("inbox")}>In OneDrive…</button>
            {/if}
          </li>
        </ul>
      </section>
      <section>
        <h2>OneDrive</h2>
        <ul class="list boxed">
          <li class="setting">
            {#if !od?.configured}
              <span class="main"><span class="sub wrap">This copy of Rupert isn't registered with Microsoft yet.</span></span>
            {:else if od.signedIn}
              <span class="main">
                <span class="name">{od.account ?? "Signed in"}</span>
                {#if remoteProgress?.active}
                  <span class="sub">Reading {remoteProgress.current ?? "OneDrive"}…</span>
                {:else if od.error}
                  <span class="sub error">{od.error}</span>
                {/if}
              </span>
              <button class="plain" disabled={busy} onclick={() => act(phone.onedriveSignOut)}>Sign out</button>
            {:else}
              <span class="main"><span class="name">Not signed in</span></span>
              <button class="plain" disabled={busy} onclick={() => act(phone.onedriveSignIn)}>Sign in…</button>
            {/if}
          </li>
        </ul>
        <p class="note small left">
          OneDrive folders can't be opened through Files, so Rupert reads them directly from OneDrive. It reads only
          its own small catalog files, downloads a document when you open it, and adds scans to the inbox.
        </p>
      </section>
      <section>
        <h2>Index</h2>
        <p class="note small left">
          {overview.stats.documents.toLocaleString()} documents; {overview.stats.withText.toLocaleString()} can be
          searched by their text. Text comes from Rupert on your computer, through the hidden catalog in each folder.
          Documents stay in the cloud until you open them.
        </p>
        <button class="plain add" onclick={() => phone.refresh()} disabled={busyIndexing}>Check folders again</button>
      </section>
      <section>
        <h2>About</h2>
        <ul class="list boxed">
          <li class="setting">
            <span class="main"><span class="name">Version</span></span>
            <span class="value selectable">
              {#if version}{version.version}{#if version.build}&nbsp;({version.build}){/if}{#if version.development}&nbsp;· development build{/if}{/if}
            </span>
          </li>
        </ul>
        <p class="note small left">
          Rupert is free and open source (MIT): <span class="nowrap">github.com/micki-b/Rupert</span>. When reporting a problem, please mention the
          version above.
        </p>
      </section>
      {#if settingsError}<p class="error pad">{settingsError}</p>{/if}
    {/if}
  </main>

  <nav>
    {#each [["library", "Library"], ["search", "Search"], ["scan", "Scan"], ["settings", "Settings"]] as [t, label] (t)}
      <button class:on={tab === t} onclick={() => switchTab(t as Tab)} data-tab={t}>
        <Icon name={t === "scan" ? "camera" : (t as "library" | "search" | "settings")} size={24} />
        <span>{label}</span>
      </button>
    {/each}
  </nav>

  {#if tagSheet}
    <div class="backdrop" role="presentation" onclick={() => ((tagSheet = false), runSearch())}></div>
    <div class="sheet" role="dialog" aria-modal="true" aria-label="Tags">
      <div class="grabber"></div>
      <div class="chooser-head">
        <h2 class="doc-title">Tags</h2>
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
          {selectedTags.length ? "Show documents" : "Done"}
        </button>
      </div>
    </div>
  {/if}

  {#if chooser}
    <div class="backdrop" role="presentation" onclick={() => (chooser = null)}></div>
    <div class="sheet chooser" role="dialog" aria-modal="true" aria-label="Choose a OneDrive folder">
      <div class="grabber"></div>
      <div class="chooser-head">
        {#if chooser.path.length}
          <button class="back" onclick={chooserBack} aria-label="Back"><Icon name="back" size={22} /></button>
        {/if}
        <h2 class="doc-title">{chooser.path.at(-1)?.name ?? "OneDrive"}</h2>
      </div>
      <p class="sub">
        {chooser.purpose === "library" ? "Choose a folder for the library." : "Choose the inbox folder for scans."}
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
          <li class="note">No folders in here.</li>
        {/if}
      </ul>
      <div class="sheet-actions">
        <button class="plain" onclick={() => (chooser = null)}>Cancel</button>
        <button class="primary" onclick={chooserUse} disabled={!chooser.path.length}>
          {chooser.path.length ? `Use “${chooser.path.at(-1)?.name}”` : "Open a folder"}
        </button>
      </div>
    </div>
  {/if}

  {#if sheet}
    <div class="backdrop" role="presentation" onclick={() => (sheet = null)}></div>
    <div class="sheet" role="dialog" aria-modal="true" aria-label="Document">
      <div class="grabber"></div>
      <h2 class="doc-title">{sheet.info?.title || sheet.name}</h2>
      {#if sheet.info?.title}<p class="sub">{sheet.name}</p>{/if}
      <dl>
        {#if sheet.info?.documentDate}<dt>Date</dt><dd>{formatDocDate(sheet.info.documentDate)}</dd>{/if}
        {#if sheet.info?.tags.length}
          <dt>Tags</dt>
          <dd class="tags">{#each sheet.info.tags as t (t)}<span class="chip on">{t}</span>{/each}</dd>
        {/if}
        {#if sheet.info?.notes}<dt>Notes</dt><dd class="notes">{sheet.info.notes}</dd>{/if}
        <dt>Folder</dt>
        <dd>{[sourceName(sheet.sourceId), folderOf(sheet.relPath)].filter(Boolean).join(" / ")}</dd>
        <dt>Size</dt>
        <dd>{formatSize(sheet.size)}{sheet.cloudOnly ? " · in the cloud" : ""}</dd>
      </dl>
      {#if openError}<p class="error">{openError}</p>{/if}
      <div class="sheet-actions">
        <button class="plain" onclick={() => (sheet = null)}>Close</button>
        <button class="primary" onclick={openDoc} disabled={opening}>
          {opening ? (sheet.cloudOnly ? "Downloading…" : "Opening…") : "Open"}
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
    padding: 0 env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
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
    padding: 8px calc(env(safe-area-inset-right) + 20px) calc(env(safe-area-inset-bottom) + 16px)
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
