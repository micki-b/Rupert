<script lang="ts">
  // Settings ▸ Text recognition: OCR for scanned PDFs, on this computer.
  import { onMount } from "svelte";
  import { api, type Settings } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let { settings = $bindable() }: { settings: Settings } = $props();

  /** The OCR engine found, null if none, undefined while checking. */
  let engine = $state<string | null | undefined>(undefined);
  onMount(async () => {
    engine = await api.ocrEngine().catch(() => null);
  });
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
    {:else}
      <p class="note">{m.settings.ocrNote}</p>
    {/if}
  </div>
</div>
