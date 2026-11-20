# Rupert privacy policy

*Last updated: November 18, 2026*

Rupert is a free, open-source app for organizing your scanned documents, made by Micki Braun. It runs on your
computer (macOS, Windows, Linux) and on your iPhone. This policy explains what happens to your information when you
use it.

## The short version

- **Rupert collects nothing about you.** There are no accounts with us, no analytics, no tracking, no ads and no
  crash reports sent anywhere. We have no servers that receive your data.
- **Your documents stay where they are**: in the folders you choose, on your device or in your own cloud storage
  (iCloud Drive, OneDrive, Google Drive). Rupert reads and changes them there, and keeps its search index on your device.
- **Some optional features talk to other services** that you choose and set up yourself. They are listed below,
  with exactly what is sent.

## What Rupert stores, and where

- **Titles, tags, dates, notes and favorites** are written into your documents (inside PDFs) or into small hidden
  files next to them (a `.rupert` folder in the same folder). They sync with your files through your own cloud
  storage, like any other file.
- **A search index and a cache of page pictures** are kept in the app's own folder on each device. They can be
  deleted at any time; Rupert rebuilds them from your files.
- **A shared catalog** (the hidden `.rupert/catalog` folder in iCloud Drive, OneDrive and Google Drive folders) holds the text
  Rupert has read from your documents, so your other devices can search documents that are stored only online.
  It stays in your cloud storage.
- **Settings** are stored on your device. **Sign-ins and API keys** for optional services are stored in your
  device's secure storage (the macOS or iOS Keychain, the Windows Credential Manager, or the Linux keyring), never
  in plain files.

## Optional features that use other services

None of these is on unless you turn it on.

- **Text recognition (OCR)** runs on your device: Apple's built-in text recognition on Mac and iPhone, or Tesseract
  on Windows and Linux. Nothing is sent anywhere.
- **Filing suggestions** use LM Studio, an app you install yourself that runs AI models on your own computer (or one
  you choose on your network). Rupert sends it the beginning of a new scan's text, its file name, your tags and the
  names of some of your folders, so it can suggest a title, date, tags and folder. It never uses an online service.
- **Ask Rupert (the chat assistant)** uses LM Studio by default (on your computer). If you choose a cloud model in
  Settings, your questions and what the assistant looks at to answer them (document titles, tags, folder names, and
  the text of documents it reads) are sent to that service, under its own privacy policy:
  - **Claude**, by Anthropic (with your API key or Anthropic account): [Anthropic's privacy policy](https://www.anthropic.com/legal/privacy).
  - **ChatGPT with your plan**, by OpenAI (signed in with your ChatGPT account): requests are made with
    `store: false`, so OpenAI is asked not to keep them; Rupert doesn't see your ChatGPT conversations.
    [OpenAI's privacy policy](https://openai.com/policies/privacy-policy).
  - **An OpenAI-compatible service** you enter yourself (with your API key): that service's policy applies.

  While a cloud model is in use, the chat panel shows a cloud symbol and the model's name.
- **OneDrive on iPhone**: when you sign in with your Microsoft account, Rupert reads the OneDrive folders you choose
  through Microsoft Graph, and saves new scans to your inbox folder there. It asks only for access to your files
  (`Files.ReadWrite`) and to stay signed in. [Microsoft's privacy statement](https://privacy.microsoft.com/privacystatement).
- **Google Drive on iPhone** (preview): when you sign in with your Google account, Rupert reads the Google Drive
  folders you choose through the Google Drive API: their file lists, Rupert's own small catalog and details files,
  and a document when you open it. It saves new scans to your inbox folder there and, when you star a folder, that
  folder's small details file. It asks for access to your Drive files (`drive`); the sign-in stays in your iPhone's
  Keychain, and nothing goes to us. [Google's privacy policy](https://policies.google.com/privacy).
- **Network scanners**: Rupert looks for scanners on your local network (Bonjour) and talks to the one you choose
  directly. Nothing leaves your network.
- **Markdown documents** that link to pictures on the web load those pictures from the web when shown, as a browser
  would.
- **Updates** (desktop): Rupert checks GitHub for a newer version and downloads it from there. GitHub sees the
  request, as with any download. [GitHub's privacy statement](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement).

## Camera and photos (iPhone)

The camera is used only while you scan, to take pictures of pages. They become a PDF in the inbox folder you chose
and are not kept anywhere else.

## Children

Rupert isn't directed at children and collects no information from anyone.

## Changes and contact

If this policy changes, the new version will be published here with a new date. Questions: open an issue at
[github.com/micki-b/Rupert](https://github.com/micki-b/Rupert/issues).
