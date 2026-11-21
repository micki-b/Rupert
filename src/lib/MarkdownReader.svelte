<script lang="ts">
  // Rupert's reader for Markdown documents: opened by double-clicking one
  // (or Open). Links to other Markdown files in the same source open here
  // too, with Back.
  import { api } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import MarkdownView from "#lib/MarkdownView.svelte";
  import { m } from "#lib/i18n/index.ts";

  let { sourceId, relPath, onclose }: { sourceId: string; relPath: string; onclose: () => void } = $props();

  /** Documents followed by links; the last one is shown. */
  let trail = $state<string[]>([]);
  $effect(() => {
    trail = [relPath];
  });
  const current = $derived(trail.at(-1) ?? relPath);
  const name = $derived(current.split("/").pop() ?? current);

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }
</script>

<svelte:window {onkeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="reader" role="dialog" tabindex="-1" aria-modal="true" aria-label={name} onclick={(e) => e.stopPropagation()}>
    <header>
      {#if trail.length > 1}
        <button class="icon" onclick={() => (trail = trail.slice(0, -1))} title={m.common.back} aria-label={m.common.back}>
          <Icon name="back" size={18} />
        </button>
      {/if}
      <div class="title">
        <strong>{name}</strong>
        <span class="muted">{current.includes("/") ? current.slice(0, current.lastIndexOf("/")) : ""}</span>
      </div>
      <button class="open" onclick={() => api.openEntry(sourceId, current)}>{m.common.openInDefaultApp}</button>
      <button class="icon" onclick={onclose} title={m.common.closeEsc} aria-label={m.common.close}>
        <Icon name="close" size={18} />
      </button>
    </header>
    <div class="page">
      {#key current}
        <MarkdownView {sourceId} relPath={current} onnavigate={(rel) => (trail = [...trail, rel])} />
      {/key}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 40; /* above the chat panel (20) and menus (30) */
    background: rgb(0 0 0 / 0.3);
    display: grid;
    place-items: center;
  }
  .reader {
    width: min(860px, calc(100vw - 48px));
    height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.3);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 10px 18px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .title strong,
  .title span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .muted {
    font-size: 12px;
    color: var(--muted);
  }
  .open {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 12px;
  }
  .icon {
    flex: none;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--muted);
    display: grid;
    place-items: center;
  }
  .icon:hover {
    background: var(--hover);
    color: var(--text);
  }
  .page {
    flex: 1;
    overflow-y: auto;
    padding: 24px 40px 48px;
  }
</style>
