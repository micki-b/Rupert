<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { onMenuUndo } from "#lib/api.ts";
  let { children } = $props();

  // Edit ▸ Undo (macOS menu) is Rupert's own item: in the focused window it
  // undoes typing in a text field, or else tells the page ("rupert-undo"),
  // which undoes the last renaming of documents (#64).
  onMount(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;
    const off = onMenuUndo(() => {
      if (!document.hasFocus()) return;
      const el = document.activeElement;
      if (el instanceof HTMLElement && el.closest("input, textarea, [contenteditable]")) {
        document.execCommand("undo");
      } else {
        window.dispatchEvent(new CustomEvent("rupert-undo"));
      }
    });
    return () => {
      off.then((f) => f());
    };
  });
</script>

{@render children()}
