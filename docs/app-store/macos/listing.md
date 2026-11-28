# App Store listing (Mac)

The Mac App Store edition shares the iPhone app's record ("Rupert - The Librarian", bundle ID `io.github.micki-b.rupert`, SKU `rupert`); name, subtitle, categories, privacy answers, pricing and availability are set once for both (see `../ios/listing.md`). The macOS version page has its own media and texts. Character limits in brackets.

## App Previews and Screenshots (Mac)

**App previews**, one set per language in `previews/<lang>/` (`en`, `es`, `de`; 1920 × 1080, 30 fps, H.264, stereo AAC; up to 3, shown before the screenshots), in this order:

1. `1-find.mp4` (23 s): folders in the gallery, large icons, search inside documents.
2. `2-inbox.mp4` (20 s): new scans in the inbox get suggestions and are filed with a click.
3. `3-ask.mp4` (18–24 s): Ask Rupert answers from the plumber's invoice. English runs on Apple Intelligence; Spanish and German on a Local AI app (LM Studio), because the sample documents are in English and Apple's model didn't find them from a Spanish or German question.

App Store Connect picks a poster frame at 5 s; change it per preview (Edit Poster Frame) if another moment looks better.

**Screenshots**, per language in `screenshots/<lang>/` (2880 × 1800, PNG without alpha), in this order: `1-inbox.png`, `2-search.png`, `3-gallery.png`, `4-ask.png`.

In App Store Connect, upload each language's set on its own localization of the macOS version (English (U.S.), Spanish (Mexico), German).

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
• Uses Apple Intelligence on your Mac by default, or a free app such as LM Studio or Ollama. You can choose Claude or ChatGPT instead; Rupert always shows when a cloud service is in use.

YOUR DOCUMENTS STAY YOURS
• Titles, tags and dates are stored inside the PDFs (or in a small hidden file next to other kinds), so they go wherever the file goes and appear on your iPhone too.
• Text recognition and filing suggestions never leave your Mac.
• Online-only files in iCloud Drive, OneDrive and Google Drive are shown without downloading them.
• No account with us, no tracking, no ads.

Rupert is free and open source.

## What's new in 0.10.1 [4000]

Small fixes.

• An × next to a due date takes it away, in the inbox and in Details.
• Settings and Help at the bottom left of the window are now icons.

## What's new in 0.10.0 [4000]

A smarter library: due dates and reminders, saved searches, a health check, and an inbox that learns.

• From, amount and due date: Rupert suggests who a document is from, its amount and when it's due, and adds up amounts for a selection or a search.
• Coming up: documents with a due date soon, with Done, Remind me later and Add to Calendar. Optional notifications and a count on the Dock icon.
• Saved searches in the sidebar: one click runs a search again.
• Library health: find duplicate copies and compare them side by side, recognize text in old scans, and fill in missing dates and tags.
• The inbox learns from how you filed similar documents, and File All Sure Ones files the obvious ones at once.
• An inbox in iCloud Drive or OneDrive works with online-only scans.
• Fixes.

## What's new in 0.9.0 [4000]

Work with many documents at once.

• Select several: ⌘-click or Shift-click, then move, tag, star, share, export or trash them together.
• Sort the list by any column, and narrow searches by date, kind and place.
• Undo: Edit ▸ Undo (⌘Z) takes back the last move, filing, rename or tag change.
• Quick open: ⌘K goes to any folder, document, tag or command. Press Space to preview the selected document.
• Search words are marked on the page, ⌘F finds words in a document, and Ask Rupert names the page an answer comes from; click it to see the passage.
• Export…: copies of documents in a zip or a folder, with a list for a spreadsheet.
• A page preview in Details while browsing, and a resizable preview in the inbox.
• List columns you can resize, move, show and hide, with stars for folders too.
• Faster search in long documents, and fixes.

## What's new in 0.8.0 [4000]

Get documents into the inbox more easily: scan with your iPhone, print to Rupert, or import many files at once.

• Scan with iPhone: click Scan… in the inbox, then Scan with iPhone…, and choose Scan Documents under a nearby iPhone or iPad. The PDF goes straight into the inbox. Or scan with the Rupert app on your iPhone or Android phone: point its camera at the code in the same window.
• Print to Rupert: turn it on in Settings ▸ Inbox, then choose File ▸ Print ▸ PDF ▸ Save to Rupert in any app.
• Import into Inbox…: bring in many files and folders at once. Documents already in your library aren't copied again.
• Rupert no longer scans from network scanners itself. Scanner software that saves to the inbox folder still works.

## What's new in 0.7.0 [4000]

Rename many documents at once, and send documents by Mail, Messages or AirDrop.

• Rename documents: right-click a folder ▸ Rename documents… New names come from each document's date and title. Change one, and Rupert names the others the same way, with Local AI if needed. Check the before and after list, then rename; Undo puts the old names back.
• Share… and Email…: right-click a document, or use the buttons in the details and the inbox. A PDF goes without your notes unless you include them in Settings ▸ Library.
• The Ask Rupert button no longer covers the inbox's buttons in a small window.
• Dragging a document out and back no longer leaves a copy behind.

## What's new in 0.6.1 [4000]

<!-- The stores never got 0.6.0 (only GitHub did); this is its text. -->

A roomier inbox, Settings in their own window, and renaming, dragging out and a page viewer.

• Inbox: your waiting scans fill the middle of the window, with a search field and the List, Large icons and Gallery views. The selected scan's page stays above File it.
• Choosing the folder: File it shows the whole path. Type to search all your folders, or browse them as a tree and make a new folder there.
• Page viewer: click a page to see it large. Zoom, move around it and flip pages.
• Rename documents and folders in Rupert: right-click ▸ Rename…, or select one and press Return.
• Drag documents out of Rupert into Mail, Messages or the Finder; they get a copy.
• Settings open in their own window, with the parts in a sidebar and a search field.
• Drag the edge between columns to make the sidebar, Details or the inbox's form wider or narrower.
• Online-only documents in iCloud Drive, OneDrive and Google Drive show a picture of their first page.
• Stop and resume indexing from the status line, and right-click a folder ▸ Index this folder.

## What's new in 0.5.2 [4000]

Ask Rupert finds more and makes up less, and inbox suggestions explain themselves in your language.

• Ask Rupert: when a search finds nothing, Rupert tries again with fewer words and, for a question in Spanish or German, with the words in English, since documents are often in another language than the question.
• With Apple Intelligence, Rupert reads a document before saying what something cost, answers in the language of your question, and searches instead of asking which document you mean.
• Inbox suggestions: the short reason under the suggested folder is now in Rupert's language. The suggested title stays in the document's own language, since it becomes the file name.

## What's new in 0.5.1 [4000]

Everything since the first Mac App Store version (0.1.0); Spanish and German in `listing-es.md` and `listing-de.md`.

Rupert now speaks Spanish and German, runs on Apple Intelligence, and lets you move documents and drag new ones in.

APPLE INTELLIGENCE AND LOCAL AI
• On a Mac with Apple Intelligence (macOS 26 or later), inbox suggestions and Ask Rupert run on Apple's built-in model: nothing to install, and nothing leaves your Mac. Rupert starts with it on such a Mac.
• Or use any app that runs AI models on your Mac, such as LM Studio or Ollama: Settings ▸ Local AI has presets for both.
• Rupert says where Local AI runs: on this Mac, on another computer on your network, or elsewhere. An address outside your network counts as a cloud service: Ask Rupert shows a cloud symbol, and inbox suggestions don't use it.
• Thinking models get more room for inbox suggestions, and a clear message if they still run out.

SPANISH AND GERMAN
• Screens, menus, messages and help in Spanish (as spoken in Argentina) and German. Rupert follows your Mac's language; Settings ▸ Language can choose another. Ask Rupert answers in the language you write in.
• In Spanish the owl wears the albiceleste; in German, lederhosen.

MOVE AND DRAG IN
• Move documents and folders by dragging them onto a folder, or right-click ▸ Move to…. Their details go with them, and nothing is ever replaced.
• Drop files from the Finder or another app onto a folder to copy them there, names and dates kept. Drop them on Inbox, and Rupert reads them and suggests where to file them, like a new scan.

ALSO NEW
• Google Drive folders (Google Drive for desktop) work like iCloud Drive and OneDrive: online-only files are shown without downloading them.
• A star on every document: in the list, the icons, the gallery and search results, one click makes it a favorite.
• Click an online-only document's cloud badge to download just that file; Rupert then reads its text.

FIXED
• Open, Show in Finder and Show in Rupert in the right-click menu work again.
• A folder's … menu no longer runs off the window when its path is long.
• Ask Rupert shows a model chosen in Settings ▸ Assistant right away.

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

Inbox: Settings ▸ Inbox, choose a folder, then put a PDF in it. Rupert reads it and suggests a title, date, tags and folder with Local AI: Apple Intelligence on a Mac that has it (the default there; macOS 26 or later with Apple Intelligence turned on), or a Local AI app (LM Studio, lmstudio.ai, or Ollama, ollama.com) running with a model; "File it" moves it there. Without either, the inbox still works, with the details entered by hand.

Ask Rupert (the speech bubble, bottom right) also uses Local AI by default; cloud models (Claude, ChatGPT) are optional in Settings ▸ Assistant and need the user's own key or account.

Network server entitlement (com.apple.security.network.server): "Sign in with ChatGPT…" (Settings ▸ Assistant ▸ ChatGPT, with your plan): OpenAI's sign-in redirects the browser to http://127.0.0.1:1455/auth/callback, so Rupert listens on the loopback address for that one request and stops listening right after. Nothing else in the app accepts incoming connections.

Rupert is free and open source: https://github.com/micki-b/Rupert
