<script lang="ts">
  // Settings ▸ Text recognition: OCR for scanned PDFs, on this computer.
  // On Windows without Tesseract, the winget command to install it, with a
  // copy button, and Check again (Rupert looks where winget puts it).
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api, type Settings } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let { settings = $bindable(), os }: { settings: Settings; os: string | undefined } = $props();

  /** The OCR engine found, null if none, undefined while checking. */
  let engine = $state<string | null | undefined>(undefined);
  let checking = $state(false);
  let copied = $state<string | null>(null);

  // UB Mannheim's build, the one Tesseract's own documentation points
  // Windows users to; it installs into C:\Program Files\Tesseract-OCR.
  const WINGET = "winget install --id UB-Mannheim.TesseractOCR -e";
  const WINGET_LANGUAGES = `${WINGET} --interactive`;
  const MANUAL = "https://github.com/UB-Mannheim/tesseract/wiki";

  onMount(check);

  async function check() {
    checking = true;
    engine = await api.ocrEngine().catch(() => null);
    checking = false;
  }

  async function copy(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copied = text;
      setTimeout(() => copied === text && (copied = null), 2000);
    } catch {
      copied = null;
    }
  }
</script>

<div class="form">
  <span class="lab">{m.settings.textRecognition}</span>
  <div>
    <label class="check">
      <input type="checkbox" bind:checked={settings.ocr} disabled={!engine} />
      {m.settings.recognize}{#if engine}&nbsp;<span class="muted">({engine})</span>{/if}
    </label>
    {#if engine === null}
      <p class="note">{m.settings.noEngine}</p>
      {#if os === "windows"}
        <p class="note">{m.settings.wingetIntro}</p>
        <div class="cmd">
          <code>{WINGET}</code>
          <button onclick={() => copy(WINGET)}>{copied === WINGET ? m.settings.copied : m.settings.copy}</button>
        </div>
        <p class="note">{m.settings.wingetLanguages}</p>
        <div class="cmd">
          <code>{WINGET_LANGUAGES}</code>
          <button onclick={() => copy(WINGET_LANGUAGES)}
            >{copied === WINGET_LANGUAGES ? m.settings.copied : m.settings.copy}</button
          >
        </div>
        <div class="line">
          <button onclick={check} disabled={checking}>{checking ? m.settings.checking : m.settings.checkAgain}</button>
          <button class="link" onclick={() => openUrl(MANUAL).catch(() => {})}>{m.settings.wingetManual}</button>
        </div>
      {/if}
    {:else}
      <p class="note">{m.settings.ocrNote}</p>
    {/if}
  </div>
</div>

<style>
  .cmd {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 6px 0 8px;
    max-width: 56ch;
  }
  code {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    white-space: nowrap;
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px;
    padding: 5px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--panel);
    user-select: all;
  }
  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-strong);
    text-decoration: underline;
    cursor: pointer;
  }
</style>
