# App Store listing (Mac)

The Mac App Store edition shares the iPhone app's record ("Rupert - The Librarian", bundle ID `io.github.micki-b.rupert`, SKU `rupert`); name, subtitle, categories, privacy answers, pricing and availability are set once for both (see `../ios/listing.md`). The macOS version page has its own media and texts. Character limits in brackets.

## App Previews and Screenshots (Mac)

**App previews** (`previews/`, 1920 × 1080, 30 fps, H.264, stereo AAC; up to 3, shown before the screenshots), in this order:

1. `1-find.mp4` (23 s): folders in the gallery, large icons, search inside documents.
2. `2-inbox.mp4` (20 s): new scans in the inbox get suggestions and are filed with a click.
3. `3-ask.mp4` (18 s): Ask Rupert answers from a receipt, with the local model (LM Studio).

App Store Connect picks a poster frame at 5 s; change it per preview (Edit Poster Frame) if another moment looks better.

**Screenshots** (`screenshots/`, 1440 × 900, PNG without alpha), in this order: `1-inbox.png`, `2-search.png`, `3-gallery.png`, `4-ask.png`.

All of it shows a made-up library (`../tools/sample-library/`) in a copy of the App Store edition; `../tools/README.md` explains how to make them again.

## Promotional text [170]

Search the text of every scanned document, let new scans name and file themselves, and ask Rupert where things are. Works on the folders you already have.

## Description [4000]

Rupert is the librarian for your scanned documents. It works directly on the folders you already keep on your Mac, in iCloud Drive, OneDrive or Google Drive, so there's nothing to import and nothing to lock in. Your folders stay your library.

FIND ANYTHING
• Search titles, tags, notes and the text inside every PDF, with the matching words highlighted.
• Browse as a list, as large icons, or in a gallery that flips through the pages.
• Filter by tags, and keep favorite documents and folders in the sidebar.
• Tidy up by dragging documents and folders to another folder; their details move with them.
• Markdown notes open formatted.

AN INBOX THAT FILES ITSELF
• Scan into the inbox folder from your scanner, ScanSnap or iPhone.
• Rupert recognizes the text on your Mac and suggests a title, date, tags and folder.
• Check the suggestion and file it with a click: the PDF is renamed, moved to its folder, and the details are written inside the file.

ASK RUPERT
• Ask in plain words: "How much did the plumber charge in April?" or "Show my car insurance."
• Rupert searches your library and answers from your own documents.
• Uses Local AI on your Mac by default: a free app such as LM Studio or Ollama. You can choose Claude or ChatGPT instead; Rupert always shows when a cloud service is in use.

YOUR DOCUMENTS STAY YOURS
• Titles, tags and dates are stored inside the PDFs (or in a small hidden file next to other kinds), so they go wherever the file goes and appear on your iPhone too.
• Text recognition and filing suggestions never leave your Mac.
• Online-only files in iCloud Drive, OneDrive and Google Drive are shown without downloading them.
• No account with us, no tracking, no ads.

Rupert is free and open source.

## Keywords [100]

`scanner,documents,pdf,search,ocr,receipts,paperless,files,inbox,filing,archive,tags,lm studio,ollama`

## URLs and copyright

- Support URL: `https://github.com/micki-b/Rupert/issues`
- Marketing URL: `https://github.com/micki-b/Rupert`
- Copyright: `2026 Micki Braun`

## Notes for App Review

Sign-in required: no. Attachment: none.

Rupert organizes PDF documents in folders the user picks. No account is needed.

To try it: click "Add a folder…" (or the + next to Sources) and choose any folder with PDF files. Rupert lists them; the search field finds them by name and by the text inside them. Gallery and Large icons (top right of a folder) show the pages.

Inbox: Settings ▸ Inbox, choose a folder, then put a PDF in it. Rupert reads it and, if a Local AI app (a free app that runs language models on the Mac, such as LM Studio, lmstudio.ai, or Ollama, ollama.com) is running with a model, suggests a title, date, tags and folder; "File" moves it there. Without one the inbox still works, with the details entered by hand.

Ask Rupert (the speech bubble, bottom right) also uses Local AI by default; cloud models (Claude, ChatGPT) are optional in Settings ▸ Assistant and need the user's own key or account.

Network server entitlement (com.apple.security.network.server): Rupert receives incoming connections in two places. (1) Scanning from network scanners (File ▸ Scan…): Rupert finds AirScan/eSCL scanners with Bonjour (mDNS), which binds UDP port 5353 and receives the scanners' multicast replies. (2) "Sign in with ChatGPT…" (Settings ▸ Assistant ▸ ChatGPT, with your plan): OpenAI's sign-in redirects the browser to http://127.0.0.1:1455/auth/callback, so Rupert listens on the loopback address for that one request and stops listening right after. Nothing else in the app accepts incoming connections.

Rupert is free and open source: https://github.com/micki-b/Rupert
