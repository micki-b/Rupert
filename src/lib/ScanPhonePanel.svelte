<script lang="ts">
  // Scan with your phone (#123, #124): what the inbox's Scan… button opens.
  // On a Mac, Continuity Camera first: a nearby iPhone or iPad scans
  // straight into the inbox, no app needed. Then the Rupert phone app: one
  // QR code that opens its Scan tab (or shows where to get it), the inbox
  // folder to choose there, and a note when the phone can't reach the inbox.
  import { onMount } from "svelte";
  import { api, providerLabel, type InboxLocation, type Source } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    inbox,
    sources,
    onclose,
    onsettings,
  }: {
    inbox: InboxLocation | null;
    sources: Source[];
    onclose: () => void;
    onsettings: () => void;
  } = $props();

  /** A universal link (iPhone) and App Link (Android) into the phone app's
   * Scan tab; in a browser, the page sends people to the app's store. */
  const SCAN_LINK = "https://rupertlibrarian.com/scan/";

  let code = $state<string | null>(null);
  let mac = $state(false);

  const source = $derived(inbox ? (sources.find((s) => s.id === inbox.sourceId) ?? null) : null);
  const folder = $derived.by(() => {
    if (!inbox || !source) return "";
    const path = [source.name, ...inbox.relPath.split("/").filter(Boolean)].join(" ▸ ");
    return `${path} (${providerLabel(source.provider)})`;
  });

  onMount(() => {
    api.qrSvg(SCAN_LINK).then((svg) => (code = svg), () => {});
    api.appInfo().then((i) => (mac = i.os === "macos"), () => {});
  });

  function continuity(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    api.continuityMenu(r.left, r.bottom + 4).catch(() => {});
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="scan-phone-title">
    <h1 id="scan-phone-title">{m.scanPhone.title}</h1>
    <p class="muted">{m.scanPhone.intro}</p>

    {#if !inbox || !source}
      <div class="note">
        <p>{m.scanPhone.noInbox}</p>
        <button onclick={onsettings}>{m.scanPhone.inboxSettings}</button>
      </div>
    {:else}
      {#if mac}
        <section>
          <h2>{m.scanPhone.continuityTitle}</h2>
          <p class="muted small">{m.scanPhone.continuityNote}</p>
          <button class="primary" onclick={continuity}>{m.scanPhone.continuityButton}</button>
        </section>
      {/if}

      <section>
        {#if mac}<h2>{m.scanPhone.appTitle}</h2>{/if}
        {#if source.provider === "local"}
          <div class="note">
            <p>{m.scanPhone.localInbox}</p>
            <button onclick={onsettings}>{m.scanPhone.inboxSettings}</button>
          </div>
        {:else}
          <div class="app">
            <div class="code" title={SCAN_LINK}>
              <!-- Made by rupert_core::qr from a fixed link. -->
              {#if code}{@html code}{/if}
            </div>
            <div class="steps">
              <p>{m.scanPhone.pointCamera}</p>
              <p>{m.scanPhone.chooseInbox(folder)}</p>
              {#if source.provider === "icloud"}<p class="muted small">{m.scanPhone.icloudInbox}</p>{/if}
            </div>
          </div>
        {/if}
      </section>
    {/if}

    <p class="muted small scanners">{m.scanPhone.scannerApps}</p>

    <div class="footer">
      <button onclick={() => api.openHelp("scanning")}>{m.main.help}</button>
      <span class="grow"></span>
      <button class="primary" onclick={onclose}>{m.common.done}</button>
    </div>
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
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
  }
  h1 {
    margin: 0 0 6px;
    font-size: 15px;
  }
  h2 {
    margin: 0 0 4px;
    font-size: 13px;
  }
  p {
    margin: 0;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  section {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  .note {
    margin-top: 12px;
    padding: 10px 12px;
    border-radius: 8px;
    background: var(--accent-soft);
    color: var(--text);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }
  section .note {
    margin-top: 0;
  }
  /* The code centered in the window, alone, so the phone's camera finds
     it easily; the steps below. */
  section:has(.app) {
    align-items: stretch;
  }
  .app {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }
  .steps {
    align-self: stretch;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .code {
    flex: none;
    width: 152px;
    height: 152px;
    border-radius: 6px;
    overflow: hidden;
    background: #fff;
    border: 1px solid var(--border);
  }
  .code :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }
  .scanners {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 16px;
  }
  .grow {
    flex: 1;
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 12px;
    font: inherit;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
    font-weight: 600;
  }
</style>
