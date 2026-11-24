const T = window.__TAURI_INTERNALS__;
const log = (m) => T.invoke("test_log", { message: String(m) });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const $$ = (sel) => [...document.querySelectorAll(sel)];
const byText = (sel, text) => $$(sel).find((e) => e.textContent.trim().startsWith(text));
// The app's language for this take ("en", "es", "de"; RUPERT_MEDIA_LANG in launch.sh).
// The sample library stays in English (folder names, documents, search words);
// only Rupert's own text changes, so elements are found by place, not by label.
const LANG = "__LANG__";
const t = (texts) => texts[LANG] ?? texts.en;
const folder = (name) => byText("[role=treeitem]", name)?.click();
const view = (mode) =>
  $$('span.view-switch[role="group"] button')[{ List: 0, "Large icons": 1, Gallery: 2 }[mode]]?.click();
const searchBox = () => document.querySelector(".searchbar input");
const inboxItem = () => document.querySelector("button.lib-item"); // the first one is the inbox
const fileButton = () => document.querySelector(".inbox .actions button.primary");
const showFiled = () => document.querySelector(".inbox .filed button.link");
async function type(input, text, ms = 90) {
  input.focus();
  for (let i = 1; i <= text.length; i++) {
    input.value = text.slice(0, i);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    await sleep(ms);
  }
}
const mark = (name) => log(`mark ${name} ${Date.now()}`);
// Ask Rupert's question in each language (screenshot 4 and preview 3). The
// sample documents are in English and models search in the question's
// language ("plomero", "Klempner" find nothing), so Spanish and German ask by
// the company's name. Record those with RUPERT_MEDIA_ENGINE=lmstudio: in tests
// Apple Intelligence missed the invoice in Spanish and answered German in English.
const QUESTION = t({
  en: "How much did the plumber charge in April, and what for?",
  es: "¿Cuánto me cobró Pipeline Plumbing en abril, y por qué?",
  de: "Was hat Pipeline Plumbing im April berechnet, und wofür?",
});
