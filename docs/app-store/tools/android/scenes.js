(async () => {
  const T = window.__TAURI_INTERNALS__;
  const log = (m) => T.invoke("test_log", { message: String(m) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const rows = () => [...document.querySelectorAll("button.row")];
  const row = (t) => rows().find((b) => b.textContent.includes(t));
  const tab = (t) => document.querySelector(`[data-tab="${t}"]`)?.click();
  const shot = async (n) => { log("shot " + n); await sleep(3500); };
  try {
    await sleep(2500);
    row("Home Documents")?.click();
    await sleep(2000);
    row("Insurance")?.click();
    await sleep(3000);
    await shot("1-folder");
    tab("search");
    await sleep(800);
    const q = document.querySelector('input[type="search"]');
    q.value = "insurance";
    q.dispatchEvent(new Event("input", { bubbles: true }));
    q.blur();
    await sleep(3000);
    await shot("2-search");
    row("Car insurance")?.click();
    await sleep(3000);
    await shot("3-document");
    document.querySelector(".backdrop")?.click();
    await sleep(1200);
    tab("scan");
    await sleep(2500);
    await shot("4-scan");
  } catch (e) { log("failed " + e); }
  log("done");
})();
