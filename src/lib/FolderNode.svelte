<script lang="ts">
  // One folder in the sidebar tree. Children are listed only when expanded,
  // and re-listed when the watcher reports a change in this folder.
  import { onMount, type Snippet } from "svelte";
  import { api, type Entry } from "./api";
  import { changes } from "./changes.svelte";
  import Icon from "./Icon.svelte";
  import FolderNode from "./FolderNode.svelte";
  import { dropTarget, isDropOver, type MoveItem } from "./drag.svelte";
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
  }
  let { sourceId, relPath, name, depth, selected, onselect, startExpanded, trailing, onmove }: Props =
    $props();

  const drop = $derived(onmove ? dropTarget(sourceId, relPath, (item) => onmove(item, sourceId, relPath)) : {});

  let expanded = $state(false);
  let children = $state<Entry[] | null>(null);

  onMount(() => {
    if (startExpanded) {
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
  <span class="label">{name}</span>
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
</style>
