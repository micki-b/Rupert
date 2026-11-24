// Preview 3: Ask Rupert finds a document and answers from it (Apple Intelligence on the Mac).
const cap = (texts) => log(`mark cap:${t(texts)} ${Date.now()}`);
await sleep(1000);
folder("Receipts");
await sleep(800);
view("List");
await sleep(6500); // the recorder starts meanwhile
mark("start");
cap({ en: "Ask Rupert about your documents", es: "Preguntale a Rupert por tus documentos", de: "Fragen Sie Rupert nach Ihren Dokumenten" });
await sleep(1000);
document.querySelector("button.fab")?.click();
await sleep(1400);
const box = document.querySelector(".panel textarea");
await type(box, QUESTION, 55);
await sleep(600);
cap({ en: "He searches your library on your Mac", es: "Busca en tu biblioteca, en tu Mac", de: "Er durchsucht Ihre Bibliothek auf Ihrem Mac" });
document.querySelector(".panel button.send")?.click();
mark("sent");
for (let i = 0; i < 30 && !document.querySelector(".panel button.send.stop"); i++) await sleep(100);
for (let i = 0; i < 240 && document.querySelector(".panel button.send.stop"); i++) await sleep(250);
mark("answered");
cap({ en: "Answers come from your own files", es: "Las respuestas salen de tus archivos", de: "Antworten aus Ihren eigenen Dateien" });
await sleep(5500);
mark("end");
log("answer: " + (document.querySelector(".panel")?.innerText ?? "").slice(-600).replace(/\n/g, " ⏎ "));
log("ready");
