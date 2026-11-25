<script lang="ts">
  // Settings ▸ General: Rupert's language, and updates (not in the Mac App
  // Store edition, which the App Store updates).
  import { onMount } from "svelte";
  import type { Settings } from "#lib/api.ts";
  import { updates } from "#lib/updates.svelte.ts";
  import { LANGS, languageNames, loadLanguage, m, type Lang } from "#lib/i18n/index.ts";

  let { settings = $bindable(), appStore }: { settings: Settings; appStore: boolean } = $props();

  /** The system's language, which "Automatic" follows. */
  let systemLang = $state<Lang>("en");
  onMount(() => {
    loadLanguage().then((l) => l && (systemLang = l.system));
  });
</script>

<div class="form">
  <label class="lab" for="language">{m.settings.language}</label>
  <div>
    <select
      id="language"
      value={settings.language ?? ""}
      onchange={(e) => (settings.language = e.currentTarget.value || null)}
    >
      <option value="">{m.settings.automatic(languageNames[systemLang])}</option>
      {#each LANGS as lang (lang)}<option value={lang}>{languageNames[lang]}</option>{/each}
    </select>
    <p class="note">{m.settings.languageNote}</p>
  </div>

  {#if !appStore}
    <hr />
    <span class="lab">{m.settings.updates}</span>
    <div>
      <label class="check">
        <input type="checkbox" bind:checked={settings.checkUpdates} />
        {m.settings.checkAutomatically}
      </label>
      <div class="line">
        <button
          onclick={() => updates.check(true)}
          disabled={updates.status === "checking" || updates.status === "downloading"}
        >
          {updates.status === "checking" ? m.settings.checking : m.settings.checkNow}
        </button>
        {#if updates.status === "none"}
          <span class="ok">{m.settings.upToDate}</span>
        {:else if updates.status === "available"}
          <span>{m.settings.available(updates.version ?? "")}</span>
          <button class="primary" onclick={() => updates.install()}>{m.settings.installRestart}</button>
        {:else if updates.status === "downloading"}
          <span class="note">
            {m.settings.downloading(updates.progress !== null ? Math.round(updates.progress * 100) : null)}
          </span>
        {:else if updates.status === "error"}
          <span class="error">{m.settings.couldntCheck(updates.problem ?? "")}</span>
        {/if}
      </div>
      <p class="note">{m.settings.updatesNote}</p>
    </div>
  {/if}
</div>
