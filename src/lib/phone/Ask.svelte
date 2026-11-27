<script lang="ts">
  // Ask Rupert on the iPhone (the Ask tab), with Apple Intelligence on the
  // phone. Things Rupert would show in the window on a computer become
  // buttons under his answer here, so the answer stays on screen.
  import { onMount, tick } from "svelte";
  import RupertOwl, { type OwlMood } from "#lib/RupertOwl.svelte";
  import { m } from "#lib/i18n/index.ts";
  import {
    onAssistantStep,
    phone,
    type AssistantAction,
    type AssistantStatus,
    type AssistantStep,
  } from "#lib/phone/api.ts";

  let {
    status,
    hidden = false,
    onaction,
  }: { status: AssistantStatus; hidden?: boolean; onaction: (a: AssistantAction) => void } = $props();

  type Message =
    | { role: "user"; text: string }
    | { role: "assistant"; text: string; steps: AssistantStep[]; actions: AssistantAction[] }
    | { role: "error"; text: string }
    | { role: "stopped" };

  let messages = $state<Message[]>([]);
  let draft = $state("");
  let busy = $state(false);
  let stopping = $state(false);
  let live = $state<AssistantStep[]>([]);
  let list: HTMLDivElement | undefined = $state();
  let after = $state<OwlMood>("idle");
  let afterTimer: ReturnType<typeof setTimeout> | undefined;

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

  onMount(() => {
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

  function boldParts(text: string): { text: string; bold: boolean }[] {
    return text.split("**").map((t, i) => ({ text: t, bold: i % 2 === 1 }));
  }

  function label(a: AssistantAction): string {
    const last = (p: string) => p.split("/").pop() ?? p;
    switch (a.kind) {
      case "openFolder":
        return m.phone.askOpenFolder(a.relPath ? last(a.relPath) : m.phone.library);
      case "showDocument":
        return m.phone.askShowDocument(last(a.relPath));
      case "search":
      case "savedSearch":
        return m.phone.askShowResults;
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
      const reply = await phone.assistantSend(text);
      if (reply.stopped) {
        messages = [...messages, { role: "stopped" }];
        if (!draft.trim()) draft = text;
        return;
      }
      messages = [
        ...messages,
        { role: "assistant", text: reply.text || m.assistant.noAnswer, steps: reply.steps, actions: reply.actions },
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
    }
  }

  function stop() {
    if (!busy || stopping) return;
    stopping = true;
    phone.assistantCancel().catch(() => {});
  }

  /** Starts a new conversation (the header's button). */
  export async function reset() {
    if (busy) stop();
    await phone.assistantReset().catch(() => {});
    messages = [];
    draft = "";
    after = "idle";
  }
</script>

<section class="ask" {hidden} aria-label={m.assistant.title}>
  <div class="messages" bind:this={list}>
    {#if !messages.length}
      <div class="hello">
        <RupertOwl mood={busy ? mood : "idle"} size={132} />
        {#if status.available}
          <p>{m.assistant.hello}</p>
          <ul>
            {#each m.assistant.examples as example (example)}<li>{example}</li>{/each}
          </ul>
          <p class="privacy">{m.phone.askPrivacy}</p>
        {:else}
          <p class="unavailable">{status.message}</p>
        {/if}
      </div>
    {:else}
      <div class="owl-small"><RupertOwl {mood} size={56} /></div>
    {/if}
    {#each messages as msg, i (i)}
      {#if msg.role === "user"}
        <div class="msg user">{msg.text}</div>
      {:else if msg.role === "assistant"}
        {#if msg.steps.length}<div class="steps">{msg.steps.map((s) => s.summary).join(" · ")}</div>{/if}
        <div class="msg bot">
          {#each boldParts(msg.text) as p, j (j)}{#if p.bold}<strong>{p.text}</strong>{:else}{p.text}{/if}{/each}
        </div>
        {#if msg.actions.length}
          <div class="actions">
            {#each msg.actions as a, j (j)}
              <button class="chip" onclick={() => onaction(a)}>{label(a)}</button>
            {/each}
          </div>
        {/if}
      {:else if msg.role === "stopped"}
        <div class="stopped">{m.assistant.stopped}</div>
      {:else}
        <div class="msg error">{msg.text}</div>
      {/if}
    {/each}
    {#if busy}
      <div class="working">
        {stopping ? m.assistant.stopping : live.length ? m.common.ellipsis(live[live.length - 1].summary) : m.assistant.thinking}
      </div>
    {/if}
  </div>

  <form
    class="compose"
    onsubmit={(e) => {
      e.preventDefault();
      send();
    }}
  >
    <input
      type="text"
      bind:value={draft}
      placeholder={m.assistant.placeholder}
      disabled={busy || !status.available}
      enterkeyhint="send"
      autocomplete="off"
    />
    {#if busy}
      <button type="button" class="send stop" onclick={stop} disabled={stopping} aria-label={m.assistant.stop}>
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><rect width="12" height="12" rx="2" fill="currentColor" /></svg>
      </button>
    {:else}
      <button type="submit" class="send" disabled={!draft.trim() || !status.available} aria-label={m.assistant.send}>↑</button>
    {/if}
  </form>
</section>

<style>
  .ask {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .ask[hidden] {
    display: none;
  }
  .messages {
    flex: 1;
    overflow-y: auto;
    -webkit-overflow-scrolling: touch;
    padding: 12px calc(env(safe-area-inset-right) + 16px) 12px calc(env(safe-area-inset-left) + 16px);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .hello {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    color: var(--muted);
    font-size: 15px;
    line-height: 1.45;
    margin-top: 8px;
  }
  .hello ul {
    list-style: none;
    padding: 0;
    margin: 4px 0 10px;
  }
  .hello li {
    margin: 4px 0;
  }
  .privacy {
    font-size: 13px;
  }
  .unavailable {
    color: var(--text);
    max-width: 30em;
  }
  .owl-small {
    display: flex;
    justify-content: center;
  }
  .msg {
    max-width: 85%;
    padding: 9px 12px;
    border-radius: 16px;
    line-height: 1.4;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    -webkit-user-select: text;
    user-select: text;
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
  .stopped,
  .working {
    font-size: 12px;
    color: var(--muted);
    padding: 0 4px;
  }
  .stopped {
    align-self: flex-end;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    border: 1px solid var(--accent);
    background: none;
    color: var(--accent);
    border-radius: 999px;
    padding: 6px 12px;
    font: inherit;
    font-size: 14px;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .compose {
    display: flex;
    gap: 8px;
    padding: 8px calc(env(safe-area-inset-right) + 12px) 8px calc(env(safe-area-inset-left) + 12px);
    border-top: 1px solid var(--border);
  }
  .compose input {
    flex: 1;
    min-width: 0;
    border: 1px solid var(--border);
    border-radius: 18px;
    padding: 8px 14px;
    font: inherit;
    font-size: 16px;
    background: var(--bg);
    color: var(--text);
  }
  .send {
    width: 36px;
    height: 36px;
    flex: none;
    border: 0;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    font-size: 18px;
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
</style>
