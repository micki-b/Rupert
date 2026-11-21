#!/bin/bash
# capture.sh <lang> <question> <outdir>: App Store screenshots of Rupert (iPhone) in one language.
# Build the simulator app first: npm run phone -- ios build --debug --target aarch64-sim
# (then restore mobile/gen/apple/rupert-mobile_iOS/Info.plist with git checkout).
set -u
S=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$S/../../../.." && pwd); BID=io.github.micki-b.rupert
# The "Rupert Media" simulator (iPhone 17 Pro Max: 1320 × 2868 screenshots), made once.
D=$(xcrun simctl list devices | grep "Rupert Media (" | head -1 | grep -oE "[0-9A-F-]{36}")
[ -z "$D" ] && D=$(xcrun simctl create "Rupert Media" com.apple.CoreSimulator.SimDeviceType.iPhone-17-Pro-Max)
TMP=$(mktemp -d)
# Ask Rupert in the Simulator (it can't run Apple's model): an OpenAI-compatible server on the Mac.
ASSISTANT=${RUPERT_MEDIA_ASSISTANT:-http://localhost:1234|qwen/qwen3.6-35b-a3b}
APP=$REPO/mobile/gen/apple/build/arm64-sim/Rupert.app
LANG_=$1; Q=$2; OUT=$3; mkdir -p "$OUT"
xcrun simctl boot $D 2>/dev/null; xcrun simctl bootstatus $D -b >/dev/null 2>&1
xcrun simctl ui $D appearance light
xcrun simctl status_bar $D override --time 9:41 --batteryState discharging --batteryLevel 100 --cellularBars 4 --wifiBars 3 --operatorName ""
xcrun simctl terminate $D $BID 2>/dev/null; xcrun simctl uninstall $D $BID 2>/dev/null
xcrun simctl install $D "$APP"
C=$(xcrun simctl get_app_container $D $BID data)
mkdir -p "$C/Documents"; cp -R "$REPO/docs/app-store/tools/sample-library/Home Documents" "$C/Documents/"
LIB="$C/Documents/Home Documents"
# Text for the phone comes from the catalog Rupert on a computer writes.
rm -f $TMP/index.db*
CATALOG=1 $REPO/target/release/examples/index_check $REPO/src-tauri/pdfium "$LIB" $TMP/index.db > /dev/null 2>&1
ls "$LIB/.rupert" 
run() { # $1 js file, $2 log; waits for "done"/"ready", screenshots on "shot <name>"
  local js; js=$(sed -e "s/__LANG__/$LANG_/" -e "s|__QUESTION__|$Q|" "$1")
  : > "$2"
  xcrun simctl spawn $D log stream --style compact --predicate 'process == "Rupert"' > "$2" 2>&1 &
  local logpid=$!
  sleep 2
  SIMCTL_CHILD_RUPERT_TEST_FOLDER="$LIB" SIMCTL_CHILD_RUPERT_TEST_INBOX="$LIB/Inbox" \
  SIMCTL_CHILD_RUPERT_TEST_ASSISTANT="$ASSISTANT" \
  SIMCTL_CHILD_RUPERT_TEST_JS="$js" xcrun simctl launch --terminate-running-process $D $BID > /dev/null 2>&1
  local seen=""
  for i in $(seq 1 400); do
    for name in $(grep -o "rupert-test: shot [0-9a-z-]*" "$2" | awk '{print $3}'); do
      case " $seen " in *" $name "*) ;; *) sleep 1.2; xcrun simctl io $D screenshot --type=png "$OUT/$name.png" >/dev/null 2>&1; seen="$seen $name";; esac
    done
    grep -q "rupert-test: \(done\|ready\)" "$2" && break; sleep 1
  done
  xcrun simctl terminate $D $BID 2>/dev/null; kill $logpid 2>/dev/null; wait $logpid 2>/dev/null
}
run $S/setup.js "$OUT/setup.log"
run $S/scenes.js "$OUT/scenes.log"
grep "rupert-test:" "$OUT/setup.log" "$OUT/scenes.log" | cut -c1-300
ls "$OUT"
