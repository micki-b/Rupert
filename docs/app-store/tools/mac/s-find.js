// Preview 1: find any document (folders, gallery, search inside documents).
const cap = (text) => log(`mark cap:${text} ${Date.now()}`);
await sleep(1000);
folder("Receipts");
await sleep(800);
view("Gallery");
await sleep(7000); // the recorder starts meanwhile
mark("start");
cap("Your folders, just as they are");
const frames = () => $$(".strip .frame");
for (const i of [1, 2, 3]) { await sleep(1300); frames()[i]?.click(); }
await sleep(1400);
cap("Every document at a glance");
folder("Insurance");
await sleep(1500);
view("Large icons");
await sleep(2800);
cap("Search the text inside every document");
const search = document.querySelector('input[placeholder="Search all documents"]');
await type(search, "warranty", 110);
await sleep(2200);
(byText("button.result", "Roof warranty") ?? $$("button.result")[0])?.click();
await sleep(2600);
cap("Find anything in seconds");
search.select();
await type(search, "car insurance", 95);
await sleep(1800);
$$("button.result")[0]?.click();
await sleep(3400);
mark("end");
log("ready");
