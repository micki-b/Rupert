(async () => {
  const T = window.__TAURI_INTERNALS__;
  const log = (m) => T.invoke("test_log", { message: String(m) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const rows = () => [...document.querySelectorAll("button.row")];
  const row = (t) => rows().find((b) => b.textContent.includes(t));
  const tab = (t) => document.querySelector(`[data-tab="${t}"]`)?.click();
  async function type(input, text, ms = 70) {
    input.focus();
    for (let i = 1; i <= text.length; i++) {
      input.value = text.slice(0, i);
      input.dispatchEvent(new Event("input", { bubbles: true }));
      await sleep(ms);
    }
  }
  try {
    await sleep(2000);
    log("mark start");
    await sleep(1500);
    row("Home Documents")?.click();
    await sleep(1600);
    row("Insurance")?.click();
    await sleep(2200);
    tab("search");
    await sleep(600);
    await type(document.querySelector('input[type="search"]'), "insurance", 110);
    document.activeElement?.blur();
    await sleep(1800);
    row("Car insurance")?.click();
    await sleep(2800);
    document.querySelector(".backdrop")?.click();
    await sleep(900);
    tab("ask");
    await sleep(1200);
    const input = document.querySelector(".compose input");
    await type(input, "__QUESTION__", 55);
    await sleep(500);
    document.querySelector(".compose").requestSubmit();
    document.activeElement?.blur();
    log("mark asked");
    for (let i = 0; i < 600 && !document.querySelector(".msg.bot, .msg.error"); i++) await sleep(250);
    log("mark answered");
    await sleep(4500);
    document.querySelector(".chip")?.click();
    await sleep(3000);
    document.querySelector(".backdrop")?.click();
    await sleep(800);
    tab("scan");
    await sleep(3000);
    log("mark end");
  } catch (e) { log("failed " + e); }
  log("done");
})();
