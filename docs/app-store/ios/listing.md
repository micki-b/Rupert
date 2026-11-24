# App Store listing (iPhone)

Draft texts and answers for App Store Connect. Character limits in brackets.

## App information

- **Name** [30]: `Rupert - The Librarian` ("Rupert" alone was taken)
- **Subtitle** [30]: `Your scanned documents, found`
- **Bundle ID:** `io.github.micki-b.rupert`
- **SKU:** `rupert` (one record for iPhone and the Mac App Store edition)
- **Primary category:** Productivity. **Secondary:** Business.
- **Price:** Free. No in-app purchases.
- **Devices:** iPhone (iOS 16 or later).
- **Copyright:** © 2026 Micki Braun
- **Support URL:** https://github.com/micki-b/Rupert/issues
- **Marketing URL:** https://github.com/micki-b/Rupert
- **Privacy policy URL:** https://github.com/micki-b/Rupert/blob/main/PRIVACY.md

## Promotional text [170]

Ask Rupert about your documents and get answers right on your iPhone. Find any scan in seconds in iCloud Drive or OneDrive, and scan paper straight into your inbox.

## Description [4000]

Rupert is the librarian for your scanned documents. It works with the folders you already have in iCloud Drive, OneDrive or on your iPhone, so there's nothing to import and nothing to lock in.

FIND ANYTHING
• Search titles, tags, notes and the text of your documents.
• Filter by tags, and keep favorite documents and folders one tap away.
• Open any document to read, share or print it.

SCAN STRAIGHT INTO YOUR INBOX
• Rupert's own camera scanner finds the page's edges, straightens it and saves a sharp PDF.
• Choose color, grayscale or black & white, and the resolution.
• Several pages become one PDF, saved to your inbox folder in iCloud Drive or OneDrive.
• Your inbox is always at the top of the Library, so new scans are easy to find.

ASK RUPERT
On an iPhone with Apple Intelligence (iOS 26 or later), ask about your documents in your own words. Rupert searches and reads them on the iPhone itself and answers, with buttons to open what he found. Nothing leaves your iPhone.

IN YOUR LANGUAGE
Rupert speaks English, Spanish and German, and uses your iPhone's language. In Spanish the owl wears the sky-blue and white striped jersey; in German, lederhosen.

WORKS WITH RUPERT ON YOUR COMPUTER
Rupert for Mac, Windows and Linux (free) recognizes the text in new scans, suggests a title, date, tags and folder, and files them when you agree. Titles, tags and dates are stored inside the PDFs, so they appear on your iPhone too.

YOUR DOCUMENTS STAY YOURS
• No account with us, no tracking, no ads. Rupert collects nothing.
• Your files stay in your own storage; Rupert reads them where they are.
• Online-only files aren't downloaded until you open one.
• OneDrive: sign in with your Microsoft account; Rupert only reads your folders and adds new scans to your inbox.
• Google Drive (preview): sign in with your Google account, the same way.

Rupert is free and open source.

## Keywords [100]

`scanner,documents,pdf,search,ocr,receipts,paperless,files,onedrive,icloud,archive,tags,ai,assistant`

## What's new (first version)

First release.

## What's new in 0.5.2 [4000]

Ask Rupert finds more and makes up less.

• Ask (on iPhones with Apple Intelligence): when a search finds nothing, Rupert tries again with fewer words and, for a question in Spanish or German, with the words in English, since documents are often in another language than the question.
• Rupert reads a document before saying what something cost, answers in the language of your question, and searches instead of asking which document you mean.
• Settings ▸ About has a link to the privacy policy.

## What's new in 0.5.0 [4000]

See your documents, and send new ones to Rupert from any app.

LARGE ICONS AND A GALLERY
• At the top of a folder, switch between the list, large icons and a gallery. Rupert remembers the view for each folder.
• The pictures come from OneDrive and Google Drive, which make them without downloading your documents, and from the iPhone itself for documents in Files folders that are on the phone.

SHARE TO THE INBOX
• In Mail, Files, Photos or Safari, tap Share and choose Rupert: the document is saved to your inbox (in Files, OneDrive or Google Drive) under its own name, without opening Rupert. Rupert on your computer reads it and suggests where to file it, like a new scan.
• If it can't be saved right away, Rupert sends it the next time you open the app.

## App Privacy

- **Data collection:** "No, we do not collect data from this app." The developer has no servers and receives nothing. The app talks only to services the user signs in to for their own files (Microsoft OneDrive) and keeps everything else on the device. (If Apple's reviewer reads OneDrive access as collection, the closest category is "User Content ▸ Other User Content", not linked to identity, not used for tracking, purpose "App Functionality".)
- **Tracking:** No.

## Age rating

All answers "None" / "No" (no user-generated content shared with others, no web browsing, no gambling, no medical advice). Result: 4+.

## Export compliance

The app uses only standard encryption (HTTPS through the system and rustls). `ITSAppUsesNonExemptEncryption` is `false` in Info.plist, so App Store Connect doesn't ask.

## Notes for App Review

Sign-in required: no (untick it). Attachment: none.

Rupert organizes PDF documents that the user already keeps in Files (iCloud Drive or On My iPhone) or OneDrive. No account is needed.

To try it with PDFs already on the device: Library tab ▸ "Add a folder…", choose any folder with PDF files. The Library tab lists them, and the Search tab finds them by name and by the text inside them.

To try it without any PDFs: Scan tab ▸ "Choose in Files…", choose or create a folder (for example in On My iPhone) as the inbox. Tap Rupert the owl to scan a page with the camera; it's saved there as a PDF. Then Library tab ▸ "Add a folder…" and choose that same folder to see the scan in the library.

OneDrive is optional (Settings ▸ OneDrive) and needs a personal Microsoft account.

Google Drive is optional too (Settings ▸ Google Drive, marked "preview"). Rupert's Google project is used by a few known people and isn't verified by Google, so Google's sign-in page shows its "Google hasn't verified this app" warning; that is expected. It isn't needed to review the app.

The Ask tab (Ask Rupert) appears only on iPhones that can run Apple Intelligence (iPhone 15 Pro or later with iOS 26 or later) and answers once Apple Intelligence is turned on in Settings; it runs entirely on the device. On other iPhones the tab is hidden by design. To try it, add a folder with PDFs first, then ask, for example, "Which documents mention insurance?".

The share extension ("Rupert" in the share sheet) saves a shared document (PDF, image, Word, Excel, PowerPoint or RTF) into the inbox folder chosen on the Scan tab. To try it: choose an inbox as above, then in Files (or Safari, Mail, Photos) tap Share ▸ Rupert. The extension uses the app group only to read the inbox setting and to hand over documents it couldn't save, which the app sends the next time it opens.

Rupert is free and open source: https://github.com/micki-b/Rupert

## Screenshots, App Previews and artwork

Per language (`en`, `es` for Spanish (Mexico), `de`), made in the iPhone 17 Pro Max simulator with the made-up sample library (`docs/app-store/tools/ios/`). The app's screens are in each language; the sample documents are in English. Ask Rupert's answers are real (in the simulator, through a model on the Mac, since the Simulator can't run Apple's model; on an iPhone it's Apple Intelligence). PNGs without transparency (App Store Connect refuses alpha channels).

- **Screenshots**, upload in this order: `1-folder`, `2-search`, `3-document`, `4-ask`, `5-scan`.
  - `screenshots/<lang>/medium/` (1206 × 2622): **iPhone with Dynamic Island (medium display)**, the required slot.
  - `screenshots/<lang>/large/` (1320 × 2868): **iPhone with Dynamic Island (large display)**, optional; without it the medium set is scaled up.
- **App Preview**: `previews/<lang>-ask.mp4` (886 × 1920, 30 fps, H.264, silent stereo AAC, under 30 s): browse, search, a document, a question to Ask Rupert and its answer, the Scan tab. Upload it as the first item of each language's previews and screenshots.
- **Artwork** (App Store Connect ▸ the app ▸ App Store ▸ media for featuring): `creative/<lang>-header-3840x1646.png` (**Header**) and `creative/<lang>-search-results-3840x2560.png` (**Search Results**), rendered by `creative/creative.html` from that language's screenshots, with the owl in his outfit in Spanish and German.

Sizes: [Apple's screenshot specifications](https://developer.apple.com/help/app-store-connect/reference/app-information/screenshot-specifications), [app preview specifications](https://developer.apple.com/help/app-store-connect/reference/app-information/app-preview-specifications).
