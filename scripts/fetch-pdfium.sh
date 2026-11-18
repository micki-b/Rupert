#!/usr/bin/env bash
# Downloads the prebuilt PDFium library (https://github.com/bblanchon/pdfium-binaries)
# for this platform into src-tauri/pdfium/, where the app bundle picks it up.
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

if [ -f "$DEST/VERSION" ] && [ "$(cat "$DEST/VERSION")" = "$VERSION" ]; then
  echo "PDFium $VERSION already in $DEST"; exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
URL="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${VERSION}/${ASSET}.tgz"
echo "Downloading $URL"
curl -fsSL "$URL" -o "$TMP/pdfium.tgz"
mkdir -p "$TMP/x" && tar -xzf "$TMP/pdfium.tgz" -C "$TMP/x"

rm -rf "$DEST" && mkdir -p "$DEST"
cp "$TMP/x/$LIB" "$DEST/"
cp "$TMP/x/LICENSE" "$DEST/LICENSE-pdfium.txt"
[ -d "$TMP/x/licenses" ] && cp -R "$TMP/x/licenses" "$DEST/licenses"
echo "$VERSION" > "$DEST/VERSION"
echo "PDFium $VERSION → $DEST"
