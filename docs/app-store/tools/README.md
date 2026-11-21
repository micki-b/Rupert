# Making the App Store media

Everything shown in the App Store screenshots and previews comes from a made-up library, never real documents.

- `sample-library/Home Documents/`: about 30 made-up PDFs and two Markdown notes, with their details (titles, tags, dates, favorites) in the folders' `.rupert/` files. The PDFs got their letterheads from `restyle.py` (made-up issuers around each document's original text).
- `inbox-scans/`: three made-up new scans (electricity bill, vet invoice, tire receipt) for the inbox scenes.
- `mac/`: scripts that drive a copy of the Mac App Store edition and record it.

## iPhone (`../ios/`)

Screenshots and App Previews come from the iOS Simulator ("Rupert Media", an iPhone 17 Pro Max the scripts create once), per language, with a copy of the sample library in the app's container. The scripts in `ios/` drive a debug build through its test hooks (`mobile/src/lib.rs`, `mobile/src/assistant.rs`): `RUPERT_TEST_FOLDER`/`_INBOX` add the library, `RUPERT_TEST_JS` runs a scene, `test_log` reports to the simulator's log (the scripts read it to know when to shoot), and `RUPERT_TEST_ASSISTANT=<url>|<model>` points Ask Rupert at a model on the Mac, because the Simulator can't run Apple's model (default: LM Studio with qwen/qwen3.6-35b-a3b; set `RUPERT_MEDIA_ASSISTANT` for another).

1. Build the simulator app and the catalog writer (the phone reads text from the catalog Rupert on a computer writes):

   ```sh
   npm run phone -- ios build --debug --target aarch64-sim
   git checkout mobile/gen/apple/rupert-mobile_iOS/Info.plist
   cargo build --release -p rupert-core --example index_check
   ```

2. Screenshots, one language at a time (light mode, 9:41, then `1-folder` … `5-scan` in the output folder, 1320 × 2868):

   ```sh
   T=docs/app-store/tools/ios
   $T/capture.sh es "¿Cuál es la franquicia del seguro del auto?" /tmp/shots-es
   ```

   Store them without alpha as `../ios/screenshots/<lang>/large/`, and scaled to 1206 × 2622 as `…/medium/` (`ffmpeg -i in.png -vf scale=1206:2622:flags=lanczos -pix_fmt rgb24 out.png`).

3. App Preview, after `capture.sh` for the same language (it leaves the app installed with the library): `$T/preview.sh es "<question>" docs/app-store/ios/previews/es-ask.mp4`. It records a take, makes it a steady 30 fps, jump-cuts the model's thinking to 1.5 s, and encodes 886 × 1920, 30 fps, H.264 High 4.0, silent stereo AAC, at most 29.5 s.

4. Artwork: `../ios/creative/creative.html?layout=header|search&lang=<lang>` in headless Chrome (the command is at the top of the file), saved without alpha as `<lang>-header-3840x1646.png` and `<lang>-search-results-3840x2560.png`.

5. Delete the simulator's app afterwards if you like (`xcrun simctl uninstall "Rupert Media" io.github.micki-b.rupert`); its data is only the sample library.

## Android (`../android/`)

### Google Play screenshots

`android/capture.sh <lang> <outdir>` makes `1-folder` … `4-scan` (1080 × 1920, light mode, 9:41) on the running "Rupert Pixel" emulator. It reinstalls the debug build with a copy of the sample library and its catalog, runs `ios/setup.js` and `android/scenes.js` through the test hooks, and takes each shot when the page logs it (`test_log` goes to logcat on Android). It wipes the emulator app's sign-ins.

```sh
npm run phone -- android build --debug --target aarch64 --apk
cargo build --release -p rupert-core --example index_check
docs/app-store/tools/android/capture.sh es /tmp/play-es
```

Store them without alpha as `../android/screenshots/<lang>/` (`ffmpeg -i in.png -pix_fmt rgb24 out.png`), then render the feature graphic with `../android/feature.html` (the command is at the top of the file).

### Website screenshots

The website's Android screens (`site/img/<lang>/android-folder-{light,dark}.png`, 603 × 1340) come from the "Rupert Pixel" emulator and a debug build. Android can't pass environment variables to an app, so the debug test hooks read the same names from `rupert-test.env` in the app's data folder.

1. Catalog a copy of the sample library: `CATALOG=1 target/release/examples/index_check src-tauri/pdfium "<copy>/Home Documents" /tmp/index.db`.
2. Install the debug APK, then `adb shell pm clear io.github.micki_b.rupert`. This wipes the app's sign-ins on the emulator.
3. Copy the library in through `/data/local/tmp` and `run-as`:
   - put it at `/data/user/0/io.github.micki_b.rupert/files/Home Documents`;
   - write `rupert-test.env` with `RUPERT_TEST_FOLDER=…/files/Home Documents` and `RUPERT_TEST_INBOX=…/files/Home Documents/Inbox`.
4. Clean status bar: `adb shell settings put global sysui_demo_allowed 1`, then `am broadcast -a com.android.systemui.demo -e command …`:
   - `enter`;
   - `clock -e hhmm 0941`;
   - `network -e mobile hide`;
   - `notifications -e visible false`.
5. Open Home Documents ▸ Insurance and take `adb exec-out screencap -p` shots:
   - switch dark mode with `adb shell cmd uimode night yes|no`;
   - switch the language in Settings ▸ Language;
   - scale the shots to 1340 px high (`sips -Z 1340`).

## Mac (`../macos/`)

Needs: Google Chrome (renders images), ffmpeg, Local AI running with a model (LM Studio at its default address; inbox suggestions and Ask Rupert), and Screen Recording permission for the terminal (System Settings ▸ Privacy & Security). The recorder captures only Rupert's window, even under other windows.

1. **Capture copy** of Rupert: the App Store feature set, the test identifier (its own data, separate from your Rupert), titled "Rupert", light mode, with the debug test hooks:

   ```sh
   TAURI_APP_PATH=src-tauri npm run tauri build -- --debug --bundles app \
     --config docs/app-store/tools/mac/capture.conf.json
   mv target/debug/rupert target/debug/Rupert.tmp && mv target/debug/Rupert.tmp target/debug/Rupert
   ```

   It lands in `target/debug/bundle/macos/Rupert.app`.

2. **A fresh library copy** (filing moves the scans, so repeat this before each inbox take):

   ```sh
   export RUPERT_MEDIA_LIB="$(mktemp -d)"
   cp -Rp docs/app-store/tools/sample-library/. "$RUPERT_MEDIA_LIB/"
   cp -p docs/app-store/tools/inbox-scans/*.pdf "$RUPERT_MEDIA_LIB/Home Documents/Inbox/"
   ```

3. **Set up once** (adds the library, sets the inbox and Local AI, waits for the suggestions):

   ```sh
   T=docs/app-store/tools/mac
   $T/launch.sh 1440x900 $T/setup.js /tmp/setup.log; until grep -q 'rupert-test: ready' /tmp/setup.log; do sleep 1; done
   ```

4. **Screenshots** (1440 × 900; the display here is 1×, so window points are pixels): `$T/shots.sh docs/app-store/macos/screenshots`

5. **Previews**, one take per scene (`s-find.js`, `s-inbox.js`, `s-ask.js`). The window is 1664 × 936 and goes on a 1920 × 1080 background with captions. The scene scripts wait about 7 s before acting, so start the recorder right away:

   ```sh
   swiftc -O $T/recorder.swift -o /tmp/recorder
   read P W < <($T/launch.sh 1664x936 $T/s-find.js /tmp/find.log)
   /tmp/recorder $W 45 /tmp/take-find.mov > /tmp/rec-find.txt
   python3 $T/compose.py /tmp/take-find.mov /tmp/rec-find.txt /tmp/find.log docs/app-store/macos/previews/1-find.mp4
   ```

   Scenes mark their start, end and captions in the log (`mark cap:<text>`); `compose.py` cuts the take to the scene, rounds the window's corners, adds the captions and encodes a preview Apple accepts (1920 × 1080, 30 fps, H.264 High 4.0, stereo AAC, 15–30 s).

6. **Clean up**: quit the capture copy, then delete its data: `~/Library/Application Support/io.github.micki-b.rupert-test`, `~/Library/Caches/io.github.micki-b.rupert-test`, `~/Library/WebKit/io.github.micki-b.rupert-test`.

A screen at 2× (Retina) would capture at double size; then use a 1440 × 900 window for 2880 × 1800 screenshots, and scale previews down to 1920 × 1080 in `compose.py`.
