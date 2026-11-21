<script lang="ts">
  // Scan from a network (eSCL / AirScan) scanner straight into the inbox.
  import { onMount } from "svelte";
  import {
    api,
    onScanDone,
    onScanProgress,
    type Capabilities,
    type InputSource,
    type PageSize,
    type ScanDone,
    type ScanRequest,
    type Scanner,
  } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let {
    hasInbox,
    onclose,
    onscanned,
    onsettings,
  }: {
    hasInbox: boolean;
    onclose: () => void;
    onscanned: (done: ScanDone) => void;
    onsettings: () => void;
  } = $props();

  const STORE_KEY = "rupert.scan";
  type Saved = { scanner: Scanner | null; known: Scanner[]; request: ScanRequest };

  let found = $state<Scanner[]>([]);
  let known = $state<Scanner[]>([]);
  let searching = $state(true);
  let scanner = $state<Scanner | null>(null);
  let caps = $state<Capabilities | null>(null);
  let capsError = $state<string | null>(null);
  let address = $state("");
  let request = $state<ScanRequest>({
    source: "feeder",
    duplex: true,
    color: "gray",
    resolution: 300,
    pageSize: "letter",
  });
  let scanning = $state(false);
  let pages = $state(0);
  let error = $state<string | null>(null);

  const all = $derived([...known, ...found.filter((f) => !known.some((k) => k.url === f.url))]);
  const sourceCaps = $derived(caps ? (request.source === "feeder" ? caps.feeder : caps.platen) : null);

  onMount(() => {
    try {
      const saved: Saved = JSON.parse(localStorage.getItem(STORE_KEY) ?? "null");
      if (saved) {
        known = saved.known ?? [];
        request = { ...request, ...saved.request };
        if (saved.scanner) choose(saved.scanner);
      }
    } catch {
      /* first time */
    }
    api
      .scannersDiscover()
      .then((list) => {
        found = list;
        if (!scanner && list.length) choose(list[0]);
      })
      .catch(() => {})
      .finally(() => (searching = false));
    const unlisten = [
      onScanProgress((n) => (pages = n)),
      onScanDone((done) => {
        scanning = false;
        if (done.error) error = done.error;
        else {
          onscanned(done);
          onclose();
        }
      }),
    ];
    return () => unlisten.forEach((u) => u.then((f) => f()));
  });

  async function choose(s: Scanner) {
    scanner = s;
    caps = null;
    capsError = null;
    try {
      const c = await api.scannerCapabilities(s);
      if (scanner?.url !== s.url) return;
      caps = c;
      // Keep the choices valid for this scanner.
      if (request.source === "feeder" && !c.feeder) request.source = "platen";
      if (request.source === "platen" && !c.platen) request.source = "feeder";
      const res = (request.source === "feeder" ? c.feeder : c.platen)?.resolutions ?? [];
      if (res.length && !res.includes(request.resolution)) {
        request.resolution = res.reduce((a, b) => (Math.abs(b - 300) < Math.abs(a - 300) ? b : a));
      }
    } catch (e) {
      capsError = String(e);
    }
  }

  async function addByAddress() {
    if (!address.trim()) return;
    const s = await api.scannerAt(address.trim());
    if (!known.some((k) => k.url === s.url)) known = [...known, s];
    address = "";
    choose(s);
  }

  function save() {
    try {
      const saved: Saved = { scanner, known, request: $state.snapshot(request) };
      localStorage.setItem(STORE_KEY, JSON.stringify(saved));
    } catch {
      /* private mode */
    }
  }

  /** "Scan 2026-11-18 15.04.12", in local time. */
  function scanName(): string {
    const d = new Date();
    const p = (n: number) => String(n).padStart(2, "0");
    return `Scan ${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}.${p(d.getMinutes())}.${p(d.getSeconds())}`;
  }

  async function start() {
    if (!scanner) return;
    save();
    error = null;
    pages = 0;
    scanning = true;
    try {
      await api.scanStart(scanner, $state.snapshot(request), scanName());
    } catch (e) {
      scanning = false;
      error = String(e);
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !scanning) onclose();
  }
</script>

<svelte:window {onkeydown} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="scan-title">
    <h1 id="scan-title">{m.scan.title}</h1>
    {#if !hasInbox}
      <p>{m.scan.needsInbox}</p>
      <div class="actions">
        <span class="spacer"></span>
        <button onclick={onclose}>{m.common.cancel}</button>
        <button class="primary" onclick={onsettings}>{m.scan.setUpInbox}</button>
      </div>
    {:else}
      <section>
        <h2>{m.scan.scanner}</h2>
        <ul class="scanners">
          {#each all as s (s.url)}
            <li>
              <label>
                <input type="radio" name="scanner" checked={scanner?.url === s.url} onchange={() => choose(s)} />
                {s.name}
              </label>
            </li>
          {/each}
        </ul>
        {#if searching}
          <p class="muted small">{m.scan.looking}</p>
        {:else if all.length === 0}
          <p class="muted small">{m.scan.noneFound}</p>
        {/if}
        <div class="row">
          <input
            placeholder={m.scan.addressPlaceholder}
            bind:value={address}
            onkeydown={(e) => e.key === "Enter" && addByAddress()}
          />
          <button onclick={addByAddress} disabled={!address.trim()}>{m.scan.add}</button>
        </div>
        {#if capsError}<p class="error small">{capsError}</p>{/if}
      </section>

      {#if caps}
        <section>
          <h2>{m.scan.settings} {#if caps.makeAndModel}<span class="muted">· {caps.makeAndModel}</span>{/if}</h2>
          <div class="grid">
            <span class="label">{m.scan.source}</span>
            <div class="seg">
              {#each [["feeder", m.scan.feeder], ["platen", m.scan.glass]] as [value, label] (value)}
                {#if value === "feeder" ? caps.feeder : caps.platen}
                  <label
                    ><input type="radio" bind:group={request.source} value={value as InputSource} />
                    {label}</label
                  >
                {/if}
              {/each}
            </div>
            {#if request.source === "feeder" && caps.duplex}
              <span class="label">{m.scan.sides}</span>
              <label><input type="checkbox" bind:checked={request.duplex} /> {m.scan.bothSides}</label>
            {/if}
            <span class="label">{m.scan.color}</span>
            <div class="seg">
              <label><input type="radio" bind:group={request.color} value="gray" /> {m.scan.grayscale}</label>
              {#if sourceCaps?.colorModes.includes("RGB24")}
                <label><input type="radio" bind:group={request.color} value="color" /> {m.scan.inColor}</label>
              {/if}
            </div>
            <label class="label" for="res">{m.scan.resolution}</label>
            <select id="res" bind:value={request.resolution}>
              {#each sourceCaps?.resolutions ?? [] as r (r)}<option value={r}>{m.scan.dpi(r)}</option>{/each}
            </select>
            <label class="label" for="size">{m.scan.pageSize}</label>
            <select id="size" bind:value={request.pageSize}>
              {#each [["letter", m.scan.letter], ["legal", m.scan.legal], ["a4", m.scan.a4], ["max", m.scan.max]] as [v, l] (v)}
                <option value={v as PageSize}>{l}</option>
              {/each}
            </select>
          </div>
          <p class="muted small">{m.scan.hint}</p>
        </section>
      {/if}

      {#if error}<p class="error">{error}</p>{/if}
      <div class="actions">
        {#if scanning}
          <span class="progress"><span class="spinner"></span>{m.scan.scanning} {pages ? m.scan.pages(pages) : ""}</span>
        {/if}
        <span class="spacer"></span>
        <button onclick={onclose} disabled={scanning}>{m.common.cancel}</button>
        <button class="primary" disabled={!caps || scanning} onclick={start}>{m.scan.scan}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.3);
    display: grid;
    place-items: center;
    z-index: 40; /* above the chat panel (20) and menus (30) */
  }
  .dialog {
    width: min(520px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    overflow: auto;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 10px 40px rgb(0 0 0 / 0.25);
    padding: 18px 20px;
  }
  h1 {
    font-size: 16px;
    margin: 0 0 6px;
  }
  h2 {
    font-size: 11px;
    margin: 16px 0 6px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
  }
  p {
    margin: 6px 0;
    line-height: 1.4;
  }
  .muted {
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
  .scanners {
    list-style: none;
    margin: 0 0 6px;
    padding: 0;
  }
  .scanners li {
    padding: 2px 0;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row input {
    flex: 1;
  }
  .grid {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 8px 10px;
    align-items: center;
  }
  .label {
    font-size: 12px;
    color: var(--muted);
  }
  .seg {
    display: flex;
    gap: 14px;
  }
  input:not([type="checkbox"]):not([type="radio"]),
  select {
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    background: var(--bg);
    color: var(--text);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 18px;
  }
  .spacer {
    flex: 1;
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  button {
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--text);
    border-radius: 6px;
    padding: 4px 12px;
  }
  button.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }
  button:disabled {
    opacity: 0.5;
  }
  .error {
    color: #c0392b;
  }
</style>
