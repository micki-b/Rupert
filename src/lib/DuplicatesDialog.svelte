<script lang="ts">
  // Duplicates (#111): finds copies (Stop between files), shows each group
  // with the copy Rupert would keep chosen, and moves the others to the
  // Trash after the kept one has taken their tags and notes. "Probably the
  // same" (online-only, same size and name) is shown, never cleared up.
  import { onMount } from "svelte";
  import { api, onHealthProgress, revealLabel, type DupGroup, type Source } from "#lib/api.ts";
  import { formatDay, formatSize } from "#lib/format.ts";
  import { m } from "#lib/i18n/index.ts";
  import Icon from "#lib/Icon.svelte";
  import Thumb from "#lib/Thumb.svelte";
  import CompareCopies from "#lib/CompareCopies.svelte";

  let {
    sourceId,
    sources,
    os,
    onfound,
    onclose,
  }: {
    sourceId: string | null;
    sources: Source[];
    os: string | undefined;
    /** What the search found: groups, and the copies that could go. */
    onfound: (groups: number, copies: number) => void;
    onclose: () => void;
  } = $props();

  type Group = DupGroup & { keepIndex: number; include: boolean };
  let groups = $state<Group[]>([]);
  let searching = $state(true);
  let progress = $state<{ done: number; total: number }>({ done: 0, total: 0 });
  let error = $state<string | null>(null);
  let clearing = $state(false);
  let result = $state<{ trashed: number; failed: [string, string][] } | null>(null);
  /** The group being compared side by side, and the copy clicked. */
  let comparing = $state<{ group: Group; first: number } | null>(null);
  /** Show the groups kept as they are, too. */
  let showKept = $state(false);

  onMount(() => {
    const off = onHealthProgress((p) => (progress = { done: p.done, total: p.total }));
    api
      .duplicatesFind(sourceId)
      .then((found) => {
        groups = found.map((g) => ({ ...g, keepIndex: g.keep, include: g.kind !== "probably" }));
        report();
      })
      .catch((e) => (error = String(e)))
      .finally(() => (searching = false));
    return () => {
      off.then((f) => f());
      if (searching) api.healthStop();
    };
  });

  /** Groups still to review (not kept as they are), and those shown. */
  const active = $derived(groups.filter((g) => !g.kept));
  const keptCount = $derived(groups.length - active.length);
  const visible = $derived(groups.filter((g) => showKept || !g.kept));
  const chosen = $derived(active.filter((g) => g.include && g.kind !== "probably"));

  /** Tells Library health what's left to review. */
  function report() {
    onfound(active.length, active.reduce((n, g) => n + g.docs.length - 1, 0));
  }

  /** Keep all: these copies are fine; Rupert skips the group from now on
   * (remembered in its source's hidden .rupert folder). Or undoes that. */
  async function setKept(g: Group, keep: boolean) {
    error = null;
    try {
      await api.duplicatesKeep(g.docs[0].sourceId, g.key, g.docs.length, keep);
      g.kept = keep;
      report();
    } catch (e) {
      error = String(e);
    }
  }
  const copies = $derived(chosen.reduce((n, g) => n + g.docs.length - 1, 0));
  const sourceName = (id: string) => sources.find((s) => s.id === id)?.name ?? "";
  const folderOf = (h: { sourceId: string; relPath: string }) =>
    [sourceName(h.sourceId), ...h.relPath.split("/").slice(0, -1)].join(" › ");

  /** The choice before a row's first click, for a double-click to put back. */
  let before: { g: Group; keep: number } | null = null;

  function rowClick(e: MouseEvent, g: Group, i: number) {
    if ((e.target as Element).closest("button, input") || g.kind === "probably" || g.kept) return;
    if (e.detail === 1) before = { g, keep: g.keepIndex };
    g.keepIndex = i;
  }

  function rowOpen(e: MouseEvent, g: Group, d: Group["docs"][number]) {
    if ((e.target as Element).closest("button, input")) return;
    if (before?.g === g) g.keepIndex = before.keep;
    before = null;
    api.openEntry(d.sourceId, d.relPath);
  }

  /** Whether a copy goes to the Trash when the group is cleared up. */
  const goes = (g: Group, i: number) => i !== g.keepIndex && g.include && !g.kept && g.kind !== "probably";

  /** Moves one copy to the Trash now, after the copy kept takes its tags
   * and notes; the group shrinks, and goes once only the kept one is left. */
  async function trashOne(g: Group, i: number) {
    const d = g.docs[i];
    const k = g.docs[g.keepIndex];
    error = null;
    try {
      const r = await api.duplicatesClear([
        { keep: { sourceId: k.sourceId, relPath: k.relPath }, others: [{ sourceId: d.sourceId, relPath: d.relPath }] },
      ]);
      if (r.failed.length) {
        error = r.failed.map(([p, why]) => `${p}: ${why}`).join("\n");
        return;
      }
      g.docs.splice(i, 1);
      if (g.keepIndex > i) g.keepIndex--;
      if (g.docs.length < 2) {
        groups = groups.filter((x) => x !== g);
        if (comparing?.group === g) comparing = null;
      }
      report();
    } catch (e) {
      error = String(e);
    }
  }

  async function clearUp() {
    clearing = true;
    error = null;
    try {
      result = await api.duplicatesClear(
        chosen.map((g) => ({
          keep: { sourceId: g.docs[g.keepIndex].sourceId, relPath: g.docs[g.keepIndex].relPath },
          others: g.docs.filter((_, i) => i !== g.keepIndex).map((d) => ({ sourceId: d.sourceId, relPath: d.relPath })),
        })),
      );
      groups = groups.filter((g) => !chosen.includes(g));
      report();
    } catch (e) {
      error = String(e);
    } finally {
      clearing = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && !clearing && !comparing && onclose()} />

<div class="backdrop" role="presentation">
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="dup-title">
    <h1 id="dup-title">{m.health.duplicates}</h1>
    <p class="small muted">{m.health.dupIntro}</p>

    {#if searching}
      <div class="progress">
        <progress max={progress.total || 1} value={progress.done}></progress>
        <span class="small muted">{m.health.comparing(progress.done, progress.total)}</span>
        <button onclick={() => api.healthStop()}>{m.exportDocs.stop}</button>
      </div>
    {:else if !groups.length && !error}
      <p>{result ? m.health.cleared(result.trashed) : m.health.noDuplicates}</p>
    {/if}
    {#if error}<p class="small error" role="alert">{error}</p>{/if}
    {#if result?.failed.length}
      <p class="small error">{m.readDetails.failed(result.failed.length)}</p>
      <ul class="small failed">{#each result.failed as [p, why] (p)}<li><b>{p}</b>: {why}</li>{/each}</ul>
    {/if}

    {#if groups.length && !visible.length && !searching}
      <p>{m.health.noneLeft}</p>
    {/if}
    {#if visible.length}
      <div class="groups">
        {#each visible as g, gi (gi + g.docs[0].relPath)}
          <section class="group" class:off={!g.include || g.kept} class:kept={g.kept}>
            <div class="ghead">
              <label class="check">
                <input type="checkbox" bind:checked={g.include} disabled={g.kind === "probably" || g.kept} />
                <b>{g.kind === "sameFile" ? m.health.sameFile : g.kind === "sameText" ? m.health.sameText : m.health.probably}</b>
              </label>
              <span class="small muted">
                {g.kept ? m.health.keptNote : g.kind === "probably" ? m.health.probablyNote : m.health.keepNote}
              </span>
              <span class="grow"></span>
              <button onclick={() => (comparing = { group: g, first: 0 })}>{m.common.ellipsis(m.health.compare)}</button>
              {#if g.kept}
                <button onclick={() => setKept(g, false)} title={m.health.reviewAgainTip}>{m.health.reviewAgain}</button>
              {:else}
                <button onclick={() => setKept(g, true)} title={m.health.keepAllTip}>{m.health.keepAll}</button>
              {/if}
            </div>
            {#each g.docs as d, i (d.sourceId + "\n" + d.relPath)}
              <!-- A click on the row chooses the copy to keep, as its radio
                   button does; a double-click opens it and leaves the choice
                   as it was. The buttons in the row do only their own job. -->
              <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
              <div
                class="doc"
                class:keep={i === g.keepIndex}
                class:choosable={g.kind !== "probably" && !g.kept}
                onclick={(e) => rowClick(e, g, i)}
                ondblclick={(e) => rowOpen(e, g, d)}
              >
                <input
                  type="radio"
                  name={"keep" + gi}
                  checked={i === g.keepIndex}
                  onchange={() => (g.keepIndex = i)}
                  disabled={g.kind === "probably" || g.kept}
                  aria-label={m.health.keepThis + ": " + d.name}
                />
                <button class="pic" onclick={() => (comparing = { group: g, first: i })} title={m.health.compareTip}>
                  <Thumb sourceId={d.sourceId} entry={d} side={120} iconSize={22} />
                </button>
                <span class="info">
                  <span class="name">{d.name}{#if d.title}<span class="muted"> · {d.title}</span>{/if}</span>
                  <span class="small muted">{folderOf(d)}</span>
                  <span class="small muted">
                    {d.documentDate ?? formatDay(d.modifiedMs)} · {formatSize(d.size)}{d.cloud === "cloudOnly" ? " · " + m.health.onlineOnly : ""}
                    {#if d.tags.length}· {d.tags.join(", ")}{/if}
                  </span>
                </span>
                <span class="small verdict">{i === g.keepIndex ? m.health.keepIt : g.include && g.kind !== "probably" ? m.health.toTrash : ""}</span>
                {#if goes(g, i)}
                  <button class="icon danger" onclick={() => trashOne(g, i)} title={m.health.trashThis} aria-label={m.health.trashThis + ": " + d.name}
                    ><Icon name="trash" size={14} /></button
                  >
                {:else}
                  <span class="icon-space"></span>
                {/if}
                <button class="icon" onclick={() => api.openEntry(d.sourceId, d.relPath)} title={m.main.open} aria-label={m.main.open}
                  ><Icon name="file" size={14} /></button
                >
                <button class="icon" onclick={() => api.revealEntry(d.sourceId, d.relPath)} title={revealLabel(os)} aria-label={revealLabel(os)}
                  ><Icon name="folder" size={14} /></button
                >
              </div>
            {/each}
          </section>
        {/each}
      </div>
    {/if}

    <div class="footer">
      <span class="small muted">
        {active.length ? m.health.groupsFound(active.length) : ""}
        {#if keptCount}
          {active.length ? " · " : ""}{m.health.keptGroups(keptCount)}
          <button class="link" onclick={() => (showKept = !showKept)}>{showKept ? m.health.hideKept : m.health.showKept}</button>
        {/if}
      </span>
      <span class="grow"></span>
      <button onclick={onclose} disabled={clearing}>{groups.length && !result ? m.common.cancel : m.common.done}</button>
      {#if active.length}
        <button class="primary danger" onclick={clearUp} disabled={clearing || searching || copies === 0}>
          {m.health.moveCopies(copies)}
        </button>
      {/if}
    </div>
  </div>
</div>

{#if comparing}
  {@const c = comparing}
  <!-- Opened again when a copy goes, so its panes follow the group. -->
  {#key c.group.docs.length}
    <CompareCopies
      docs={c.group.docs}
      keep={c.group.keepIndex}
      first={c.first < c.group.docs.length ? c.first : c.group.keepIndex}
      {sources}
      {os}
      canChoose={c.group.kind !== "probably"}
      cantrash={(i) => goes(c.group, i)}
      onkeep={(i) => (c.group.keepIndex = i)}
      ontrash={(i) => trashOne(c.group, i)}
      onclose={() => (comparing = null)}
    />
  {/key}
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.3);
    display: grid;
    place-items: center;
    z-index: 40;
  }
  .dialog {
    width: min(820px, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 20px 60px rgb(0 0 0 / 0.3);
    padding: 18px 20px;
  }
  h1 {
    margin: 0 0 4px;
    font-size: 15px;
  }
  p {
    margin: 6px 0 0;
  }
  .small {
    font-size: 12px;
  }
  .muted {
    color: var(--muted);
  }
  .error {
    color: var(--danger, #c0392b);
  }
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 12px;
  }
  .progress progress {
    flex: 1;
  }
  .groups {
    margin-top: 12px;
    overflow: auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .group {
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 8px 10px;
  }
  .group.off {
    opacity: 0.6;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }
  .check {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .doc {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 6px;
    border-radius: 6px;
  }
  .doc.choosable {
    cursor: pointer;
  }
  .doc.keep {
    background: var(--accent-soft);
  }
  .pic {
    width: 44px;
    height: 56px;
    flex: none;
    display: grid;
    place-items: center;
    overflow: hidden;
    padding: 0;
    border: 0;
    background: none;
    cursor: zoom-in;
  }
  .group.kept {
    border-style: dashed;
  }
  button.link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-strong);
    text-decoration: underline;
    font-size: 12px;
  }
  button.icon.danger {
    color: var(--danger, #c0392b);
  }
  .icon-space {
    width: 28px;
    flex: none;
  }
  button.icon {
    width: 28px;
    height: 26px;
    padding: 0;
    display: grid;
    place-items: center;
    color: var(--accent-strong);
  }
  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .verdict {
    width: 110px;
    text-align: right;
    color: var(--muted);
  }
  .failed {
    margin: 6px 0 0;
    padding-left: 18px;
    max-height: 100px;
    overflow: auto;
  }
  .footer {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  .grow {
    flex: 1;
  }
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
  button.primary.danger {
    background: var(--danger, #c0392b);
    border-color: var(--danger, #c0392b);
  }
  button:disabled {
    opacity: 0.5;
  }
</style>
