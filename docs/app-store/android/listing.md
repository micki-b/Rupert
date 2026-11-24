# Google Play listing (Android)

Draft texts and Play Console answers. Character limits in brackets. Spanish and German: `listing-es.md`, `listing-de.md`.

## App details

- **App name** [30]: `Rupert - The Librarian` (as on the App Store)
- **Package:** `io.github.micki_b.rupert`
- **App or game:** App. **Free.** No in-app purchases, no ads.
- **Category:** Productivity. **Tags:** Document management, Productivity, PDF.
- **Default language:** English (United States). Translations: Spanish (Latin America, `es-419`), Spanish (Spain, `es-ES`, the same text) and German (`de-DE`).
- **Contact details:**
  - **Email** (shown on Google Play, required): contact@rupertlibrarian.com
  - **Website:** https://rupertlibrarian.com
- **Privacy policy URL:** https://rupertlibrarian.com/privacy/

## Short description [80]

Find, search and scan your documents in Google Drive and OneDrive.

## Full description [4000]

Rupert is the librarian for your scanned documents. It works with the folders you already have in Google Drive or OneDrive, so there's nothing to import and nothing to lock in.

FIND ANYTHING
• Search titles, tags, notes and the text of your documents.
• Filter by tags, and keep favorite folders one tap away.
• Open any document in your PDF viewer to read, share or print it.

SCAN STRAIGHT INTO YOUR INBOX
• Google's document scanner finds the page's edges, straightens and cleans it up.
• Choose color, grayscale or black & white, and the resolution.
• Several pages become one PDF, saved to your inbox folder in Google Drive or OneDrive.
• Your inbox is always at the top of the Library, so new scans are easy to find.

WORKS WITH RUPERT ON YOUR COMPUTER
Rupert for Mac, Windows and Linux (free) recognizes the text in new scans, suggests a title, date, tags and folder, and files them when you agree. Titles, tags and dates are stored inside the PDFs, so they appear on your phone too.

IN YOUR LANGUAGE
Rupert speaks English, Spanish and German, and uses your phone's language. In Spanish the owl wears the sky-blue and white striped jersey; in German, lederhosen.

YOUR DOCUMENTS STAY YOURS
• No account with us, no tracking, no ads. Rupert collects nothing.
• Your files stay in your own storage; Rupert reads them where they are.
• Sign in with your Google or Microsoft account; Rupert only reads your folders, adds new scans to your inbox and remembers the folders you star.

Rupert is free and open source.

## Graphics

- **App icon:** `icon-512.png` (512 × 512).
- **Feature graphic:** `feature-<lang>.png` (1024 × 500, no transparency), rendered by `feature.html` from that language's screenshots.
- **Phone screenshots:** `screenshots/<lang>/` (1080 × 1920, no transparency), in order: `1-folder`, `2-search`, `3-document`, `4-scan`. Made on the "Rupert Pixel" emulator with the made-up sample library (`../tools/README.md`, "Android"); the documents are in English, the app's screens in each language.
- **Tablet screenshots:** none yet. The app runs on tablets as a phone-sized layout.

## Store settings and app content (Play Console ▸ Policy and programs ▸ App content)

- **Privacy policy:** the URL above.
- **Ads:** No, the app doesn't contain ads.
- **App access:** "All or some functionality is restricted." Instructions:

  > Rupert shows PDFs that the user keeps in their own Google Drive or OneDrive. No account with us is needed; use any Google account. Settings ▸ Google Drive ▸ Sign in, then Library ▸ Add a folder, and choose any Drive folder with PDF files. The Library lists them, Search finds them by name, and tapping one opens it. Scan ▸ choose an inbox folder, then tap the owl to scan a page with Google's document scanner; it's saved to that folder as a PDF. Google's sign-in may say "Google hasn't verified this app" (Rupert's Google project is in testing); continue past it.

- **Content rating** (IARC questionnaire): category "All other app types". Every answer No (no violence, sexuality, language, controlled substances, gambling; users don't interact or share content; no location sharing; no purchases). Expected result: Everyone / PEGI 3 / USK 0.
- **Target audience:** 18 and over. The app isn't directed at children.
- **News app:** No. **COVID-19 apps:** No. **Government app:** No. **Financial features:** none. **Health:** none.
- **Advertising ID:** the app doesn't use it (no `AD_ID` permission).
- **Data safety:**
  - **Does your app collect or share any of the required user data types?** Yes, because the app sends files off the phone, to the user's own cloud storage:
    - **Files and docs** (scans saved to the inbox, the folders' details files): *collected*, not shared, not processed ephemerally; **optional** (the user chooses to sign in); purpose **App functionality**.
    - Nothing else: no personal info, location, contacts, photos kept, app activity, device IDs or diagnostics.
  - **Encrypted in transit:** Yes (HTTPS).
  - **Deletion:** the files are in the user's own Google Drive or OneDrive; they delete them there. Signing out in Settings removes the sign-in from the phone.

  (Google Play counts sending data off the device as "collection" even when it goes to the user's own storage and never to the developer.)

## Release notes, 0.5.2 [500]

Google Drive sign-in now says why it failed (for example, when Google can't be reached) instead of quietly going back to "Not signed in".

## Release notes, first closed test [500]

First test version of Rupert for Android: browse and search your Google Drive and OneDrive folders, open documents, star folders, and scan into your inbox. In English, Spanish and German.
