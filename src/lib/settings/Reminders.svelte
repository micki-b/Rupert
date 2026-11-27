<script lang="ts">
  // Settings ▸ Reminders (#109): the whole feature on or off, how early
  // documents show in Coming up, notifications and the Dock badge.
  import { onMount } from "svelte";
  import { api, LEADS, type Lead, type NotifyPermission, type Settings } from "#lib/api.ts";
  import { m } from "#lib/i18n/index.ts";

  let { settings = $bindable(), os }: { settings: Settings; os: string | undefined } = $props();

  const r = $derived(settings.reminders);
  const mac = $derived(os === "macos");
  const windows = $derived(os === "windows");
  let permission = $state<NotifyPermission | null>(null);

  onMount(async () => {
    permission = await api.notificationsPermission().catch(() => "unavailable" as const);
  });

  /** Turning notifications on asks macOS once. */
  async function setNotify(on: boolean) {
    settings.reminders.notify = on;
    if (on && permission === "notDetermined") {
      permission = await api.notificationsRequest().catch(() => permission);
    }
  }

  const leads = (keep: Lead[]) => LEADS.filter((l) => keep.includes(l));
</script>

<div class="form">
  <span class="lab">{m.settings.reminders}</span>
  <div>
    <label class="check">
      <input type="checkbox" bind:checked={settings.reminders.enabled} />
      {m.reminders.enable}
    </label>
    <p class="note">{m.reminders.enableNote}</p>
  </div>

  {#if r.enabled}
    <hr />
    <span class="lab">{m.reminders.howEarly}</span>
    <div class="grid">
      <label for="lead-bills">{m.reminders.bills}</label>
      <select id="lead-bills" bind:value={settings.reminders.bills}>
        {#each leads(["0d", "3d", "1w", "2w", "1m"]) as l (l)}<option value={l}>{m.reminders.leads[l]}</option>{/each}
      </select>
      <label for="lead-other">{m.reminders.others}</label>
      <select id="lead-other" bind:value={settings.reminders.other}>
        {#each leads(["1w", "2w", "1m", "3m", "6m"]) as l (l)}<option value={l}>{m.reminders.leads[l]}</option>{/each}
      </select>
      <p class="note span">{m.reminders.perDoc}</p>
    </div>

    <hr />
    <span class="lab">{m.reminders.notifications}</span>
    <div>
      {#if permission === "unavailable"}
        <p class="note">{m.reminders.unavailable}</p>
      {:else}
        <label class="check">
          <input type="checkbox" checked={r.notify} onchange={(e) => setNotify(e.currentTarget.checked)} />
          {m.reminders.notify}
        </label>
        <div class="kinds" class:off={!r.notify}>
          <label class="check">
            <input type="radio" name="kind" value="summary" bind:group={settings.reminders.notifyKind} disabled={!r.notify} />
            {m.reminders.summary}
          </label>
          <label class="check">
            <input type="radio" name="kind" value="each" bind:group={settings.reminders.notifyKind} disabled={!r.notify} />
            {m.reminders.each}
          </label>
          {#if mac && r.notifyKind === "each"}<p class="note indent">{m.reminders.eachMac}</p>{/if}
          <label class="line">
            {m.reminders.at}
            <input type="time" bind:value={settings.reminders.notifyTime} disabled={!r.notify} />
          </label>
        </div>
        {#if mac && permission === "denied" && r.notify}
          <p class="error">{m.reminders.denied}</p>
        {/if}
        <p class="note">{mac ? m.reminders.closedMac : windows ? m.reminders.closedWindows : ""}</p>
        <button onclick={() => api.notificationsOpenSettings()}>{m.reminders.openSettings}</button>
      {/if}
      {#if mac}
        <label class="check badge">
          <input type="checkbox" bind:checked={settings.reminders.dockBadge} />
          {m.reminders.dockBadge}
        </label>
        <p class="note">{m.reminders.dockBadgeNote}</p>
      {/if}
      <p class="note">{m.reminders.calendarNote}</p>
    </div>

    <hr />
    <span class="lab"></span>
    <p class="note">{m.reminders.everywhere}</p>
  {/if}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: minmax(0, max-content) minmax(0, 170px);
    gap: 8px 12px;
    align-items: center;
  }
  /* Over the Settings window's 220 px minimum for fields. */
  .grid select[id] {
    width: 100%;
    min-width: 0;
  }
  .span {
    grid-column: 1 / -1;
  }
  .kinds {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 6px 0 4px 22px;
  }
  .kinds.off {
    opacity: 0.55;
  }
  .indent {
    margin: -2px 0 0 22px;
  }
  .badge {
    margin-top: 14px;
  }
  .error {
    color: var(--danger);
  }
</style>
