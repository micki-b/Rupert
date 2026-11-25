<script lang="ts">
  // Settings ▸ Local AI: Apple Intelligence on the Mac, or an OpenAI-compatible
  // server (LM Studio, Ollama…) on this computer or the local network.
  import { onMount } from "svelte";
  import { api, type AppleAiStatus, type LocalAiPlace, type Settings } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let { settings = $bindable() }: { settings: Settings } = $props();

  let models = $state<string[]>([]);
  let lmStatus = $state<"checking" | "ok" | "down">("checking");
  /** Where the address points. */
  let place = $state<LocalAiPlace | null>(null);
  /** Apple Intelligence on this Mac; offered unless it's not supported at all. */
  let apple = $state<AppleAiStatus | null>(null);
  const offerApple = $derived(apple !== null && apple.reason !== "notSupported");
  /** Local AI servers people are likely to run, by their usual address. */
  const PRESETS = [
    { id: "lmstudio", name: "LM Studio", url: "http://localhost:1234" },
    { id: "ollama", name: "Ollama", url: "http://localhost:11434" },
  ];
  const norm = (u: string) => u.trim().replace(/\/+$/, "").replace("127.0.0.1", "localhost");
  const preset = $derived(
    settings.localEngine === "apple" ? "apple" : (PRESETS.find((p) => norm(settings.localUrl) === p.url)?.id ?? "other"),
  );

  onMount(() => {
    checkLm();
    api.appleAiStatus().then(
      (s) => (apple = s),
      () => (apple = null),
    );
  });

  function choosePreset(id: string) {
    if (id === "apple") {
      settings.localEngine = "apple";
      return;
    }
    const p = PRESETS.find((x) => x.id === id);
    if (!p) return;
    settings.localEngine = "server";
    settings.localUrl = p.url;
    settings.localModel = null; // model names differ between servers
    checkLm();
  }

  async function checkLm() {
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
</script>

<div class="form">
  <label class="lab" for="lm-preset">{m.settings.server}</label>
  <div>
    <select id="lm-preset" value={preset} onchange={(e) => choosePreset(e.currentTarget.value)}>
      {#if offerApple || preset === "apple"}<option value="apple">Apple Intelligence</option>{/if}
      {#each PRESETS as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
      <option value="other" disabled={preset !== "other"}>{m.settings.otherServer}</option>
    </select>
  </div>

  {#if settings.localEngine !== "apple"}
    <label class="lab" for="lm-url">{m.settings.address}</label>
    <div class="line">
      <input id="lm-url" class="wide" bind:value={settings.localUrl} onchange={checkLm} />
      <span class="status {lmStatus}">
        {lmStatus === "ok" ? m.settings.connected : lmStatus === "down" ? m.settings.notRunning : m.settings.checking}
      </span>
    </div>
    <label class="lab" for="lm-model">{m.settings.model}</label>
    <div>
      <select
        id="lm-model"
        value={settings.localModel ?? ""}
        onchange={(e) => (settings.localModel = e.currentTarget.value || null)}
      >
        <option value="">{m.settings.loadedModel}</option>
        {#each models as name (name)}<option value={name}>{name}</option>{/each}
        {#if settings.localModel && !models.includes(settings.localModel)}
          <option value={settings.localModel}>{settings.localModel}</option>
        {/if}
      </select>
    </div>
  {/if}

  <span></span>
  <div>
    {#if settings.localEngine === "apple"}
      {#if apple?.available}
        <p class="note">{m.settings.appleWhere}</p>
      {:else}
        <p class="note warn">{apple?.message ?? m.settings.checking}</p>
      {/if}
      <p class="note">{m.settings.appleNote}</p>
    {:else if place?.reach === "elsewhere"}
      <p class="note cloud-note">{m.settings.localWhere.elsewhere(place.host)}</p>
    {:else if place?.reach === "localNetwork"}
      <p class="note">{m.settings.localWhere.localNetwork(place.host)}</p>
    {:else}
      <p class="note">{m.settings.localWhere.thisComputer}</p>
    {/if}
    {#if settings.localEngine !== "apple"}<p class="note">{m.settings.localAiNote}</p>{/if}
  </div>
</div>
