<script lang="ts">
  // A name being edited in place (#65): the list, the large icons, the
  // gallery and the sidebar. Return renames, Escape or clicking away without
  // a change cancels; a document's name is selected up to its extension, as
  // in the Finder and File Explorer.
  import { onMount, untrack } from "svelte";

  let {
    name,
    isDir,
    oncommit,
    oncancel,
  }: {
    name: string;
    isDir: boolean;
    /** The name typed (unchanged names never get here). */
    oncommit: (name: string) => void;
    oncancel: () => void;
  } = $props();

  let input: HTMLInputElement | undefined = $state();
  // Starts from the name as it was; later changes to it don't reset typing.
  let value = $state(untrack(() => name));
  let done = false;

  /** Where the extension starts, if it's a document with one. */
  function stemEnd(n: string) {
    const dot = n.lastIndexOf(".");
    return !isDir && dot > 0 ? dot : n.length;
  }

  onMount(() => {
    input?.focus();
    input?.setSelectionRange(0, stemEnd(name));
  });

  function finish(save: boolean) {
    if (done) return;
    done = true;
    let next = value.trim();
    // Typed without an extension: keep the old one. (A new one, such as
    // ".txt", is kept as typed.)
    const ext = name.slice(stemEnd(name));
    if (save && ext && !/\.[A-Za-z0-9]{1,6}$/.test(next)) next += ext;
    if (save && next && next !== name) oncommit(next);
    else oncancel();
  }
</script>

<input
  bind:this={input}
  bind:value
  class="rename-field"
  spellcheck="false"
  autocomplete="off"
  onkeydown={(e) => {
    e.stopPropagation();
    if (e.key === "Enter") {
      e.preventDefault();
      finish(true);
    } else if (e.key === "Escape") {
      e.preventDefault();
      finish(false);
    }
  }}
  onclick={(e) => e.stopPropagation()}
  ondblclick={(e) => e.stopPropagation()}
  onblur={() => finish(true)}
/>

<style>
  .rename-field {
    font: inherit;
    color: var(--text);
    background: var(--bg);
    border: 1px solid var(--accent);
    border-radius: 4px;
    padding: 1px 4px;
    margin: -2px 0;
    width: 100%;
    min-width: 80px;
    box-shadow: 0 0 0 3px var(--accent-soft);
  }
</style>
