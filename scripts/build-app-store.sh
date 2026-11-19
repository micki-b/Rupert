#!/usr/bin/env bash
# Builds Rupert's Mac App Store edition: the sandboxed app
# (src-tauri/tauri.appstore.conf.json, AppStore.entitlements) without the
# self-updater or the `ant` sign-in.
#
#   scripts/build-app-store.sh --test     Sandboxed app to try on this Mac:
#                                         "Rupert Test" (io.github.micki-b.rupert-test,
#                                         its own container), signed with your
#                                         "Apple Development" identity. No profile needed.
#   scripts/build-app-store.sh            Submission build: universal app with the
#                                         provisioning profile, signed for the Mac App
#                                         Store, packaged as target/Rupert-<version>.pkg.
#   scripts/build-app-store.sh --upload   …and uploads it to App Store Connect.
#
# Submission needs (see docs/RELEASING.md):
#   RUPERT_MAS_PROFILE   path to the "Mac App Store Connect" provisioning profile
#   In the keychain: "Apple Distribution: …" (or "3rd Party Mac Developer Application: …")
#   and "3rd Party Mac Developer Installer: …" certificates.
#   --upload: APPLE_API_KEY (Key ID) and APPLE_API_ISSUER, with the key file at
#   ~/.appstoreconnect/private_keys/AuthKey_<Key ID>.p8 (where altool looks).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
TEAM="${APPLE_TEAM_ID:-9FP28N8GW3}"
BUNDLE_ID="io.github.micki-b.rupert"
MODE="${1:-submit}"
OUT="$ROOT/target/app-store"
mkdir -p "$OUT"
# Notarization is for the DMG edition; Tauri would try it with these set.
unset APPLE_ID APPLE_PASSWORD
export TAURI_APP_PATH=src-tauri
# Rust from Homebrew's rustup, which isn't on every shell's PATH.
[ -d /opt/homebrew/opt/rustup/bin ] && export PATH="/opt/homebrew/opt/rustup/bin:$PATH"

identity() { # pattern
  security find-identity -v | sed -n "s/.*\"\(.*$1.*\)\"/\1/p" | head -1
}

if [ "$MODE" = "--test" ]; then
  SIGN="$(identity "Apple Development")"
  [ -n "$SIGN" ] || { echo "No \"Apple Development\" identity in the keychain" >&2; exit 1; }
  cat > "$OUT/test.conf.json" <<EOF
{
  "identifier": "io.github.micki-b.rupert-test",
  "productName": "Rupert Test",
  "app": { "windows": [{ "title": "Rupert Test", "width": 1200, "height": 800, "minWidth": 800, "minHeight": 500 }] },
  "bundle": { "macOS": { "signingIdentity": "$SIGN" } }
}
EOF
  # Not notarized: keep Tauri from trying.
  env -u APPLE_API_KEY -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH \
    npm run tauri build -- --debug --bundles app \
    --config src-tauri/tauri.appstore.conf.json --config "$OUT/test.conf.json"
  APP="$ROOT/target/debug/bundle/macos/Rupert Test.app"
  codesign -d --entitlements - "$APP" 2>/dev/null | grep -q app-sandbox \
    && echo "Sandboxed: yes ($APP)" || { echo "Not sandboxed?" >&2; exit 1; }
  # `tauri build` renames the dev binary on a case-insensitive disk (CLAUDE.md).
  [ -e target/debug/rupert ] && mv target/debug/rupert target/debug/Rupert.tmp && mv target/debug/Rupert.tmp target/debug/Rupert
  exit 0
fi

# MARK: Submission build

PROFILE="${RUPERT_MAS_PROFILE:?set RUPERT_MAS_PROFILE to the Mac App Store provisioning profile}"
APP_SIGN="$(identity "Apple Distribution")"
[ -n "$APP_SIGN" ] || APP_SIGN="$(identity "3rd Party Mac Developer Application")"
PKG_SIGN="$(identity "3rd Party Mac Developer Installer")"
[ -n "$APP_SIGN" ] || { echo "No \"Apple Distribution\" identity in the keychain" >&2; exit 1; }
[ -n "$PKG_SIGN" ] || { echo "No \"3rd Party Mac Developer Installer\" identity in the keychain" >&2; exit 1; }

# Entitlements for submission: the sandbox ones plus the app and team ids.
ENT="$OUT/AppStore.submit.entitlements"
cp src-tauri/AppStore.entitlements "$ENT"
# PlistBuddy, not plutil: plutil reads the dots in these keys as a path.
/usr/libexec/PlistBuddy -c "Add :com.apple.application-identifier string $TEAM.$BUNDLE_ID" \
  -c "Add :com.apple.developer.team-identifier string $TEAM" "$ENT"

# Every upload needs a higher build number.
BUILD="$(date -u +%y%m%d%H%M)"
cat > "$OUT/submit.conf.json" <<EOF
{
  "bundle": {
    "macOS": {
      "signingIdentity": "$APP_SIGN",
      "entitlements": "$ENT",
      "bundleVersion": "$BUILD",
      "files": { "embedded.provisionprofile": "$PROFILE" }
    }
  }
}
EOF

PDFIUM_TARGET=universal ./scripts/fetch-pdfium.sh
env -u APPLE_API_KEY -u APPLE_API_ISSUER -u APPLE_API_KEY_PATH \
  npm run tauri build -- --target universal-apple-darwin --bundles app \
  --config src-tauri/tauri.appstore.conf.json --config "$OUT/submit.conf.json"

APP="$ROOT/target/universal-apple-darwin/release/bundle/macos/Rupert.app"
VERSION="$(plutil -extract CFBundleShortVersionString raw "$APP/Contents/Info.plist")"
PKG="$ROOT/target/Rupert-$VERSION-$BUILD.pkg"
productbuild --sign "$PKG_SIGN" --component "$APP" /Applications "$PKG"
echo "Built $PKG"

if [ "$MODE" = "--upload" ]; then
  xcrun altool --upload-app --type macos --file "$PKG" \
    --apiKey "${APPLE_API_KEY:?set APPLE_API_KEY}" --apiIssuer "${APPLE_API_ISSUER:?set APPLE_API_ISSUER}"
fi
