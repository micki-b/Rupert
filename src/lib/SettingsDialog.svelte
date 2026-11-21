<script lang="ts">
  // Settings: the inbox, text recognition, Local AI and its suggestions,
  // naming, and the chat assistant's model.
  import { onMount, tick } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import Icon from "#lib/Icon.svelte";
  import { updates } from "#lib/updates.svelte.ts";
  import {
    api,
    fileNameFor,
    type AssistantProvider,
    type ChatGptStatus,
    type LocalAiPlace,
    type Settings,
    type Source,
  } from "#lib/api.ts";
  import { LANGS, languageNames, loadLanguage, m, type Lang } from "#lib/i18n/index.ts";

  let {
    sources,
    section = null,
    onclose,
    onsaved,
  }: {
    sources: Source[];
    /** A section to scroll to when opening. */
    section?: "assistant" | "updates" | null;
    onclose: () => void;
    onsaved: (s: Settings) => void;
  } = $props();

  let settings = $state<Settings | null>(null);
  let engine = $state<string | null | undefined>(undefined);
  let models = $state<string[]>([]);
  let lmStatus = $state<"checking" | "ok" | "down">("checking");
  /** Where the Local AI address points. */
  let place = $state<LocalAiPlace | null>(null);
  /** Local AI servers people are likely to run, by their usual address. */
  const PRESETS = [
    { id: "lmstudio", name: "LM Studio", url: "http://localhost:1234" },
    { id: "ollama", name: "Ollama", url: "http://localhost:11434" },
  ];
  const norm = (u: string) => u.trim().replace(/\/+$/, "").replace("127.0.0.1", "localhost");
  const preset = $derived(PRESETS.find((p) => settings && norm(settings.localUrl) === p.url)?.id ?? "other");
  let error = $state<string | null>(null);
  // Assistant: keys go straight to the keychain, never into settings.
  let hasKey = $state<Record<string, boolean>>({});
  let newKey = $state("");
  let cli = $state<{ installed: boolean; signedIn: boolean } | null>(null);
  let cliBusy = $state(false);
  let chatgpt = $state<ChatGptStatus | null>(null);
  /** The Mac App Store edition: no self-updater, no `ant` sign-in. */
  let appStore = $state(false);
  let chatgptBusy = $state(false);
  /** The system's language, which "Automatic" follows. */
  let systemLang = $state<Lang>("en");
  const provider = $derived(settings?.assistant.provider ?? "local");

  const inboxSource = $derived(sources.find((s) => s.id === settings?.inbox?.sourceId) ?? null);
  const example = $derived(
    settings ? fileNameFor(settings.namePattern, "2026-04-29", m.settings.exampleTitle) + ".pdf" : "",
  );

  onMount(async () => {
    api.appInfo().then((i) => (appStore = i.edition === "app-store"));
    loadLanguage().then((l) => l && (systemLang = l.system));
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

  function choosePreset(id: string) {
    const p = PRESETS.find((x) => x.id === id);
    if (!p || !settings) return;
    settings.localUrl = p.url;
    settings.localModel = null; // model names differ between servers
    checkLm();
  }

  async function checkLm() {
    if (!settings) return;
    lmStatus = "checking";
    place = await api.localAiPlace(settings.localUrl).catch(() => null);
    try {
      models = await api.lmModels(settings.localUrl);
      lmStatus = "ok";
    } catch {
      models = [];
      lmStatus = "down";
    }
  }

  /** Picks a folder; it must be inside one of the sources. */
  async function chooseInbox() {
    const picked = await openDialog({ directory: true, title: m.settings.chooseInbox });
    if (typeof picked !== "string" || !settings) return;
    const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "");
    const path = norm(picked);
    const source = sources
      .filter((s) => path === norm(s.root) || path.startsWith(norm(s.root) + "/"))
      .sort((a, b) => b.root.length - a.root.length)[0];
    if (!source) {
      error = m.settings.inboxOutside;
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
      await loadLanguage();
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
        <h1 id="settings-title">{m.settings.title}</h1>
        <p class="muted small">{m.settings.applyNow}</p>
      </div>
      <button class="close" onclick={done} title={m.common.closeEsc} aria-label={m.settings.closeSettings}>
        <Icon name="close" size={18} />
      </button>
    </header>
    {#if settings}
      <section>
        <h2>{m.settings.language}</h2>
        <div class="grid">
          <label for="language">{m.settings.language}</label>
          <select
            id="language"
            value={settings.language ?? ""}
            onchange={(e) => (settings!.language = e.currentTarget.value || null)}
          >
            <option value="">{m.settings.automatic(languageNames[systemLang])}</option>
            {#each LANGS as lang (lang)}<option value={lang}>{languageNames[lang]}</option>{/each}
          </select>
        </div>
        <p class="muted small">{m.settings.languageNote}</p>
      </section>

      <section>
        <h2>{m.settings.inbox}</h2>
        <p class="muted small">{m.settings.inboxIntro}</p>
        <div class="row">
          <span class="path">
            {#if settings.inbox && inboxSource}
              {inboxSource.name}{settings.inbox.relPath ? "/" + settings.inbox.relPath : ""}
            {:else}
              <span class="muted">{m.settings.notSet}</span>
            {/if}
          </span>
          <button onclick={chooseInbox}>{m.settings.choose}</button>
          {#if settings.inbox}<button onclick={() => (settings!.inbox = null)}>{m.settings.clear}</button>{/if}
        </div>
      </section>

      <section>
        <h2>{m.settings.textRecognition}</h2>
        <label class="check">
          <input type="checkbox" bind:checked={settings.ocr} disabled={!engine} />
          {m.settings.recognize}{#if engine}&nbsp;<span class="muted">({engine})</span>{/if}
        </label>
        {#if engine === null}
          <p class="muted small">{m.settings.noEngine}</p>
        {:else}
          <p class="muted small">{m.settings.ocrNote}</p>
        {/if}
      </section>

      <section>
        <h2 id="settings-local-ai">{m.settings.localAi}</h2>
        <div class="grid">
          <label for="lm-preset">{m.settings.server}</label>
          <select id="lm-preset" value={preset} onchange={(e) => choosePreset(e.currentTarget.value)}>
            {#each PRESETS as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
            <option value="other" disabled={preset !== "other"}>{m.settings.otherServer}</option>
          </select>
          <label for="lm-url">{m.settings.address}</label>
          <div class="row">
            <input id="lm-url" bind:value={settings.localUrl} onchange={checkLm} />
            <span class="status {lmStatus}">
              {lmStatus === "ok" ? m.settings.connected : lmStatus === "down" ? m.settings.notRunning : m.settings.checking}
            </span>
          </div>
          <label for="lm-model">{m.settings.model}</label>
          <select
            id="lm-model"
            value={settings.localModel ?? ""}
            onchange={(e) => (settings!.localModel = e.currentTarget.value || null)}
          >
            <option value="">{m.settings.loadedModel}</option>
            {#each models as name (name)}<option value={name}>{name}</option>{/each}
            {#if settings.localModel && !models.includes(settings.localModel)}
              <option value={settings.localModel}>{settings.localModel}</option>
            {/if}
          </select>
        </div>
        {#if place?.reach === "elsewhere"}
          <p class="small cloud-note">{m.settings.localWhere.elsewhere(place.host)}</p>
        {:else if place?.reach === "localNetwork"}
          <p class="muted small">{m.settings.localWhere.localNetwork(place.host)}</p>
        {:else}
          <p class="muted small">{m.settings.localWhere.thisComputer}</p>
        {/if}
        <p class="muted small">{m.settings.localAiNote}</p>
      </section>

      <section>
        <h2>{m.settings.suggestions}</h2>
        <label class="check">
          <input type="checkbox" bind:checked={settings.suggestions} />
          {m.settings.suggest}
        </label>
        <p class="muted small">{m.settings.suggestionsNote}</p>
      </section>

      <section>
        <h2 id="settings-assistant">{m.settings.assistant}</h2>
        <div class="grid">
          <label for="ai-provider">{m.settings.model}</label>
          <select
            id="ai-provider"
            bind:value={settings.assistant.provider}
            onchange={() => (settings!.assistant.model = null)}
          >
            <option value="local">{m.settings.providerLocal}</option>
            <option value="anthropic">{m.settings.providerAnthropic}</option>
            <option value="openai">{m.settings.providerOpenai}</option>
            <option value="chatgpt">{m.settings.providerChatgpt}</option>
          </select>
          {#if provider === "anthropic"}
            <label for="ai-model">{m.settings.claudeModel}</label>
            <input
              id="ai-model"
              placeholder={m.settings.claudeDefault}
              value={settings.assistant.model ?? ""}
              oninput={(e) => (settings!.assistant.model = e.currentTarget.value.trim() || null)}
            />
            {#if !appStore}<span class="label">{m.settings.signIn}</span>{/if}
            <div class="stack" hidden={appStore}>
              <label class="check">
                <input type="radio" name="ai-auth" checked={!settings.assistant.anthropicCliLogin} onchange={() => (settings!.assistant.anthropicCliLogin = false)} />
                {m.settings.apiKey}
              </label>
              <label class="check">
                <input type="radio" name="ai-auth" checked={settings.assistant.anthropicCliLogin} onchange={() => (settings!.assistant.anthropicCliLogin = true)} />
                {m.settings.antAccount[0]} <code>ant</code> {m.settings.antAccount[1]}
              </label>
            </div>
          {:else if provider === "chatgpt"}
            <span class="label">{m.settings.account}</span>
            <div class="row">
              {#if chatgpt?.signedIn}
                <span class="muted">{chatgpt.email ?? m.settings.signedIn}</span>
                <button onclick={chatgptSignOut}>{m.settings.signOut}</button>
              {:else}
                <span class="muted">{m.settings.notSignedIn}</span>
              {/if}
              <button onclick={chatgptSignIn} disabled={chatgptBusy}>
                {chatgptBusy ? m.settings.waitingForBrowser : chatgpt?.signedIn ? m.settings.signInAgain : m.settings.signInChatgpt}
              </button>
            </div>
            {#if chatgpt?.signedIn}
              <label for="ai-model3">{m.settings.model}</label>
              <select
                id="ai-model3"
                value={settings.assistant.model ?? ""}
                onchange={(e) => (settings!.assistant.model = e.currentTarget.value || null)}
              >
                {#each chatgpt.models as cm (cm.slug)}<option value={cm.slug}>{cm.displayName}</option>{/each}
                {#if settings.assistant.model && !chatgpt.models.some((cm) => cm.slug === settings!.assistant.model)}
                  <option value={settings.assistant.model}>{settings.assistant.model}</option>
                {/if}
                {#if !chatgpt.models.length && !settings.assistant.model}<option value="">{m.settings.firstPlanModel}</option>{/if}
              </select>
              {#if chatgpt.problem}<span></span><span class="small error">{chatgpt.problem}</span>{/if}
            {/if}
          {:else if provider === "openai"}
            <label for="ai-url">{m.settings.service}</label>
            <input
              id="ai-url"
              placeholder="https://api.openai.com/v1"
              value={settings.assistant.baseUrl ?? ""}
              oninput={(e) => (settings!.assistant.baseUrl = e.currentTarget.value.trim() || null)}
            />
            <label for="ai-model2">{m.settings.model}</label>
            <input
              id="ai-model2"
              placeholder={m.settings.serviceModel}
              value={settings.assistant.model ?? ""}
              oninput={(e) => (settings!.assistant.model = e.currentTarget.value.trim() || null)}
            />
          {/if}
          {#if provider !== "local" && !(provider === "anthropic" && settings.assistant.anthropicCliLogin && !appStore)}
            <label for="ai-key">{m.settings.apiKey}</label>
            <div class="row">
              {#if hasKey[provider]}
                <span class="muted">{m.settings.savedInKeychain}</span>
                <button onclick={removeKey}>{m.settings.remove}</button>
              {:else}
                <input id="ai-key" type="password" bind:value={newKey} placeholder={m.settings.pasteKey} autocomplete="off" />
                <button onclick={saveKey} disabled={!newKey.trim()}>{m.settings.saveKey}</button>
              {/if}
            </div>
          {/if}
          {#if provider === "anthropic" && settings.assistant.anthropicCliLogin && !appStore}
            <span class="label">{m.settings.account}</span>
            <div class="row">
              {#if !cli?.installed}
                <span class="muted">{m.settings.antMissing[0]} <code>ant</code> {m.settings.antMissing[1]}</span>
              {:else if cli.signedIn}
                <span class="muted">{m.settings.signedIn}</span>
              {:else}
                <span class="muted">{m.settings.notSignedIn}</span>
              {/if}
              {#if cli?.installed}
                <button onclick={cliLogin} disabled={cliBusy}>{cliBusy ? m.settings.waitingForBrowser : cli.signedIn ? m.settings.signInAgain : m.settings.signInEllipsis}</button>
              {/if}
            </div>
          {/if}
        </div>
        <p class="small">
          <button class="link" onclick={() => api.openHelp("ai-models")}>{m.settings.howToSetUp}</button>
        </p>
        {#if provider === "chatgpt"}
          <p class="muted small">{m.settings.chatgptNote}</p>
        {/if}
        {#if provider === "local" && place?.reach !== "elsewhere"}
          <p class="muted small">{m.settings.localNote}</p>
        {:else}
          <p class="small cloud-note">{m.settings.cloudNote}</p>
        {/if}
      </section>

      <section hidden={appStore}>
        <h2 id="settings-updates">{m.settings.updates}</h2>
        <label class="check">
          <input type="checkbox" bind:checked={settings.checkUpdates} />
          {m.settings.checkAutomatically}
        </label>
        <div class="row update-row">
          <button onclick={() => updates.check(true)} disabled={updates.status === "checking" || updates.status === "downloading"}>
            {updates.status === "checking" ? m.settings.checking : m.settings.checkNow}
          </button>
          {#if updates.status === "none"}
            <span class="muted small">{m.settings.upToDate}</span>
          {:else if updates.status === "available"}
            <span class="small">{m.settings.available(updates.version ?? "")}</span>
            <button onclick={() => updates.install()}>{m.settings.installRestart}</button>
          {:else if updates.status === "downloading"}
            <span class="muted small">
              {m.settings.downloading(updates.progress !== null ? Math.round(updates.progress * 100) : null)}
            </span>
          {:else if updates.status === "error"}
            <span class="small error">{m.settings.couldntCheck(updates.problem ?? "")}</span>
          {/if}
        </div>
        <p class="muted small">{m.settings.updatesNote}</p>
      </section>

      <section>
        <h2>{m.settings.fileNames}</h2>
        <div class="grid">
          <label for="pattern">{m.settings.pattern}</label>
          <input id="pattern" bind:value={settings.namePattern} />
        </div>
        <p class="muted small">
          {m.settings.patternHint[0]}<code>{"{date}"}</code>{m.settings.patternHint[1]}<code>{"{title}"}</code>{m.settings.patternHint[2]}{example}
        </p>
      </section>

      {#if error}<p class="error">{error}</p>{/if}
    {:else}
      <p class="muted">{m.common.loading}</p>
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
  .update-row {
    margin: 6px 0 2px;
    flex-wrap: wrap;
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
