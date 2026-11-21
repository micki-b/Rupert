(async () => {
  const T = window.__TAURI_INTERNALS__;
  const log = (m) => T.invoke("test_log", { message: String(m) });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  await T.invoke("set_language", { language: "__LANG__" });
  for (let i = 0; i < 120; i++) {
    const o = await T.invoke("overview");
    if (o.ready && !o.progress.active && o.stats.documents > 0) { log("indexed " + o.stats.documents); break; }
    await sleep(1000);
  }
  log("ready");
})();
