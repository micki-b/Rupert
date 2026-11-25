<script lang="ts">
  // Settings ▸ Inbox: where new scans arrive, filing suggestions, and how
  // filed documents are named.
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { fileNameFor, type Settings, type Source } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    settings = $bindable(),
    sources,
    onerror,
  }: { settings: Settings; sources: Source[]; onerror: (message: string | null) => void } = $props();

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
</div>
