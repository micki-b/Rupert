#!/usr/bin/env python3
"""Collects the release files the build jobs uploaded and writes `latest.json`
for Tauri's updater (https://v2.tauri.app/plugin/updater/#static-json-file).

Usage: release-manifest.py <artifacts dir> <tag> <out dir>
Copies the files to publish into <out dir> (with versioned names) and
writes <out dir>/latest.json pointing at them on the GitHub release.
"""

import json
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path

REPO = "micki-b/Rupert"

src, tag, out = Path(sys.argv[1]), sys.argv[2], Path(sys.argv[3])
version = tag.removeprefix("v")
out.mkdir(parents=True, exist_ok=True)
files = [p for p in src.rglob("*") if p.is_file()]


def publish(path: Path, name: str | None = None) -> str:
    name = name or path.name
    shutil.copy2(path, out / name)
    return name


def url(name: str) -> str:
    return f"https://github.com/{REPO}/releases/download/{tag}/{name}"


platforms: dict[str, dict] = {}


def updater(bundle: Path, name: str, keys: list[str]) -> None:
    sig = bundle.with_name(bundle.name + ".sig")
    if not sig.exists():
        sys.exit(f"missing signature for {bundle.name}: is TAURI_SIGNING_PRIVATE_KEY set?")
    published = publish(bundle, name)
    publish(sig, published + ".sig")
    for key in keys:
        platforms[key] = {"signature": sig.read_text().strip(), "url": url(published)}


for f in files:
    n = f.name
    if n.endswith(".sig"):
        continue
    if n.endswith(".dmg") or n.endswith(".msi") or n.endswith(".deb") or n.endswith(".rpm"):
        publish(f)
    elif n.endswith(".app.tar.gz"):
        # The universal app updates both kinds of Mac.
        updater(f, f"Rupert_{version}_universal.app.tar.gz", ["darwin-aarch64", "darwin-x86_64"])
    elif n.endswith("-setup.exe"):
        updater(f, n, ["windows-x86_64"])
    elif n.endswith(".AppImage"):
        updater(f, n, ["linux-x86_64"])

missing = {"darwin-aarch64", "windows-x86_64", "linux-x86_64"} - platforms.keys()
if missing:
    sys.exit(f"no update bundle for: {', '.join(sorted(missing))}")

manifest = {
    "version": version,
    "notes": f"Rupert {version}. See https://github.com/{REPO}/releases/tag/{tag}",
    "pub_date": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "platforms": platforms,
}
(out / "latest.json").write_text(json.dumps(manifest, indent=2))
print(json.dumps(manifest, indent=2))
print(sorted(p.name for p in out.iterdir()))
