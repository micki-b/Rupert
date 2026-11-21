#!/bin/bash
# capture.sh <lang> <outdir>: Google Play screenshots of Rupert (Android) in one language,
# 1080 × 1920, light mode, 9:41, from the "Rupert Pixel" emulator (running) and a debug build:
#   npm run phone -- android build --debug --target aarch64 --apk
#   cargo build --release -p rupert-core --example index_check
# Reinstalls the app, so the emulator's Rupert loses its sign-ins.
# The scripts run through the debug test hooks (mobile/src/lib.rs), which Android reads
# from rupert-test.env in the app's data folder, one line per name.
set -u
S=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$S/../../../.." && pwd)
PKG=io.github.micki_b.rupert; DATA=/data/user/0/$PKG
A="${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb"
APK=$REPO/mobile/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk
LANG_=$1; OUT=$2; mkdir -p "$OUT"; TMP=$(mktemp -d)

# Google Play wants at most 2:1; 9:16 fits every slot.
$A shell wm size 1080x1920
$A shell cmd uimode night no
$A shell settings put global sysui_demo_allowed 1
demo() { $A shell am broadcast -a com.android.systemui.demo -e command "$@" > /dev/null; }
demo enter; demo clock -e hhmm 0941; demo network -e mobile hide -e wifi show -e fully true
demo battery -e level 100 -e plugged false; demo notifications -e visible false

# A fresh app with a copy of the sample library and its catalog.
$A uninstall $PKG > /dev/null 2>&1; $A install "$APK" > /dev/null
cp -R "$REPO/docs/app-store/tools/sample-library/Home Documents" "$TMP/"
CATALOG=1 "$REPO/target/release/examples/index_check" "$REPO/src-tauri/pdfium" "$TMP/Home Documents" "$TMP/index.db" > /dev/null 2>&1
(cd "$TMP" && tar cf lib.tar "Home Documents")
$A push "$TMP/lib.tar" /data/local/tmp/rupert-lib.tar > /dev/null
$A shell "run-as $PKG sh -c 'cd files && tar xf /data/local/tmp/rupert-lib.tar'" 2>/dev/null \
  || { $A shell "run-as $PKG mkdir -p files" && $A shell "run-as $PKG sh -c 'cd files && tar xf /data/local/tmp/rupert-lib.tar'"; }
$A shell rm /data/local/tmp/rupert-lib.tar

run() { # $1 js file, $2 log; screenshots on "shot <name>", ends on "done"/"ready"
  local js; js=$(sed -e "s/__LANG__/$LANG_/" "$1" | tr '\n' ' ')
  printf 'RUPERT_TEST_FOLDER=%s\nRUPERT_TEST_INBOX=%s\nRUPERT_TEST_JS=%s\n' \
    "$DATA/files/Home Documents" "$DATA/files/Home Documents/Inbox" "$js" > "$TMP/rupert-test.env"
  $A push "$TMP/rupert-test.env" /data/local/tmp/rupert-test.env > /dev/null
  $A shell "run-as $PKG cp /data/local/tmp/rupert-test.env $DATA/rupert-test.env"
  $A shell am force-stop $PKG; $A logcat -c
  $A shell am start -n $PKG/.MainActivity > /dev/null
  local seen=""
  for i in $(seq 1 300); do
    $A logcat -d -s Rupert:I > "$2"
    for name in $(grep -o "rupert-test: shot [0-9a-z-]*" "$2" | awk '{print $3}'); do
      case " $seen " in *" $name "*) ;; *) sleep 1.2; $A exec-out screencap -p > "$OUT/$name.png"; seen="$seen $name";; esac
    done
    grep -q "rupert-test: \(done\|ready\)" "$2" && break; sleep 1
  done
}
run "$REPO/docs/app-store/tools/ios/setup.js" "$OUT/setup.log"
run "$S/scenes.js" "$OUT/scenes.log"
grep -h "rupert-test:" "$OUT/setup.log" "$OUT/scenes.log" | cut -c1-200

# Back to normal: the test hooks off, the emulator's own screen and status bar.
$A shell "run-as $PKG rm -f $DATA/rupert-test.env"; $A shell rm -f /data/local/tmp/rupert-test.env
$A shell am force-stop $PKG; $A shell wm size reset; demo exit
rm -rf "$TMP"; ls "$OUT"
