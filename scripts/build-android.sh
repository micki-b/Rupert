#!/usr/bin/env bash
# Builds Rupert for Android, signed with the upload key, for Google Play.
#
#   scripts/build-android.sh     target/android/Rupert-<version>-<code>.aab (Google Play)
#                                and …-<code>.apk (installs directly: adb install, or a download)
#
# Google Play re-signs the app with its own app signing key (Play App Signing);
# the upload key only proves an upload comes from us. See docs/RELEASING.md.
#
# Needs:
#   the upload key at ~/.android/rupert-upload.jks (RUPERT_UPLOAD_KEYSTORE for another path)
#   its password in the login keychain: service "rupert-android-upload"
#   JDK 17, the Android SDK and NDK (CLAUDE.md, "Commands")
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
OUT="$ROOT/target/android"
mkdir -p "$OUT"

# The tools, where Homebrew and the SDK manager put them on this Mac.
[ -d /opt/homebrew/opt/rustup/bin ] && export PATH="/opt/homebrew/opt/rustup/bin:$PATH"
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk@17/libexec/openjdk.jdk/Contents/Home}"
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
if [ -z "${NDK_HOME:-}" ]; then
  NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1)"
  export NDK_HOME
fi

# MARK: Upload key

export RUPERT_UPLOAD_KEYSTORE="${RUPERT_UPLOAD_KEYSTORE:-$HOME/.android/rupert-upload.jks}"
[ -f "$RUPERT_UPLOAD_KEYSTORE" ] || { echo "No upload key at $RUPERT_UPLOAD_KEYSTORE" >&2; exit 1; }
RUPERT_UPLOAD_PASSWORD="$(security find-generic-password -s rupert-android-upload -w)" \
  || { echo "No \"rupert-android-upload\" password in the keychain" >&2; exit 1; }
export RUPERT_UPLOAD_PASSWORD

# MARK: Build

# Every upload needs a higher version code. Like Apple's build numbers it's the
# build's UTC time: year, day of the year, hour, minute (263251430 for
# 2026-11-21 14:30), below Google Play's limit of 2,100,000,000 until 2099.
CODE="$((10#$(date -u +%y%j%H%M)))"
echo "{ \"bundle\": { \"android\": { \"versionCode\": $CODE } } }" > "$OUT/release.conf.json"

# 64- and 32-bit ARM phones, plus x86_64 (Chromebooks, emulators on Intel).
npm run phone -- android build --aab --apk --ci \
  --target aarch64 armv7 x86_64 --config "$OUT/release.conf.json"

VERSION="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' mobile/tauri.conf.json)"
BUILT="$ROOT/mobile/gen/android/app/build/outputs"
AAB="$OUT/Rupert-$VERSION-$CODE.aab"
APK="$OUT/Rupert-$VERSION-$CODE.apk"
cp "$BUILT/bundle/universalRelease/app-universal-release.aab" "$AAB"
cp "$BUILT/apk/universal/release/app-universal-release.apk" "$APK"

# MARK: Check the signature

"$ANDROID_HOME/build-tools/$(ls "$ANDROID_HOME/build-tools" | sort -V | tail -1)/apksigner" \
  verify --print-certs "$APK" | grep -q 'SHA-256' || { echo "The APK isn't signed" >&2; exit 1; }
"$JAVA_HOME/bin/jarsigner" -verify "$AAB" >/dev/null || { echo "The bundle isn't signed" >&2; exit 1; }
echo "Built $AAB"
echo "Built $APK"
