# Changelog

## Unreleased

- **Apple Intelligence on the Mac.** On a Mac with Apple silicon and macOS 26 or later, choose **Apple Intelligence** in Settings ▸ Local AI: inbox suggestions and Ask Rupert run on Apple's built-in model, with nothing to install and nothing leaving the Mac. It's where Rupert starts on such a Mac, unless you've already set up LM Studio or another server. It answers in seconds; a larger model in LM Studio or Ollama still files and answers better.
- **Local AI, not just LM Studio.** Rupert works with any app that runs AI models on your computer behind an OpenAI-compatible server: tested with LM Studio and Ollama, and others such as llama.cpp's server or Jan should work too. **Settings ▸ Local AI** has presets for LM Studio and Ollama, and Ask Rupert shows "Local AI · *model*".
- **Rupert says where Local AI runs.** On this computer, on another computer on your network, or elsewhere. An address outside your network counts as a cloud service: Ask Rupert shows ☁, and inbox suggestions don't use it.
- Thinking models get more room for inbox suggestions, and a clear message if they still run out.
- Updated the screens' framework to SvelteKit 3. This closes five security alerts in SvelteKit's server features, which Rupert doesn't use (it has no server).

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
