// Setup of the capture copy, once per language: the sample library as a
// source, its Inbox folder as the inbox, Rupert's language, and inbox
// suggestions and Ask Rupert on Local AI: Apple Intelligence (macOS 26 or
// later with it turned on), or with RUPERT_MEDIA_ENGINE=lmstudio LM Studio at
// its default address with qwen/qwen3.6-35b-a3b loaded.
try {
  // A source left from an earlier run points at a library copy that's gone.
  const lib = "__LIB__/Home Documents";
  for (const old of await T.invoke("list_sources")) {
    if (old.root !== lib) await T.invoke("remove_source", { id: old.id });
  }
  let src = (await T.invoke("list_sources")).find((s) => s.root === lib);
  if (!src) src = await T.invoke("add_source", { path: lib });
  const s = await T.invoke("get_settings");
  s.inbox = { sourceId: src.id, relPath: "Inbox" };
  s.ocr = true;
  s.suggestions = true;
  s.language = LANG;
  if ("__ENGINE__" === "lmstudio") {
    s.localEngine = "server";
    s.localUrl = "http://localhost:1234";
    s.localModel = "qwen/qwen3.6-35b-a3b";
  } else {
    s.localEngine = "apple";
  }
  s.assistant.provider = "local";
  await T.invoke("set_settings", { settings: s });
  // Let the inbox suggestions finish once, so scenes don't wait for them.
  for (let i = 0; i < 120 && (await T.invoke("inbox_items")).some((x) => x.stage !== "ready"); i++) await sleep(1000);
  log("setup done " + JSON.stringify(await T.invoke("apple_ai_status")));
} catch (e) {
  log("setup failed: " + e);
}
log("ready");
