<script lang="ts">
  // Save Search… and Rename… for saved searches (#110): a name, what the
  // search finds, and which source it's kept with.
  import { m } from "#lib/i18n/index.ts";

  let {
    mode,
    name: initial,
    finds,
    keptWith,
    onsave,
    onclose,
  }: {
    mode: "save" | "rename";
    name: string;
    /** What the search finds, in words. */
    finds: string;
    /** The source's name. */
    keptWith: string;
    onsave: (name: string) => Promise<void>;
    onclose: () => void;
  } = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(initial);
  let working = $state(false);
  let error = $state<string | null>(null);

  async function save() {
    if (!name.trim() || working) return;
    working = true;
    error = null;
    try {
      await onsave(name.trim());
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      working = false;
    }
  }

  function focus(el: HTMLInputElement) {
    el.focus();
    el.select();
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !working && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="save-search-title">
    <h1 id="save-search-title">{mode === "save" ? m.savedSearches.saveTitle : m.savedSearches.renameTitle}</h1>
    <label class="field">
      <span>{m.savedSearches.name}</span>
      <input
        bind:value={name}
        use:focus
        onkeydown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            save();
          }
        }}
      />
    </label>
    <div class="field">
      <span>{m.savedSearches.finds}</span>
      <p>{finds}</p>
    </div>
    <p class="muted small">{m.savedSearches.keptWith(keptWith)}</p>
    {#if error}<p class="small error" role="alert">{error}</p>{/if}
    <div class="footer">
      <span class="grow"></span>
      <button onclick={onclose} disabled={working}>{m.common.cancel}</button>
      <button class="primary" onclick={save} disabled={working || !name.trim()}>
        {mode === "save" ? m.common.save : m.savedSearches.rename}
      </button>
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
    z-index: 40;
  }
  .dialog {
    width: min(420px, calc(100vw - 32px));
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h1 {
    margin: 0;
    font-size: 15px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .field > span {
    font-size: 11px;
    color: var(--muted);
  }
  .field p {
    margin: 0;
    line-height: 1.4;
  }
  input {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 8px;
  }
  p {
    margin: 0;
  }
  .small {
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
  }
  .error {
    color: var(--danger, #c0392b);
  }
  .footer {
    display: flex;
    gap: 8px;
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
  button:disabled {
    opacity: 0.5;
  }
</style>
