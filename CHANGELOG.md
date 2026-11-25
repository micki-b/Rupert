# Changelog

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
