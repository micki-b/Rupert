<script lang="ts">
  // A Notes field that's made taller or shorter by its corner handle, never
  // wider than its column (#127). The handle is the system's own, but an
  // invisible grip over it does the dragging: the box grows by exactly as
  // much as the pointer moves. (The browser keeps the box's edge under the
  // pointer, and with the page preview above giving way as the form grows,
  // the box slid up and grew twice as fast as the drag.)
  import { m } from "#lib/i18n/index.ts";

  let {
    value = $bindable(""),
    rows = 2,
    disabled = false,
    title,
  }: {
    value?: string;
    rows?: number;
    disabled?: boolean;
    title?: string;
  } = $props();

  let box: HTMLTextAreaElement | undefined = $state();
  /** The height chosen by dragging (null: from `rows`). */
  let height = $state<number | null>(null);
  let drag: { y: number; h: number } | null = null;

  /** Never taller than this share of the window, so the grip stays in reach. */
  const MAX_SHARE = 0.4;

  function down(ev: PointerEvent) {
    if (ev.button !== 0 || !box) return;
    ev.preventDefault();
    (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
    drag = { y: ev.clientY, h: box.offsetHeight };
    document.body.classList.add("resizing-rows");
  }

  function move(ev: PointerEvent) {
    if (!drag || !box) return;
    const min = parseFloat(getComputedStyle(box).lineHeight) * 1.5 + 10 || 40;
    const max = Math.max(min, window.innerHeight * MAX_SHARE);
    height = Math.round(Math.min(max, Math.max(min, drag.h + ev.clientY - drag.y)));
  }

  function up() {
    drag = null;
    document.body.classList.remove("resizing-rows");
  }
</script>

<div class="notes">
  <textarea bind:this={box} bind:value {rows} {disabled} {title} style:height={height === null ? null : `${height}px`}></textarea>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    class="grip"
    title={m.preview.notesResizeTip}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    ondblclick={() => (height = null)}
  ></span>
</div>

<style>
  .notes {
    position: relative;
    display: grid;
    min-width: 0;
  }
  textarea {
    resize: vertical;
    width: 100%;
    box-sizing: border-box;
    max-height: 40vh;
  }
  /* Over the system's handle (which shows through), so the browser's own
     resizing never starts. */
  .grip {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 16px;
    height: 16px;
    touch-action: none;
  }
</style>
