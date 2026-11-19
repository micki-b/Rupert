#!/usr/bin/env python3
"""Writes the extra Tauri configuration a release build uses
(`target/release.conf.json`, passed with `--config`):

- `createUpdaterArtifacts`: signed update bundles for the updater (needs
  TAURI_SIGNING_PRIVATE_KEY and its password in the environment).
- Windows: code signing with Azure Artifact Signing (scripts/windows-sign.ps1)
  when the release workflow has set up SignTool and the signing plug-in
  (RUPERT_SIGN_METADATA set).

macOS signing and notarization need no file: Tauri reads APPLE_CERTIFICATE,
APPLE_SIGNING_IDENTITY and APPLE_API_* from the environment.
"""

import json
import os
import platform
from pathlib import Path

conf: dict = {"bundle": {"createUpdaterArtifacts": True}}

if platform.system() == "Windows" and os.environ.get("RUPERT_SIGN_METADATA"):
    script = Path(__file__).resolve().parent / "windows-sign.ps1"
    conf["bundle"]["windows"] = {"signCommand": f"pwsh -NoProfile -File {script} %1"}
    print("Windows builds will be signed with Azure Artifact Signing")
elif platform.system() == "Windows":
    print("Windows builds will NOT be signed (Azure Artifact Signing isn't configured)")

out = Path(__file__).resolve().parent.parent / "target" / "release.conf.json"
out.parent.mkdir(exist_ok=True)
out.write_text(json.dumps(conf, indent=2))
print(f"wrote {out}")
