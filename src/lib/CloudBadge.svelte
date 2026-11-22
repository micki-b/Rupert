<script lang="ts">
  // The cloud on an online-only document. Clicking it downloads that one file,
  // so Rupert can read it; it pulses meanwhile.
  import Icon from "#lib/Icon.svelte";
  import { isBusy } from "#lib/doc-actions.svelte.ts";
  import type { Entry } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    sourceId,
    doc,
    size = 14,
    round = false,
    onclick,
  }: { sourceId: string; doc: Entry; size?: number; round?: boolean; onclick: () => void } = $props();

  const busy = $derived(isBusy(sourceId, doc.relPath));
  const title = $derived(busy ? m.main.downloadingFile : m.main.onlineOnlyTip);
</script>

<button
  type="button"
  class="cloud-badge"
  class:round
  class:busy
  {title}
  aria-label={title}
  draggable="false"
  onclick={(ev) => {
    ev.stopPropagation();
    onclick();
  }}
  ondblclick={(ev) => ev.stopPropagation()}
>
  <Icon name="cloud" {size} />
</button>

<style>
  .cloud-badge {
    flex: none;
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    padding: 1px;
    border-radius: 4px;
    color: var(--cloud);
    cursor: pointer;
  }
  .cloud-badge:hover {
    background: var(--hover);
  }
  .cloud-badge.round {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--bg);
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.25);
  }
  .cloud-badge.busy {
    pointer-events: none;
    animation: pulse 1s ease-in-out infinite alternate;
  }
  @keyframes pulse {
    to {
      opacity: 0.35;
    }
  }
</style>
