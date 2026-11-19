const T = window.__TAURI_INTERNALS__;
const log = (m) => T.invoke("test_log", { message: String(m) });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const $$ = (sel) => [...document.querySelectorAll(sel)];
const byText = (sel, text) => $$(sel).find((e) => e.textContent.trim().startsWith(text));
const folder = (name) => byText("[role=treeitem]", name)?.click();
const view = (label) => document.querySelector(`.view-switch button[aria-label="${label}"]`)?.click();
async function type(input, text, ms = 90) {
  input.focus();
  for (let i = 1; i <= text.length; i++) {
    input.value = text.slice(0, i);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await sleep(ms);
  }
}
const mark = (name) => log(`mark ${name} ${Date.now()}`);
