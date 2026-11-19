#!/usr/bin/env bash
# Downloads the prebuilt PDFium library (https://github.com/bblanchon/pdfium-binaries)
# for this platform into src-tauri/pdfium/, where the app bundle picks it up.
# On macOS, PDFIUM_TARGET=universal fetches both Apple silicon and Intel and
# combines them (for a universal app); otherwise the Mac's own kind.
# The version must match the `pdfium_XXXX` feature of pdfium-render in
# crates/rupert-core/Cargo.toml.
set -euo pipefail

VERSION="${PDFIUM_VERSION:-8125}"
DEST="$(cd "$(dirname "$0")/.." && pwd)/src-tauri/pdfium"

case "$(uname -s)-$(uname -m)" in
  Darwin-arm64)  ASSET=pdfium-mac-arm64;   LIB=lib/libpdfium.dylib ;;
  Darwin-x86_64) ASSET=pdfium-mac-x64;     LIB=lib/libpdfium.dylib ;;
  Linux-x86_64)  ASSET=pdfium-linux-x64;   LIB=lib/libpdfium.so ;;
  Linux-aarch64) ASSET=pdfium-linux-arm64; LIB=lib/libpdfium.so ;;
  MINGW*|MSYS*|CYGWIN*) ASSET=pdfium-win-x64; LIB=bin/pdfium.dll ;;
  *) echo "unsupported platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

STAMP="$VERSION${PDFIUM_TARGET:+-$PDFIUM_TARGET}"
if [ -f "$DEST/VERSION" ] && [ "$(cat "$DEST/VERSION")" = "$STAMP" ]; then
  echo "PDFium $VERSION already in $DEST"; exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
fetch() { # asset, folder
  local url="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${VERSION}/$1.tgz"
  echo "Downloading $url"
  curl -fsSL "$url" -o "$TMP/$1.tgz"
  mkdir -p "$TMP/$2" && tar -xzf "$TMP/$1.tgz" -C "$TMP/$2"
}

rm -rf "$DEST" && mkdir -p "$DEST"
if [ "$(uname -s)" = Darwin ] && [ "${PDFIUM_TARGET:-}" = universal ]; then
  fetch pdfium-mac-arm64 arm && fetch pdfium-mac-x64 x
  lipo -create "$TMP/arm/$LIB" "$TMP/x/$LIB" -output "$DEST/libpdfium.dylib"
else
  fetch "$ASSET" x
  cp "$TMP/x/$LIB" "$DEST/"
fi
# macOS: PDFium must not need a newer macOS than Rupert says it runs on
# (minimumSystemVersion in src-tauri/tauri.macos.conf.json).
if [ "$(uname -s)" = Darwin ]; then
  APP_MIN="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["bundle"]["macOS"]["minimumSystemVersion"])' \
    "$(dirname "$DEST")/tauri.macos.conf.json")"
  for LIB_MIN in $(vtool -show-build "$DEST/libpdfium.dylib" | awk '/minos/{print $2}'); do
    if [ "$(printf '%s\n%s\n' "$LIB_MIN" "$APP_MIN" | sort -V | tail -1)" != "$APP_MIN" ]; then
      echo "PDFium $VERSION needs macOS $LIB_MIN, but Rupert's minimum is $APP_MIN (src-tauri/tauri.macos.conf.json)" >&2
      rm -rf "${DEST:?}"; exit 1
    fi
  done
fi
cp "$TMP/x/LICENSE" "$DEST/LICENSE-pdfium.txt"
[ -d "$TMP/x/licenses" ] && cp -R "$TMP/x/licenses" "$DEST/licenses"
echo "$STAMP" > "$DEST/VERSION"
echo "PDFium $VERSION → $DEST"
