// Preview 1: find any document (folders, gallery, search inside documents).
const cap = (texts) => log(`mark cap:${t(texts)} ${Date.now()}`);
await sleep(1000);
folder("Receipts");
await sleep(800);
view("Gallery");
await sleep(7000); // the recorder starts meanwhile
mark("start");
cap({ en: "Your folders, just as they are", es: "Tus carpetas, tal como están", de: "Ihre Ordner, so wie sie sind" });
const frames = () => $$(".strip .frame");
for (const i of [1, 2, 3]) { await sleep(1300); frames()[i]?.click(); }
await sleep(1400);
cap({ en: "Every document at a glance", es: "Cada documento de un vistazo", de: "Jedes Dokument auf einen Blick" });
folder("Insurance");
await sleep(1500);
view("Large icons");
await sleep(2800);
cap({ en: "Search the text inside every document", es: "Buscá en el texto de cada documento", de: "Suchen Sie im Text jedes Dokuments" });
const search = searchBox();
await type(search, "warranty", 110);
await sleep(2200);
(byText("button.result", "Roof warranty") ?? $$("button.result")[0])?.click();
await sleep(2600);
cap({ en: "Find anything in seconds", es: "Encontrá cualquier cosa en segundos", de: "Alles in Sekunden finden" });
search.select();
await type(search, "car insurance", 95);
await sleep(1800);
$$("button.result")[0]?.click();
await sleep(3400);
mark("end");
log("ready");
