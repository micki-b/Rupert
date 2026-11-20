<script lang="ts">
  // Rupert Help, shown in its own window (Help ▸ Rupert Help, F1, or the
  // Help button in the status bar). Written for people using Rupert, not
  // for developers. Keep it in step with the app's labels.
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { invoke } from "@tauri-apps/api/core";
  import { editionLabel, type AppInfo } from "$lib/api";
  import { currentLang, m } from "$lib/i18n";
  import HelpEn, { sections as enSections } from "$lib/help/En.svelte";
  import HelpEs, { sections as esSections } from "$lib/help/Es.svelte";

  const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;
  const mac = /Mac/.test(ua);
  const windows = /Windows/.test(ua);
  /** The command key as people know it on this computer. */
  const cmd = mac ? "⌘" : "Ctrl+";

  const sections = $derived(currentLang() === "es" ? esSections : enSections);
  let current = $state("start");
  let version = $state("");
  const REPO = "https://github.com/micki-b/Rupert";

  onMount(() => {
    invoke<AppInfo>("app_info")
      .then((i) => (version = m.help.version(i.version, editionLabel(i))))
      .catch(() => {});
    const start = (window as unknown as { __rupertHelpSection?: string }).__rupertHelpSection;
    const target = start || location.hash.slice(1);
    if (target) document.getElementById(target)?.scrollIntoView();

    // Highlight the section being read.
    const seen = new Map<string, boolean>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const e of entries) seen.set(e.target.id, e.isIntersecting);
        const first = sections.find((s) => seen.get(s.id));
        if (first) current = first.id;
      },
      { rootMargin: "0px 0px -70% 0px" },
    );
    for (const s of sections) {
      const el = document.getElementById(s.id);
      if (el) observer.observe(el);
    }
    return () => observer.disconnect();
  });

  /** At the very end the last sections can't reach the top, so the
   * highlight goes to the last one. */
  function onscroll(e: Event) {
    const el = e.currentTarget as HTMLElement;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 4) current = sections[sections.length - 1].id;
  }

  /** Web links open in the browser, not in this window. */
  function onclick(e: MouseEvent) {
    const a = (e.target as HTMLElement).closest("a");
    const href = a?.getAttribute("href");
    if (href && /^https?:/.test(href)) {
      e.preventDefault();
      openUrl(href).catch(() => {});
    }
  }
</script>

<svelte:head><title>{m.help.title}</title></svelte:head>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="help" {onclick} {onscroll}>
  <nav aria-label={m.help.contents}>
    <div class="brand">{m.help.title}</div>
    {#each sections as s (s.id)}
      <a href="#{s.id}" class:sub={s.sub} class:current={current === s.id}>{s.title}</a>
    {/each}
  </nav>

  <main>
    {#if currentLang() === "es"}
      <HelpEs {mac} {windows} {cmd} {version} repo={REPO} />
    {:else}
      <HelpEn {mac} {windows} {cmd} {version} repo={REPO} />
    {/if}
  </main>
</div>

<style>
  .help {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    /* The app's stylesheet stops the page body from scrolling (the main
       window lays out its own panes), so this is the help's scroller; the
       contents column stays put with `position: sticky`. */
    height: 100vh;
    overflow-y: auto;
    background: var(--bg);
    color: var(--text);
    user-select: text;
    -webkit-user-select: text;
  }
  nav {
    position: sticky;
    top: 0;
    height: 100vh;
    overflow-y: auto;
    padding: 18px 10px;
    background: var(--panel);
    border-right: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .brand {
    font-weight: 600;
    font-size: 15px;
    padding: 0 8px 12px;
  }
  nav a {
    color: var(--text);
    text-decoration: none;
    padding: 5px 8px;
    border-radius: 6px;
    font-size: 13px;
  }
  nav a.sub {
    padding-left: 20px;
    color: var(--muted);
  }
  nav a:hover {
    background: var(--hover);
  }
  nav a.current {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  main {
    max-width: 720px;
    padding: 28px 36px 120px;
    line-height: 1.55;
    font-size: 14px;
  }
  main :global(section) {
    scroll-margin-top: 16px;
    margin-bottom: 36px;
  }
  main :global(h1) {
    font-size: 24px;
    margin: 0 0 12px;
  }
  main :global(h2) {
    font-size: 19px;
    margin: 0 0 10px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }
  main :global(h3) {
    font-size: 15px;
    margin: 18px 0 6px;
  }
  main :global(h3.sub-h) {
    font-size: 17px;
    margin-top: 0;
  }
  main :global(h4) {
    font-size: 14px;
    margin: 16px 0 4px;
  }
  main :global(#online-only) {
    scroll-margin-top: 16px;
  }
  main :global(p),
  main :global(li) {
    margin: 6px 0;
  }
  main :global(ol),
  main :global(ul) {
    padding-left: 22px;
  }
  main :global(a) {
    color: var(--accent);
  }
  main :global(code),
  main :global(kbd) {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12.5px;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 0 4px;
    overflow-wrap: anywhere;
  }
  main :global(.note) {
    background: var(--panel);
    border-left: 3px solid var(--accent);
    border-radius: 4px;
    padding: 8px 12px;
    margin: 12px 0;
  }
  main :global(.note.cloud-note) {
    border-left-color: var(--cloud);
  }
  main :global(.tip) {
    color: var(--muted);
  }
  main :global(.cloud) {
    color: var(--cloud);
  }
  main :global(.ok) {
    color: #2e8540;
  }
  main :global(table) {
    border-collapse: collapse;
    width: 100%;
    margin: 10px 0;
    font-size: 13px;
  }
  main :global(th),
  main :global(td) {
    text-align: left;
    vertical-align: top;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border);
  }
  main :global(th) {
    color: var(--muted);
    font-weight: 600;
  }
  main :global(table.keys td:first-child) {
    width: 150px;
    white-space: nowrap;
  }
  main :global(dl) {
    margin: 8px 0;
  }
  main :global(dt) {
    font-weight: 600;
    margin-top: 8px;
  }
  main :global(dd) {
    margin: 2px 0 0 0;
  }
  @media (max-width: 640px) {
    .help {
      grid-template-columns: 1fr;
    }
    nav {
      display: none;
    }
    main {
      padding: 20px 16px 80px;
    }
  }
</style>
