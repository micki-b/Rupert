<script lang="ts">
  // Settings: the inbox, text recognition, LM Studio suggestions, naming,
  // and the chat assistant's model.
  import { onMount, tick } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "$lib/Icon.svelte";
  import { api, fileNameFor, type AssistantProvider, type ChatGptStatus, type Settings, type Source } from "$lib/api";

  let {
    sources,
    section = null,
    onclose,
    onsaved,
  }: {
    sources: Source[];
    /** A section to scroll to when opening. */
    section?: "assistant" | null;
    onclose: () => void;
    onsaved: (s: Settings) => void;
  } = $props();

  let settings = $state<Settings | null>(null);
  let engine = $state<string | null | undefined>(undefined);
  let models = $state<string[]>([]);
  let lmStatus = $state<"checking" | "ok" | "down">("checking");
  let error = $state<string | null>(null);
  // Assistant: keys go straight to the keychain, never into settings.
  let hasKey = $state<Record<string, boolean>>({});
  let newKey = $state("");
  let cli = $state<{ installed: boolean; signedIn: boolean } | null>(null);
  let cliBusy = $state(false);
  let chatgpt = $state<ChatGptStatus | null>(null);
  let chatgptBusy = $state(false);
  const provider = $derived(settings?.assistant.provider ?? "local");

  const inboxSource = $derived(sources.find((s) => s.id === settings?.inbox?.sourceId) ?? null);
  const example = $derived(
    settings ? fileNameFor(settings.namePattern, "2026-04-29", "Acme auto policy cancellation") + ".pdf" : "",
  );

  onMount(async () => {
    settings = await api.getSettings();
    if (section) {
      await tick();
      document.getElementById(`settings-${section}`)?.scrollIntoView({ block: "start" });
    }
    engine = await api.ocrEngine();
    checkLm();
    for (const p of ["anthropic", "openai"] as AssistantProvider[]) {
      hasKey[p] = await api.assistantHasKey(p).catch(() => false);
    }
    cli = await api.assistantCliStatus().catch(() => null);
    if (settings?.assistant.provider === "chatgpt") loadChatgpt();
  });

  async function loadChatgpt() {
    chatgpt = await api.chatgptStatus().catch(() => null);
    // Default to the first model the plan offers.
    if (settings?.assistant.provider === "chatgpt" && !settings.assistant.model && chatgpt?.models.length)
      settings.assistant.model = chatgpt.models[0].slug;
  }

  async function chatgptSignIn() {
    chatgptBusy = true;
    try {
      error = null;
      chatgpt = await api.chatgptSignIn();
      // Pick the first model the plan offers, unless one is chosen.
      if (settings && !settings.assistant.model && chatgpt.models.length)
        settings.assistant.model = chatgpt.models[0].slug;
    } catch (e) {
      error = String(e);
    } finally {
      chatgptBusy = false;
    }
  }

  async function chatgptSignOut() {
    await api.chatgptSignOut().catch((e) => (error = String(e)));
    chatgpt = { signedIn: false, email: null, models: [], problem: null };
  }

  // The status is fetched when ChatGPT is chosen (it asks OpenAI for models).
  $effect(() => {
    if (provider === "chatgpt" && (chatgpt === null || (!settings?.assistant.model && chatgpt.models.length)))
      loadChatgpt();
  });

  async function saveKey() {
    if (!newKey.trim() || provider === "local") return;
    try {
      await api.assistantSetKey(provider, newKey.trim());
      hasKey[provider] = true;
      newKey = "";
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  async function removeKey() {
    if (provider === "local") return;
    await api.assistantSetKey(provider, null).catch((e) => (error = String(e)));
    hasKey[provider] = false;
  }

  async function cliLogin() {
    cliBusy = true;
    try {
      await api.assistantCliLogin();
    } catch (e) {
      error = String(e);
    } finally {
      cli = await api.assistantCliStatus().catch(() => null);
      cliBusy = false;
    }
  }

  async function checkLm() {
    if (!settings) return;
    lmStatus = "checking";
    try {
      models = await api.lmModels(settings.lmStudioUrl);
      lmStatus = "ok";
    } catch {
      models = [];
      lmStatus = "down";
    }
  }

  /** Picks a folder; it must be inside one of the sources. */
  async function chooseInbox() {
    const picked = await openDialog({ directory: true, title: "Choose the inbox folder" });
    if (typeof picked !== "string" || !settings) return;
    const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
    const path = norm(picked);
    const source = sources
      .filter((s) => path === norm(s.root) || path.startsWith(norm(s.root) + "/"))
      .sort((a, b) => b.root.length - a.root.length)[0];
    if (!source) {
      error = "The inbox has to be inside one of Rupert's sources, so filed documents stay in your library.";
      return;
    }
    error = null;
    settings.inbox = { sourceId: source.id, relPath: path.slice(norm(source.root).length).replace(/^\//, "") };
  }

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
    saveTimer = setTimeout(() => persist(snap, json), 600);
  });

  async function persist(snap: Settings, json: string) {
    saveTimer = undefined;
    try {
      onsaved(await api.setSettings(snap));
      lastSaved = json;
      error = null;
    } catch (e) {
      error = String(e);
    }
  }

  /** Saves anything still waiting, then closes. */
  async function done() {
    clearTimeout(saveTimer);
    if (settings) {
      const snap = $state.snapshot(settings);
      const json = JSON.stringify(snap);
      if (lastSaved && json !== lastSaved) await persist(snap, json);
    }
    onclose();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") done();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
    <header class="head">
      <div>
        <h1 id="settings-title">Settings</h1>
        <p class="muted small">Changes apply right away.</p>
      </div>
      <button class="close" onclick={done} title="Close (Esc)" aria-label="Close settings">
        <Icon name="close" size={18} />
      </button>
    </header>
    {#if settings}
      <section>
        <h2>Inbox</h2>
        <p class="muted small">
          New scans arrive here, e.g. the folder a ScanSnap Home profile saves to. Rupert recognizes their text,
          suggests how to file them, and moves them where you decide.
        </p>
        <div class="row">
          <span class="path">
            {#if settings.inbox && inboxSource}
              {inboxSource.name}{settings.inbox.relPath ? "/" + settings.inbox.relPath : ""}
            {:else}
              <span class="muted">Not set</span>
            {/if}
          </span>
          <button onclick={chooseInbox}>Choose…</button>
          {#if settings.inbox}<button onclick={() => (settings!.inbox = null)}>Clear</button>{/if}
        </div>
      </section>

      <section>
        <h2>Text recognition</h2>
        <label class="check">
          <input type="checkbox" bind:checked={settings.ocr} disabled={!engine} />
          Recognize the text in scans{#if engine}&nbsp;<span class="muted">({engine})</span>{/if}
        </label>
        {#if engine === null}
          <p class="muted small">No OCR engine found. Install Tesseract to enable this.</p>
        {:else}
          <p class="muted small">
            Adds an invisible text layer to scanned PDFs, so any PDF reader can search them. Runs on this computer.
          </p>
        {/if}
      </section>

      <section>
        <h2>Suggestions</h2>
        <label class="check">
          <input type="checkbox" bind:checked={settings.suggestions} />
          Suggest a title, date, tags and folder with LM Studio
        </label>
        <div class="grid">
          <label for="lm-url">Server</label>
          <div class="row">
            <input id="lm-url" bind:value={settings.lmStudioUrl} onchange={checkLm} />
            <span class="status {lmStatus}">
              {lmStatus === "ok" ? "Connected" : lmStatus === "down" ? "Not running" : "Checking…"}
            </span>
          </div>
          <label for="lm-model">Model</label>
          <select
            id="lm-model"
            value={settings.lmStudioModel ?? ""}
            onchange={(e) => (settings!.lmStudioModel = e.currentTarget.value || null)}
          >
            <option value="">The model loaded in LM Studio</option>
            {#each models as m (m)}<option value={m}>{m}</option>{/each}
            {#if settings.lmStudioModel && !models.includes(settings.lmStudioModel)}
              <option value={settings.lmStudioModel}>{settings.lmStudioModel}</option>
            {/if}
          </select>
        </div>
        <p class="muted small">Only document text from the inbox is sent, and only to this local server.</p>
      </section>

      <section>
        <h2 id="settings-assistant">Assistant</h2>
        <div class="grid">
          <label for="ai-provider">Model</label>
          <select
            id="ai-provider"
            bind:value={settings.assistant.provider}
            onchange={() => (settings!.assistant.model = null)}
          >
            <option value="local">LM Studio on this computer</option>
            <option value="anthropic">Claude (Anthropic, cloud)</option>
            <option value="openai">OpenAI-compatible service (cloud)</option>
            <option value="chatgpt">ChatGPT, with your plan (cloud)</option>
          </select>
          {#if provider === "anthropic"}
            <label for="ai-model">Claude model</label>
            <input
              id="ai-model"
              placeholder="claude-opus-5-5 (default)"
              value={settings.assistant.model ?? ""}
              oninput={(e) => (settings!.assistant.model = e.currentTarget.value.trim() || null)}
            />
            <span class="label">Sign-in</span>
            <div class="stack">
              <label class="check">
                <input type="radio" name="ai-auth" checked={!settings.assistant.anthropicCliLogin} onchange={() => (settings!.assistant.anthropicCliLogin = false)} />
                API key
              </label>
              <label class="check">
                <input type="radio" name="ai-auth" checked={settings.assistant.anthropicCliLogin} onchange={() => (settings!.assistant.anthropicCliLogin = true)} />
                Anthropic account, through the <code>ant</code> command
              </label>
            </div>
          {:else if provider === "chatgpt"}
            <span class="label">Account</span>
            <div class="row">
              {#if chatgpt?.signedIn}
                <span class="muted">{chatgpt.email ?? "Signed in"}</span>
                <button onclick={chatgptSignOut}>Sign out</button>
              {:else}
                <span class="muted">Not signed in</span>
              {/if}
              <button onclick={chatgptSignIn} disabled={chatgptBusy}>
                {chatgptBusy ? "Waiting for the browser…" : chatgpt?.signedIn ? "Sign in again…" : "Sign in with ChatGPT…"}
              </button>
            </div>
            {#if chatgpt?.signedIn}
              <label for="ai-model3">Model</label>
              <select
                id="ai-model3"
                value={settings.assistant.model ?? ""}
                onchange={(e) => (settings!.assistant.model = e.currentTarget.value || null)}
              >
                {#each chatgpt.models as m (m.slug)}<option value={m.slug}>{m.displayName}</option>{/each}
                {#if settings.assistant.model && !chatgpt.models.some((m) => m.slug === settings!.assistant.model)}
                  <option value={settings.assistant.model}>{settings.assistant.model}</option>
                {/if}
                {#if !chatgpt.models.length && !settings.assistant.model}<option value="">The plan's first model</option>{/if}
              </select>
              {#if chatgpt.problem}<span></span><span class="small error">{chatgpt.problem}</span>{/if}
            {/if}
          {:else if provider === "openai"}
            <label for="ai-url">Service</label>
            <input
              id="ai-url"
              placeholder="https://api.openai.com/v1"
              value={settings.assistant.baseUrl ?? ""}
              oninput={(e) => (settings!.assistant.baseUrl = e.currentTarget.value.trim() || null)}
            />
            <label for="ai-model2">Model</label>
            <input
              id="ai-model2"
              placeholder="The service's model name"
              value={settings.assistant.model ?? ""}
              oninput={(e) => (settings!.assistant.model = e.currentTarget.value.trim() || null)}
            />
          {/if}
          {#if provider !== "local" && !(provider === "anthropic" && settings.assistant.anthropicCliLogin)}
            <label for="ai-key">API key</label>
            <div class="row">
              {#if hasKey[provider]}
                <span class="muted">Saved in the keychain</span>
                <button onclick={removeKey}>Remove</button>
              {:else}
                <input id="ai-key" type="password" bind:value={newKey} placeholder="Paste the key" autocomplete="off" />
                <button onclick={saveKey} disabled={!newKey.trim()}>Save key</button>
              {/if}
            </div>
          {/if}
          {#if provider === "anthropic" && settings.assistant.anthropicCliLogin}
            <span class="label">Account</span>
            <div class="row">
              {#if !cli?.installed}
                <span class="muted">The <code>ant</code> command isn't installed.</span>
              {:else if cli.signedIn}
                <span class="muted">Signed in</span>
              {:else}
                <span class="muted">Not signed in</span>
              {/if}
              {#if cli?.installed}
                <button onclick={cliLogin} disabled={cliBusy}>{cliBusy ? "Waiting for the browser…" : cli.signedIn ? "Sign in again…" : "Sign in…"}</button>
              {/if}
            </div>
          {/if}
        </div>
        <p class="small">
          <button class="link" onclick={() => api.openHelp("ai-models")}>How to set up each kind of model</button>
        </p>
        {#if provider === "chatgpt"}
          <p class="muted small">
            Answers count against your ChatGPT plan's limits, not an API key. You can cap Rupert's share in ChatGPT's
            settings. This is a preview from OpenAI and may change.
          </p>
        {/if}
        {#if provider === "local"}
          <p class="muted small">Uses the LM Studio server and model above. Nothing leaves this computer.</p>
        {:else}
          <p class="small cloud-note">
            ☁ With a cloud model, your questions and what the assistant looks at (document titles, tags, folders and
            the text of documents it reads) are sent to that service. Inbox suggestions still use LM Studio only.
          </p>
        {/if}
      </section>

      <section>
        <h2>File names</h2>
        <div class="grid">
          <label for="pattern">Pattern</label>
          <input id="pattern" bind:value={settings.namePattern} />
        </div>
        <p class="muted small">
          <code>{"{date}"}</code> and <code>{"{title}"}</code> are filled in. Example: {example}
        </p>
      </section>

      {#if error}<p class="error">{error}</p>{/if}
    {:else}
      <p class="muted">Loading…</p>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.3);
    display: grid;
    place-items: center;
    z-index: 40; /* above the chat panel (20) and menus (30) */
  }
  .dialog {
    width: min(560px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.25);
    padding: 18px 20px;
  }
  h1 {
    font-size: 16px;
    margin: 0;
  }
  /* Title and close button, kept in view while the settings scroll. */
  .head {
    position: sticky;
    top: -18px;
    z-index: 1;
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin: -18px -20px 0;
    padding: 18px 14px 6px 20px;
    background: var(--bg);
  }
  button.close {
    flex: none;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: grid;
    place-items: center;
  }
  button.close:hover {
    background: var(--hover);
    color: var(--text);
  }
  h2 {
    scroll-margin-top: 64px; /* clear of the title bar above */
    font-size: 11px;
    margin: 16px 0 6px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  p {
    margin: 4px 0;
    line-height: 1.4;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .grid {
    display: grid;
    grid-template-columns: 70px 1fr;
    gap: 6px 8px;
    align-items: center;
    margin: 8px 0 4px;
  }
  .grid label {
    font-size: 12px;
    color: var(--muted);
  }
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  input:not([type="checkbox"]),
  select {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
    min-width: 0;
    flex: 1;
  }
  .status {
    font-size: 12px;
    white-space: nowrap;
  }
  .status.ok {
    color: #2e8540;
  }
  .status.down {
    color: #c0392b;
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 12px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .label {
    font-size: 12px;
    color: var(--muted);
  }
  .cloud-note {
    color: var(--cloud);
  }
  button.link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent);
    text-decoration: underline;
  }
  .error {
    color: #c0392b;
  }
</style>
