<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";

  type AppInfo = { name: string; version: string; os: string; arch: string };

  let info = $state<AppInfo | null>(null);

  $effect(() => {
    invoke<AppInfo>("app_info").then((i) => (info = i));
  });
</script>

<div class="shell">
  <aside class="sources">
    <h2>Sources</h2>
    <p class="empty">No sources yet. Adding folders comes in milestone 1.</p>
  </aside>

  <main class="documents">
    <h1>Rupert</h1>
    <p class="tagline">The librarian for your scanned documents.</p>
  </main>

  <section class="preview">
    <h2>Preview</h2>
    <p class="empty">Select a document.</p>
  </section>
</div>

<footer>
  {#if info}{info.name} {info.version} · {info.os}/{info.arch}{/if}
</footer>

<style>
  :root {
    --bg: #faf8f5;
    --panel: #f1ede6;
    --text: #2b2723;
    --muted: #7a7168;
    --border: #ddd5ca;
    font-family: system-ui, -apple-system, "Segoe UI", Ubuntu, sans-serif;
    font-size: 14px;
    color: var(--text);
    background: var(--bg);
  }

  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #1e1c1a;
      --panel: #262321;
      --text: #ece6de;
      --muted: #9a9087;
      --border: #3a3531;
    }
  }

  :global(body) {
    margin: 0;
    height: 100vh;
    display: flex;
    flex-direction: column;
    background: var(--bg);
  }

  .shell {
    flex: 1;
    display: grid;
    grid-template-columns: 240px 1fr 320px;
    min-height: 0;
  }

  .sources,
  .preview {
    background: var(--panel);
    padding: 12px 16px;
    overflow: auto;
  }

  .sources {
    border-right: 1px solid var(--border);
  }

  .preview {
    border-left: 1px solid var(--border);
  }

  .documents {
    padding: 24px;
    overflow: auto;
  }

  h1 {
    margin: 0 0 4px;
    font-size: 24px;
  }

  h2 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--muted);
  }

  .tagline,
  .empty {
    color: var(--muted);
  }

  footer {
    padding: 4px 12px;
    font-size: 11px;
    color: var(--muted);
    border-top: 1px solid var(--border);
  }
</style>
