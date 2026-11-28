# Changelog

## 0.10.2 — 2026-11-28

Coming up and saved searches on the phones.

- **Phones: Coming up and saved searches** (#162): the Library tab on the iPhone and Android is now arranged like the computer's sidebar: Inbox, Coming up and Favorites, then Saved searches, then the folders. **Coming up** lists every document that's due and not done, overdue first, with who it's from and the amount; the done ones are a tap away. A **saved search** opens with all its filters, also when Ask Rupert shows one. Both are read-only: mark documents done and save searches on the computer.
- **Coming up on the computer** (#166): its **Show** menu looks like the search filters above it.

## 0.10.1 — 2026-11-28

Small fixes.

- **Clear a due date** (#160): an × beside **Due date** in File it and in Details takes away a date that doesn't apply, for example one the inbox suggested.
- **Tesseract on Windows** (#163): when no text recognition is found, Settings ▸ Text recognition shows the winget command to install Tesseract, with a Copy button, and **Check Again** finds it without restarting Rupert.
- **Tidier window** (#161): Settings and Help at the bottom left are now a gear and a ? icon, and Settings ▸ General has more room above **Check now**.

## 0.10.0 — 2026-11-27

A smarter library.

- **From, amount and due date** (#108): documents can record who they're from, the amount on them with its currency, and when they're due (to pay by, or valid until). The inbox suggests them, only when the text shows them; **Read details…** fills them in for documents you already have. Search by **From** and by due date, show them as list columns, see totals for a selection or a search, and ask Rupert "how much did I pay the dentist this year?".
- **Coming up** (#109): a list in the sidebar of documents whose due date is near (bills a week ahead, passports and contracts three months ahead), with **Done**, **Remind me later** and **Add to Calendar…**. The count beside it shows what's overdue or due within a week. In Settings ▸ Reminders, notifications from macOS or Windows (one summary a day, or one per document) that arrive even when Rupert is closed, and the count on the Dock icon. One switch turns it all off.
- **Saved searches** (#110): **Save Search…** keeps a search with its filters under a name in the sidebar; one click runs it again. They're kept with the library, so your other computers see them, and Ask Rupert can show one.
- **Library health and duplicates** (#111, #113): **Library health** in the sidebar shows what could be better, with a fix for each: duplicate copies (compare side by side, keep one, or keep all as they are), scans without text (recognized in place), online-only documents not yet indexed, documents without a date or tags (suggested), and scanner-style names.
- **Suggestions that learn, and File all** (#112): inbox suggestions learn from how you filed similar documents. A suggestion that matches them is marked **Sure**, and **File All Sure Ones…** files them all after a review, with one Undo.
- **Inbox in a cloud folder** (#144): online-only scans in the inbox are downloaded so they can be read and filed, and Settings ▸ Inbox recommends keeping the folder downloaded.
- **Google Drive on Windows** (#153, #155): the inbox shows its page previews, and a source can be marked **Files here are available offline**, so its files count as on the computer.
- **Fixed:** the right column no longer flickers at some widths on Windows. Finder aliases are left out of the library. Counts past 999 read 1.2K.
- **Phones:** a document's details show who it's from, its amount and its due date, and Ask Rupert can add up amounts.

## 0.9.0 — 2026-11-26

Working with many documents.

- **Select several** (#102): ⌘-click (Ctrl-click) and Shift-click select several documents and folders in the list, large icons, gallery and search results; ⌘A selects all. Details then shows how many, their size and the tags they share, with actions for all of them: **Move to…**, **Add tag** / **Remove tag**, **Star** / **Unstar**, **Share…**, **Email…**, **Export…** and **Move to Trash…**, also in the right-click menu. Dragging one of them drags them all.
- **Sort and filter** (#103): click a column title (Name, Kind, Size, Date, Modified) to sort the list, again to reverse it; **Date** is the date on the document. Large icons and the gallery have a **Sort** menu. Folders stay on top, and the order is kept for all folders. Searches can be narrowed by date (this year, last year, a range), by kind and by place (a source or folder), and sorted by best match or newest.
- **Undo** (#104): Edit ▸ Undo (⌘Z, Ctrl+Z) or the bar at the bottom of the window takes back the last move, filing, rename or tag change, as long as the files are still as Rupert left them. Edit ▸ Undo says what it would undo.
- **Quick open and a spacebar preview** (#105): ⌘K (Ctrl+K), or File ▸ Quick Open…, goes to a folder, a document (by title or name), a tag or a command by typing a few letters, with your recent documents and favorite folders first. **Space** opens the selected document in the page viewer from any view; ↑ and ↓ move to the neighbors.
- **Search hits on the page, find, and page citations** (#106): a document opened from search results shows the search's words marked on its pages, with next and previous. ⌘F (Ctrl+F) in the page viewer finds words in a document. Ask Rupert names the page an answer comes from, like "[Lease 2024, p. 3]"; click it to open that page with the passage marked. Searching long documents is much faster, and typing in the search field no longer stalls the window.
- **Export…** (#107): copies of a selection, a folder, a source or the search results go into a zip file or a folder, keeping their folders and names, with a list (index.csv for a spreadsheet, index.html to read) of each document's title, date, tags and original place. Notes go along only if you include them. The originals stay as they are.
- **Page preview while browsing** (#127): the selected PDF or picture's page is at the top of Details in the list, large icons and search results, with page flipping and a click to the page viewer; online-only documents show the picture Rupert kept. The inbox's page and this one have a divider to make them taller or shorter; on a short window they shrink first and give way to a **Show page** button, so the form below never has to scroll. × on the page, or View ▸ Hide Page Preview (⇧⌘P), hides them. The Notes boxes now resize only up and down.
- **List columns** (#138): drag the line between two column titles to resize a column, drag a title sideways to move it, and right-click the titles to show or hide columns or **Reset Columns**; Rupert keeps the layout on this computer. A new Star column stars folders too, as favorite folders.
- **Fixed:** the inbox's right-click **Open** and **Show in Finder** did nothing. Menus and dialogs in the middle of the window are no longer placed off to the side or cut off.
- **Phones:** Ask Rupert names the page an answer comes from, and search is faster in long documents.

## 0.8.0 — 2026-11-25

Getting documents in.

- **Print to Rupert** (#101): on a Mac, turn on **Add “Save to Rupert” to the Print dialog's PDF menu** in Settings ▸ Inbox; then **File ▸ Print ▸ PDF ▸ Save to Rupert** in any app puts the PDF in the inbox, to be read and filed like a scan (the Mac App Store edition asks once where to save the entry: the PDF Services folder in your Library folder). On Windows, the switch adds **Send to ▸ Rupert inbox** to File Explorer. The entry follows the inbox when it moves, and Rupert's language.
- **Import into Inbox…** (#122): **Import…** in the inbox (or **File ▸ Import into Inbox…**, ⇧⌘I) takes many files and folders at once: drop them on the window, or **Add files…** / **Add folder…**. Rupert copies them into the inbox (the originals stay, with their names and dates), shows the progress at the top of the inbox with **Cancel**, and reads and suggests a folder for each as it arrives. A document whose content is already in your library isn't copied again; **Show** lists those with where they are. Files that aren't documents are left out.
- **Scan with your phone** (#123, #124): **Scan…** in the inbox now opens a panel for scanning with a phone.
  - On a Mac, **Scan with iPhone…** (or **File ▸ Import from iPhone or iPad**) uses Continuity Camera: choose **Scan Documents** under a nearby iPhone or iPad, and the PDF goes straight into the inbox. No app is needed on the phone.
  - A QR code opens the Scan tab of Rupert on the iPhone or Android phone, or shows where to get Rupert if it isn't there yet (a new page, rupertlibrarian.com/scan/). The panel names the inbox folder to choose there, and says when the phone can't reach the inbox (a folder on this computer, or iCloud Drive for Android).
  - A scanner's own software that saves to the inbox folder works as before; the help explains Image Capture and Windows Scan for network scanners without software of their own.
- **Network scanning is removed** (#123): Rupert no longer finds and drives AirScan (eSCL) scanners itself. It was never tested with a real scanner, and the phone and scanner software cover it. Rupert no longer looks for devices on the local network.

## 0.7.0 — 2026-11-25

- **Rename documents…** (#64): right-click a folder (or a document, or a source's ⋯ menu) ▸ **Rename documents…** gives a folder's documents new names together, in a before/after table with what changes highlighted and the selected document's first page and details beside it. New names start from each document's date and title (the name pattern). Change one, and Rupert names the others the same way: right away when the rule is in the date and title (another date format, the date last, a word in front), otherwise with Local AI following your example. With Local AI you can also describe the names ("insurer - policy - year") and **Propose with AI**, one document at a time with progress and Stop. Taken names get " (2)", names that aren't allowed or don't change are left out, nothing is ever replaced, and optionally the titles follow the new names. Afterwards **Undo** (the bar at the bottom of the window, Edit ▸ Undo / ⌘Z, Ctrl+Z) gives them their old names back.
- **Share… and Email…** (#69): right-click a document ▸ **Share…** (the Mac's share menu: Mail, Messages, AirDrop, Notes…; the Windows Share window) or **Email…** (a new message with the document attached and its title as the subject; on Linux through `xdg-email`), also as buttons beside Show in Finder in the details and beside Open in the inbox. A PDF goes as a copy **without your notes** unless Settings ▸ Library ▸ **Include my notes when sending a PDF** is on; the copy is rewritten whole, so no earlier revision keeps them. An inbox scan goes under the name it would be filed under. Online-only documents are downloaded first, with Cancel.
- **Share on the phone** (#69): a **Share** button beside Open in a document's details opens the iPhone's share sheet or Android's share chooser; notes are left out the same way (Settings ▸ Sharing).
- **Fixed:** in a small window, the Ask Rupert button no longer covers Suggest again and Move to Trash: the inbox's note shares its row (#92). Dragging a document over another folder and back no longer leaves a copy behind (#96).

## 0.6.1 — 2026-11-25

- **Page viewer zoom steps:** **+** and **−** zoom by a quarter again (×1.25), rounded to two digits: 100, 130, 160, 200, 250 % up and 80, 64, 51 % down. Fit width rounds down the same way (230 %).

## 0.6.0 — 2026-11-25

- **A roomier inbox** (#59): the waiting scans fill the middle of the window, with a search field (name, suggested title, tags, folder and the scanned text) and the same **List**, **Large icons** and **Gallery** views as folders, remembered for the inbox. The selected scan's page moves to the top of the right column, above **File it**, and its pages can be flipped through there. In the gallery the page is large in the middle and the right column has only the form.
- **Long folder paths stay readable when filing** (#63): the inbox list shows the last two folders ("…/Sol y Mar/Purchase"); point at one to see the whole path. File it's fields show their full text on hover too.
- **Resizable columns** (#61): drag the edge between the sidebar, the middle and Details (or the inbox's preview and File it) to make a column wider or narrower, double-click it for the usual width, or move it with the arrow keys. Each width is remembered; the middle always keeps room.
- **Pictures for online-only documents** (#62): while a PDF in an iCloud Drive, OneDrive or Google Drive folder is on this computer (when Rupert indexes it, downloads it to index it, or shows it; with **Download online-only files to index them** on, also PDFs whose text was already known get downloaded once for it), Rupert keeps a small picture of its first page. Once the file is online-only, the large icons and the gallery show that picture, and any page pictures still in the cache, instead of a blank icon. Rupert still never reads the file itself; the cloud stays on it, and opening it downloads it. These pictures have their own space (up to 1 GB) so they outlast the ordinary picture cache. The status line shows these downloads ("for their text and pictures") and the pictures being made for documents already indexed.
- **Stop, Resume and Index this folder** (#78): while Rupert indexes or downloads, the status line offers **Stop**: the file in hand finishes, then nothing more is indexed, downloaded or drawn until **Resume** (or the next start); waiting downloads pick up where they left off. Right-click a folder (sidebar or folder view, or a source's ⋯ menu) ▸ **Index this folder** re-reads every document in it and its subfolders and downloads its online-only ones to read them and keep a picture, even with the download setting off.
- **Settings in their own window** (#80): Settings (⌘, or Ctrl+,) open in a separate window with the parts listed in a sidebar and a search field above them: **General** (language, updates), **Library** (your sources with their options, also still in each source's ⋯ menu; add and remove folders), **Inbox** (inbox folder, suggestions, file names), **Text recognition**, **Local AI** and **Assistant**. Settings reopen on the part used last, the chat's gear opens **Assistant**, and changes apply right away in both windows.
- **Rename documents and folders** (#65): right-click ▸ **Rename…**, or Return on the Mac (⌘O or ⌘↓ opens) and F2 on Windows and Linux, then type the new name in place in the list, large icons, gallery or sidebar. The extension stays unless you type a new one; the hidden details file and catalog entries go along; online-only files rename without downloading. Taken names and names some computers can't use are refused, and a renamed inbox folder keeps being the inbox.
- **Choosing the folder when filing** (#63): File it's **Folder** shows the whole path, wrapping, with the folder itself in bold. Click it and type to search every folder (empty ones too): the folder's name in bold with where it is below, suggested folders first, the same on Mac, Windows and Linux. A folder button (or **Browse all folders…**) opens the folder tree at the current choice, with **New folder…**. Suggestion chips show the shortest ending that tells them apart, and the source's name is left out when there's only one source.
- **A page viewer** (#83): click the page in the inbox's preview or a folder's gallery to see it large: zoom (buttons, ⌘+ ⌘− ⌘0, pinch or ⌘-scroll, double-click), pan by dragging or scrolling, flip pages (arrows, ← →, Page Up/Down, or type a number), rotate a sideways scan for viewing, or open it in its usual app. Zoomed in, a sharper picture is drawn. The inbox keeps its page preview above File it in every view, the gallery too.
- **File it's buttons**: Suggest again is now two circling arrows (they turn while Local AI works) and Move to Trash a red trash can, so File, Open and both icons fit on one line.
- **Drag documents out** (#84): drag a document, folder or inbox scan out of Rupert into Mail, Messages, a browser's upload area, the Finder, File Explorer or the desktop; that app gets a copy, and the library keeps the file. Dropped on Rupert's own folders, it still moves.

## 0.5.2 — 2026-11-24

- **Inbox suggestions speak Rupert's language** (#58): the short reason under the suggested folder is now in the language Rupert shows (Spanish or German too), not always English. The suggested title stays in the document's own language, since it becomes the file name.
- **Ask Rupert finds more, and makes up less** (#57): when a search finds nothing, Rupert searches again with fewer words (without the month, say) and, for a question in Spanish or German, with the words in English, since documents are often in another language than the question. With Apple Intelligence, an answer that gives amounts Rupert never read, or comes back in another language than the question, is asked again; it now reads the document before saying what it costs, and searches instead of asking which document you mean.
- **Android: Google Drive sign-in says why it failed** (#53): when Google refuses the sign-in (for example, because it doesn't recognize this copy of Rupert, or can't be reached), Rupert says so instead of quietly going back to **Not signed in**, as if you had cancelled.

## 0.5.1 — 2026-11-23

- **The privacy policy, from the phone**: Settings ▸ About has a **Privacy policy** link (on iPhone and Android), as Google Play asks.
- **The privacy policy** now gives a contact address, contact@rupertlibrarian.com, and says how to remove what Rupert keeps on a device (sign out, remove a folder, uninstall).

## 0.5.0 — 2026-11-21

- **A star on every document** (#22): in the list, the icons, the gallery and the search results, the ☆ shows when you point at a document (favorites keep their ★), and one click stars or unstars it. The star in the details pane also saves at once now; it no longer waits for **Save**.
- **Click the cloud to download a file** (#22): an online-only document's cloud badge downloads just that file, and Rupert reads its text.
- **Drag files in** (#36): drop files from the Finder, File Explorer or another app onto a folder (the open one, or one in the sidebar or the folder view), and Rupert copies them there, keeping their names and dates. Drop them on **Inbox** instead, and Rupert reads them and suggests where to file them, like a new scan.
- **Large icons and a gallery on the phone** (#41): at the top of a folder, switch between the list, large icons and a gallery; the phone remembers the view for each folder. Pictures come from OneDrive and Google Drive, which make them without the document being downloaded, and on the iPhone from the system for documents in Files folders that are on the phone.
- **Share documents to the inbox from the phone** (#42): on the iPhone, tap Share in Mail, Files, Photos or Safari and choose **Rupert**: the document is saved to the inbox (in Files, OneDrive or Google Drive) under its own name, without opening Rupert. If that can't happen right away, Rupert sends it the next time you open the app. On Android, sharing a document with Rupert opens it and sends the document to the inbox.
- **Rupert for Android is in the works** (#24): the phone app also builds for Android, with Google Drive and OneDrive libraries, Google's document scanner, Rupert's own icon, the back gesture and Android's wording. Not released yet.

## 0.4.1 — 2026-11-20

(The first release since 0.3.0; 0.4.0 was tagged but never released.)

- **Ask Rupert on the iPhone.** On an iPhone with Apple Intelligence (iOS 26 or later), the new **Ask** tab answers questions about your documents on the phone itself; what Rupert would show appears as buttons under his answer.
- **Apple Intelligence on the Mac.** On a Mac with Apple silicon and macOS 26 or later, choose **Apple Intelligence** in Settings ▸ Local AI: inbox suggestions and Ask Rupert run on Apple's built-in model, with nothing to install and nothing leaving the Mac. It's where Rupert starts on such a Mac, unless you've already set up LM Studio or another server. It answers in seconds; a larger model in LM Studio or Ollama still files and answers better.
- **Local AI, not just LM Studio.** Rupert works with any app that runs AI models on your computer behind an OpenAI-compatible server: tested with LM Studio and Ollama, and others such as llama.cpp's server or Jan should work too. **Settings ▸ Local AI** has presets for LM Studio and Ollama, and Ask Rupert shows "Local AI · *model*".
- **Rupert says where Local AI runs.** On this computer, on another computer on your network, or elsewhere. An address outside your network counts as a cloud service: Ask Rupert shows ☁, and inbox suggestions don't use it.
- Thinking models get more room for inbox suggestions, and a clear message if they still run out.
- Updated the screens' framework to SvelteKit 3. This closes five security alerts in SvelteKit's server features, which Rupert doesn't use (it has no server).
- Fixed: a library folder's **…** menu ran off the window when the folder's path was long (as Google Drive paths are), hiding **Remove from Rupert…**. The path now wraps inside the menu.

## 0.3.0 — 2026-11-20

- **Rupert speaks Spanish** (Rioplatense, as in Argentina): the screens, menus, messages and help, on the computer and on the iPhone. Rupert follows the system's language; **Settings ▸ Language** can choose another. Ask Rupert answers in the language you write in.
- **In Spanish, the owl wears the albiceleste**: the sky-blue and white striped jersey with the number 10.
- **Rupert speaks German** (formal "Sie"), everywhere Spanish is: screens, menus, messages, help, the iPhone app and the Windows installer.
- **In German, the owl wears lederhosen**: leather braces and a chest strap with an edelweiss, over a white German football shirt with the black-red-gold chevron.

## 0.2.0 — 2026-11-20

- **Move documents and folders** to another folder: drag them onto a folder in the sidebar or the folder view, or right-click ▸ **Move to…**. A document's details go with it, and nothing is ever replaced (a " (2)" is added instead). Online-only files move within their own cloud folder without being downloaded.
- **Google Drive** folders are supported on the computer (Google Drive for desktop), like iCloud Drive and OneDrive: online-only files are shown without downloading them. On Windows, where Google Drive's streaming drive doesn't say which files are on the computer, Rupert treats them all as online-only rather than downloading your whole Drive.
- **Right-click menu:** Open, Show in Finder (Show in File Explorer) and Show in Rupert work again; they did nothing since 0.1.0.
- **iPhone: the Inbox is always in the Library**, at the top, so new scans can be seen there until they're filed, even when the inbox folder isn't one of the library folders.
- **iPhone: Google Drive (preview).** Sign in with your Google account in Settings to add Google Drive folders and to scan into a Google Drive inbox, as with OneDrive. Google shows an "unverified app" warning at sign-in.
- **iPhone: Settings ▸ About** shows the app's version and build number.

## 0.1.1 — 2026-11-19

- **Windows: Sign in with ChatGPT works.** The sign-in was approved in the browser but never saved, because Windows keeps too little in one password entry for it; it's now saved in several. The browser page also waits until the sign-in is saved before saying it worked.
- **Ask Rupert** shows a model chosen in Settings ▸ Assistant right away, without closing and reopening the panel.
- **iPhone:** the layout keeps clear of the screen's sides where iOS asks for it: the iPhone Duo's outer screen (its status icons covered the favorite star) and any iPhone turned sideways.
- **macOS 13 (Ventura) or later** is now stated as the requirement, so older Macs are told before installing. Rupert's PDF engine already needed it.

## 0.1.0 — 2026-11-19 (first public release)

The first release for macOS, Windows, Linux and iPhone.

- **Library:** add the folders you already use (on this computer, iCloud Drive, OneDrive) as sources; Rupert shows them as they are and keeps up with changes. Online-only files are shown without downloading them.
- **Details inside your files:** titles, tags, document dates, notes and favorites are stored inside PDFs (or a small hidden file next to other kinds), so they travel with the files.
- **Search** across titles, tags, notes and the text of your documents; a tag filter (any or all); favorite documents and folders.
- **Views:** list, large icons and a gallery with page flipping. Markdown files open formatted in Rupert's reader.
- **Inbox:** new scans get their text recognized (on your device), a suggested title, date, tags and folder (LM Studio, on your computer), and are filed when you agree. Scan straight from network scanners (AirScan/eSCL), or import from ScanSnap Home.
- **Ask Rupert:** a chat assistant that finds documents and shows them, with LM Studio on your computer by default, or Claude, ChatGPT (with your plan) or another service if you choose.
- **Move to Trash** from Rupert, with the document's details file.
- **iPhone app:** browse and search the same library (OneDrive through your Microsoft account), open documents, star folders, and scan with the camera into the inbox.
- **Updates:** the desktop app updates itself from GitHub releases.
