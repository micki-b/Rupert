// Preview 3: Ask Rupert finds a document and answers from it (local model).
const cap = (text) => log(`mark cap:${text} ${Date.now()}`);
await sleep(1000);
folder("Receipts");
await sleep(800);
view("List");
await sleep(6500); // the recorder starts meanwhile
mark("start");
cap("Ask Rupert about your documents");
await sleep(1000);
document.querySelector("button.fab")?.click();
await sleep(1400);
const box = document.querySelector(".panel textarea");
await type(box, "How much did the plumber charge in April, and what for?", 55);
await sleep(600);
cap("He searches your library on your Mac");
document.querySelector(".panel button.send")?.click();
mark("sent");
for (let i = 0; i < 30 && !document.querySelector(".panel button.send.stop"); i++) await sleep(100);
for (let i = 0; i < 240 && document.querySelector(".panel button.send.stop"); i++) await sleep(250);
mark("answered");
cap("Answers come from your own files");
await sleep(5500);
mark("end");
log("answer: " + (document.querySelector(".panel")?.innerText ?? "").slice(-600).replace(/\n/g, " ⏎ "));
log("ready");
