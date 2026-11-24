// Preview 2: new scans in the inbox get suggestions and are filed with a click.
const cap = (texts) => log(`mark cap:${t(texts)} ${Date.now()}`);
await sleep(1000);
folder("Receipts");
await sleep(800);
view("List");
for (let i = 0; i < 60 && (await T.invoke("inbox_items")).some((x) => x.stage !== "ready"); i++) await sleep(1000);
await sleep(6500); // the recorder starts meanwhile
mark("start");
cap({ en: "New scans arrive in your inbox", es: "Los escaneos nuevos llegan a tu bandeja", de: "Neue Scans landen in Ihrem Eingang" });
await sleep(1200);
inboxItem()?.click();
await sleep(2600);
cap({ en: "Rupert reads them and suggests the details", es: "Rupert los lee y sugiere los datos", de: "Rupert liest sie und schlägt Angaben vor" });
await sleep(3600);
cap({ en: "File each one with a click", es: "Archivá cada uno con un clic", de: "Jeden mit einem Klick ablegen" });
for (let i = 0; i < 3; i++) {
  fileButton()?.click();
  await sleep(i < 2 ? 2600 : 2000);
}
cap({ en: "Filed where it belongs, details inside", es: "Archivado donde va, con sus datos adentro", de: "Abgelegt, wo er hingehört, mit allen Angaben" });
showFiled()?.click();
await sleep(4200);
mark("end");
log("ready");
