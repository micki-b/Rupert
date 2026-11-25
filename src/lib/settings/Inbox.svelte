<script lang="ts">
  // Settings ▸ Inbox: where new scans arrive, filing suggestions, how
  // filed documents are named, and Print to Rupert (#101).
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { api, fileNameFor, type Settings, type Source } from "#lib/api.ts";
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
