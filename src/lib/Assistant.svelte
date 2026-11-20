<script lang="ts">
  // The chat assistant, lower right: ask for documents or tell it what to
  // show. Uses the model chosen in Settings ▸ Assistant (LM Studio on this
  // computer by default; a cloud model only when chosen there).
  import { onMount, tick } from "svelte";
  import { api, onAssistantStep, type AssistantAction, type AssistantInfo, type AssistantStep } from "$lib/api";
  import Icon from "$lib/Icon.svelte";
  import RupertOwl, { type OwlMood } from "$lib/RupertOwl.svelte";
  import { m } from "$lib/i18n";

  let { onaction, onsettings }: { onaction: (a: AssistantAction) => void; onsettings: () => void } = $props();

  type Message =
    | { role: "user"; text: string }
    | { role: "assistant"; text: string; steps: AssistantStep[]; shown: string[]; model: string }
    | { role: "error"; text: string }
    | { role: "stopped" };

  let open = $state(false);
  let info = $state<AssistantInfo | null>(null);
  let messages = $state<Message[]>([]);
  let draft = $state("");
  let busy = $state(false);
  /** Stop was pressed; waiting for the answer to end. */
  let stopping = $state(false);
  let live = $state<AssistantStep[]>([]);
  let list: HTMLDivElement | undefined = $state();
  let input: HTMLTextAreaElement | undefined = $state();
  /** How Rupert looks between answers: happy for a moment, puzzled after an error. */
  let after = $state<OwlMood>("idle");
  let afterTimer: ReturnType<typeof setTimeout> | undefined;

  /** What Rupert acts out while working, from the tool he's using. */
  function moodFor(tool: string | undefined): OwlMood {
    switch (tool) {
      case undefined:
        return "thinking";
      case "read_document":
        return "studying";
      case "open_folder":
      case "show_document":
      case "show_results":
        return "opening";
      default:
        return "searching";
    }
  }
  const mood = $derived<OwlMood>(busy ? moodFor(live.at(-1)?.tool) : after);

  /** Rereads which model is set up (the window calls it when Settings are
   * saved, so the panel shows a model chosen while it's open). */
  export async function refreshInfo() {
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

  /** Opens the panel (from elsewhere in the window, e.g. the welcome screen). */
  export async function show() {
    if (open) input?.focus();
    else await toggle();
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
        return m.assistant.openedFolder(a.relPath);
      case "showDocument":
        return m.assistant.showedDocument(a.relPath.split("/").pop() ?? "");
      case "search":
        return m.assistant.showedResults;
    }
  }

  async function send() {
    const text = draft.trim();
    if (!text || busy) return;
    draft = "";
    messages = [...messages, { role: "user", text }];
    clearTimeout(afterTimer);
    after = "idle";
    busy = true;
    live = [];
    scrollDown();
    try {
      const reply = await api.assistantSend(text);
      if (reply.stopped) {
        messages = [...messages, { role: "stopped" }];
        // Back in the box to change and ask again.
        if (!draft.trim()) draft = text;
        return;
      }
      for (const a of reply.actions) onaction(a);
      messages = [
        ...messages,
        {
          role: "assistant",
          text: reply.text || m.assistant.noAnswer,
          steps: reply.steps,
          shown: reply.actions.map(describe),
          model: reply.model,
        },
      ];
      after = "happy";
      afterTimer = setTimeout(() => (after = "idle"), 2500);
    } catch (e) {
      messages = [...messages, { role: "error", text: String(e) }];
      after = "puzzled";
    } finally {
      busy = false;
      stopping = false;
      live = [];
      scrollDown();
      await tick();
      input?.focus();
    }
  }

  /** Stops the answer in progress: nothing more is looked up or shown. */
  function stop() {
    if (!busy || stopping) return;
    stopping = true;
    api.assistantCancel().catch(() => {});
  }

  async function reset() {
    await api.assistantReset();
    messages = [];
    after = "idle";
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
    } else if (e.key === "Escape" && open) {
      // Esc stops Rupert first, then closes the panel.
      if (busy) stop();
      else open = false;
    }
  }
</script>

<svelte:window onkeydown={onwindowkey} />

{#if open}
  <div class="panel" role="dialog" aria-label={m.assistant.panel}>
    <header>
      <RupertOwl {mood} size={44} />
      <div class="title">
        <strong>{m.assistant.title}</strong>
        {#if info}
          <span class="model" class:cloud={info.cloud} title={info.cloud ? m.assistant.cloudTip : m.assistant.localTip}>
            {info.cloud ? "☁ " : ""}{info.label}
          </span>
        {/if}
      </div>
      <button class="icon" onclick={onsettings} title={m.assistant.settings} aria-label={m.assistant.settings}>
        <Icon name="gear" size={20} />
      </button>
      <button
        class="icon"
        onclick={reset}
        disabled={busy || !messages.length}
        title={m.assistant.newConversation}
        aria-label={m.assistant.newConversation}
      >
        <Icon name="new-chat" size={20} />
      </button>
      <button class="icon" onclick={() => (open = false)} title={m.common.closeEsc} aria-label={m.common.close}>
        <Icon name="close" size={20} />
      </button>
    </header>

    <div class="messages" bind:this={list}>
      {#if !messages.length}
        <div class="hello">
          <div class="hello-owl"><RupertOwl mood={busy ? mood : "idle"} size={120} /></div>
          <p>{m.assistant.hello}</p>
          <ul>
            {#each m.assistant.examples as example (example)}<li>{example}</li>{/each}
          </ul>
          {#if info?.cloud}
            <p class="note">{m.assistant.cloudNote}</p>
          {/if}
        </div>
      {/if}
      {#each messages as msg, i (i)}
        {#if msg.role === "user"}
          <div class="msg user">{msg.text}</div>
        {:else if msg.role === "assistant"}
          {#if msg.steps.length}
            <div class="steps">{msg.steps.map((s) => s.summary).join(" · ")}</div>
          {/if}
          <div class="msg bot">
            {#each boldParts(msg.text) as p, j (j)}{#if p.bold}<strong>{p.text}</strong>{:else}{p.text}{/if}{/each}
          </div>
          {#if msg.shown.length}<div class="shown">{msg.shown.join(" · ")}</div>{/if}
        {:else if msg.role === "stopped"}
          <div class="stopped">{m.assistant.stopped}</div>
        {:else}
          <div class="msg error">
            {msg.text}
            <button class="link" onclick={onsettings}>{m.assistant.settingsLink}</button>
          </div>
        {/if}
      {/each}
      {#if busy}
        <div class="working">
          {stopping ? m.assistant.stopping : live.length ? m.common.ellipsis(live[live.length - 1].summary) : m.assistant.thinking}
        </div>
      {/if}
    </div>

    <div class="compose">
      <textarea
        bind:this={input}
        bind:value={draft}
        {onkeydown}
        rows="2"
        placeholder={m.assistant.placeholder}
        disabled={busy}
      ></textarea>
      {#if busy}
        <button class="send stop" onclick={stop} disabled={stopping} title={m.assistant.stopKey} aria-label={m.assistant.stop}>
          <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><rect width="12" height="12" rx="2" fill="currentColor" /></svg>
        </button>
      {:else}
        <button class="send" onclick={send} disabled={!draft.trim()} aria-label={m.assistant.send}>↑</button>
      {/if}
    </div>
  </div>
{/if}

<button class="fab" class:open onclick={toggle} title={m.assistant.openKey} aria-label={m.assistant.title} aria-expanded={open}>
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
    gap: 6px;
    padding: 8px 10px 8px 10px;
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
  .title strong {
    font-size: 15px;
  }
  .icon {
    border: 0;
    background: none;
    color: var(--muted);
    width: 34px;
    height: 34px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    flex: none;
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
  .hello-owl {
    display: flex;
    justify-content: center;
    color: var(--text);
    margin: 4px 0 10px;
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
  .send.stop {
    display: grid;
    place-items: center;
    background: var(--text);
    color: var(--bg);
  }
  .stopped {
    align-self: flex-end;
    font-size: 11px;
    color: var(--muted);
    padding: 0 4px;
  }
  .link {
    border: 0;
    background: none;
    color: inherit;
    text-decoration: underline;
    padding: 0 0 0 4px;
  }
</style>
