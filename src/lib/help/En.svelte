<script lang="ts" module>
  // Rupert Help in English. Every language has its own complete help
  // (src/lib/help/*.svelte) with the same section ids; keep them in step
  // with each other and with the app's labels.
  export const sections: { id: string; title: string; sub?: boolean }[] = [
    { id: "start", title: "Getting started" },
    { id: "library", title: "Your library" },
    { id: "finding", title: "Finding documents" },
    { id: "details", title: "Titles, tags and dates" },
    { id: "tags", title: "Managing tags" },
    { id: "inbox", title: "The inbox" },
    { id: "scanning", title: "Scanning" },
    { id: "scansnap", title: "Moving from ScanSnap Home" },
    { id: "assistant", title: "Ask Rupert" },
    { id: "ai-models", title: "Setting up AI models" },
    { id: "local-ai", title: "Local AI (on this computer)", sub: true },
    { id: "claude", title: "Claude", sub: true },
    { id: "chatgpt-plan", title: "ChatGPT with your plan", sub: true },
    { id: "openai", title: "OpenAI-compatible services", sub: true },
    { id: "ai-trouble", title: "When the assistant doesn't work", sub: true },
    { id: "iphone", title: "The iPhone app" },
    { id: "android", title: "The Android app" },
    { id: "storage", title: "Where Rupert keeps things" },
    { id: "keys", title: "Keyboard shortcuts" },
    { id: "about", title: "About Rupert" },
  ];
</script>

<script lang="ts">
  let { mac, windows, cmd, version, repo: REPO }: { mac: boolean; windows: boolean; cmd: string; version: string; repo: string } =
    $props();
  const linux = $derived(!mac && !windows);
  const fileManager = $derived(mac ? "Finder" : windows ? "File Explorer" : "your file manager");
  const reveal = $derived(mac ? "Show in Finder" : windows ? "Show in Explorer" : "Show in Files");
  const keychain = $derived(
    mac ? "the macOS Keychain" : windows ? "the Windows Credential Manager" : "the system keyring",
  );
</script>

<!-- MARK: Getting started -->
<section id="start">
  <h1>Rupert, the librarian for your documents</h1>
  <p>
    Rupert keeps track of your scanned documents and other files in the folders you already use: on this computer,
    in iCloud Drive, OneDrive or Google Drive. It doesn't import or copy anything. Your folders stay the library, and Rupert adds
    search, tags, dates, favorites and an inbox for new scans.
  </p>
  <h3>First steps</h3>
  <ol>
    <li>
      <strong>Add a folder.</strong> Click <b>+</b> next to <b>Sources</b> in the sidebar and pick a folder, for example
      your Documents folder, or a folder in OneDrive or Google Drive. Its subfolders are included. Add as many sources as you like.
    </li>
    <li>
      <strong>Wait for the first pass.</strong> Rupert reads each document once so it can search its text. The status
      bar at the bottom shows the progress. You can keep working meanwhile.
    </li>
    <li>
      <strong>Set up the inbox</strong> (optional). In <b>Settings</b>, choose the folder new scans arrive in. Rupert
      recognizes their text and suggests how to file them. See <a href="#inbox">The inbox</a>.
    </li>
    <li>
      <strong>Set up Local AI</strong> (optional) for filing suggestions and the chat assistant, with LM Studio, Ollama
      or a similar app. See <a href="#local-ai">Local AI</a>.
    </li>
  </ol>
  <div class="note">
    Anything you do in {fileManager} or your sync app shows up in Rupert, and what you do in Rupert is an ordinary
    change to the files. Nothing is locked into Rupert.
  </div>
</section>

<!-- MARK: Library -->
<section id="library">
  <h2>Your library</h2>
  <p>
    The sidebar lists your <b>Sources</b>, the folders you added. Click a folder to see its documents, and a document
    to see its details on the right. Double-click (or <b>Open</b>) opens it in its usual app; <b>{reveal}</b> shows it
    in {fileManager}.
  </p>
  <p>The two buttons above the sources switch the sidebar between your <b>folders</b> and your <b>tags</b>.</p>
  <p>
    <b>Settings</b> ({mac ? "⌘," : "Ctrl+,"}, or the <b>Settings</b> button at the bottom left) opens in its own window,
    with its parts listed on the left: <b>General</b>, <b>Library</b>, <b>Inbox</b>, <b>Text recognition</b>,
    <b>Local AI</b> and <b>Assistant</b>. Type in the search field above them to find a setting. <b>Library</b> lists your
    sources with their options, the same ones as the <b>⋯</b> next to each source; <b>+</b> adds a folder. Changes
    apply right away.
  </p>
  <p>
    To give the sidebar, <b>Details</b> or the inbox's preview and <b>File it</b> more or less room, drag the edge
    between the columns. Double-click the edge for the usual width. Rupert remembers each width, and the middle always
    keeps some room. With the keyboard, press {mac ? "Option-Tab" : "Tab"} until the edge is selected and use the arrow keys (Shift for bigger
    steps).
  </p>
  <p>
    <b>Markdown files</b> (<code>.md</code>) open in Rupert's own reader, formatted with headings, lists, tables and
    pictures. Links to other Markdown files open there too (<b>Back</b> returns), web links open in your browser, and
    <b>Open in default app</b> hands the file to your usual editor. Online-only Markdown files open in your usual app,
    which downloads them.
  </p>
  <h3 id="views">Views</h3>
  <p>The three buttons at the right end of the folder path change how folders are shown, for all folders:</p>
  <ul>
    <li><b>List</b>: names, kinds, sizes and dates.</li>
    <li>
      <b>Large icons</b>: a picture of each document's first page (the start of the text for Markdown files). Use the
      arrow keys to move between them.
    </li>
    <li>
      <b>Gallery</b>: the selected document large, with a strip of all documents below. <kbd>←</kbd> and
      <kbd>→</kbd> move to the previous or next document; <kbd>↑</kbd> and <kbd>↓</kbd> (or the arrow buttons on the
      page) flip through its pages. Click the page to open it in the <a href="#viewer">page viewer</a>.
    </li>
  </ul>
  <p>
    <b>Sorting:</b> in the list, click a column title (Name, Size, Date, Modified, and Kind when there's room) to sort
    by it; click again to reverse it. <b>Date</b> is the date on the document (its file's date if it has none). In large
    icons and the gallery, choose the order in <b>Sort</b> beside the view buttons. Folders stay on top, and the order is
    kept for all folders.
  </p>
  <h3 id="viewer">The page viewer</h3>
  <p>
    Click a page (in the gallery, or the inbox's preview) to see it large. Zoom with <b>+</b> and <b>−</b>,
    {mac ? "⌘+ and ⌘−" : "Ctrl++ and Ctrl+−"}, pinching on a trackpad or {mac ? "⌘" : "Ctrl"} and the scroll wheel;
    double-click to zoom into a spot and again to see the whole page. <b>Fit page</b> ({mac ? "⌘0" : "Ctrl+0"}) and
    <b>Fit width</b> size it to the window. Drag the page, or scroll, to move around it. Flip pages with the arrows,
    <kbd>←</kbd> <kbd>→</kbd>, Page Up and Page Down, or type a page number. <b>Rotate</b> turns a sideways scan for
    viewing only; the file isn't changed. <b>Open in default app</b> opens it in your usual app. Escape, or a click
    outside, closes the viewer.
  </p>
  <p>
    Pictures are kept in a cache on this computer and made again whenever a document changes. Rupert never reads an
    online-only file to show it, since that would download it. Instead, while a PDF in an iCloud Drive, OneDrive or
    Google Drive folder is on this computer, Rupert keeps a small picture of its first page, so it still shows one after
    the file goes online-only. Online-only documents without one show their icon. Either way the cloud stays on them,
    and opening one downloads it.
  </p>
  <h3 id="selecting">Selecting several</h3>
  <p>
    Click a document, then {mac ? "⌘-click" : "Ctrl-click"} others to add them (or to take one out again), or Shift-click
    to select everything in between. <kbd>{cmd}A</kbd> selects everything in the folder or the search results, and Shift
    with <kbd>↑</kbd> <kbd>↓</kbd> (<kbd>←</kbd> <kbd>→</kbd> in large icons) extends the selection from the keyboard. This
    works in the list, large icons, the gallery's strip and search results.
  </p>
  <p>
    With several selected, <b>Details</b> shows how many, their total size and the tags they have in common, with what
    you can do to all of them. Right-click one of them for the same in a menu:
  </p>
  <ul>
    <li>
      <b>Move to…</b> one folder for all of them, or drag them onto a folder. Dragged out of the window, other apps get
      copies of all of them.
    </li>
    <li>
      <b>Add tags…</b> and <b>Remove tags…</b>, saved where each document keeps its details, as in
      <a href="#tags">Managing tags</a>. Online-only PDFs that would be left without tags are downloaded, after asking.
    </li>
    <li><b>Star</b> or unstar them.</li>
    <li>
      {#if !linux}<b>Share…</b> and {/if}<b>Email…</b>: {#if !linux}one share, or {/if}one message with all of them attached.
    </li>
    <li><b>Rename documents…</b> with just these ticked (in a folder, not in search results).</li>
    <li><b>{windows ? "Delete" : "Move to Trash"}</b>, with one question for all of them.</li>
  </ul>
  <p>
    Longer batches show their progress at the bottom of the window, then a summary, with anything that didn't work.
  </p>
  <h3 id="dragging-out">Dragging documents out</h3>
  <p>
    Drag a document (or a folder, or a scan in the inbox) out of Rupert's window onto Mail, Messages, a website's upload
    area, {fileManager} or the desktop, and that app gets a <em>copy</em> of the file, under its own name. Your library
    keeps the file. Online-only files are fetched by the app you drop them on. Dropped on one of Rupert's own folders
    (it lights up while you're over it), a document is still moved there, as before.
  </p>
  <h3 id="sending">Sending documents</h3>
  <p>
    Right-click a document and choose {#if linux}<b>Email…</b>{:else}<b>Share…</b> or <b>Email…</b>{/if}, or use the
    buttons beside {reveal} in the details and
    beside Open in the inbox.
    {#if mac}<b>Share…</b> opens the Mac's share menu: Mail, Messages, AirDrop, Notes and the apps you've added.
    {:else if windows}<b>Share…</b> opens the Windows Share window.{/if}
    <b>Email…</b> starts a new message in your mail program with the document attached and its title as the subject.
  </p>
  <p>
    Rupert keeps your notes inside each PDF, so a PDF is sent as a copy <b>without its notes</b> (the title, date and
    tags go along). To send them too, turn on <b>Include my notes when sending a PDF</b> in Settings ▸ Library. A scan in
    the inbox goes out under the name it would be filed under, not the scanner's. An online-only document is downloaded
    first; the bottom of the window shows it, with Cancel.
  </p>
  <h3 id="renaming">Renaming</h3>
  <p>
    Right-click a document or folder and choose <b>Rename…</b>, or select it and press
    {mac ? "Return" : "F2"}{mac ? " (⌘O or ⌘↓ opens it)" : ""}. Type the new name in place and press
    {mac ? "Return" : "Enter"}; Escape leaves it as it was. Only the part before the extension is selected, and if you
    leave the extension out, the old one stays. It's renamed on disk right away, together with its hidden details file,
    and online-only files are renamed without downloading them.
  </p>
  <p>
    Rupert never replaces anything: a name that's already taken in the folder is refused. So are names some computers
    can't use, since your folders may sync between a Mac, Windows and Linux: names with <code>/ \ : * ? " &lt; &gt; |</code>,
    names that start with a dot or end with a dot or a space, and names Windows keeps for itself, such as CON or NUL. If
    you rename the inbox folder (or a folder holding it), Rupert's inbox setting follows it. Search results can't be
    renamed: open the folder first.
  </p>
  <h3 id="renaming-documents">Renaming many documents</h3>
  <p>
    Right-click a folder and choose <b>Rename documents…</b> to give its documents new names together; from a document,
    the same command lists its folder with only that document ticked. A table shows each current name and its new name,
    with what changes highlighted, and next to it the selected document's first page, title, date and tags. Untick the
    ones to leave alone. Nothing changes until you click <b>Rename</b>.
  </p>
  <p>
    The new names start from each document's date and title, by your name pattern (Settings ▸ Inbox). Click a new name to
    change it, and Rupert names the others <b>the same way</b>: right away when the rule is in the date and title (date
    last, another date format, a word in front), otherwise with Local AI, which follows your example. Names you typed are
    marked <b>Yours</b> and stay as they are. With Local AI you can also describe the names ("insurer - policy - year")
    and click <b>Propose with AI</b>; proposals arrive one by one, and <b>Stop</b> keeps those so far. Local AI only
    reads what Rupert has already indexed, so online-only files aren't downloaded, and their text never goes to a cloud
    service. <b>Undo</b> in the hint goes back to the previous proposals.
  </p>
  <p>
    Rupert flags problems before renaming: a name that's taken gets " (2)" added, and names that aren't allowed or don't
    change are left out. The extension and each document's details stay with it; tick <b>Also set each document's title
    from its new name</b> to change the titles too. Afterwards, <b>Undo</b> at the bottom of the window ({mac
      ? "or Edit ▸ Undo, ⌘Z"
      : "or Ctrl+Z"}) gives all of them their old names (and titles) back, until you do something else.
  </p>
  <h3 id="moving">Moving</h3>
  <p>
    Drag a document or folder onto a folder, in the sidebar or in the folder view, or right-click it and choose
    <b>Move to…</b> to pick a folder. It moves on disk right away, also into another of your folders, and a document's
    hidden details file goes along with it, so its title, tags and notes stay with it. If a document with that name is
    already there, Rupert adds " (2)" rather than replacing anything.
  </p>
  <p>
    Moving never downloads anything. An online-only file therefore moves only within its own cloud folder (for
    example from one OneDrive folder to another); to move it elsewhere, open it once so it's on this computer, then move
    it. Folders move only within the same disk. The folder that holds the <a href="#inbox">inbox</a> can't be moved
    until you choose another inbox in <b>Settings ▸ Inbox</b>.
  </p>
  <h3 id="adding">Adding files</h3>
  <p>
    Drag files from {fileManager} or another app onto a folder in Rupert: the open folder, or a folder in the sidebar
    or the folder view. Rupert copies them there and opens that folder; the originals stay where they were. Each copy
    keeps its name and date, with " (2)" added if the name is taken. Drop them on <b>Inbox</b> in the sidebar instead,
    and Rupert reads them and suggests where to file them, as with a new scan.
  </p>
  <p>
    Only files can be dropped, not folders. A source shows documents only unless <b>Show all file types</b> is on, so
    other kinds of files are turned away with a note saying why.
  </p>
  <h3 id="deleting">Deleting</h3>
  <p>
    Right-click a document or folder and choose <b>{windows ? "Delete" : "Move to Trash"}</b>, or select it and press
    <kbd>{mac ? "⌘⌫" : "Delete"}</kbd>. It goes to the {windows ? "Recycle Bin" : "Trash"}, so you can get it back from
    there. A document's hidden details file goes along with it. Documents go right away; for a folder, Rupert asks first,
    since everything inside it goes too.
  </p>
  <p>
    Rupert doesn't open online-only files to delete them; the deletion syncs like any other, and iCloud Drive,
    OneDrive and Google Drive also keep deleted files for a while on their websites. Folders you added as sources can't be deleted this way; use
    <b>Remove from Rupert…</b> in the source's <b>⋯</b> menu (that leaves the folder on disk).
  </p>
  <h3 id="undo">Undo</h3>
  <p>
    Right after Rupert changes something, a notice at the bottom of the window says what, with <b>Undo</b>. The same is
    {#if mac}<b>Edit ▸ Undo</b> (<kbd>⌘Z</kbd>), which names the change{:else}<kbd>Ctrl+Z</kbd>{/if}. It undoes the last
    one of these: moving documents or folders (dragging or <b>Move to…</b>), renaming one or many, filing a scan from the
    inbox (it goes back into the inbox with its old name, the details it got stay in it), and changing tags (in the tag
    manager or with <b>Add tags…</b> / <b>Remove tags…</b>). Rupert only undoes what's still as it left it: a file moved
    or changed since, or a name taken again, stays as it is, and the notice says so. Undo covers the last change only,
    until you quit Rupert. Something moved to the {windows ? "Recycle Bin" : "Trash"} comes back from there.
  </p>
  <h3>Source options</h3>
  <p>Click <b>⋯</b> next to a source:</p>
  <ul>
    <li>
      <b>Show all file types</b>: by default Rupert shows documents only (PDFs, images, Office files, text). Turn this
      on to see every file.
    </li>
    <li>
      <b>Download online-only files to index them</b> (iCloud Drive, OneDrive and Google Drive sources): see
      <a href="#online-only">online-only files</a> below.
    </li>
    <li><b>Rename…</b> changes the name shown in Rupert, not the folder's name on disk.</li>
    <li><b>Remove from Rupert…</b> stops showing the folder. The folder and its files stay exactly where they are.</li>
  </ul>
  <h3 id="online-only">Online-only files</h3>
  <p>
    iCloud Drive, OneDrive and Google Drive can keep files in the cloud only, to save space. Rupert shows them with a cloud icon
    <span class="cloud">☁</span> and never opens them on its own, because opening one downloads it. You can still find
    them by name and by the details Rupert knows about them, and opening one downloads it as usual. Click the cloud to
    download just that file: Rupert then reads its text, and the file stays on the computer.
  </p>
  <p>
    On Windows, Google Drive set to <b>Stream files</b> doesn't tell other apps which files are on the computer, so
    Rupert treats every file on its <b>Google Drive (G:)</b> drive as online-only. Titles, tags and text then come
    from Rupert on another computer (through the shared catalog), or from <b>Download online-only files to index
    them</b>. With Google Drive set to <b>Mirror files</b>, everything is on the computer and read as usual.
  </p>
  <p>
    To search the <em>text</em> of online-only documents, turn on <b>Download online-only files to index them</b> for that
    source. Rupert then downloads them a few at a time, reads their text, keeps a small picture of each PDF's first page, and
    frees the space again. PDFs whose text is already known are downloaded once too if they have no picture on this computer yet. Nothing in the
    files changes. It can take hours for a big folder; files over 100 MB are skipped. What Rupert learns is saved in the
    folder itself, so your other computers (and the iPhone app) can search those documents without downloading them
    again.
  </p>
  <p>
    To bring one folder fully up to date right away, right-click it (in the sidebar or the folder view, or use a
    source's <b>⋯</b> menu) and choose <b>Index this folder</b>. Rupert reads every document in it and its subfolders
    again, and downloads the online-only ones to read them and keep a picture, then frees the space again. This works
    even with <b>Download online-only files to index them</b> turned off.
  </p>
  <p>
    While Rupert indexes or downloads, the bottom right of the window shows how far it is, with <b>Stop</b>. Stop lets
    the file in hand finish and then halts; what's done is kept. <b>Resume</b>, or the next start of Rupert, carries on.
  </p>
</section>

<!-- MARK: Finding -->
<section id="finding">
  <h2>Finding documents</h2>
  <h3>Search</h3>
  <p>
    Type in the search field at the top ({cmd}F). Rupert looks in titles, file names, tags, notes and the text of the
    documents, across all sources. Every word you type has to match; the last one may be the start of a word, so
    <kbd>impel</kbd> finds “impeller”. Accents don't matter. Press <kbd>Esc</kbd> to clear the search.
  </p>
  <p>
    Below the search field, narrow the search down by <b>date</b> (this year, last year or dates you choose: the date on
    the document, else its file's date), by <b>kind</b> (PDF, image, Office, text, email) and by <b>where</b>:
    everywhere, the open folder's source, or the open folder with its subfolders. They work without words too: choose
    <b>Last year</b> and <b>PDF</b> for all of last year's PDFs. <b>Best match</b> and <b>Newest</b> above the results
    change their order. <kbd>Esc</kbd> clears the words and the filters.
  </p>
  <p>
    In the results, <b>Show in Rupert</b> (in the details) takes you to the document's folder.
  </p>
  <h3>Tags</h3>
  <p>
    Switch the sidebar to <b>Tags</b>. With nothing typed it shows your most-used tags; type to narrow the list, or
    click <b>Show all</b> below it. Click tags to select several, and choose under <b>Match</b>:
  </p>
  <ul>
    <li><b>Any</b>: documents with at least one of the selected tags.</li>
    <li><b>All</b>: documents with every selected tag. The list then offers only tags that go together with them.</li>
  </ul>
  <p>The tag filter combines with the search text and with Favorites. Click a chip's × above the list to remove it.</p>
  <h3>Favorites</h3>
  <p>
    Click the ☆ on a document to make it a favorite. It shows when you point at a document in the list, the icons or
    the search results, and always in its details and in the gallery; favorites keep a ★. Click the ★ to remove it.
    Removing the star from an online-only PDF downloads it, because the star is stored inside the file. Mark a folder
    with the ☆ next to its path above the document list. <b>Favorites</b> in the sidebar lists both. Favorites are saved with the files, so they show up on your other
    computers and on the iPhone.
  </p>
</section>

<!-- MARK: Details -->
<section id="details">
  <h2>Titles, tags and dates</h2>
  <p>Select a document to see and edit its details on the right:</p>
  <ul>
    <li><b>Title</b>: a readable name, independent of the file name.</li>
    <li><b>Document date</b>: the date on the paper (an invoice date, a letter's date), not when it was scanned.</li>
    <li><b>Tags</b>: as many as you like. Type and press Return or a comma to add one.</li>
    <li><b>Notes</b>: anything you want to remember about it.</li>
  </ul>
  <p>
    Click <b>Save</b> to keep your changes, or <b>Revert</b> to undo them. They're saved inside the PDF itself, in the
    standard place other programs (Acrobat, Preview and many more) read too. Files that can't hold details, like Word
    documents or photos, keep them in a small hidden file next to them. The details panel says which.
  </p>
  <p>
    For an online-only PDF, click <b>Download and show details</b> to read what's stored inside it. Saving changes to an
    online-only file downloads it first.
  </p>
</section>

<!-- MARK: Tags -->
<section id="tags">
  <h2>Managing tags</h2>
  <p>
    With the sidebar on <b>Tags</b>, click <b>Manage tags</b>. Pick one tag to <b>rename</b> it, several to
    <b>merge</b> them into one, or use <b>Delete…</b> to remove tags from every document. Rupert shows how many documents
    will change before it does anything, and changes each document wherever its details are kept.
  </p>
  <p>
    Online-only documents aren't downloaded for this when it can be avoided: the change is noted beside them and moves
    into the PDF the next time it's saved. If a change would leave an online-only PDF with no tags at all, Rupert asks
    before downloading it.
  </p>
</section>

<!-- MARK: Inbox -->
<section id="inbox">
  <h2>The inbox</h2>
  <p>
    The inbox is a folder where new scans arrive: the folder your scanner software saves to, or the one scans from
    Rupert and the iPhone go to. It has to be inside one of your sources. Choose it in <b>Settings ▸ Inbox</b>.
    To add files from your computer, drag them onto <b>Inbox</b> in the sidebar or into the open inbox.
  </p>
  <p>For each new file, Rupert:</p>
  <ol>
    <li>
      <b>Recognizes the text</b> if it's a scan without any, and adds an invisible text layer to the PDF, so any PDF app
      can search it. This happens on this computer ({mac ? "with Apple's built-in text recognition" : "with Tesseract, if it is installed"}).
      Turn it on or off in <b>Settings ▸ Text recognition</b>.
    </li>
    <li>
      <b>Suggests a title, date, tags and folder</b>, using <a href="#local-ai">Local AI</a> (LM Studio, Ollama or a
      similar app), if it's running. Rupert suggests folders where similar documents already are.
    </li>
    <li>Waits for you. Nothing moves until you say so.</li>
  </ol>
  <p>
    Click <b>Inbox</b> in the sidebar to review. Check the suggestion, change anything, and click <b>File</b>
    ({cmd}{mac ? "↩" : "Enter"}). Rupert writes the details into the file, renames it and moves it to the folder.
    The two circling arrows next to the red trash can (<b>Suggest again</b>) ask Local AI for a new suggestion; they
    turn while it's working.
  </p>
  <p>
    The waiting scans fill the middle of the window, with the titles Rupert suggests. Search them (by name, suggested
    title, tags, folder or their text), and switch between <b>List</b>, <b>Large icons</b> and <b>Gallery</b> as in a
    folder; Rupert remembers the view for the inbox. The selected scan's page is at the top of the right column, above
    the form, in every view: flip through its pages with the arrows on the page or <kbd>Page Up</kbd> and
    <kbd>Page Down</kbd>, and click it to open the <a href="#viewer">page viewer</a>. In the gallery the page is also
    large in the middle.
  </p>
  <p>
    <b>Folder</b> shows the whole path to where the document goes, with the folder itself in bold (the source's name is
    left out when you have only one). To choose another, click it and type any part of a folder's name: the suggested
    folders come first, then every folder that matches, each with where it is. Use the arrow keys and
    {mac ? "Return" : "Enter"}, or click one. The folder button next to the field, or <b>Browse all folders…</b> at the
    end of the list, opens all your folders as a tree; there, <b>New folder…</b> makes a folder inside the selected one
    and chooses it. The suggestions under the field show just enough of each path to tell them apart.
  </p>
  <p>
    In the inbox list, long folder paths show the last two folders, like “…/Sol y Mar/Purchase”. Point at a folder, or
    at any field that doesn't show all its text, to see all of it.
  </p>
  <p>
    To get rid of a scan instead, click the red trash can under the form (<b>{windows ? "Delete…" : "Move to Trash…"}</b>), right-click
    it in the list, or press <kbd>{mac ? "⌘⌫" : "Delete"}</kbd>. Rupert asks first, since a new scan may not be saved
    anywhere else yet; it goes to the {windows ? "Recycle Bin" : "Trash"}, where you can get it back.
  </p>
  <p>
    The file name follows the pattern in <b>Settings ▸ Inbox</b>, under <b>File names</b>: <code>{"{date}"}</code> and
    <code>{"{title}"}</code> are filled in, so the default <code>{"{date} {title}"}</code> gives names like
    “2026-04-29 Car insurance renewal.pdf”. If a file with that name exists, Rupert adds “(2)”; it never overwrites.
  </p>
  {#if !mac}
    <div class="note">
      Text recognition on Windows and Linux uses the free <b>Tesseract</b> program. If Settings says no engine was
      found, install Tesseract and restart Rupert.
    </div>
  {/if}
</section>

<!-- MARK: Scanning -->
<section id="scanning">
  <h2>Scanning</h2>
  <h3 id="scan-with-phone">With your phone</h3>
  {#if mac}
    <p>
      <b>With an iPhone or iPad nearby</b> (Continuity Camera), no app is needed: click <b>Scan…</b> in the inbox, then
      <b>Scan with iPhone…</b>, or choose <b>File ▸ Import from iPhone or iPad</b>, and pick <b>Scan Documents</b> under
      your iPhone. Scan the pages on the iPhone and tap <b>Save</b>: the PDF goes straight into the inbox. The iPhone has
      to be signed in to the same Apple Account as the Mac, with Wi-Fi and Bluetooth on.
    </p>
  {/if}
  <p>
    <b>With the Rupert app</b> on the iPhone or Android, the camera scans paper into the same inbox. Click <b>Scan…</b>
    in the inbox and point your phone's camera at the code: it opens Rupert's Scan tab, or shows where to get Rupert if
    it isn't on the phone yet. In the Scan tab, choose the inbox folder the window names. The phone can only reach an
    inbox in iCloud Drive (iPhone), OneDrive or Google Drive, not one in a folder on this computer. See
    <a href="#iphone">The iPhone app</a> and <a href="#android">The Android app</a>.
  </p>
  <h3>ScanSnap and other scanner software</h3>
  <p>
    Any scanner app that saves to a folder works with Rupert. For a ScanSnap, create a ScanSnap Home profile that saves
    PDFs to Rupert's inbox folder. Rupert picks up each new scan from there. A network scanner without software of its own
    works the same way: {#if mac}in <b>Image Capture</b>, choose the inbox folder under <b>Scan To</b>{:else if windows}in the
      <b>Windows Scan</b> app, choose the inbox folder as where scans are saved{/if}.
  </p>
  <h3 id="import-into-inbox">Importing many files</h3>
  <p>
    To bring in a batch of existing files (old scans, a folder of statements), click <b>Import…</b> in the inbox{#if mac}
      or choose <b>File ▸ Import into Inbox…</b> (⇧⌘I){/if}. Drop files or folders on the window, or use <b>Add files…</b> and
    <b>Add folder…</b>; folders come with everything inside. It lists how many documents each brings; click <b>Import</b>.
    Rupert copies them into the inbox (the originals stay where they are, with their names and dates) and shows the progress
    at the top of the inbox, with <b>Cancel</b>; each document appears as it arrives, and Rupert reads it and suggests where
    to file it, as with a scan. A document whose content is already in your library isn't copied again: the summary says how
    many, and <b>Show</b> lists them with where they are (click one to see it there). Files that aren't documents are left
    out.
  </p>
  <h3 id="print-to-rupert">Printing to Rupert</h3>
  <p>
    Many documents never pass a scanner: an invoice in a browser, a statement page, a booking confirmation.
    {#if mac}
      Turn on <b>Add “Save to Rupert” to the Print dialog's PDF menu</b> in <b>Settings ▸ Inbox</b>. Then, in any app,
      choose <b>File ▸ Print</b>, open the <b>PDF</b> menu at the bottom and choose <b>Save to Rupert</b>: the PDF goes to
      the inbox, where Rupert reads it and suggests where to file it, like a scan. (In the Mac App Store edition, turning
      it on asks where to save the entry: keep the <b>PDF Services</b> folder in your Library folder, making it if it isn't
      there.)
    {:else if windows}
      Turn on <b>Add “Rupert inbox” to File Explorer's Send to menu</b> in <b>Settings ▸ Inbox</b>. Then right-click files
      in File Explorer and choose <b>Send to ▸ Rupert inbox</b> to put them in the inbox. To bring in what you print, choose
      the <b>Microsoft Print to PDF</b> printer and save into the inbox folder.
    {:else}
      Print to a PDF file and save it in the inbox folder: Rupert reads it and suggests where to file it, like a scan.
    {/if}
  </p>
</section>

<!-- MARK: ScanSnap Home -->
<section id="scansnap">
  <h2>Moving from ScanSnap Home</h2>
  <p>
    If you used ScanSnap Home to organize your scans, Rupert can bring over what it knows that isn't in the files:
    document dates, favorites, receipt details and its recognized text. Choose <b>Import from ScanSnap Home…</b>
    at the bottom of the sidebar{mac ? " or in the File menu" : ""}.
  </p>
  <p>
    Rupert first shows what it found and what it will change, including ScanSnap folders that aren't in your sources yet
    (<b>Add as source</b>). Nothing you've set in Rupert is overwritten, online-only files aren't downloaded, and running
    it again changes nothing.
  </p>
</section>

<!-- MARK: Assistant -->
<section id="assistant">
  <h2>Ask Rupert</h2>
  <p>
    Click the chat button in the lower-right corner ({cmd}J) to ask Rupert for documents in plain words. He searches,
    reads and shows what he finds in the main window. For example:
  </p>
  <ul>
    <li>“Find the impeller invoice from last spring”</li>
    <li>“What's the deductible on the boat insurance?”</li>
    <li>“Show everything tagged Taxes and 2025”</li>
    <li>“Open the Boat folder”</li>
  </ul>
  <p>
    The owl shows what he's doing: thinking, searching, reading a document, or opening something. A line under the
    conversation names the current step. Ask in any language: Rupert answers in the same one.
  </p>
  <ul>
    <li>
      <b>Stop</b> (the ■ button, or <kbd>Esc</kbd>) ends an answer at once. Your question goes back into the box so you
      can change it.
    </li>
    <li><b>New conversation</b> (the circular arrow) starts over.</li>
    <li>The gear opens the assistant's settings.</li>
  </ul>
  <p>
    Rupert can look at your library, but <b>never changes</b> anything: no renaming, moving, tagging or deleting. He only
    reads the text Rupert already knows, so he never downloads online-only files.
  </p>
  <p>
    By default he runs on Local AI, on this computer. You can choose Claude or another cloud service instead: see
    <a href="#ai-models">Setting up AI models</a>.
  </p>
</section>

<!-- MARK: AI models -->
<section id="ai-models">
  <h2>Setting up AI models</h2>
  <p>Rupert uses AI in two places, set up separately:</p>
  <table>
    <thead><tr><th>Feature</th><th>Where it runs</th><th>Settings</th></tr></thead>
    <tbody>
      <tr>
        <td>Inbox suggestions (title, date, tags, folder)</td>
        <td>Local AI, always (on this computer or your local network)</td>
        <td><b>Settings ▸ Local AI</b>, and <b>Suggestions</b> in <b>Settings ▸ Inbox</b></td>
      </tr>
      <tr>
        <td>Ask Rupert (the chat)</td>
        <td>Local AI by default, or a cloud model you choose</td>
        <td><b>Settings ▸ Assistant</b></td>
      </tr>
    </tbody>
  </table>
  <p>
    Text recognition doesn't use AI services at all: it runs on this computer. In <b>Settings ▸ Assistant</b>, the
    <b>Model</b> menu picks one of three kinds of connection:
  </p>
  <ul>
    <li>
      <a href="#local-ai"><b>Local AI</b></a> (LM Studio, Ollama or a similar app): private and free, needs a capable
      computer.
    </li>
    <li>
      <a href="#claude"><b>Claude (Anthropic, cloud)</b></a>: with an API key or your Anthropic account (a Max,
      Ultra or Team plan's monthly credits can pay for it).
    </li>
    <li>
      <a href="#chatgpt-plan"><b>ChatGPT, with your plan (cloud)</b></a>: sign in with ChatGPT; no API key, uses your
      ChatGPT plan.
    </li>
    <li>
      <a href="#openai"><b>OpenAI-compatible service (cloud)</b></a>: OpenAI or another service that works the same way.
    </li>
  </ul>
  <div class="note cloud-note">
    <b>☁ About privacy.</b> With a cloud model, your questions and what the assistant looks at (document titles, tags,
    folder names, and the text of documents it reads) are sent to that service. Whenever a cloud model is in use, the
    chat panel shows <span class="cloud">☁</span> and the model's name under “Ask Rupert”. Inbox suggestions and text
    recognition never use a cloud service.
  </div>
</section>

<!-- MARK: Local AI -->
<section id="local-ai">
  <h3 class="sub-h">Local AI (on this computer)</h3>
  <p>
    Local AI runs a model on your own computer, so nothing goes to the internet. Rupert works with any app that offers an
    OpenAI-compatible server. It's tested with <a href="https://lmstudio.ai">LM Studio</a> and
    <a href="https://ollama.com">Ollama</a>; others, such as llama.cpp's <code>llama-server</code> or Jan, should work
    too. Local AI works best on a Mac with Apple silicon and plenty of memory, or a PC with a recent graphics card.
  </p>
  {#if mac}
    <p>
      <b>With Apple Intelligence</b> (a Mac with Apple silicon and macOS 26 or later): nothing to install. Turn on
      Apple Intelligence in <b>System Settings ▸ Apple Intelligence &amp; Siri</b>, then in Rupert choose
      <b>Apple Intelligence</b> under <b>Settings ▸ Local AI ▸ Server</b> (on such a Mac, Rupert starts with it). It answers in seconds and nothing leaves the
      Mac. It's a small model, though: a larger model in LM Studio or Ollama files and answers better.
    </p>
  {/if}
  <p><b>With LM Studio:</b></p>
  <ol>
    <li>Install LM Studio from <a href="https://lmstudio.ai">lmstudio.ai</a> and open it.</li>
    <li>
      Download a model in LM Studio's search. For the assistant, choose one that supports <b>tool use</b> (LM Studio marks
      these); for example a recent Qwen model. Bigger models give better answers but are slower.
    </li>
    <li>Start the local server: in LM Studio's <b>Developer</b> tab, switch the server on. Load your model there too.</li>
  </ol>
  <p><b>With Ollama:</b></p>
  <ol>
    <li>Install Ollama from <a href="https://ollama.com">ollama.com</a> and open it. It runs in the background.</li>
    <li>
      Download a model that supports <b>tools</b> (Ollama's model library marks these), for example in a terminal:
      <code>ollama pull qwen3:4b</code>.
    </li>
  </ol>
  <p><b>Then, in Rupert:</b></p>
  <ol>
    <li>
      Open <b>Settings ▸ Local AI</b>. Under <b>Server</b>, choose LM Studio or Ollama, which fills in the
      <b>Address</b> (<code>http://localhost:1234</code> or <code>http://localhost:11434</code>); for another server,
      type its address. It should show <span class="ok">Connected</span>.
    </li>
    <li>
      Under <b>Model</b>, pick your model. “Automatic” takes the first model the server lists: with LM Studio that's the
      loaded model, but Ollama lists every model you've downloaded, so pick one there.
    </li>
    <li>
      In <b>Settings ▸ Assistant</b>, choose <b>Local AI</b>. The assistant uses the same server and model as the inbox
      suggestions. Changes in Settings apply right away; click <b>Done</b> when you're finished.
    </li>
  </ol>
  <p class="tip">
    Local models take their time: a question can take a minute or two, mostly spent thinking. You can keep using Rupert
    meanwhile, and stop the answer at any time.
  </p>
  <p class="tip">
    If answers stop short or the assistant loses track of what it read, give the model more room (its context length):
    in Ollama's settings, or when you load the model in LM Studio.
  </p>
  <p class="tip">
    The server can run on another computer in your home, too: let it serve on the local network (LM Studio's option to
    serve on the local network, or Ollama's “Expose Ollama to the network”), and enter that computer's address, for
    example <code>http://192.168.1.20:11434</code>. Only do this on a network you trust; these servers have no password.
    Settings says where documents' text goes. An address outside your home network counts as a cloud service: Ask
    Rupert shows <span class="cloud">☁</span>, and inbox suggestions don't use it.
  </p>
</section>

<!-- MARK: Claude -->
<section id="claude">
  <h3 class="sub-h">Claude</h3>
  <p>
    Claude, by Anthropic, answers quickly and well. It's a paid cloud service, billed by use through an Anthropic
    developer account (the Claude Console). That account is separate from a claude.ai subscription: Anthropic doesn't
    let other apps use a subscription's own usage, so Rupert can't run on your Claude plan directly.
  </p>
  <div class="note" id="claude-credits">
    <b>Have a Max, Ultra or Team plan?</b> It includes monthly credits for the Claude Console ($100 a month with
    Max, $200 with Ultra, up to $500 pooled with Team), which Rupert can spend:
    <ol>
      <li>
        On <a href="https://claude.ai">claude.ai</a>, open <b>Settings ▸ Billing</b> (Team: <b>Organization settings ▸
        Billing</b>) and, under <b>API credits</b>, link a Claude Console organization. You can create one there. Pick
        carefully: the link can't be changed later without Anthropic's support.
      </li>
      <li>Sign in to Claude below with that organization (option 1 or 2).</li>
    </ol>
    The credits are used first, expire at the end of each month and don't roll over. When they're used up, requests
    stop until next month (unless you've bought credits too); nothing is ever charged to your plan. Free and Pro plans
    don't include them. New subscribers can claim them after 7 days.
  </div>
  <p>In <b>Settings ▸ Assistant</b>, choose <b>Claude (Anthropic, cloud)</b>. Then choose how to sign in.</p>

  <h4>Option 1: API key</h4>
  <ol>
    <li>
      Sign in or create an account at <a href="https://platform.claude.com">platform.claude.com</a>, and add credits under
      Billing.
    </li>
    <li>
      Create a key at <a href="https://platform.claude.com/settings/keys">platform.claude.com/settings/keys</a> and copy
      it.
    </li>
    <li>In Rupert, choose <b>API key</b> under Sign-in, paste the key and click <b>Save key</b>.</li>
  </ol>
  <p>
    The key is kept in {keychain}, never in Rupert's settings file. <b>Remove</b> deletes it from there. You can
    revoke the key on the Claude Console at any time.
  </p>

  <h4>Option 2: your Anthropic account, without a key</h4>
  <p>
    Rupert can use your Anthropic account through Anthropic's command-line tool, <code>ant</code>, so you don't handle
    a key at all.
  </p>
  <ol>
    <li>
      Install <code>ant</code>:
      {#if mac}
        in Terminal, run <code>brew install anthropics/tap/ant</code> (this needs
        <a href="https://brew.sh">Homebrew</a>).
      {:else if windows}
        download it from <a href="https://github.com/anthropics/anthropic-cli/releases">its releases page</a> and put
        <code>ant.exe</code> in a folder on your PATH.
      {:else}
        follow the steps in <a href="https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart">Anthropic's
        CLI quickstart</a>.
      {/if}
    </li>
    <li>
      In Rupert, choose <b>Anthropic account, through the <code>ant</code> command</b> under Sign-in, then click
      <b>Sign in…</b>. Your browser opens; sign in and approve. The account line changes to “Signed in”.
    </li>
  </ol>
  <p>
    Use is billed to the Console organization you picked when signing in. The sign-in lasts some weeks; when it runs out,
    the assistant says so and <b>Sign in again…</b> renews it.
  </p>

  <h4>Which Claude model</h4>
  <p>
    Leave <b>Claude model</b> empty to use <code>claude-opus-5-5</code>, the most capable. For faster, cheaper answers,
    enter <code>claude-sonnet-5-5</code> or <code>claude-haiku-5-5</code>.
  </p>
</section>

<!-- MARK: ChatGPT plan -->
<section id="chatgpt-plan">
  <h3 class="sub-h">ChatGPT with your plan</h3>
  <p>
    If you pay for ChatGPT, Rupert can use your plan instead of an API key: you sign in with your ChatGPT account, and
    Rupert's questions count against your plan's limits. OpenAI calls this “Sign in with ChatGPT”. It's a preview, so
    OpenAI may still change how it works and which plans it covers.
  </p>
  <ol>
    <li>In <b>Settings ▸ Assistant</b>, choose <b>ChatGPT, with your plan (cloud)</b>.</li>
    <li>
      Click <b>Sign in with ChatGPT…</b>. Your browser opens OpenAI's sign-in page; sign in and allow Rupert to use your
      ChatGPT plan. The page then says you can close it, and Settings shows your account.
    </li>
    <li>Pick a <b>Model</b>: the list shows what your plan offers other apps.</li>
  </ol>
  <p>
    Rupert keeps the sign-in in {keychain}. It doesn't see your ChatGPT conversations, and nothing it sends is saved at
    OpenAI for later. In ChatGPT's settings you can see Rupert among the apps using your plan, cap how much it may use
    each week, or remove it. <b>Sign out</b> in Rupert forgets the sign-in on this computer.
  </p>
  <p class="tip">
    When Rupert's share of your plan is used up, the assistant says so; it starts working again when your limit resets.
    Nothing is charged beyond your plan.
  </p>
</section>

<!-- MARK: OpenAI-compatible -->
<section id="openai">
  <h3 class="sub-h">OpenAI-compatible services</h3>
  <p>
    Many AI services accept requests the way OpenAI's does: OpenAI itself, and others such as OpenRouter, Groq or Mistral.
    In <b>Settings ▸ Assistant</b>, choose <b>OpenAI-compatible service (cloud)</b> and fill in:
  </p>
  <dl>
    <dt>Service</dt>
    <dd>
      The service's API address. Leave it empty for OpenAI (<code>https://api.openai.com/v1</code>). Others publish theirs,
      for example <code>https://openrouter.ai/api/v1</code>, <code>https://api.groq.com/openai/v1</code> or
      <code>https://api.mistral.ai/v1</code>.
    </dd>
    <dt>Model</dt>
    <dd>The model's name exactly as the service lists it. Pick one that supports tool use (also called function calling).</dd>
    <dt>API key</dt>
    <dd>
      Create one on the service's website (for OpenAI:
      <a href="https://platform.openai.com/api-keys">platform.openai.com/api-keys</a>), paste it and click
      <b>Save key</b>. It's kept in {keychain}.
    </dd>
  </dl>
  <p>Open the chat: its header shows the model with <span class="cloud">☁</span>.</p>
</section>

<!-- MARK: Troubleshooting AI -->
<section id="ai-trouble">
  <h3 class="sub-h">When the assistant doesn't work</h3>
  <table>
    <thead><tr><th>Message or symptom</th><th>What to do</th></tr></thead>
    <tbody>
      <tr>
        <td>“Local AI isn't running, or has no model.”</td>
        <td>Open LM Studio or Ollama, start its server and make sure it has a model. Settings ▸ Local AI should say Connected.</td>
      </tr>
      <tr>
        <td>“The model ran out of room before answering…”</td>
        <td>A thinking model spent its room thinking. Choose a model that doesn't think, or thinks less.</td>
      </tr>
      <tr>
        <td>“Add an Anthropic API key…” or “Add an API key…”</td>
        <td>Paste a key in Settings ▸ Assistant and click Save key.</td>
      </tr>
      <tr>
        <td>“The ant command isn't installed.”</td>
        <td>
          Install it (see <a href="#claude">Claude</a>), then reopen Settings. Rupert looks for it on your PATH and in the
          usual install folders.
        </td>
      </tr>
      <tr>
        <td>“Not signed in to Anthropic”</td>
        <td>Click Sign in… in Settings ▸ Assistant.</td>
      </tr>
      <tr>
        <td>“your ChatGPT plan's limit for Rupert is used up”</td>
        <td>Wait for the limit to reset, or raise Rupert's limit in ChatGPT's settings.</td>
      </tr>
      <tr>
        <td>“this ChatGPT account or plan can't be used by other apps”</td>
        <td>Your plan or workspace doesn't allow it. Choose another model in Settings ▸ Assistant.</td>
      </tr>
      <tr>
        <td>“The ChatGPT sign-in has expired”</td>
        <td>Click Sign in again… in Settings ▸ Assistant.</td>
      </tr>
      <tr>
        <td>“not signed in or no access”</td>
        <td>The key was revoked or mistyped, or the sign-in ran out. Save a new key, or sign in again.</td>
      </tr>
      <tr>
        <td>A message about your credit balance</td>
        <td>
          Add credits to the account on the service's website. With a Claude Max, Ultra or Team plan, check that its
          monthly credits are linked to the organization you signed in with (see <a href="#claude-credits">Claude</a>).
        </td>
      </tr>
      <tr>
        <td>“rate limited, try again shortly”</td>
        <td>The service is busy or you've hit its limit. Wait a minute and ask again.</td>
      </tr>
      <tr>
        <td>Answers without looking anything up, or “I couldn't finish that…”</td>
        <td>The model may not handle tools well. Try a larger model, or one marked for tool use.</td>
      </tr>
      <tr>
        <td>Very slow answers</td>
        <td>Normal for big local models. Choose a smaller model in Settings ▸ Local AI, or a cloud model.</td>
      </tr>
    </tbody>
  </table>
</section>

<!-- MARK: iPhone -->
<section id="iphone">
  <h2>The iPhone app</h2>
  <p>Rupert on the iPhone shows the same library, so you can look things up away from the computer.</p>
  <ul>
    <li>
      <b>Add folders</b> from iCloud Drive or On My iPhone with the Files picker: the same folders Rupert uses on the
      computer. For <b>OneDrive</b>, sign in with your Microsoft account in the app instead (the Files picker can't share
      OneDrive folders with other apps). <b>Google Drive</b> works the same way (a preview): the Google Drive app doesn't
      share its folders with other apps either, so sign in with your Google account in <b>Settings ▸ Google
      Drive</b>. Google hasn't reviewed Rupert, so its sign-in page warns that the app isn't verified; tap
      <b>Advanced</b>, then <b>Go to Rupert</b>.
    </li>
    <li>
      <b>Search and browse</b> titles, tags and text. The text comes from what Rupert on your computer has read, so let it
      finish its first pass there.
    </li>
    <li><b>Open</b> a document to download and view it, then share or print it.</li>
    <li>
      <b>Share</b> a document (beside Open) to send it with Mail, Messages, AirDrop or another app. As on the computer, a
      PDF goes as a copy without your notes, unless Settings ▸ Sharing includes them.
    </li>
    <li>
      <b>Views</b>: at the top of a folder, switch between a list, large icons and a gallery (one document large, the
      others in a strip below; swipe to go through them). Rupert remembers the view for each folder. Pictures come from
      OneDrive or Google Drive, which make them without downloading the document; in folders from Files, the iPhone
      makes them for documents that are on it. Online-only documents there show their icon.
    </li>
    <li>
      <b>Scan</b> with the camera. Choose color, grayscale or black &amp; white and the resolution in the Scan tab. Scans
      are saved to the inbox folder you pick, which should be the computer's inbox, so Rupert there can recognize and
      file them. The <b>Inbox</b> is always at the top of the Library tab, so you can see your scans until they are
      filed, even if the inbox folder isn't one of your library folders.
    </li>
    <li>
      <b>Send documents to the inbox</b> from other apps: in Mail, Files, Photos or Safari, tap Share and choose
      <b>Rupert</b>. The document is saved to the inbox right away, under its own name, without opening Rupert. If it
      can't be saved just then (offline, say), Rupert sends it the next time you open the app. Rupert takes documents
      only: PDFs, pictures, Office files, text and emails. If Rupert isn't in the row of apps, look under <b>More</b>.
    </li>
    <li><b>Star folders</b> to add them to Favorites on all your devices.</li>
    <li>
      <b>Ask Rupert</b> in the <b>Ask</b> tab, on an iPhone with Apple Intelligence (iOS 26 or later, with Apple
      Intelligence turned on in iOS Settings). It runs on the iPhone, and nothing leaves it. It searches and reads like
      on the computer; the documents, folders and results it found appear as buttons under its answer. On other iPhones the tab isn't there.
    </li>
    <li>
      <b>Language</b>: the app follows the iPhone's language, or the one you choose in <b>Settings ▸ Language</b> (or
      in iOS Settings for Rupert).
    </li>
  </ul>
  <p>
    Its version and build number are at the bottom of its <b>Settings</b>, under <b>About</b>, with a link to the
    privacy policy.
  </p>
  <p>The iPhone app doesn't change your documents: only Rupert on the computer edits details and files scans.</p>
</section>

<!-- MARK: Android -->
<section id="android">
  <h2>The Android app</h2>
  <p>Rupert for Android (in the works) shows the same library on an Android phone, like the iPhone app.</p>
  <ul>
    <li>
      <b>Add folders</b> from <b>Google Drive</b> or <b>OneDrive</b>: sign in under <b>Settings</b>, then add the same
      folders Rupert uses on your computer. iCloud Drive doesn't exist on Android, and folders on the phone itself aren't
      supported yet.
    </li>
    <li>
      <b>Search, browse and open</b> documents. Opening one downloads it and shows it in another app, such as your PDF
      viewer.
    </li>
    <li>
      <b>Views</b>: a list, large icons or a gallery, as on the iPhone, with pictures from Google Drive or OneDrive.
    </li>
    <li>
      <b>Share</b> a document (beside Open) to send it with Gmail, Messages or another app; a PDF goes without your notes
      unless Settings ▸ Sharing includes them.
    </li>
    <li>
      <b>Scan</b> with Google's document scanner: in the Scan tab, choose the inbox folder in Google Drive or OneDrive.
      The pages are saved there as one PDF.
    </li>
    <li>
      <b>Send documents to the inbox</b> from other apps: tap Share and choose <b>Rupert</b>. Rupert opens and sends the
      document to the inbox in Google Drive or OneDrive, under its own name.
    </li>
    <li><b>Star folders</b>, and choose the <b>Language</b> in Settings, as on the iPhone.</li>
    <li>The back gesture closes what's open, then goes up a folder. Ask Rupert isn't on Android yet.</li>
  </ul>
  <p>The Android app doesn't change your documents: only Rupert on the computer edits details and files scans.</p>
</section>

<!-- MARK: Storage -->
<section id="storage">
  <h2>Where Rupert keeps things</h2>
  <ul>
    <li>
      <b>Your documents</b> stay in your folders. Rupert renames or moves a file only when you ask: when you file it from the inbox, or
      move it to another folder.
    </li>
    <li>
      <b>Titles, tags, dates, notes and favorites</b> are stored inside PDFs, or in a hidden <code>.rupert</code> folder
      next to files that can't hold them. They travel with the files to your other computers.
    </li>
    <li>
      <b>The search index</b> is a cache on this computer. Rupert rebuilds it from your files whenever needed, so nothing
      is lost if it's deleted.
    </li>
    <li>
      <b>The shared catalog</b>: in iCloud Drive, OneDrive and Google Drive sources, a hidden <code>.rupert/catalog</code> folder holds
      the text Rupert has read, so your other devices can search online-only documents.
    </li>
    <li><b>API keys</b> are kept in {keychain}.</li>
    <li>
      <b>The language</b> changes nothing on disk: folders and files Rupert creates (like <code>.rupert</code>) have the
      same names in every language, since devices set to different languages share them.
    </li>
  </ul>
</section>

<!-- MARK: Keys -->
<section id="keys">
  <h2>Keyboard shortcuts</h2>
  <table class="keys">
    <tbody>
      <tr><td><kbd>{cmd}F</kbd></td><td>Search</td></tr>
      <tr><td><kbd>{mac ? "⌘⌫" : "Delete"}</kbd></td><td>Move what's selected to the {windows ? "Recycle Bin" : "Trash"}</td></tr>
      <tr><td><kbd>{cmd}A</kbd></td><td>Select everything in the folder or the search results</td></tr>
      <tr><td><kbd>←</kbd> <kbd>→</kbd></td><td>Large icons and gallery: previous or next document</td></tr>
      <tr><td><kbd>↑</kbd> <kbd>↓</kbd></td><td>Gallery: previous or next page. List and search results: previous or next document (with Shift, also select it)</td></tr>
      <tr><td><kbd>Esc</kbd></td><td>Clear the search; in the chat, stop the answer or close the panel</td></tr>
      <tr><td><kbd>{cmd}J</kbd></td><td>Open or close Ask Rupert</td></tr>
      <tr><td><kbd>Return</kbd></td><td>In the chat: send (<kbd>Shift+Return</kbd> starts a new line)</td></tr>
      <tr><td><kbd>{cmd}{mac ? "↩" : "Enter"}</kbd></td><td>In the inbox: file the document</td></tr>
      {#if mac}<tr><td><kbd>⌘,</kbd></td><td>Settings</td></tr>{/if}
      <tr><td><kbd>F1</kbd>{#if mac} or <kbd>⇧⌘/</kbd>{/if}</td><td>This help</td></tr>
    </tbody>
  </table>
</section>
<!-- MARK: About -->
<section id="about">
  <h2>About Rupert</h2>
  <p>
    Rupert{version ? ` ${version}` : ""}, the librarian for your scanned documents. Free and open source under the
    <a href="{REPO}/blob/main/LICENSE">MIT license</a>, © 2026 Micki Braun.
  </p>
  <ul>
    <li>
      <b>Language:</b> Rupert follows your computer's language, or the one you choose in <b>Settings ▸ General</b>. It
      speaks English, Rioplatense Spanish and German. In Spanish the owl wears the sky-blue and white jersey, in German
      lederhosen.
    </li>
    <li>
      <b>Updates:</b> Rupert looks for a new version once a day (turn it off in <b>Settings ▸ General</b>). When one is
      out, <b>Update to …</b> appears in the status bar; click it to install, and Rupert restarts.{#if mac}
        Or choose <b>Rupert ▸ Check for Updates…</b>.{/if}
    </li>
    <li>
      <b>Asking for help:</b> mention the version and edition shown at the bottom left of the window (for example
      “Rupert 0.1.0 · GitHub”). <b>GitHub</b> means installed from GitHub (it updates itself); <b>App Store</b> means
      the Mac App Store edition (the App Store updates it).
    </li>
    <li><a href={REPO}>Source code, releases and issues</a> on GitHub</li>
    <li>
      <a href="{REPO}/blob/main/PRIVACY.md">Privacy policy</a>: Rupert collects nothing about you; optional cloud
      features are listed there with exactly what they send.
    </li>
    <li>
      <a href="{REPO}/blob/main/THIRD_PARTY_NOTICES.md">Third-party notices</a>: the open-source components Rupert is
      built with, and their licenses (also included with the app).
    </li>
  </ul>
</section>
