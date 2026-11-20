<script lang="ts">
  // A Markdown document, formatted: in the reader, the gallery, and (small,
  // `compact`) on a large-icon tile. Online-only files aren't read, since
  // that would download them.
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "$lib/api";
  import { renderMarkdown, resolveRel } from "$lib/markdown";
  import { m } from "$lib/i18n";

  let {
    sourceId,
    relPath,
    compact = false,
    onnavigate,
  }: {
    sourceId: string;
    relPath: string;
    /** A tile: only the start, small, and not interactive. */
    compact?: boolean;
    /** A link to another Markdown file in the same source was followed. */
    onnavigate?: (relPath: string) => void;
  } = $props();

  let html = $state<string | null>(null);
  let phase = $state<"loading" | "ready" | "online" | "error">("loading");
  let problem = $state("");

  $effect(() => {
    const [src, rel] = [sourceId, relPath];
    let live = true;
    phase = "loading";
    api
      .readMarkdown(src, rel, compact ? 2000 : undefined)
      .then((text) => {
        if (!live) return;
        if (text === null) phase = "online";
        else {
          html = renderMarkdown(text, src, rel);
          phase = "ready";
        }
      })
      .catch((e) => {
        if (!live) return;
        problem = String(e);
        phase = "error";
      });
    return () => {
      live = false;
    };
  });

  /** Links: web pages in the browser, other Markdown files here, other
   * files in their usual app, anchors within the page. */
  function onclick(e: MouseEvent) {
    const a = (e.target as Element).closest?.("a");
    if (!a || compact) return;
    const href = a.getAttribute("href") ?? "";
    e.preventDefault();
    if (/^(https?|mailto):/i.test(href)) {
      openUrl(href).catch(() => {});
    } else if (href.startsWith("#")) {
      const id = decodeURIComponent(href.slice(1));
      (e.currentTarget as HTMLElement).querySelector(`[id="${CSS.escape(id)}"]`)?.scrollIntoView();
    } else {
      const rel = resolveRel(relPath, href);
      if (!rel) return;
      if (/\.(md|markdown)$/i.test(rel) && onnavigate) onnavigate(rel);
      else api.openEntry(sourceId, rel).catch(() => {});
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="md" class:compact {onclick}>
  {#if phase === "ready" && html !== null}
    <!-- Cleaned by DOMPurify in renderMarkdown. -->
    <!-- eslint-disable-next-line svelte/no-at-html-tags -->
    {@html html}
  {:else if phase === "online"}
    <p class="note">{m.markdown.onlineOnly}</p>
  {:else if phase === "error"}
    <p class="note">{m.markdown.couldntShow(problem)}</p>
  {:else if !compact}
    <p class="note">{m.common.loading}</p>
  {/if}
</div>

<style>
  .md {
    color: var(--text);
    line-height: 1.55;
    font-size: 14px;
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .md :global(h1),
  .md :global(h2),
  .md :global(h3),
  .md :global(h4) {
    line-height: 1.25;
    margin: 1.2em 0 0.45em;
    text-wrap: balance;
  }
  .md :global(h1) {
    font-size: 1.6em;
  }
  .md :global(h2) {
    font-size: 1.3em;
    padding-bottom: 0.2em;
    border-bottom: 1px solid var(--border);
  }
  .md :global(h3) {
    font-size: 1.1em;
  }
  .md :global(:first-child) {
    margin-top: 0;
  }
  .md :global(p),
  .md :global(ul),
  .md :global(ol),
  .md :global(blockquote),
  .md :global(pre),
  .md :global(table) {
    margin: 0 0 0.8em;
  }
  .md :global(ul),
  .md :global(ol) {
    padding-left: 1.5em;
  }
  .md :global(li > input[type="checkbox"]) {
    margin: 0 0.4em 0 -1.3em;
  }
  .md :global(a) {
    color: var(--accent);
  }
  .md :global(code) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.88em;
    background: var(--panel);
    border-radius: 4px;
    padding: 0.1em 0.3em;
  }
  .md :global(pre) {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.7em 0.9em;
    overflow-x: auto;
  }
  .md :global(pre code) {
    background: none;
    padding: 0;
  }
  .md :global(blockquote) {
    border-left: 3px solid var(--accent);
    padding: 0.1em 0 0.1em 0.9em;
    color: var(--muted);
  }
  .md :global(hr) {
    border: 0;
    border-top: 1px solid var(--border);
    margin: 1.2em 0;
  }
  .md :global(table) {
    border-collapse: collapse;
    display: block;
    overflow-x: auto;
  }
  .md :global(th),
  .md :global(td) {
    border: 1px solid var(--border);
    padding: 0.3em 0.6em;
    text-align: left;
  }
  .md :global(th) {
    background: var(--panel);
  }
  .md :global(img) {
    max-width: 100%;
    height: auto;
  }
  .note {
    color: var(--muted);
    margin: 0;
  }

  /* A tile: a little page with the start of the document. */
  .md.compact {
    font-size: 5.5px;
    line-height: 1.35;
    pointer-events: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .md.compact .note {
    font-size: 8px;
  }
</style>
