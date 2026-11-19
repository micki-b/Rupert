// Preview 2: new scans in the inbox get suggestions and are filed with a click.
const cap = (text) => log(`mark cap:${text} ${Date.now()}`);
const fileBtn = () => $$(".inbox .actions button.primary").find((b) => b.textContent.trim() === "File");
await sleep(1000);
folder("Receipts");
await sleep(800);
view("List");
for (let i = 0; i < 60 && (await T.invoke("inbox_items")).some((x) => x.stage !== "ready"); i++) await sleep(1000);
await sleep(6500); // the recorder starts meanwhile
mark("start");
cap("New scans arrive in your inbox");
await sleep(1200);
byText(".lib-item", "Inbox")?.click();
await sleep(2600);
cap("Rupert reads them and suggests the details");
await sleep(3600);
cap("File each one with a click");
for (let i = 0; i < 3; i++) {
  fileBtn()?.click();
  await sleep(i < 2 ? 2600 : 2000);
}
cap("Filed where it belongs, details inside");
byText(".inbox .filed button.link", "Show")?.click();
await sleep(4200);
mark("end");
log("ready");
