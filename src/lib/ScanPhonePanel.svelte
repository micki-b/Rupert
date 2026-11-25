<script lang="ts">
  // Scan with your phone (#123): what the inbox's Scan… button opens. Rupert
  // doesn't drive scanners itself any more; the phone app scans with the
  // camera into the same inbox. QR codes lead to the app, and a note says
  // when the inbox is somewhere the phone can't reach.
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

  const APP_STORE = "https://apps.apple.com/us/app/rupert-the-librarian/id6836673217";
  // The Play listing isn't public yet: the website's download section.
  const ANDROID = "https://rupertlibrarian.com/#download";

  let codes = $state<{ iphone: string | null; android: string | null }>({ iphone: null, android: null });

  const source = $derived(inbox ? (sources.find((s) => s.id === inbox.sourceId) ?? null) : null);
  const folder = $derived.by(() => {
    if (!inbox || !source) return "";
    const path = [source.name, ...inbox.relPath.split("/").filter(Boolean)].join(" ▸ ");
    return `${path} (${providerLabel(source.provider)})`;
  });

  onMount(async () => {
    const [iphone, android] = await Promise.all([api.qrSvg(APP_STORE), api.qrSvg(ANDROID)]).catch(() => [null, null]);
    codes = { iphone, android };
  });
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
    {:else if source.provider === "local"}
      <div class="note">
        <p>{m.scanPhone.localInbox}</p>
        <button onclick={onsettings}>{m.scanPhone.inboxSettings}</button>
      </div>
    {/if}

    <ol>
      <li>
        {m.scanPhone.getApp}
        <div class="codes">
          {#each [{ label: m.scanPhone.iphone, svg: codes.iphone, url: APP_STORE }, { label: m.scanPhone.android, svg: codes.android, url: ANDROID }] as c (c.url)}
            <figure>
              <div class="code" title={c.url}>
                <!-- Made by rupert_core::qr from a fixed link. -->
                {#if c.svg}{@html c.svg}{/if}
              </div>
              <figcaption>{c.label}</figcaption>
            </figure>
          {/each}
        </div>
      </li>
      {#if inbox && source && source.provider !== "local"}
        <li>
          {m.scanPhone.chooseInbox(folder)}
          {#if source.provider === "icloud"}<p class="muted small">{m.scanPhone.icloudInbox}</p>{/if}
        </li>
      {/if}
    </ol>

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
    width: min(500px, calc(100vw - 32px));
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
  p {
    margin: 0;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
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
  ol {
    margin: 14px 0 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  li p {
    margin-top: 4px;
  }
  .codes {
    display: flex;
    gap: 24px;
    margin-top: 10px;
  }
  figure {
    margin: 0;
    text-align: center;
  }
  .code {
    width: 132px;
    height: 132px;
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
  figcaption {
    margin-top: 4px;
    font-size: 12px;
    font-weight: 600;
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
