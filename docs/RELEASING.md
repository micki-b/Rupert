# Releasing Rupert

## Desktop (macOS, Windows, Linux)

1. Set the version in `src-tauri/tauri.conf.json`, `mobile/tauri.conf.json`, `Cargo.toml` (`[workspace.package] version`) and `package.json`, and move the changelog's "Unreleased" entries under the new version.
2. If dependencies changed: `python3 scripts/third-party-notices.py` and commit `THIRD_PARTY_NOTICES.md`.
3. Commit, then tag and push: `git tag v1.0.0 && git push origin v1.0.0`.
4. The **Release** workflow (`.github/workflows/release.yml`) builds a universal macOS app (DMG), Windows installers (`-setup.exe` and MSI) and Linux packages (AppImage, deb, rpm), signs what it can, and creates a **draft** GitHub release with them, the signed update bundles and `latest.json`.
5. Check the draft (download and try an installer), then **Publish**. The apps' updater only sees published releases: `https://github.com/micki-b/Rupert/releases/latest/download/latest.json`.

### Secrets and variables

Repository ▸ Settings ▸ Secrets and variables ▸ Actions. Each one is optional except the update key; without the others the installer for that platform is simply unsigned.

| Name | Kind | What |
|---|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | secret | Contents of `~/.tauri/rupert-updater.key` (on the release Mac). Required. |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | secret | Its password: Keychain Access ▸ "Rupert updater signing key password". |
| `APPLE_CERTIFICATE` | secret | The "Developer ID Application" certificate with its private key, exported from Keychain Access as `.p12`, then `base64 -i cert.p12 \| pbcopy`. |
| `APPLE_CERTIFICATE_PASSWORD` | secret | The password chosen when exporting the `.p12`. |
| `APPLE_SIGNING_IDENTITY` | secret | `Developer ID Application: <name> (<team id>)`, as `security find-identity -v -p codesigning` shows it. |
| `APPLE_API_ISSUER` | secret | App Store Connect ▸ Users and Access ▸ Integrations ▸ App Store Connect API: the Issuer ID. |
| `APPLE_API_KEY` | secret | The key's Key ID (role: Developer). |
| `APPLE_API_KEY_P8` | secret | The contents of the downloaded `AuthKey_<id>.p8` (it can be downloaded only once). |
| `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, `AZURE_TENANT_ID` | secrets | An Azure app registration allowed to sign ("Trusted Signing Certificate Profile Signer" role on the signing account). |
| `AZURE_SIGNING_ENDPOINT` | variable | The Trusted Signing account's endpoint, e.g. `https://eus.codesigning.azure.net`. |
| `AZURE_SIGNING_ACCOUNT` | variable | The Trusted Signing account name. |
| `AZURE_SIGNING_PROFILE` | variable | The certificate profile name. |

**Keep `~/.tauri/rupert-updater.key` and its password safe** (e.g. in a password manager). If it's lost, installed copies can't verify new updates and have to be reinstalled by hand.

## iPhone

App Store builds are made with Xcode from `mobile/gen/apple` (Product ▸ Archive, then upload to App Store Connect), after `npm run phone -- ios build`. The App Store listing text and privacy answers are in `docs/app-store/`.
