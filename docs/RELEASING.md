# Releasing Rupert

## Desktop (macOS, Windows, Linux)

1. Set the version: `scripts/set-version.py 1.0.0`. It writes it into `Cargo.toml` (`[workspace.package] version`), `package.json`, `package-lock.json`, `src-tauri/tauri.conf.json` and `mobile/tauri.conf.json`, and refreshes `Cargo.lock`. Then move the changelog's "Unreleased" entries under the new version. Use a patch version (0.1.1) for bug fixes only, a minor one (0.2.0) for new features.
2. If dependencies changed: `python3 scripts/third-party-notices.py` and commit `THIRD_PARTY_NOTICES.md`.
3. Commit, then tag and push: `git tag v1.0.0 && git push origin v1.0.0`.
4. The **Release** workflow (`.github/workflows/release.yml`) builds a universal macOS app (DMG), Windows installers (`-setup.exe` and MSI) and Linux packages (AppImage, deb, rpm), signs what it can, and creates a **draft** GitHub release with them, the signed update bundles and `latest.json`.
5. Attach the Android app to the draft (it's built on the Mac, with the upload key): `scripts/build-android.sh`, copy the `.apk` to the name the website links, then upload it: `cp target/android/Rupert-1.0.0-<code>.apk /tmp/Rupert_1.0.0_android.apk && gh release upload v1.0.0 /tmp/Rupert_1.0.0_android.apk` (`gh`'s `file#name` only sets a display label, not the file name). See "Android" below.
6. Check the draft (download and try an installer), then **Publish**. The apps' updater only sees published releases: `https://github.com/micki-b/Rupert/releases/latest/download/latest.json`.
7. Upload the website: `npm run site -- --upload`. Its download links use the latest *published* release's version, and the build stops if a linked file (such as the Android `.apk`) isn't in that release.

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
| `AZURE_CLIENT_ID`, `AZURE_TENANT_ID` | secrets of the `release` environment | The Azure app registration that may sign. No password: see "Windows signing". |
| `AZURE_SIGNING_ENDPOINT`, `AZURE_SIGNING_ACCOUNT`, `AZURE_SIGNING_PROFILE` | variables | `https://wus2.codesigning.azure.net`, `mickibraun`, `prelease`. |

**Keep `~/.tauri/rupert-updater.key` and its password safe** (e.g. in a password manager). If it's lost, installed copies can't verify new updates and have to be reinstalled by hand.

## Windows signing (Azure Artifact Signing)

Without it the Windows installers are unsigned and SmartScreen warns about an "unknown publisher". Rupert uses the Azure Artifact Signing account set up for 2ndPressing (account `mickibraun`, Public Trust certificate profile `prelease`, region West US 2, endpoint `https://wus2.codesigning.azure.net`); the certificates are issued in the developer's name, so one profile signs any number of apps. Setting up a new account from scratch: [Microsoft's quickstart](https://learn.microsoft.com/azure/artifact-signing/quickstart) (individuals must live in the US or Canada).

**How it signs.** The release workflow's "Windows signing setup" step fetches Microsoft's SignTool plug-in (`Microsoft.ArtifactSigning.Client`, pinned) and writes its `metadata.json`; `tauri build` then calls `scripts/windows-sign.ps1` for `rupert.exe`, the uninstaller and both installers (PDFium's DLL is signed before the build). Sign-in uses no password: each signature fetches a fresh GitHub OIDC token for the run, which Azure accepts through a federated credential on the app registration. The workflow fails if any of those files isn't validly signed.

**One-time setup:**

1. **Azure** ([portal](https://portal.azure.com)) ▸ Microsoft Entra ID ▸ App registrations ▸ `github-secondpressing-signing` (the 2ndPressing app, which already has the **Artifact Signing Certificate Profile Signer** role on `prelease`) ▸ Certificates & secrets ▸ **Federated credentials** ▸ Add credential:
   - Scenario: **GitHub Actions deploying Azure resources**
   - Organization `micki-b`, repository `Rupert`, entity type **Environment**, environment `release`
   - Name `rupert-release`, audience `api://AzureADTokenExchange` (default)
   - The subject must read `repo:micki-b@163963902/Rupert@1462436924:environment:release` (the repository uses GitHub's immutable subject format with numeric IDs; if the portal shows `repo:micki-b/Rupert:environment:release`, pick the custom "Other issuer" scenario, issuer `https://token.actions.githubusercontent.com`, and enter the subject by hand).
2. **GitHub:** environment `release` with secrets `AZURE_CLIENT_ID` (the app's Application (client) ID) and `AZURE_TENANT_ID` (Directory (tenant) ID), and the three `AZURE_SIGNING_*` variables above.

Then a tagged release signs the Windows files. SmartScreen can still warn for a while after the first signed release, until the signed downloads build up reputation. Windows shows the certificate's name (the validated legal name) as the publisher.

## Mac App Store edition

A second macOS edition for the Mac App Store, built from the same code with the `app-store` feature (`src-tauri/tauri.appstore.conf.json`): it runs in the App Sandbox (`src-tauri/AppStore.entitlements`), keeps access to the folders the user picked with security-scoped bookmarks, has no self-updater (the App Store updates it) and no "Anthropic account through the `ant` command" sign-in (API keys work). It shares the iPhone app's App Store record and bundle ID (universal purchase). The DMG from GitHub stays the main Mac release; a Mac should have one edition or the other.

Try it locally (sandboxed "Rupert Test", its own container, signed with your Apple Development identity): `scripts/build-app-store.sh --test`, then open `target/debug/bundle/macos/Rupert Test.app`.

### One-time setup for submission

1. **App Store Connect:** the Rupert record already has the **macOS** platform. Rename its version from 1.0 to the app's version (e.g. 0.1.0), as for iPhone.
2. **Certificates**, easiest from Xcode: Xcode ▸ Settings ▸ Accounts ▸ your Apple ID ▸ the team ▸ **Manage Certificates…** ▸ **+**:
   - **Apple Distribution** (signs the app; the same certificate works for iPhone and Mac);
   - **Mac Installer Distribution** (signs the `.pkg`; shows in Keychain Access as "3rd Party Mac Developer Installer: …").

   Xcode creates them and puts them with their private keys in the login keychain. Check: `security find-identity -v` lists both. (Without Xcode: developer.apple.com ▸ Certificates ▸ +, with a request made in Keychain Access ▸ Certificate Assistant ▸ Request a Certificate from a Certificate Authority, then double-click the downloaded certificate.) Back them up: Keychain Access ▸ My Certificates ▸ export both as `.p12` into the password manager.
3. **Provisioning profile:** developer.apple.com ▸ Certificates, IDs & Profiles ▸ Profiles ▸ **+** ▸ Distribution ▸ **Mac App Store Connect** ▸ platform **macOS** ▸ App ID `io.github.micki-b.rupert` ▸ the Apple Distribution certificate ▸ name it "Rupert Mac App Store" ▸ Generate ▸ **Download**. Keep the `.provisionprofile` somewhere stable (e.g. `~/Library/MobileDevice/Provisioning Profiles/`); the build script takes its path.
4. **Upload key:** copy the App Store Connect API key file to `~/.appstoreconnect/private_keys/AuthKey_<Key ID>.p8` (where `altool` looks). Or upload the `.pkg` with the Transporter app instead and skip this.

### Building and uploading

```sh
RUPERT_MAS_PROFILE=~/Downloads/Rupert_Mac_App_Store.provisionprofile \
APPLE_API_KEY=<Key ID> APPLE_API_ISSUER=<Issuer ID> \
scripts/build-app-store.sh --upload
```

It builds the universal sandboxed app with the profile and a new build number, packages `target/Rupert-<version>-<build>.pkg` signed with the installer certificate, and uploads it. Without `--upload` (and without the API key file), upload the `.pkg` with Apple's **Transporter** app instead: sign in, drag it in, Deliver. The first upload (0.1.0, build 2611191002) went that way. The build then appears in App Store Connect under the macOS version, for TestFlight or review.

Notes for users moving between editions: each edition keeps its own settings, index and keychain items (the App Store edition lives in its sandbox container), so folders are added again and API keys or sign-ins entered again. The documents and their details are in the folders and stay as they are.

## iPhone

App Store Connect record: "Rupert - The Librarian", bundle ID `io.github.micki-b.rupert`, SKU `rupert`, platforms iOS and macOS (the Mac App Store edition shares it). Listing text, privacy answers, screenshots and artwork: `docs/app-store/ios/` (iPhone) and `docs/app-store/macos/` (Mac App Store edition).

The app embeds a share extension, `io.github.micki-b.rupert.share` (`mobile/gen/apple/RupertShare`), and both use the App Group `group.io.github.micki-b.rupert`. Once, before the first device build with it: open `mobile/gen/apple/rupert-mobile.xcodeproj` in Xcode, select each target (`rupert-mobile_iOS` and `RupertShare`) ▸ Signing & Capabilities, and let automatic signing register the App Group and the extension's App ID. Or, in the Apple Developer site (Certificates, Identifiers & Profiles):

1. Identifiers ▸ App Groups ▸ + : `group.io.github.micki-b.rupert`.
2. Identifiers ▸ `io.github.micki-b.rupert` ▸ App Groups ▸ Configure ▸ tick the group.
3. Identifiers ▸ + App IDs ▸ App: `io.github.micki-b.rupert.share` ("Rupert Share"), with App Groups ticked and set to the group.

Keychain sharing needs no setting: both targets list the app's own keychain group.

1. Archive (each upload needs a higher build number; a UTC timestamp does that):

   ```sh
   BUILD="$(date -u +%y%m%d%H%M)"
   HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_INSTALL_UPGRADE=1 npm run phone -- ios build \
     --export-method app-store-connect --config "{\"bundle\":{\"iOS\":{\"bundleVersion\":\"$BUILD\"}}}"
   ```

   The archive lands in `mobile/gen/apple/build/rupert-mobile_iOS.xcarchive`. The command's own export step needs an Apple account visible to `xcodebuild` and may fail with "No Accounts"; the archive is fine either way.
2. Upload: `open mobile/gen/apple/build/rupert-mobile_iOS.xcarchive` (Xcode's Organizer) ▸ Distribute App ▸ App Store Connect, automatic signing. Xcode makes the distribution certificate and profile with your account.
3. App Store Connect processes the build (10–30 minutes, with an email); choose it on the version page, then Add for Review.

## Android

Google Play app: "Rupert", package `io.github.micki_b.rupert` (Android names can't contain "-"). Listing text, screenshots and the feature graphic: `docs/app-store/android/`.

### Keys

- **Upload key:** `~/.android/rupert-upload.jks` (alias `upload`), with its password in the login keychain as "rupert-android-upload" (Keychain Access ▸ "Rupert Android upload key"). It only proves that an upload comes from us. **Keep both safe** (e.g. in a password manager). If they're lost, Play Console can register a new upload key (App integrity ▸ App signing ▸ Request upload key reset).
- **App signing key:** Google's (Play App Signing, turned on with the first upload). Copies from Google Play are signed with it.
- **Google sign-in (Drive)** checks which key signed the app. Google Cloud ▸ "Rupert" project ▸ Credentials needs an **Android** OAuth client for package `io.github.micki_b.rupert` with each key's SHA-1:
  - the debug key (`~/.android/debug.keystore`; emulator and `tauri android dev` builds);
  - the upload key (APKs installed directly): `keytool -list -v -keystore ~/.android/rupert-upload.jks -alias upload`;
  - Google's app signing key (copies from Google Play): Play Console ▸ Test and release ▸ App integrity ▸ App signing.
  Without the right one, Google Drive sign-in fails in that build. OneDrive's sign-in doesn't depend on the key.
- **The scan link** (#124) opens the app only when `site/well-known/assetlinks.json` lists the SHA-256 of the key that signed it. It lists the upload key's (APKs installed directly) and Google's app signing key's (copies from Google Play; Play Console ▸ Protected with Play ▸ Play Store protection ▸ Manage Play app signing, "App signing key certificate", added 2026-11-25). A key missing there sends that copy's phones to the link's web page, whose **Open Rupert** still opens the app.

### Building and uploading

1. The version comes from `scripts/set-version.py` (`mobile/tauri.conf.json`), as for the other apps.
2. Build: `scripts/build-android.sh`. It writes `target/android/Rupert-<version>-<code>.aab` (for Google Play) and `.apk` (to install directly: `adb install -r …`), for ARM phones and x86_64. The version code is the build's UTC time, so each build is higher than the last.
3. Write the release notes in `docs/app-store/android/release_notes.md`: a new block at the top, in Play Console's format (`<en-US>`, `<es-419>`, `<es-ES>`, `<de-DE>`; at most 500 characters each), covering everything since the last build on Google Play. Upload the `.aab`: Play Console ▸ Test and release ▸ Testing ▸ Closed testing (or Production) ▸ Create new release, and paste the block into Release notes.
4. The `.apk` goes on the GitHub release too, as `Rupert_<version>_android.apk` (step 5 under "Desktop"); rupertlibrarian.com's Android download links to it. It's signed with the upload key, so Google Drive sign-in in it needs the upload key's Android OAuth client (above).

New personal developer accounts must run a **closed test with at least 12 testers for 14 days in a row** before they can apply for production.
