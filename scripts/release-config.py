#!/usr/bin/env python3
"""Writes the extra Tauri configuration a release build uses
(`target/release.conf.json`, passed with `--config`):

- `createUpdaterArtifacts`: signed update bundles for the updater (needs
  TAURI_SIGNING_PRIVATE_KEY and its password in the environment).
- Windows: code signing with Azure Trusted Signing (`trusted-signing-cli`)
  when AZURE_SIGNING_ENDPOINT, AZURE_SIGNING_ACCOUNT and
  AZURE_SIGNING_PROFILE are set; the service's sign-in comes from
  AZURE_CLIENT_ID / AZURE_CLIENT_SECRET / AZURE_TENANT_ID.

macOS signing and notarization need no file: Tauri reads APPLE_CERTIFICATE,
APPLE_SIGNING_IDENTITY and APPLE_API_* from the environment.
"""

import json
import os
import platform
from pathlib import Path

conf: dict = {"bundle": {"createUpdaterArtifacts": True}}

endpoint = os.environ.get("AZURE_SIGNING_ENDPOINT")
account = os.environ.get("AZURE_SIGNING_ACCOUNT")
profile = os.environ.get("AZURE_SIGNING_PROFILE")
if platform.system() == "Windows" and endpoint and account and profile:
    conf["bundle"]["windows"] = {
        "signCommand": f"trusted-signing-cli -e {endpoint} -a {account} -c {profile} -d Rupert %1"
    }
    print("Windows builds will be signed with Azure Trusted Signing")
elif platform.system() == "Windows":
    print("Windows builds will NOT be signed (Azure Trusted Signing isn't configured)")

out = Path(__file__).resolve().parent.parent / "target" / "release.conf.json"
out.parent.mkdir(exist_ok=True)
out.write_text(json.dumps(conf, indent=2))
print(f"wrote {out}")
