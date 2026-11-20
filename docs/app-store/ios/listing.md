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

Find any scanned document in seconds. Rupert searches the folders you already keep in iCloud Drive and OneDrive, and scans new paper straight into your inbox.

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

`scanner,documents,pdf,search,ocr,receipts,paperless,files,onedrive,icloud,archive,tags,inbox,scan`

## What's new (first version)

First release.

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

Rupert is free and open source: https://github.com/micki-b/Rupert

## Screenshots

Made in the iPhone 17 Pro Max simulator with a made-up sample library: a folder, search with snippets, a document, Scan. Upload them in that order. PNG without transparency (App Store Connect refuses alpha channels).

- `docs/app-store/ios/screenshots/medium/` (1206 × 2622): **iPhone with Dynamic Island (medium display)**, the required slot.
- `docs/app-store/ios/screenshots/large/` (1320 × 2868): **iPhone with Dynamic Island (large display)**, optional; without it the medium set is scaled up.

Sizes per slot: [Apple's screenshot specifications](https://developer.apple.com/help/app-store-connect/reference/app-information/screenshot-specifications).
