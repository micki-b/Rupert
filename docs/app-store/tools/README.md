# Making the App Store media

Everything shown in the App Store screenshots and previews comes from a made-up library, never real documents.

- `sample-library/Home Documents/`: about 30 made-up PDFs and two Markdown notes, with their details (titles, tags, dates, favorites) in the folders' `.rupert/` files. The PDFs got their letterheads from `restyle.py` (made-up issuers around each document's original text).
- `inbox-scans/`: three made-up new scans (electricity bill, vet invoice, tire receipt) for the inbox scenes.
- `mac/`: scripts that drive a copy of the Mac App Store edition and record it.

## iPhone (`../ios/`)

Screenshots come from the iOS Simulator (iPhone 17 Pro Max for `large/`, scaled to `medium/`), with the library copied into the app's container and the debug hooks `SIMCTL_CHILD_RUPERT_TEST_FOLDER`, `…_INBOX` and `…_JS` (see `mobile/src/lib.rs`). They must be PNGs without alpha. `../ios/creative/creative.html` renders the header and search-results artwork from those screenshots.

## Mac (`../macos/`)

Needs: Google Chrome (renders images), ffmpeg, LM Studio running with a model (inbox suggestions and Ask Rupert), and Screen Recording permission for the terminal (System Settings ▸ Privacy & Security). The recorder captures only Rupert's window, even under other windows.

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

3. **Set up once** (adds the library, sets the inbox and LM Studio, waits for the suggestions):

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
