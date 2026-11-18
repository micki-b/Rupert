<script lang="ts">
  // The chat assistant, lower right: ask for documents or tell it what to
  // show. Uses the model chosen in Settings ▸ Assistant (LM Studio on this
  // computer by default; a cloud model only when chosen there).
  import { onMount, tick } from "svelte";
  import { api, onAssistantStep, type AssistantAction, type AssistantInfo, type AssistantStep } from "$lib/api";

  let { onaction, onsettings }: { onaction: (a: AssistantAction) => void; onsettings: () => void } = $props();

  type Message =
    | { role: "user"; text: string }
    | { role: "assistant"; text: string; steps: AssistantStep[]; shown: string[]; model: string }
    | { role: "error"; text: string };

  let open = $state(false);
  let info = $state<AssistantInfo | null>(null);
  let messages = $state<Message[]>([]);
  let draft = $state("");
  let busy = $state(false);
  let live = $state<AssistantStep[]>([]);
  let list: HTMLDivElement | undefined = $state();
  let input: HTMLTextAreaElement | undefined = $state();

  async function refreshInfo() {
    info = await api.assistantInfo().catch(() => null);
  }

  onMount(() => {
    refreshInfo();
    const un = onAssistantStep((s) => {
      live = [...live, s];
      scrollDown();
    });
    return () => {
      un.then((f) => f());
    };
  });

  async function scrollDown() {
    await tick();
    list?.scrollTo({ top: list.scrollHeight, behavior: "smooth" });
  }

  async function toggle() {
    open = !open;
    if (open) {
      await refreshInfo();
      await tick();
      input?.focus();
    }
  }

  /** Splits **bold** markers (models use them even when asked not to). */
  function boldParts(text: string): { text: string; bold: boolean }[] {
    return text.split("**").map((t, i) => ({ text: t, bold: i % 2 === 1 }));
  }

  function describe(a: AssistantAction): string {
    switch (a.kind) {
      case "openFolder":
        return `Opened ${a.relPath || "the source"}`;
      case "showDocument":
        return `Showed ${a.relPath.split("/").pop()}`;
      case "search":
        return "Showed the results";
    }
  }

  async function send() {
    const text = draft.trim();
    if (!text || busy) return;
    draft = "";
    messages = [...messages, { role: "user", text }];
    busy = true;
    live = [];
    scrollDown();
    try {
      const reply = await api.assistantSend(text);
      for (const a of reply.actions) onaction(a);
      messages = [
        ...messages,
        {
          role: "assistant",
          text: reply.text || "(no answer)",
          steps: reply.steps,
          shown: reply.actions.map(describe),
          model: reply.model,
        },
      ];
    } catch (e) {
      messages = [...messages, { role: "error", text: String(e) }];
    } finally {
      busy = false;
      live = [];
      scrollDown();
      await tick();
      input?.focus();
    }
  }

  async function reset() {
    await api.assistantReset();
    messages = [];
    await refreshInfo();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
      e.preventDefault();
      send();
    }
  }

  function onwindowkey(e: KeyboardEvent) {
    // ⌘/Ctrl-J opens and closes the assistant.
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "j") {
      e.preventDefault();
      toggle();
    } else if (e.key === "Escape" && open && !busy) {
      open = false;
    }
  }
</script>

<svelte:window onkeydown={onwindowkey} />

{#if open}
  <div class="panel" role="dialog" aria-label="Assistant">
    <header>
      <div class="title">
        <strong>Ask Rupert</strong>
        {#if info}
          <span class="model" class:cloud={info.cloud} title={info.cloud ? "Documents the assistant looks at are sent to this cloud service" : "Runs on this computer"}>
            {info.cloud ? "☁ " : ""}{info.label}
          </span>
        {/if}
      </div>
      <button class="icon" onclick={onsettings} title="Assistant settings" aria-label="Assistant settings">⚙︎</button>
      <button class="icon" onclick={reset} disabled={busy || !messages.length} title="New conversation" aria-label="New conversation">↺</button>
      <button class="icon" onclick={() => (open = false)} title="Close (Esc)" aria-label="Close">✕</button>
    </header>

    <div class="messages" bind:this={list}>
      {#if !messages.length}
        <div class="hello">
          <p>Ask for documents, or tell me what to show. For example:</p>
          <ul>
            <li>“Find the impeller invoice from last spring”</li>
            <li>“What's the deductible on the boat insurance?”</li>
            <li>“Show everything tagged Taxes and 2025”</li>
            <li>“Open the Boat folder”</li>
          </ul>
          {#if info?.cloud}
            <p class="note">
              ☁ A cloud model is chosen: titles, tags and the text of documents I look at are sent to it.
            </p>
          {/if}
        </div>
      {/if}
      {#each messages as m, i (i)}
        {#if m.role === "user"}
          <div class="msg user">{m.text}</div>
        {:else if m.role === "assistant"}
          {#if m.steps.length}
            <div class="steps">{m.steps.map((s) => s.summary).join(" · ")}</div>
          {/if}
          <div class="msg bot">
            {#each boldParts(m.text) as p, j (j)}{#if p.bold}<strong>{p.text}</strong>{:else}{p.text}{/if}{/each}
          </div>
          {#if m.shown.length}<div class="shown">{m.shown.join(" · ")}</div>{/if}
        {:else}
          <div class="msg error">
            {m.text}
            <button class="link" onclick={onsettings}>Settings…</button>
          </div>
        {/if}
      {/each}
      {#if busy}
        <div class="working">
          <span class="spinner"></span>
          {live.length ? live[live.length - 1].summary + "…" : "Thinking…"}
        </div>
      {/if}
    </div>

    <div class="compose">
      <textarea
        bind:this={input}
        bind:value={draft}
        {onkeydown}
        rows="2"
        placeholder="Ask about your documents…"
        disabled={busy}
      ></textarea>
      <button class="send" onclick={send} disabled={busy || !draft.trim()} aria-label="Send">↑</button>
    </div>
  </div>
{/if}

<button class="fab" class:open onclick={toggle} title="Ask Rupert (⌘J)" aria-label="Ask Rupert" aria-expanded={open}>
  <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path d="M4 5h16a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9l-4 3v-3H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1z" />
    <path d="M8 10h.01M12 10h.01M16 10h.01" stroke-width="2.6" />
  </svg>
</button>

<style>
  .fab {
    position: fixed;
    right: 18px;
    bottom: 34px;
    width: 46px;
    height: 46px;
    border-radius: 50%;
    border: 0;
    background: var(--accent);
    color: #fff;
    display: grid;
    place-items: center;
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.25);
    z-index: 20;
  }
  .fab.open {
    background: var(--accent-strong);
  }
  .panel {
    position: fixed;
    right: 18px;
    bottom: 90px;
    width: min(400px, calc(100vw - 36px));
    height: min(560px, calc(100vh - 130px));
    display: flex;
    flex-direction: column;
    background: var(--bg);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 12px;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.3);
    z-index: 20;
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 10px 10px 8px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--panel);
  }
  .title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .model {
    font-size: 11px;
    color: var(--muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .model.cloud {
    color: var(--cloud);
  }
  .icon {
    border: 0;
    background: none;
    color: var(--muted);
    width: 26px;
    height: 26px;
    border-radius: 6px;
  }
  .icon:hover:not(:disabled) {
    background: var(--hover);
    color: var(--text);
  }
  .icon:disabled {
    opacity: 0.4;
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .hello {
    color: var(--muted);
    font-size: 13px;
    line-height: 1.45;
  }
  .hello ul {
    margin: 4px 0 8px;
    padding-left: 18px;
  }
  .note {
    color: var(--cloud);
  }
  .msg {
    max-width: 88%;
    padding: 7px 10px;
    border-radius: 10px;
    line-height: 1.45;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    user-select: text;
    -webkit-user-select: text;
  }
  .user {
    align-self: flex-end;
    background: var(--accent);
    color: #fff;
  }
  .bot {
    align-self: flex-start;
    background: var(--panel);
  }
  .error {
    align-self: flex-start;
    background: var(--danger-soft);
    color: var(--danger);
  }
  .steps,
  .shown {
    font-size: 11px;
    color: var(--muted);
    padding: 0 4px;
  }
  .shown {
    color: var(--accent);
  }
  .working {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--muted);
    padding: 4px;
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
  .compose {
    display: flex;
    gap: 6px;
    padding: 8px;
    border-top: 1px solid var(--border);
  }
  textarea {
    flex: 1;
    resize: none;
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 6px 8px;
    font: inherit;
    background: var(--bg);
    color: var(--text);
  }
  .send {
    width: 34px;
    border: 0;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-size: 16px;
  }
  .send:disabled {
    opacity: 0.45;
  }
  .link {
    border: 0;
    background: none;
    color: inherit;
    text-decoration: underline;
    padding: 0 0 0 4px;
  }
</style>
