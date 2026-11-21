// One-time setup of the capture copy: the sample library as a source, its
// Inbox folder as the inbox, suggestions and the assistant on Local AI (LM Studio, the default address).
try {
  let src = (await T.invoke("list_sources")).find((s) => s.name === "Home Documents");
  if (!src) src = await T.invoke("add_source", { path: "__LIB__/Home Documents" });
  const s = await T.invoke("get_settings");
  s.inbox = { sourceId: src.id, relPath: "Inbox" };
  s.ocr = true;
  s.suggestions = true;
  s.assistant.provider = "local";
  await T.invoke("set_settings", { settings: s });
  // Let the inbox suggestions finish once, so scenes don't wait for them.
  for (let i = 0; i < 120 && (await T.invoke("inbox_items")).some((x) => x.stage !== "ready"); i++) await sleep(1000);
  log("setup done");
} catch (e) {
  log("setup failed: " + e);
}
log("ready");
