<script lang="ts">
  // One folder in the sidebar tree. Children are listed only when expanded,
  // and re-listed when the watcher reports a change in this folder.
  import { onMount, tick, type Snippet } from "svelte";
  import { api, type Entry } from "./api";
  import { changes } from "./changes.svelte";
  import Icon from "./Icon.svelte";
  import FolderNode from "./FolderNode.svelte";
  import RenameField from "./RenameField.svelte";
  import { dropTarget, isDropOver, type Dropped, type MoveItem } from "./drag.svelte";
  import { m } from "#lib/i18n/index.ts";

  interface Props {
    sourceId: string;
    relPath: string;
    name: string;
    depth: number;
    selected: { sourceId: string; relPath: string } | null;
    onselect: (sourceId: string, relPath: string) => void;
    startExpanded?: boolean;
    /** Extra content at the end of the row (e.g. a source's menu button). */
    trailing?: Snippet;
    /** Documents and folders dropped on the row move here. */
    onmove?: (item: MoveItem, sourceId: string, relPath: string) => void;
    /** Files dropped from another app are copied here. */
    oncopy?: (dropped: Dropped, sourceId: string, relPath: string) => void;
    /** Right-click on the row. */
    onmenu?: (sourceId: string, relPath: string, name: string, ev: MouseEvent) => void;
    /** The folder whose name is being edited in place (#65). */
    renaming?: { sourceId: string; relPath: string } | null;
    onrename?: (sourceId: string, relPath: string, name: string) => void;
    onrenamecancel?: () => void;
    /** Opens the folders down to this one (e.g. the current choice in a picker). */
    expandTo?: { sourceId: string; relPath: string } | null;
  }
  let {
    sourceId,
    relPath,
    name,
    depth,
    selected,
    onselect,
    startExpanded,
    trailing,
    onmove,
    oncopy,
    onmenu,
    renaming = null,
    onrename,
    onrenamecancel,
    expandTo = null,
  }: Props = $props();

  const editing = $derived(!!relPath && renaming?.sourceId === sourceId && renaming.relPath === relPath);

  const drop = $derived(
    dropTarget(sourceId, relPath, {
      move: onmove && ((item) => onmove(item, sourceId, relPath)),
      files: oncopy && ((dropped) => oncopy(dropped, sourceId, relPath)),
    }),
  );

  let expanded = $state(false);
  let row: HTMLDivElement | undefined = $state();
  let children = $state<Entry[] | null>(null);

  onMount(() => {
    const leadsTo =
      !!expandTo &&
      expandTo.sourceId === sourceId &&
      (relPath === "" ? expandTo.relPath !== "" : expandTo.relPath.startsWith(relPath + "/"));
    if (isSelected) tick().then(() => row?.scrollIntoView({ block: "nearest" }));
    if (startExpanded || leadsTo) {
      expanded = true;
      load();
    }
  });

  const isSelected = $derived(selected?.sourceId === sourceId && selected?.relPath === relPath);

  async function load() {
    try {
      children = (await api.listDir(sourceId, relPath)).filter((e) => e.isDir);
    } catch {
      children = [];
    }
  }

  function toggle(event: MouseEvent) {
    event.stopPropagation();
    expanded = !expanded;
    if (expanded && children === null) load();
  }

  $effect(() => {
    changes.tick;
    if (expanded && changes.affects(sourceId, relPath)) load();
  });
</script>

<div
  bind:this={row}
  class="row"
  class:selected={isSelected}
  class:drop={isDropOver(sourceId, relPath)}
  {...drop}
  style:padding-left="{8 + depth * 14}px"
  role="treeitem"
  aria-selected={isSelected}
  aria-expanded={expanded}
  tabindex="0"
  onclick={() => onselect(sourceId, relPath)}
  oncontextmenu={onmenu && ((ev) => onmenu(sourceId, relPath, name, ev))}
  onkeydown={(e) => {
    if (e.key === "Enter") onselect(sourceId, relPath);
    if (e.key === "ArrowRight" && !expanded) toggle(e as unknown as MouseEvent);
    if (e.key === "ArrowLeft" && expanded) toggle(e as unknown as MouseEvent);
  }}
>
  <button
    class="disclosure"
    class:open={expanded}
    class:none={children !== null && children.length === 0}
    onclick={toggle}
    aria-label={expanded ? m.common.collapse : m.common.expand}
    tabindex="-1"
  >
    <Icon name="chevron" size={12} />
  </button>
  <span class="icon"><Icon name="folder" size={15} /></span>
  {#if editing && onrename && onrenamecancel}
    <span class="label editing"
      ><RenameField {name} isDir oncommit={(n) => onrename(sourceId, relPath, n)} oncancel={onrenamecancel} /></span
    >
  {:else}
    <span class="label">{name}</span>
  {/if}
  {#if trailing}{@render trailing()}{/if}
</div>

{#if expanded && children}
  {#each children as child (child.relPath)}
    <FolderNode
      {sourceId}
      relPath={child.relPath}
      name={child.name}
      depth={depth + 1}
      {selected}
      {onselect}
      {onmove}
      {oncopy}
      {onmenu}
      {renaming}
      {onrename}
      {onrenamecancel}
      {expandTo}
    />
  {/each}
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 26px;
    padding-right: 8px;
    border-radius: 6px;
    cursor: default;
    user-select: none;
    white-space: nowrap;
  }
  .row:hover {
    background: var(--hover);
  }
  .row.selected {
    background: var(--accent-soft);
    color: var(--accent-strong);
  }
  .row.drop {
    background: var(--accent-soft);
    box-shadow: inset 0 0 0 2px var(--accent);
  }
  .row:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .disclosure {
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--muted);
    transition: transform 0.12s;
  }
  .disclosure.open {
    transform: rotate(90deg);
  }
  .disclosure.none {
    visibility: hidden;
  }
  .icon {
    display: grid;
    color: var(--folder);
  }
  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  /* Room for the edit field's outline. */
  .label.editing {
    overflow: visible;
    padding: 0 4px 0 2px;
  }
</style>
