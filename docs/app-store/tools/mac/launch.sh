#!/bin/bash
# launch.sh <WxH> <scene.js> <log>: (re)starts the capture copy of Rupert
# (see ../README.md) with a scene script and prints "<pid> <window id>".
# In scene scripts, __LIB__ is replaced with $RUPERT_MEDIA_LIB.
M="$(cd "$(dirname "$0")" && pwd)"
REPO="$(git -C "$M" rev-parse --show-toplevel)"
APP="$REPO/target/debug/bundle/macos/Rupert.app"
[ -x "$M/winid" ] || swiftc -O "$M/winid.swift" -o "$M/winid"
pkill -f 'bundle/macos/Rupert.app/Contents/MacOS/rupert'
sleep 1
JS="$(cat "$M/helpers.js" "$2")"
JS="${JS//__LIB__/${RUPERT_MEDIA_LIB:-}}"
(RUPERT_TEST_WINDOW_SIZE="$1" RUPERT_TEST_JS="(async()=>{ $JS })()" "$APP/Contents/MacOS/rupert" > "$3" 2>&1 &)
P=""
W=""
for i in $(seq 40); do
  P="$(pgrep -f 'bundle/macos/Rupert.app/Contents/MacOS/rupert' | head -1)"
  if [ -n "$P" ]; then W="$("$M/winid" "$P")"; fi
  if [ -n "$W" ]; then break; fi
  sleep 0.5
done
echo "$P $W"
