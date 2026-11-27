<script lang="ts">
  // The Settings window (#80): tabs in a sidebar with a search field, the
  // selected tab on the right. Changes apply as they're made; the main
  // window hears about them (`settings-changed`, `sources-changed`).
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Icon from "#lib/Icon.svelte";
  import General from "#lib/settings/General.svelte";
  import Library from "#lib/settings/Library.svelte";
  import Inbox from "#lib/settings/Inbox.svelte";
  import TextRecognition from "#lib/settings/TextRecognition.svelte";
  import LocalAi from "#lib/settings/LocalAi.svelte";
  import Assistant from "#lib/settings/Assistant.svelte";
  import Reminders from "#lib/settings/Reminders.svelte";
  import { api, onSettingsChanged, onSettingsShow, onSourcesChanged, type Settings, type Source } from "#lib/api.ts";
  import { updates } from "#lib/updates.svelte.ts";
  import { loadLanguage, m } from "#lib/i18n/index.ts";

  type TabId = "general" | "library" | "inbox" | "reminders" | "text" | "local-ai" | "assistant";
  type IconName = "gear" | "library" | "inbox" | "calendar" | "text-scan" | "chip" | "chat";
  const TAB_KEY = "rupert.settingsTab";

  let settings = $state<Settings | null>(null);
  let sources = $state<Source[]>([]);
  let appStore = $state(false);
  let os = $state<string | undefined>(undefined);
  let error = $state<string | null>(null);
  let query = $state("");
  let tab = $state<TabId>("general");

  // MARK: Tabs and search

  /** Each tab with the words it can be found by (its labels and notes). */
  const tabs = $derived.by(() => {
    const s = m.settings;
    const all: { id: TabId; label: string; icon: IconName; words: string[] }[] = [
      { id: "general", label: s.general, icon: "gear", words: [s.language, s.languageNote, ...(appStore ? [] : [s.updates, s.checkAutomatically, s.checkNow])] },
      { id: "library", label: m.main.library, icon: "library", words: [s.libraryNote, s.sending, s.sendNotes, s.sendNotesNote, m.main.showAllTypes, m.main.downloadToIndex, m.main.indexFolder, m.main.rename, m.main.removeFromRupert, m.health.keptTitle, m.health.duplicates, ...sources.map((x) => x.name)] },
      { id: "inbox", label: s.inbox, icon: "inbox", words: [s.inboxIntro, s.inboxFolder, s.suggestions, s.suggest, s.fileNames, s.pattern, s.printEntry, s.printEntryMac, s.printEntryWindows, "PDF"] },
      { id: "reminders", label: s.reminders, icon: "calendar", words: [m.reminders.title, m.reminders.enable, m.reminders.notifications, m.reminders.summary, m.reminders.each, m.reminders.dockBadge, m.reminders.bills, m.reminders.others, m.details.dueDate, "Dock"] },
      { id: "text", label: s.textRecognition, icon: "text-scan", words: [s.recognize, s.ocrNote, "OCR", "Tesseract"] },
      { id: "local-ai", label: s.localAi, icon: "chip", words: [s.server, s.address, s.model, "Apple Intelligence", "LM Studio", "Ollama"] },
      { id: "assistant", label: s.assistant, icon: "chat", words: [s.model, s.providerAnthropic, s.providerOpenai, s.providerChatgpt, s.apiKey, "Claude", "ChatGPT", m.main.askRupert] },
    ];
    return all;
  });

  /** Tabs matching every word typed, by name first. */
  const shown = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return tabs;
    return tabs.filter((t) => {
      const text = [t.label, ...t.words].join(" ").toLowerCase();
      return words.every((w) => text.includes(w));
    });
  });
  const current = $derived(tabs.find((t) => t.id === tab) ?? tabs[0]);

  // Searching moves to the first tab found if the current one isn't.
  $effect(() => {
    if (shown.length && !shown.some((t) => t.id === tab)) tab = shown[0].id;
  });

  function select(id: TabId) {
    tab = id;
    try {
      localStorage.setItem(TAB_KEY, id);
    } catch {
      // Only a convenience.
    }
  }

  /** A tab asked for by the main window ("updates": General, checking now). */
  function showTab(t: string) {
    query = "";
    if (t === "updates") {
      select("general");
      updates.check(true);
    } else if (tabs.some((x) => x.id === t)) {
      select(t as TabId);
    }
  }

  /** Arrow keys move through the tabs. */
  function tabKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = shown.findIndex((t) => t.id === tab);
    const next = shown[Math.min(shown.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)))];
    if (next) {
      select(next.id);
      document.getElementById(`tab-${next.id}`)?.focus();
    }
  }

  async function loadSources() {
    sources = await api.listSources().catch(() => sources);
  }

  onMount(() => {
    api.appInfo().then((i) => {
      appStore = i.edition === "app-store";
      os = i.os;
    });
    api.getSettings().then((s) => (settings = s));
    loadSources();

    let saved: string | null = null;
    try {
      saved = localStorage.getItem(TAB_KEY);
    } catch {
      // Private storage unavailable: start on General.
    }
    const start = (window as unknown as { __rupertSettingsTab?: string }).__rupertSettingsTab;
    showTab(start || saved || "general");

    const unlisten = [
      onSettingsShow(showTab),
      onSourcesChanged(loadSources),
      // Saved elsewhere (this window's own saves come back unchanged).
      onSettingsChanged((s) => {
        const json = JSON.stringify(s);
        if (json === lastSaved) return;
        clearTimeout(saveTimer);
        lastSaved = json;
        settings = s;
        loadLanguage();
      }),
    ];
    // Save what's still waiting before the window closes.
    const closing = getCurrentWindow().onCloseRequested(async () => {
      await flush();
    });
    return () => {
      for (const u of [...unlisten, closing]) u.then((f) => f());
    };
  });

  // MARK: Saving
  // Changes apply as they're made (as in other Mac apps): a moment after
  // the last one, so typing in a field doesn't save every keystroke.

  let lastSaved = ""; // the settings as last saved, as JSON
  let saveTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    if (!settings) return;
    const snap = $state.snapshot(settings);
    const json = JSON.stringify(snap);
    if (!lastSaved) {
      lastSaved = json; // as loaded
      return;
    }
    if (json === lastSaved) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => persist(snap, json), 400);
  });

  async function persist(snap: Settings, json: string) {
    saveTimer = undefined;
    try {
      await api.setSettings(snap);
      lastSaved = json;
      await loadLanguage();
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function flush() {
    clearTimeout(saveTimer);
    if (!settings) return;
    const snap = $state.snapshot(settings);
    const json = JSON.stringify(snap);
    if (lastSaved && json !== lastSaved) await persist(snap, json);
  }

  const onerror = (message: string | null) => (error = message);
</script>

<svelte:head><title>{m.settings.title}</title></svelte:head>

<div class="window">
  <!-- MARK: Sidebar -->
  <nav class="side" aria-label={m.settings.title}>
    <div class="search">
      <Icon name="search" size={14} />
      <input
        type="search"
        placeholder={m.settings.search}
        aria-label={m.settings.search}
        bind:value={query}
        onkeydown={(e) => {
          if (e.key === "Escape") query = "";
          if (e.key === "ArrowDown") {
            e.preventDefault();
            document.getElementById(`tab-${tab}`)?.focus();
          }
        }}
      />
    </div>
    <div class="tabs" role="tablist" aria-orientation="vertical" tabindex="-1" onkeydown={tabKey}>
      {#each shown as t (t.id)}
        <button
          id="tab-{t.id}"
          class="tab"
          class:on={t.id === tab}
          role="tab"
          aria-selected={t.id === tab}
          aria-controls="pane"
          tabindex={t.id === tab ? 0 : -1}
          onclick={() => select(t.id)}
        >
          <Icon name={t.icon} size={18} />
          <span>{t.label}</span>
        </button>
      {:else}
        <p class="nomatch">{m.settings.noMatch(query)}</p>
      {/each}
    </div>
  </nav>

  <!-- MARK: The tab -->
  <div id="pane" class="pane" role="tabpanel" aria-labelledby="tab-{tab}">
    <h1>{current.label}</h1>
    {#if settings}
      {#if tab === "general"}
        <General bind:settings {appStore} />
      {:else if tab === "library"}
        <Library bind:settings {sources} {os} onchanged={loadSources} {onerror} />
      {:else if tab === "inbox"}
        <Inbox bind:settings {sources} {os} {appStore} {onerror} />
      {:else if tab === "reminders"}
        <Reminders bind:settings {os} />
      {:else if tab === "text"}
        <TextRecognition bind:settings />
      {:else if tab === "local-ai"}
        <LocalAi bind:settings />
      {:else if tab === "assistant"}
        <Assistant bind:settings {appStore} {onerror} />
      {/if}
      {#if error}<p class="error">{error}</p>{/if}
      <p class="applied">{m.settings.applyNow}</p>
    {:else}
      <p class="note">{m.common.loading}</p>
    {/if}
  </div>
</div>

<style>
  :global(html, body) {
    height: 100%;
    margin: 0;
    background: var(--bg);
  }
  .window {
    height: 100vh;
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    color: var(--text);
  }

  /* MARK: Sidebar */
  .side {
    background: var(--panel);
    border-right: 1px solid var(--border);
    padding: 12px 10px;
    overflow: auto;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    margin-bottom: 12px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--bg);
    color: var(--muted);
  }
  .search input {
    flex: 1;
    min-width: 0;
    border: 0;
    outline: none;
    background: none;
    color: var(--text);
    padding: 5px 0;
    font: inherit;
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .tabs {
    display: grid;
    gap: 2px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    padding: 6px 8px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--text);
    text-align: left;
    font: inherit;
  }
  .tab :global(svg) {
    flex: none;
    color: var(--muted);
  }
  .tab:hover {
    background: var(--hover);
  }
  .tab.on {
    background: var(--accent);
    color: #fff;
  }
  @media (prefers-color-scheme: dark) {
    .tab.on {
      color: #1e1c1a;
    }
  }
  .tab.on :global(svg) {
    color: inherit;
  }
  .tab:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  .nomatch {
    color: var(--muted);
    font-size: 12px;
    padding: 4px 8px;
  }

  /* MARK: The tab's form (shared by the tab components) */
  .pane {
    overflow: auto;
    padding: 18px 28px 24px;
  }
  h1 {
    font-size: 19px;
    margin: 0 0 14px;
  }
  .pane :global(.form) {
    display: grid;
    grid-template-columns: minmax(110px, max-content) minmax(0, 1fr);
    gap: 12px 14px;
    align-items: baseline;
    max-width: 640px;
  }
  .pane :global(.form > .lab) {
    text-align: right;
    min-width: 110px;
  }
  .pane :global(.form > hr) {
    grid-column: 1 / -1;
    width: 100%;
    border: 0;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
  .pane :global(.intro) {
    color: var(--muted);
    margin: -4px 0 14px;
    max-width: 62ch;
    line-height: 1.45;
  }
  .pane :global(.note) {
    color: var(--muted);
    font-size: 12px;
    margin: 4px 0 0;
    max-width: 56ch;
    line-height: 1.4;
  }
  .pane :global(.line) {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  .pane :global(.stack) {
    display: grid;
    gap: 4px;
  }
  .pane :global(.check) {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .pane :global(input[type="checkbox"]),
  .pane :global(input[type="radio"]) {
    accent-color: var(--accent);
  }
  .pane :global(.path) {
    min-width: 0;
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pane :global(input:not([type="checkbox"]):not([type="radio"])),
  .pane :global(select) {
    font: inherit;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    min-width: 220px;
  }
  .pane :global(input.wide) {
    min-width: 0;
    width: min(360px, 100%);
  }
  .pane :global(button) {
    font: inherit;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 3px 12px;
  }
  .pane :global(button.primary) {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  .pane :global(button:disabled) {
    opacity: 0.5;
  }
  .pane :global(button.link) {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
    font-size: 12px;
  }
  .pane :global(code) {
    font-size: 12px;
    background: var(--hover);
    padding: 1px 4px;
    border-radius: 4px;
  }
  .pane :global(.muted) {
    color: var(--muted);
  }
  .pane :global(.ok),
  .pane :global(.status.ok) {
    color: #2e8540;
  }
  .pane :global(.status) {
    font-size: 12px;
    white-space: nowrap;
  }
  .pane :global(.status.down),
  .pane :global(.warn),
  .pane :global(.error) {
    color: var(--danger);
  }
  .pane :global(.cloud-note) {
    color: var(--cloud);
  }
  .applied {
    color: var(--muted);
    font-size: 11px;
    margin-top: 22px;
  }
  .error {
    margin-top: 14px;
  }
</style>
