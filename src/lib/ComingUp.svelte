<script lang="ts">
  // Coming up (#109): documents with a due date, soonest first, in groups
  // (overdue, the next 7 and 30 days, later ones whose reminder has started).
  // Done and "remind me later" are saved in the document at once.
  import { untrack } from "svelte";
  import { api, type Hit, type ReminderCounts, type ReminderGroup, type ReminderItem, type ReminderShow } from "#lib/api.ts";
  import Icon from "#lib/Icon.svelte";
  import { m } from "#lib/i18n/index.ts";
  import { formatIsoDay, formatMoney } from "#lib/format.ts";

  let {
    tick,
    selectedKey,
    onselect,
    onopen,
    onshowinfolder,
    oncounts,
    focus = null,
    onfocused,
  }: {
    /** Changes when the index does: the list is read again. */
    tick: number;
    selectedKey: string | null;
    onselect: (h: Hit) => void;
    onopen: (h: Hit) => void;
    onshowinfolder: (h: Hit) => void;
    oncounts: (c: ReminderCounts) => void;
    /** A document to select once listed (a notification was clicked). */
    focus?: string | null;
    onfocused?: () => void;
  } = $props();

  const SHOW_KEY = "rupert.comingUpShow";
  let show = $state<ReminderShow>(savedShow());
  let items = $state<ReminderItem[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);
  let busy = $state<string | null>(null);
  let menu = $state<{ item: ReminderItem; x: number; y: number } | null>(null);

  function savedShow(): ReminderShow {
    try {
      const s = localStorage.getItem(SHOW_KEY);
      if (s === "all" || s === "done" || s === "reminding") return s;
    } catch {
      // Only a convenience.
    }
    return "reminding";
  }

  function setShow(s: ReminderShow) {
    show = s;
    try {
      localStorage.setItem(SHOW_KEY, s);
    } catch {
      // Only a convenience.
    }
    load();
  }

  async function load() {
    try {
      const r = await api.remindersList(show);
      items = r.items;
      loaded = true;
      oncounts(r.counts);
      error = null;
      takeFocus();
    } catch (e) {
      error = String(e);
    } finally {
      loaded = true;
    }
  }

  $effect(() => {
    void tick;
    untrack(load);
  });

  const key = (i: ReminderItem) => `${i.sourceId}\n${i.relPath}`;

  function takeFocus() {
    if (!focus || !loaded) return;
    const i = items.find((x) => key(x) === focus);
    if (i) onselect(hit(i));
    onfocused?.();
  }

  $effect(() => {
    if (focus) untrack(takeFocus);
  });

  /** The item as a search hit, for Details and opening. */
  function hit(i: ReminderItem): Hit {
    return {
      sourceId: i.sourceId,
      relPath: i.relPath,
      name: i.name,
      isDir: false,
      kind: i.kind,
      size: i.size,
      modifiedMs: i.modifiedMs,
      cloud: i.cloud,
      title: i.title,
      tags: [],
      documentDate: null,
      favorite: false,
      snippet: null,
      sender: i.sender,
      amount: i.amount,
      currency: i.currency,
      dueDate: i.dueDate,
      done: i.done,
    };
  }

  const ORDER: ReminderGroup[] = ["overdue", "week", "month", "later", "done"];
  const groups = $derived(
    ORDER.map((g) => ({ g, items: items.filter((i) => i.group === g) })).filter((x) => x.items.length),
  );

  function groupName(g: ReminderGroup): string {
    const r = m.reminders;
    return { overdue: r.overdue, week: r.week, month: r.month, later: show === "all" ? r.laterAll : r.later, done: r.doneGroup }[g];
  }

  function when(i: ReminderItem): string {
    const r = m.reminders;
    if (i.days < 0) return r.daysAgo(-i.days);
    if (i.days === 0) return r.today;
    if (i.days === 1) return r.tomorrow;
    if (i.days < 60) return r.inDays(i.days);
    return r.inMonths(Math.round(i.days / 30.4));
  }

  async function act(i: ReminderItem, work: () => Promise<unknown>) {
    busy = key(i);
    menu = null;
    try {
      await work();
      await load();
    } catch (e) {
      error = String(e);
    } finally {
      busy = null;
    }
  }

  const setDone = (i: ReminderItem, done: boolean) => act(i, () => api.remindersSetDone(i.sourceId, i.relPath, done));
  const snooze = (i: ReminderItem, days: number | null) => act(i, () => api.remindersSnooze(i.sourceId, i.relPath, days));
  const calendar = (i: ReminderItem) => act(i, () => api.remindersAddToCalendar(i.sourceId, i.relPath));

  function openMenu(e: MouseEvent, i: ReminderItem) {
    e.stopPropagation();
    const b = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { item: i, x: Math.max(8, b.right - 220), y: b.bottom + 4 };
  }

  const snoozedNow = (i: ReminderItem) => !!i.snoozed && i.snoozed > new Date().toLocaleDateString("sv");
</script>

<svelte:window
  onclick={() => (menu = null)}
  onkeydown={(e) => {
    if (e.key === "Escape") menu = null;
  }}
/>

<section class="coming">
  <header>
    <h1>{m.reminders.title}</h1>
    <span class="muted">{m.reminders.intro}</span>
    <span class="grow"></span>
    <label class="pick">
      <span class="muted">{m.reminders.show}</span>
      <select value={show} onchange={(e) => setShow(e.currentTarget.value as ReminderShow)}>
        <option value="reminding">{m.reminders.showReminding}</option>
        <option value="all">{m.reminders.showAll}</option>
        <option value="done">{m.reminders.showDone}</option>
      </select>
    </label>
  </header>

  {#if error}<p class="error">{error}</p>{/if}

  {#if loaded && !items.length}
    <p class="empty">
      {show === "done" ? m.reminders.nothingDone : show === "all" ? m.reminders.nothingAll : m.reminders.nothing}
    </p>
  {:else}
    <div class="list">
      {#each groups as { g, items: rows } (g)}
        <div class="group" class:late={g === "overdue"}>{groupName(g)}</div>
        {#each rows as i (key(i))}
          {@const k = key(i)}
          <div
            class="row"
            class:selected={selectedKey === k}
            role="button"
            tabindex="0"
            onclick={() => onselect(hit(i))}
            ondblclick={() => onopen(hit(i))}
            onkeydown={(e) => {
              if (e.key === "Enter") onopen(hit(i));
              else if (e.key === " ") {
                e.preventDefault();
                onselect(hit(i));
              }
            }}
          >
            <div class="when">
              <span class="day">{formatIsoDay(i.dueDate)}</span>
              <span class="rel" class:late={i.days < 0 && !i.done}>
                {i.done ? m.reminders.doneOn(formatIsoDay(i.done)) : when(i)}
              </span>
            </div>
            <div class="what">
              <span class="title">{i.title || i.name.replace(/\.[^.]+$/, "")}</span>
              <span class="from">
                {[i.sender, snoozedNow(i) ? m.reminders.hiddenUntil(formatIsoDay(i.snoozed)) : null].filter(Boolean).join(" · ")}
              </span>
            </div>
            <span class="amount">{i.amount ? formatMoney(i.amount, i.currency) : ""}</span>
            {#if i.done}
              <button
                class="done"
                title={m.reminders.notDoneTip}
                disabled={busy === k}
                onclick={(e) => {
                  e.stopPropagation();
                  setDone(i, false);
                }}>{m.reminders.notDone}</button
              >
            {:else}
              <button
                class="done"
                title={m.reminders.markDoneTip}
                disabled={busy === k}
                onclick={(e) => {
                  e.stopPropagation();
                  setDone(i, true);
                }}><span class="tick"><Icon name="check" size={13} /></span>{m.reminders.markDone}</button
              >
            {/if}
            <button
              class="more"
              aria-label={m.reminders.more}
              title={m.reminders.more}
              aria-haspopup="menu"
              onclick={(e) => openMenu(e, i)}><Icon name="more" size={14} /></button
            >
          </div>
        {/each}
      {/each}
    </div>
    <p class="note muted">{m.reminders.note}</p>
  {/if}
</section>

{#if menu}
  {@const i = menu.item}
  <div class="menu" role="menu" style:left="{menu.x}px" style:top="{menu.y}px">
    {#if !i.done}
      {#if snoozedNow(i)}
        <button role="menuitem" onclick={() => snooze(i, null)}>{m.reminders.remindNow}</button>
      {:else}
        <span class="menu-head">{m.reminders.remindLater}</span>
        <button role="menuitem" onclick={() => snooze(i, 1)}>{m.reminders.laterTomorrow}</button>
        <button role="menuitem" onclick={() => snooze(i, 3)}>{m.reminders.later3}</button>
        <button role="menuitem" onclick={() => snooze(i, 7)}>{m.reminders.laterWeek}</button>
      {/if}
      <hr />
    {/if}
    <button role="menuitem" onclick={() => calendar(i)}>{m.reminders.addToCalendar}</button>
    <button
      role="menuitem"
      onclick={() => {
        menu = null;
        onshowinfolder(hit(i));
      }}>{m.reminders.showInFolder}</button
    >
  </div>
{/if}

<style>
  .coming {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px 22px;
    min-height: 0;
    overflow: auto;
  }
  header {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }
  h1 {
    margin: 0;
    font-size: 18px;
  }
  .grow {
    flex: 1;
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .muted {
    color: var(--muted);
  }
  .empty {
    color: var(--muted);
    margin: 24px 0;
  }
  .list {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--panel);
    overflow: hidden;
  }
  .group {
    padding: 7px 16px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--muted);
    background: var(--bg-subtle, var(--bg));
    border-bottom: 1px solid var(--border);
  }
  .late {
    color: var(--danger, #b4500f);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 9px 16px;
    border-bottom: 1px solid var(--border);
    cursor: default;
  }
  .row:last-child {
    border-bottom: none;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--accent-soft);
  }
  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .when {
    width: 120px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .day {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .rel,
  .from {
    font-size: 12px;
    color: var(--muted);
  }
  .rel.late {
    color: var(--danger, #b4500f);
  }
  .what {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .amount {
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .done {
    display: flex;
    align-items: center;
    gap: 5px;
    white-space: nowrap;
  }
  .tick {
    display: inline-flex;
    color: var(--ok, #2f7a3b);
  }
  .more {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    padding: 4px 0;
  }
  .note {
    margin: 0;
    font-size: 12px;
  }
  .error {
    color: var(--danger, #b42318);
    margin: 0;
  }
  .menu {
    position: fixed;
    z-index: 50;
    min-width: 220px;
    display: flex;
    flex-direction: column;
    padding: 5px;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--panel);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.18);
  }
  .menu button {
    text-align: left;
    border: none;
    background: none;
    padding: 5px 10px;
    border-radius: 5px;
  }
  .menu button:hover {
    background: var(--accent);
    color: white;
  }
  .menu-head {
    padding: 4px 10px 2px;
    font-size: 11px;
    color: var(--muted);
  }
  .menu hr {
    width: 100%;
    border: none;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }
</style>
