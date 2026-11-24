#!/bin/bash
# shots.sh <out dir>: the Mac App Store screenshots (1440 × 900, no alpha), in the
# language set by RUPERT_MEDIA_LANG (en, es, de), after setup.js for that language.
# Needs a fresh library copy with the three inbox scans (see ../README.md).
M="$(cd "$(dirname "$0")" && pwd)"
OUT="$1"
mkdir -p "$OUT"
WORK="$(mktemp -d)"
shot() { # name, scene js
  printf '%s\nlog("ready");\n' "$2" > "$WORK/$1.js"
  read -r P W < <("$M/launch.sh" 1440x900 "$WORK/$1.js" "$WORK/$1.log")
  for i in $(seq 90); do grep -q 'rupert-test: ready' "$WORK/$1.log" && break; sleep 1; done
  sleep 1
  screencapture -x -o -l "$W" "$OUT/$1.png"
  echo "$1: window $W"
}
shot 1-inbox 'await sleep(1000);
for (let i = 0; i < 60 && (await T.invoke("inbox_items")).some((x) => x.stage !== "ready"); i++) await sleep(1000);
inboxItem()?.click(); await sleep(3500);'
shot 2-search 'await sleep(1000);
const s = searchBox();
await type(s, "warranty", 10); await sleep(1800);
(byText("button.result", "Laptop") ?? $$("button.result")[0])?.click(); await sleep(1500);'
shot 3-gallery 'await sleep(1000); folder("Receipts"); await sleep(1200); view("Gallery"); await sleep(1500);
$$(".strip .frame")[3]?.click(); await sleep(2500);'
shot 4-ask 'await sleep(1000); folder("Receipts"); await sleep(800); view("List"); await sleep(800);
document.querySelector("button.fab")?.click(); await sleep(1200);
await type(document.querySelector(".panel textarea"), QUESTION, 5);
document.querySelector(".panel button.send")?.click();
for (let i = 0; i < 30 && !document.querySelector(".panel button.send.stop"); i++) await sleep(100);
for (let i = 0; i < 240 && document.querySelector(".panel button.send.stop"); i++) await sleep(250);
await sleep(1500);'
pkill -f 'bundle/macos/Rupert.app/Contents/MacOS/rupert'
# App Store Connect refuses alpha channels (the window's rounded corners).
swift "$M/flatten.swift" "$OUT"/*.png
