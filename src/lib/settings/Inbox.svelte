<script lang="ts">
  // Settings ▸ Inbox: where new scans arrive, filing suggestions, how
  // filed documents are named, and Print to Rupert (#101).
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, fileNameFor, revealLabel, type Settings, type Source } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    settings = $bindable(),
    sources,
    os,
    appStore,
    onerror,
  }: {
    settings: Settings;
    sources: Source[];
    os: string | undefined;
    appStore: boolean;
    onerror: (message: string | null) => void;
  } = $props();

  /** Turns Print to Rupert on or off (the App Store edition asks where first). */
  let printBusy = $state(false);
  async function setPrintEntry(input: HTMLInputElement) {
    printBusy = true;
    onerror(null);
    try {
      const saved = await api.setPrintEntry(input.checked);
      settings.printEntry = saved.printEntry;
      settings.printEntryBookmark = saved.printEntryBookmark;
    } catch (e) {
      onerror(String(e));
    } finally {
      input.checked = settings.printEntry;
      printBusy = false;
    }
  }

  const inboxSource = $derived(sources.find((s) => s.id === settings.inbox?.sourceId) ?? null);
  const example = $derived(fileNameFor(settings.namePattern, "2026-04-29", m.settings.exampleTitle) + ".pdf");

  // MARK: Keep downloaded (#144)

  const SERVICE: Record<string, string> = {
    icloud: "iCloud Drive",
    onedrive: "OneDrive",
    googledrive: "Google Drive",
    dropbox: "Dropbox",
  };
  /** The cloud service the inbox folder is in, if any. */
  const service = $derived(inboxSource && inboxSource.provider !== "local" ? inboxSource.provider : null);
  /** Whether it stays on this computer (Windows can tell); null otherwise. */
  let kept = $state<boolean | null>(null);
  let keeping = $state(false);
  $effect(() => {
    void settings.inbox?.relPath;
    if (!service || os !== "windows") {
      kept = null;
      return;
    }
    api.inboxKeptDownloaded().then((k) => (kept = k)).catch(() => (kept = null));
  });

  async function keep() {
    keeping = true;
    onerror(null);
    try {
      if (await api.keepInboxDownloaded()) kept = true;
    } catch (e) {
      onerror(String(e));
    } finally {
      keeping = false;
    }
  }

  /** The step for this service on this system, where Rupert can't do it. */
  const step = $derived.by(() => {
    if (!service) return "";
    if (service === "googledrive") return m.settings.keepGoogleDrive;
    if (os === "macos") return service === "icloud" ? m.settings.keepMacICloud : service === "onedrive" ? m.settings.keepMacOneDrive : m.settings.keepMacDropbox;
    if (os === "windows") return m.settings.keepWindows;
    return m.settings.keepOther;
  });

  /** Picks a folder; it must be inside one of the sources. */
  async function chooseInbox() {
    const picked = await openDialog({ directory: true, title: m.settings.chooseInbox });
    if (typeof picked !== "string") return;
    const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
    const path = norm(picked);
    const source = sources
      .filter((s) => path === norm(s.root) || path.startsWith(norm(s.root) + "/"))
      .sort((a, b) => b.root.length - a.root.length)[0];
    if (!source) {
      onerror(m.settings.inboxOutside);
      return;
    }
    onerror(null);
    settings.inbox = { sourceId: source.id, relPath: path.slice(norm(source.root).length).replace(/^\//, "") };
  }
</script>

<p class="intro">{m.settings.inboxIntro}</p>
<div class="form">
  <span class="lab">{m.settings.inboxFolder}</span>
  <div class="line">
    <span class="path" title={settings.inbox && inboxSource ? inboxSource.root + "/" + settings.inbox.relPath : ""}>
      {#if settings.inbox && inboxSource}
        {inboxSource.name}{settings.inbox.relPath ? "/" + settings.inbox.relPath : ""}
      {:else}
        <span class="muted">{m.settings.notSet}</span>
      {/if}
    </span>
    <button onclick={chooseInbox}>{m.settings.choose}</button>
    {#if settings.inbox}<button onclick={() => (settings.inbox = null)}>{m.settings.clear}</button>{/if}
  </div>
  {#if service && settings.inbox && inboxSource}
    <span></span>
    <div class="keep">
      <p class="note">{m.settings.keepNote(SERVICE[service] ?? service)}</p>
      {#if kept === true}
        <p class="note ok">✓ {m.settings.keptOnComputer}</p>
      {:else if kept === false}
        <button onclick={keep} disabled={keeping}>{m.settings.keepButton}</button>
      {:else}
        <p class="note">{step}</p>
      {/if}
      {#if kept !== true}
        <button class="link" onclick={() => api.revealEntry(inboxSource.id, settings.inbox!.relPath)}>{revealLabel(os)}</button>
      {/if}
    </div>
  {/if}

  <span class="lab">{m.settings.suggestions}</span>
  <div>
    <label class="check">
      <input type="checkbox" bind:checked={settings.suggestions} />
      {m.settings.suggest}
    </label>
    <p class="note">{m.settings.suggestionsNote}</p>
  </div>

  <hr />
  <label class="lab" for="pattern">{m.settings.fileNames}</label>
  <div>
    <input id="pattern" class="wide" bind:value={settings.namePattern} />
    <p class="note">
      {m.settings.patternHint[0]}<code>{"{date}"}</code>{m.settings.patternHint[1]}<code>{"{title}"}</code>{m.settings
        .patternHint[2]}{example}
    </p>
  </div>

  <hr />
  <span class="lab">{m.settings.printEntry}</span>
  <div>
    {#if os === "linux"}
      <p class="note">{m.settings.printEntryLinux}</p>
    {:else}
      <label class="check">
        <input
          type="checkbox"
          checked={settings.printEntry}
          disabled={!settings.inbox || printBusy}
          onchange={(e) => setPrintEntry(e.currentTarget)}
        />
        {os === "windows" ? m.settings.printEntryWindows : m.settings.printEntryMac}
      </label>
      <p class="note">
        {os === "windows"
          ? m.settings.printEntryNoteWindows
          : appStore
            ? m.settings.printEntryNoteStore
            : m.settings.printEntryNoteMac}
      </p>
    {/if}
  </div>
</div>

<style>
  /* The inbox in a cloud folder (#144). */
  .keep {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 6px;
    margin-top: -4px;
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--panel);
  }
  .keep .note {
    margin: 0;
  }
  .ok {
    color: var(--accent-strong);
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-strong);
    text-decoration: underline;
    font: inherit;
    font-size: 12px;
  }
</style>
