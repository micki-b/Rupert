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

## Mac App Store edition (in preparation)

A second macOS edition for the Mac App Store, built from the same code with the `app-store` feature (`src-tauri/tauri.appstore.conf.json`): it runs in the App Sandbox (`src-tauri/AppStore.entitlements`), keeps access to the folders the user picked with security-scoped bookmarks, has no self-updater (the App Store updates it) and no "Anthropic account through the `ant` command" sign-in (API keys work). It shares the iPhone app's App Store record and bundle ID (universal purchase). The DMG from GitHub stays the main Mac release; a Mac should have one edition or the other.

Try it locally (sandboxed "Rupert Test", its own container, signed with your Apple Development identity): `scripts/build-app-store.sh --test`, then open `target/debug/bundle/macos/Rupert Test.app`.

### One-time setup for submission

1. **App Store Connect:** in the Rupert app record, add the **macOS** platform (App Information ▸ the platform list).
2. **Certificates** (developer.apple.com ▸ Certificates): create an **Apple Distribution** certificate and a **Mac Installer Distribution** certificate, and install both (double-click) on the release Mac.
3. **Provisioning profile** (Profiles ▸ +): **Mac App Store Connect**, App ID `io.github.micki-b.rupert`, the Apple Distribution certificate. Download it.
4. For uploading: copy the API key file to `~/.appstoreconnect/private_keys/AuthKey_<Key ID>.p8` (where `altool` looks).

### Building and uploading

```sh
RUPERT_MAS_PROFILE=~/Downloads/Rupert_Mac_App_Store.provisionprofile \
APPLE_API_KEY=<Key ID> APPLE_API_ISSUER=<Issuer ID> \
scripts/build-app-store.sh --upload
```

It builds the universal sandboxed app with the profile and a new build number, packages `target/Rupert-<version>-<build>.pkg` signed with the installer certificate, and uploads it. The build then appears in App Store Connect under the macOS version, for TestFlight or review.

Notes for users moving between editions: each edition keeps its own settings, index and keychain items (the App Store edition lives in its sandbox container), so folders are added again and API keys or sign-ins entered again. The documents and their details are in the folders and stay as they are.

## iPhone

App Store builds are made with Xcode from `mobile/gen/apple` (Product ▸ Archive, then upload to App Store Connect), after `npm run phone -- ios build`. The App Store listing text and privacy answers are in `docs/app-store/`.
