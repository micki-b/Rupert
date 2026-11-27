<script lang="ts">
  // Rename documents… (#64): new names for a folder's documents, in a
  // before/after table. They start from each document's date and title
  // (the name pattern); editing one names the others "the same way" (by
  // rule when it's a pattern of the details, else with Local AI), and
  // Local AI can propose names from a description. Nothing changes until
  // Rename; the window offers Undo afterwards.
  import { onMount, untrack } from "svelte";
  import {
    api,
    onRenameProposal,
    onRenameProposalsDone,
    type Entry,
    type RenameChecked,
    type RenameDetails,
    type RenameOutcome,
    type RenameRow,
  } from "#lib/api.ts";
  import { formatDay } from "#lib/format.ts";
  import { m, num } from "#lib/i18n/index.ts";
  import { changedParts } from "#lib/rename.ts";
  import Thumb from "#lib/Thumb.svelte";

  let {
    sourceId,
    relDir,
    where,
    only = null,
    pattern,
    ondone,
    onclose,
  }: {
    sourceId: string;
    relDir: string;
    /** The folder as shown ("Documents › Cars"). */
    where: string;
    /** Started from one document, or several selected (#102): only they are ticked. */
    only?: string[] | null;
    /** The name pattern, as set in Settings. */
    pattern: string;
    ondone: (outcome: RenameOutcome) => void;
    onclose: () => void;
  } = $props();

  // MARK: - Rows

  /** How a row's new name came about. */
  type How = "pattern" | "yours" | "same" | "ai";
  type Row = RenameRow & {
    on: boolean;
    /** The new name, without extension. */
    next: string;
    how: How;
    /** When it was typed (latest is the example that counts most). */
    typedAt: number;
    state: "idle" | "waiting" | "proposing";
    error: string | null;
  };

  let rows = $state<Row[]>([]);
  let ai = $state(false);
  let loading = $state(true);
  let instruction = $state("");
  let setTitle = $state(false);
  let selected = $state<string | null>(null);
  let editing = $state<string | null>(null);
  let editValue = $state("");
  /** Each ticked name as checked, with the name it was checked for. */
  let checked = $state<Map<string, { c: RenameChecked; next: string }>>(new Map());
  let hint = $state<"start" | "same" | "aiSame" | "ai" | "noRule">("start");
  let job = $state<number | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let typedCount = 0;

  /** Earlier proposals, for Undo in the hint. */
  type Snapshot = { relPath: string; next: string; how: How; on: boolean; typedAt: number; error: string | null }[];
  let history = $state<{ rows: Snapshot; hint: typeof hint }[]>([]);

  onMount(() => {
    api
      .renameRows(sourceId, relDir)
      .then((r) => {
        ai = r.ai;
        rows = r.rows.map((row) => {
          const next = row.patternName ?? row.stem;
          return {
            ...row,
            next,
            on: (only ? only.includes(row.relPath) : true) && next !== row.stem,
            how: "pattern" as How,
            typedAt: 0,
            state: "idle" as const,
            error: null,
          };
        });
        selected = only?.[0] ?? rows[0]?.relPath ?? null;
      })
      .catch((e) => (error = String(e)))
      .finally(() => (loading = false));
    const offs = [
      onRenameProposal((p) => {
        if (p.job !== job) return;
        const row = rows.find((r) => r.relPath === p.relPath);
        if (row && row.how !== "yours") {
          if (p.name) {
            row.next = p.name;
            row.how = "ai";
            row.error = null;
            row.on = true;
          } else {
            row.error = p.error;
          }
          row.state = "idle";
        }
        const next = rows.find((r) => r.state === "waiting");
        if (next) next.state = "proposing";
      }),
      onRenameProposalsDone((d) => {
        if (d.job !== job) return;
        job = null;
        for (const r of rows) if (r.state !== "idle") r.state = "idle";
      }),
    ];
    return () => {
      offs.forEach((p) => p.then((off) => off()));
      if (job !== null) api.renameStop();
    };
  });

  // MARK: - Checking the names

  // What the ticked names would really be (taken → " (2)", not allowed,
  // unchanged), shortly after they change.
  $effect(() => {
    const planned = rows.filter((r) => r.on).map((r) => ({ relPath: r.relPath, stem: r.next }));
    const timer = setTimeout(async () => {
      try {
        const result = await api.renameCheck(sourceId, relDir, planned);
        untrack(
          () => (checked = new Map(result.map((c, i) => [c.relPath, { c, next: planned[i].stem }]))),
        );
      } catch (e) {
        error = String(e);
      }
    }, 120);
    return () => clearTimeout(timer);
  });

  /** A ticked row's check, if it's for the name it has now. */
  function checkOf(r: Row): RenameChecked | undefined {
    const entry = r.on ? checked.get(r.relPath) : undefined;
    return entry && entry.next === r.next ? entry.c : undefined;
  }

  /** What a row's name will be, without extension. */
  function shown(r: Row): string {
    const c = checkOf(r);
    if (c && c.problem?.kind !== "invalid") return c.name.slice(0, c.name.length - r.ext.length);
    return r.next;
  }

  /** The rows Rename renames. */
  const ready = $derived(
    rows.filter((r) => {
      const kind = checkOf(r)?.problem?.kind;
      return r.state === "idle" && !!checkOf(r) && kind !== "unchanged" && kind !== "invalid";
    }),
  );

  const allOn = $derived(rows.length > 0 && rows.every((r) => r.on));
  const someOn = $derived(rows.some((r) => r.on));
  const waiting = $derived(rows.filter((r) => r.state !== "idle").length);
  const total = $state({ n: 0 });

  function toggleAll() {
    const on = !allOn;
    for (const r of rows) r.on = on;
  }

  // MARK: - History

  function remember() {
    history.push({
      rows: rows.map((r) => ({ relPath: r.relPath, next: r.next, how: r.how, on: r.on, typedAt: r.typedAt, error: r.error })),
      hint,
    });
  }

  function undoProposals() {
    const last = history.pop();
    if (!last) return;
    stopProposing();
    for (const s of last.rows) {
      const r = rows.find((x) => x.relPath === s.relPath);
      if (r) Object.assign(r, { next: s.next, how: s.how, on: s.on, typedAt: s.typedAt, error: s.error, state: "idle" });
    }
    hint = last.hint;
  }

  // MARK: - Editing a name

  function details(r: Row): RenameDetails {
    return { stem: r.stem, title: r.title, documentDate: r.documentDate, tags: r.tags };
  }

  function startEdit(r: Row) {
    if (busy) return;
    selected = r.relPath;
    editing = r.relPath;
    editValue = shown(r);
  }

  function focusAndSelect(el: HTMLInputElement) {
    el.focus();
    el.select();
  }

  async function commitEdit(r: Row) {
    if (editing !== r.relPath) return;
    editing = null;
    const value = editValue.trim();
    if (!value || value === shown(r)) return;
    remember();
    r.next = value;
    r.how = "yours";
    r.typedAt = ++typedCount;
    r.on = true;
    r.error = null;
    await sameWay();
  }

  /** Names the others the same way as the typed ones: by rule if Rupert can
   * tell it from the date and title, else with Local AI. */
  async function sameWay() {
    const typed = rows.filter((r) => r.how === "yours").sort((a, b) => a.typedAt - b.typedAt);
    const targets = rows.filter((r) => r.on && r.how !== "yours");
    if (!typed.length || !targets.length) return;
    stopProposing();
    const names = await api.renameSameWay(
      typed.map((r) => ({ details: details(r), name: r.next })),
      targets.map(details),
    );
    if (names) {
      targets.forEach((r, i) => {
        const name = names[i];
        if (name) {
          r.next = name;
          r.how = "same";
          r.error = null;
        }
      });
      hint = "same";
    } else if (ai) {
      hint = "aiSame";
      await propose(targets);
    } else {
      hint = "noRule";
    }
  }

  // MARK: - Local AI

  async function propose(targets: Row[]) {
    const examples = rows
      .filter((r) => r.how === "yours")
      .sort((a, b) => a.typedAt - b.typedAt)
      .map((r) => ({ details: details(r), name: r.next }));
    targets.forEach((r, i) => {
      r.state = i === 0 ? "proposing" : "waiting";
      r.error = null;
    });
    total.n = targets.length;
    try {
      job = await api.renamePropose(
        sourceId,
        instruction,
        examples,
        targets.map((r) => r.relPath),
      );
    } catch (e) {
      error = String(e);
      for (const r of targets) r.state = "idle";
    }
  }

  async function proposeWithAi() {
    const targets = rows.filter((r) => r.on && r.how !== "yours");
    if (!targets.length || job !== null) return;
    remember();
    hint = rows.some((r) => r.how === "yours") ? "aiSame" : "ai";
    await propose(targets);
  }

  function stopProposing() {
    if (job === null) return;
    api.renameStop();
    job = null;
    for (const r of rows) r.state = "idle";
  }

  // MARK: - Renaming

  async function rename() {
    if (!ready.length || busy) return;
    stopProposing();
    busy = true;
    error = null;
    try {
      const steps = ready.map((r) => ({ relPath: r.relPath, name: checkOf(r)!.name }));
      const outcome = await api.renameApply(sourceId, steps, setTitle);
      ondone(outcome);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function close() {
    stopProposing();
    onclose();
  }

  // MARK: - Showing a row

  /** The label beside a new name: whose it is, or what's wrong. */
  function badge(r: Row): { kind: string; text: string; tip?: string } | null {
    if (r.error) return { kind: "bad", text: m.renameDocs.couldNotPropose, tip: r.error };
    const c = checkOf(r);
    if (c?.problem?.kind === "invalid") return { kind: "bad", text: m.renameDocs.notAllowed, tip: c.problem.message };
    if (c?.problem?.kind === "taken") return { kind: "warn", text: m.renameDocs.taken };
    if (r.how === "yours") return { kind: "yours", text: m.renameDocs.yours };
    if (r.how === "same" && r.on) return { kind: "same", text: m.renameDocs.sameWay };
    if (r.next === r.stem && !r.patternName) {
      if (!r.hasText && r.cloud === "cloudOnly") return { kind: "bad", text: m.renameDocs.noText };
      return { kind: "plain", text: m.renameDocs.noTitle };
    }
    if (r.next === r.stem) return { kind: "plain", text: m.renameDocs.unchanged };
    return null;
  }

  function entryOf(r: Row): Entry {
    return {
      name: r.name,
      relPath: r.relPath,
      isDir: false,
      kind: r.kind,
      size: r.size,
      modifiedMs: r.modifiedMs,
      cloud: r.cloud,
    };
  }

  function day(iso: string | null): string {
    if (!iso) return "";
    const [y, mo, d] = iso.split("-").map(Number);
    return formatDay(new Date(y, mo - 1, d).getTime());
  }

  const current = $derived(rows.find((r) => r.relPath === selected) ?? null);

  const hintText = $derived.by(() => {
    switch (hint) {
      case "same":
        return m.renameDocs.hintSame;
      case "aiSame":
        return m.renameDocs.hintAiSame;
      case "ai":
        return m.renameDocs.hintAi;
      case "noRule":
        return ai ? m.renameDocs.hintNoRule : m.renameDocs.hintNoRuleNoAi;
      default:
        return ai ? m.renameDocs.hintStart(pattern) : m.renameDocs.hintStartNoAi(pattern);
    }
  });
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key !== "Escape" || busy) return;
    if (editing) {
      editing = null;
    } else if (job !== null) {
      stopProposing();
    } else {
      close();
    }
    e.preventDefault();
  }}
/>

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="rename-title">
    <header>
      <h1 id="rename-title">{m.renameDocs.title}</h1>
      <p class="muted small">{where} · {m.renameDocs.count(rows.length)}</p>
    </header>

    <div class="how">
      {#if ai}
        <div class="ask">
          <input
            bind:value={instruction}
            placeholder={m.renameDocs.instructionPlaceholder}
            onkeydown={(e) => e.key === "Enter" && proposeWithAi()}
            disabled={busy}
          />
          <button class="ai" onclick={proposeWithAi} disabled={busy || job !== null || !someOn}>
            <svg viewBox="0 0 24 24" width="15" height="15" aria-hidden="true"
              ><path
                d="M12 3l1.8 4.7L18.5 9.5l-4.7 1.8L12 16l-1.8-4.7L5.5 9.5l4.7-1.8zM18 15l.9 2.1L21 18l-2.1.9L18 21l-.9-2.1L15 18l2.1-.9z"
                fill="none"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linejoin="round"
              /></svg
            >
            {m.renameDocs.proposeWithAi}
          </button>
        </div>
      {/if}
      <p class="muted small hint">
        {hintText}
        {#if history.length}<button class="link" onclick={undoProposals}>{m.renameDocs.undo}</button>{/if}
      </p>
    </div>

    <div class="body">
      <div class="table">
        {#if loading}
          <p class="muted small pad">{m.renameDocs.loading}</p>
        {:else if !rows.length}
          <p class="muted small pad">{m.renameDocs.empty}</p>
        {:else}
          <table>
            <thead>
              <tr>
                <th class="cb">
                  <input
                    type="checkbox"
                    checked={allOn}
                    indeterminate={someOn && !allOn}
                    onchange={toggleAll}
                    aria-label={m.renameDocs.selectAll}
                  />
                </th>
                <th class="cur">{m.renameDocs.currentName}</th>
                <th class="arrow"></th>
                <th>{m.renameDocs.newName}</th>
              </tr>
            </thead>
            <tbody>
              {#each rows as r (r.relPath)}
                {@const b = badge(r)}
                <tr
                  class:off={!r.on}
                  class:selected={selected === r.relPath}
                  onclick={() => (selected = r.relPath)}
                  ondblclick={() => api.openEntry(sourceId, r.relPath)}
                  title={m.renameDocs.openTip}
                >
                  <td class="cb">
                    <input type="checkbox" bind:checked={r.on} aria-label={r.name} onclick={(e) => e.stopPropagation()} />
                  </td>
                  <td class="cur" title={r.name}>{r.stem}<span class="ext">{r.ext}</span></td>
                  <td class="arrow">→</td>
                  <td class="new">
                    <div class="prop">
                      {#if editing === r.relPath}
                        <input
                          class="edit"
                          bind:value={editValue}
                          use:focusAndSelect
                          onkeydown={(e) => {
                            if (e.key === "Enter") commitEdit(r);
                            else if (e.key === "Escape") {
                              e.stopPropagation();
                              editing = null;
                            }
                          }}
                          onblur={() => commitEdit(r)}
                          ondblclick={(e) => e.stopPropagation()}
                        />
                      {:else if r.state === "proposing"}
                        <span class="wait"><span class="spinner"></span>{m.renameDocs.proposing}</span>
                      {:else if r.state === "waiting"}
                        <span class="wait">{m.renameDocs.waiting}</span>
                      {:else}
                        <button
                          class="name"
                          title={`${shown(r)}${r.ext}\n${m.renameDocs.editTip}`}
                          ondblclick={(e) => (e.stopPropagation(), startEdit(r))}
                          onclick={(e) => {
                            e.stopPropagation();
                            if (selected === r.relPath) startEdit(r);
                            else selected = r.relPath;
                          }}
                          onkeydown={(e) => e.key === "Enter" && (e.preventDefault(), startEdit(r))}
                        >
                          {#each r.on ? changedParts(r.stem, shown(r)) : [{ text: r.stem, changed: false }] as p, i (i)}{#if p.changed}<ins
                                >{p.text}</ins
                              >{:else}{p.text}{/if}{/each}<span class="ext">{r.ext}</span>
                        </button>
                      {/if}
                      {#if b && editing !== r.relPath && r.state === "idle"}
                        <span class="badge {b.kind}" title={b.tip}>{b.text}</span>
                      {/if}
                    </div>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      </div>

      <aside class="preview">
        {#if current}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="page" ondblclick={() => api.openEntry(sourceId, current!.relPath)} title={m.renameDocs.openTip}>
            {#key current.relPath}
              <Thumb {sourceId} entry={entryOf(current)} side={480} iconSize={48} eager />
            {/key}
          </div>
          <dl>
            <dt>{m.renameDocs.titleLabel}</dt>
            <dd>{current.title ?? "—"}</dd>
            <dt>{m.renameDocs.dateLabel}</dt>
            <dd>{day(current.documentDate) || "—"}</dd>
            <dt>{m.renameDocs.tagsLabel}</dt>
            <dd>{current.tags.length ? current.tags.join(", ") : "—"}</dd>
          </dl>
        {:else}
          <p class="muted small">{m.renameDocs.noPreview}</p>
        {/if}
      </aside>
    </div>

    {#if error}<p class="error small" role="alert">{error}</p>{/if}
    <footer>
      <label class="small"><input type="checkbox" bind:checked={setTitle} /> {m.renameDocs.setTitle}</label>
      <span class="grow"></span>
      {#if job !== null}
        <span class="progress small muted">
          <span class="bar"><i style:width="{(100 * (total.n - waiting)) / Math.max(total.n, 1)}%"></i></span>
          {m.renameDocs.progress(num(total.n - waiting), num(total.n))}
          <button class="link" onclick={stopProposing}>{m.renameDocs.stop}</button>
        </span>
      {/if}
      <button onclick={close} disabled={busy}>{m.common.cancel}</button>
      <button class="primary" onclick={rename} disabled={!ready.length || busy || job !== null}>
        {busy ? m.renameDocs.renaming : m.renameDocs.rename(ready.length)}
      </button>
    </footer>
  </div>
</div>

<style>
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
  input[type="checkbox"] {
    accent-color: var(--accent);
  }
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.3);
    display: grid;
    place-items: center;
    z-index: 40; /* above the chat panel (20) and menus (30) */
  }
  .dialog {
    width: min(1060px, calc(100vw - 32px));
    height: min(720px, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    overflow: hidden;
  }
  header {
    padding: 14px 18px 10px;
    border-bottom: 1px solid var(--border);
  }
  h1 {
    margin: 0;
    font-size: 15px;
  }
  header p {
    margin: 2px 0 0;
  }
  .small {
    font-size: 12px;
  }
  .how {
    padding: 12px 18px;
    display: grid;
    gap: 8px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .ask {
    display: flex;
    gap: 8px;
  }
  .ask input {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 5px 9px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  button.ai {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
  }
  .hint {
    margin: 0;
  }
  .link {
    border: 0;
    background: none;
    color: var(--accent-strong);
    text-decoration: underline;
    padding: 0 2px;
    font: inherit;
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .table {
    flex: 1;
    min-width: 0;
    overflow: auto;
  }
  .pad {
    padding: 14px 18px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
  }
  th {
    position: sticky;
    top: 0;
    z-index: 1;
    text-align: left;
    font-weight: 500;
    color: var(--muted);
    font-size: 12px;
    padding: 7px 8px;
    border-bottom: 1px solid var(--border);
    background: var(--bg);
  }
  td {
    padding: 4px 8px;
    border-bottom: 1px solid var(--border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    vertical-align: middle;
  }
  th.cb,
  td.cb {
    width: 34px;
    text-align: center;
  }
  th.cur {
    width: 38%;
  }
  td.cur {
    color: var(--muted);
  }
  th.arrow,
  td.arrow {
    width: 24px;
    padding: 0;
    text-align: center;
    color: var(--muted);
  }
  tr.selected td {
    background: var(--hover, rgb(60 45 30 / 0.06));
  }
  tr.off td,
  tr.off .name {
    color: var(--muted);
    opacity: 0.75;
  }
  .ext {
    color: var(--muted);
  }
  .prop {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .name {
    flex: 1;
    min-width: 0;
    text-align: left;
    border: 1px solid transparent;
    border-radius: 5px;
    background: none;
    padding: 2px 6px;
    color: inherit;
    font: inherit;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    cursor: text;
  }
  .name:hover {
    border-color: var(--border);
  }
  .name ins {
    text-decoration: none;
    background: var(--accent-soft);
    border-radius: 3px;
  }
  .edit {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    border-radius: 5px;
    padding: 2px 6px;
    background: var(--bg);
    color: var(--text);
    font: inherit;
  }
  .wait {
    flex: 1;
    color: var(--muted);
    font-style: italic;
    padding: 2px 6px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--accent-soft);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .badge {
    flex: none;
    font-size: 11px;
    border-radius: 9px;
    padding: 0 7px;
    white-space: nowrap;
    background: var(--panel);
    color: var(--muted);
  }
  .badge.yours {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .badge.same {
    background: rgb(60 130 70 / 0.12);
    color: #2f6b3a;
  }
  .badge.warn {
    background: rgb(200 140 0 / 0.14);
    color: #8a5a00;
  }
  .badge.bad {
    background: rgb(179 38 30 / 0.1);
    color: var(--danger, #b3261e);
  }
  .preview {
    width: 280px;
    flex: none;
    border-left: 1px solid var(--border);
    background: var(--panel);
    padding: 14px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .page {
    height: 300px;
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .page :global(img) {
    max-width: 100%;
    max-height: 300px;
    box-shadow: 0 1px 4px rgb(0 0 0 / 0.15);
  }
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 4px 12px;
    font-size: 12px;
  }
  dt {
    color: var(--muted);
  }
  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
  @media (max-width: 860px) {
    .preview {
      display: none;
    }
  }
  .error {
    margin: 0;
    padding: 8px 18px 0;
    color: var(--danger, #c0392b);
  }
  footer {
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    gap: 10px;
  }
  footer label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .grow {
    flex: 1;
  }
  .progress {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .bar {
    width: 110px;
    height: 5px;
    background: var(--border);
    border-radius: 3px;
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
  }
</style>
