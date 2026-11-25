<script lang="ts">
  // Settings ▸ Assistant: which model answers Ask Rupert. Local AI by
  // default; a cloud model only when chosen here, which the page says.
  // Keys go straight to the keychain, never into settings.
  import { onMount } from "svelte";
  import { api, type AssistantProvider, type ChatGptStatus, type LocalAiPlace, type Settings } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    settings = $bindable(),
    appStore,
    onerror,
  }: { settings: Settings; appStore: boolean; onerror: (message: string | null) => void } = $props();

  let hasKey = $state<Record<string, boolean>>({});
  let newKey = $state("");
  let cli = $state<{ installed: boolean; signedIn: boolean } | null>(null);
  let cliBusy = $state(false);
  let chatgpt = $state<ChatGptStatus | null>(null);
  let chatgptBusy = $state(false);
  /** Where Local AI points: a server elsewhere counts as a cloud service. */
  let place = $state<LocalAiPlace | null>(null);
  const provider = $derived(settings.assistant.provider);
  const elsewhere = $derived(settings.localEngine !== "apple" && place?.reach === "elsewhere");

  onMount(async () => {
    api.localAiPlace(settings.localUrl).then(
      (p) => (place = p),
      () => (place = null),
    );
    for (const p of ["anthropic", "openai"] as AssistantProvider[]) {
      hasKey[p] = await api.assistantHasKey(p).catch(() => false);
    }
    cli = await api.assistantCliStatus().catch(() => null);
  });

  async function loadChatgpt() {
    chatgpt = await api.chatgptStatus().catch(() => null);
    // Default to the first model the plan offers.
    if (settings.assistant.provider === "chatgpt" && !settings.assistant.model && chatgpt?.models.length)
      settings.assistant.model = chatgpt.models[0].slug;
  }

  // The status is fetched when ChatGPT is chosen (it asks OpenAI for models).
  $effect(() => {
    if (provider === "chatgpt" && (chatgpt === null || (!settings.assistant.model && chatgpt.models.length)))
      loadChatgpt();
  });

  async function chatgptSignIn() {
    chatgptBusy = true;
    try {
      onerror(null);
      chatgpt = await api.chatgptSignIn();
      if (!settings.assistant.model && chatgpt.models.length) settings.assistant.model = chatgpt.models[0].slug;
    } catch (e) {
      onerror(String(e));
    } finally {
      chatgptBusy = false;
    }
  }

  async function chatgptSignOut() {
    await api.chatgptSignOut().catch((e) => onerror(String(e)));
    chatgpt = { signedIn: false, email: null, models: [], problem: null };
  }

  async function saveKey() {
    if (!newKey.trim() || provider === "local") return;
    try {
      await api.assistantSetKey(provider, newKey.trim());
      hasKey[provider] = true;
      newKey = "";
      onerror(null);
    } catch (e) {
      onerror(String(e));
    }
  }

  async function removeKey() {
    if (provider === "local") return;
    await api.assistantSetKey(provider, null).catch((e) => onerror(String(e)));
    hasKey[provider] = false;
  }

  async function cliLogin() {
    cliBusy = true;
    try {
      await api.assistantCliLogin();
    } catch (e) {
      onerror(String(e));
    } finally {
      cli = await api.assistantCliStatus().catch(() => null);
      cliBusy = false;
    }
  }
</script>

<div class="form">
  <label class="lab" for="ai-provider">{m.settings.model}</label>
  <div>
    <select id="ai-provider" bind:value={settings.assistant.provider} onchange={() => (settings.assistant.model = null)}>
      <option value="local">{m.settings.providerLocal}</option>
      <option value="anthropic">{m.settings.providerAnthropic}</option>
      <option value="openai">{m.settings.providerOpenai}</option>
      <option value="chatgpt">{m.settings.providerChatgpt}</option>
    </select>
  </div>

  {#if provider === "anthropic"}
    <label class="lab" for="ai-model">{m.settings.claudeModel}</label>
    <div>
      <input
        id="ai-model"
        class="wide"
        placeholder={m.settings.claudeDefault}
        value={settings.assistant.model ?? ""}
        oninput={(e) => (settings.assistant.model = e.currentTarget.value.trim() || null)}
      />
    </div>
    {#if !appStore}
      <span class="lab">{m.settings.signIn}</span>
      <div class="stack">
        <label class="check">
          <input
            type="radio"
            name="ai-auth"
            checked={!settings.assistant.anthropicCliLogin}
            onchange={() => (settings.assistant.anthropicCliLogin = false)}
          />
          {m.settings.apiKey}
        </label>
        <label class="check">
          <input
            type="radio"
            name="ai-auth"
            checked={settings.assistant.anthropicCliLogin}
            onchange={() => (settings.assistant.anthropicCliLogin = true)}
          />
          {m.settings.antAccount[0]} <code>ant</code>
          {m.settings.antAccount[1]}
        </label>
      </div>
    {/if}
  {:else if provider === "chatgpt"}
    <span class="lab">{m.settings.account}</span>
    <div class="line">
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
      <label class="lab" for="ai-model3">{m.settings.model}</label>
      <div>
        <select
          id="ai-model3"
          value={settings.assistant.model ?? ""}
          onchange={(e) => (settings.assistant.model = e.currentTarget.value || null)}
        >
          {#each chatgpt.models as cm (cm.slug)}<option value={cm.slug}>{cm.displayName}</option>{/each}
          {#if settings.assistant.model && !chatgpt.models.some((cm) => cm.slug === settings.assistant.model)}
            <option value={settings.assistant.model}>{settings.assistant.model}</option>
          {/if}
          {#if !chatgpt.models.length && !settings.assistant.model}<option value="">{m.settings.firstPlanModel}</option>{/if}
        </select>
        {#if chatgpt.problem}<p class="note error">{chatgpt.problem}</p>{/if}
      </div>
    {/if}
  {:else if provider === "openai"}
    <label class="lab" for="ai-url">{m.settings.service}</label>
    <div>
      <input
        id="ai-url"
        class="wide"
        placeholder="https://api.openai.com/v1"
        value={settings.assistant.baseUrl ?? ""}
        oninput={(e) => (settings.assistant.baseUrl = e.currentTarget.value.trim() || null)}
      />
    </div>
    <label class="lab" for="ai-model2">{m.settings.model}</label>
    <div>
      <input
        id="ai-model2"
        class="wide"
        placeholder={m.settings.serviceModel}
        value={settings.assistant.model ?? ""}
        oninput={(e) => (settings.assistant.model = e.currentTarget.value.trim() || null)}
      />
    </div>
  {/if}

  {#if provider !== "local" && !(provider === "anthropic" && settings.assistant.anthropicCliLogin && !appStore)}
    <label class="lab" for="ai-key">{m.settings.apiKey}</label>
    <div class="line">
      {#if hasKey[provider]}
        <span class="ok">{m.settings.savedInKeychain}</span>
        <button onclick={removeKey}>{m.settings.remove}</button>
      {:else}
        <input id="ai-key" class="wide" type="password" bind:value={newKey} placeholder={m.settings.pasteKey} autocomplete="off" />
        <button onclick={saveKey} disabled={!newKey.trim()}>{m.settings.saveKey}</button>
      {/if}
    </div>
  {/if}
  {#if provider === "anthropic" && settings.assistant.anthropicCliLogin && !appStore}
    <span class="lab">{m.settings.account}</span>
    <div class="line">
      {#if !cli?.installed}
        <span class="muted">{m.settings.antMissing[0]} <code>ant</code> {m.settings.antMissing[1]}</span>
      {:else if cli.signedIn}
        <span class="muted">{m.settings.signedIn}</span>
      {:else}
        <span class="muted">{m.settings.notSignedIn}</span>
      {/if}
      {#if cli?.installed}
        <button onclick={cliLogin} disabled={cliBusy}>
          {cliBusy ? m.settings.waitingForBrowser : cli.signedIn ? m.settings.signInAgain : m.settings.signInEllipsis}
        </button>
      {/if}
    </div>
  {/if}

  <span></span>
  <div>
    {#if provider === "chatgpt"}<p class="note">{m.settings.chatgptNote}</p>{/if}
    {#if provider === "local" && !elsewhere}
      <p class="note">{m.settings.localNote}</p>
    {:else}
      <p class="note cloud-note">{m.settings.cloudNote}</p>
    {/if}
    <p class="note"><button class="link" onclick={() => api.openHelp("ai-models")}>{m.settings.howToSetUp}</button></p>
  </div>
</div>
