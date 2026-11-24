//! German messages (Germany, formal "Sie"): English → German. Words shared
//! with the screens' catalog (`src/lib/i18n/de.ts`): Ordner, Bibliothek,
//! Bibliotheksordner, Eingang, Tag, nur online, Begleitdatei, anmelden,
//! Einstellungen ▸ Assistent, Rupert fragen. The macOS menu uses macOS's
//! own German names (Ablage, Bearbeiten, Darstellung, Fenster).

pub(super) static MESSAGES: &[(&str, &str)] = &[
    // MARK: - Errors (error.rs)
    (
        "couldn't read or write settings: {0}",
        "Die Einstellungen konnten nicht gelesen oder gespeichert werden: {0}",
    ),
    (
        "file watching failed: {0}",
        "Die Überwachung der Dateien ist fehlgeschlagen: {0}",
    ),
    ("not a folder: {0}", "Kein Ordner: {0}"),
    (
        "this folder is already a source",
        "Dieser Ordner ist bereits in der Bibliothek",
    ),
    (
        "this folder overlaps the source \"{0}\"; sources can't be nested",
        "Dieser Ordner überschneidet sich mit „{0}“; Bibliotheksordner können nicht ineinander liegen",
    ),
    ("unknown source", "Unbekannter Bibliotheksordner"),
    ("invalid path: {0}", "Ungültiger Pfad: {0}"),
    ("search index: {0}", "Suchindex: {0}"),
    ("PDF problem: {0}", "Problem mit dem PDF: {0}"),
    ("stopped", "Gestoppt"),
    // MARK: - Documents and their details
    (
        "the PDF is encrypted, so details are kept in a sidecar file",
        "Das PDF ist verschlüsselt, daher werden die Angaben in einer Begleitdatei gespeichert",
    ),
    (
        "the PDF has signature permissions, so details are kept in a sidecar file",
        "Das PDF hat Signaturberechtigungen, daher werden die Angaben in einer Begleitdatei gespeichert",
    ),
    (
        "the PDF is digitally signed, so details are kept in a sidecar file",
        "Das PDF ist digital signiert, daher werden die Angaben in einer Begleitdatei gespeichert",
    ),
    (
        "the PDF is encrypted, so Rupert won't add a text layer to it",
        "Das PDF ist verschlüsselt, daher fügt Rupert keine Textebene hinzu",
    ),
    (
        "the PDF has signature permissions, so Rupert won't add a text layer to it",
        "Das PDF hat Signaturberechtigungen, daher fügt Rupert keine Textebene hinzu",
    ),
    (
        "the PDF is digitally signed, so Rupert won't add a text layer to it",
        "Das PDF ist digital signiert, daher fügt Rupert keine Textebene hinzu",
    ),
    (
        "the file changed while saving; please try again",
        "Die Datei hat sich beim Speichern geändert; bitte versuchen Sie es erneut",
    ),
    (
        "the file changed while recognizing its text; please try again",
        "Die Datei hat sich während der Texterkennung geändert; bitte versuchen Sie es erneut",
    ),
    (
        "Some details are kept beside the file until it's next saved in Rupert",
        "Einige Angaben liegen neben der Datei, bis sie das nächste Mal in Rupert gespeichert wird",
    ),
    (
        "couldn't read the PDF ({0}); details are kept in a sidecar",
        "Das PDF konnte nicht gelesen werden ({0}); die Angaben werden in einer Begleitdatei gespeichert",
    ),
    (
        "document date must be YYYY-MM-DD, got \"{0}\"",
        "Das Dokumentdatum muss JJJJ-MM-TT lauten, nicht „{0}“",
    ),
    (
        "choose at least one tag",
        "Wählen Sie mindestens ein Tag aus",
    ),
    ("the new tag name is empty", "Der neue Name des Tags fehlt"),
    // MARK: - Moving and deleting
    ("the file name is empty", "Der Dateiname fehlt"),
    (
        "its details file couldn't be moved ({0}), so it was left where it was",
        "Die Begleitdatei konnte nicht bewegt werden ({0}), daher bleibt das Dokument, wo es war",
    ),
    (
        "its details file couldn't be moved ({0}), and moving the document back failed too ({1}): it is at {2}, its details file still in {3}",
        "Die Begleitdatei konnte nicht bewegt werden ({0}), und das Zurückbewegen des Dokuments ist ebenfalls fehlgeschlagen ({1}): Es liegt in {2}, seine Begleitdatei noch in {3}",
    ),
    (
        "copying the file failed",
        "Das Kopieren der Datei ist fehlgeschlagen",
    ),
    (
        "it's online-only, and moving it out of its cloud folder would download it. Download it first (open it once), then move it",
        "Es ist nur online verfügbar, und es aus seinem Cloud-Ordner zu bewegen, würde es herunterladen. Laden Sie es zuerst herunter (einmal öffnen) und bewegen Sie es dann",
    ),
    (
        "a source itself can't be moved",
        "Ein Bibliotheksordner selbst kann nicht bewegt werden",
    ),
    (
        "“{0}” can't be moved into itself",
        "„{0}“ kann nicht in sich selbst bewegt werden",
    ),
    (
        "it's a hidden or system file",
        "es ist eine versteckte Datei oder Systemdatei",
    ),
    (
        "it isn't a kind of document Rupert shows in this source. To add it, turn on \
             “Show all file types” for the source first",
        "es ist keine Dokumentart, die Rupert in dieser Quelle zeigt. Um sie hinzuzufügen, \
             schalten Sie zuerst „Alle Dateitypen zeigen“ für die Quelle ein",
    ),
    (
        "Can't move “{0}”: {1}.",
        "„{0}“ kann nicht bewegt werden: {1}.",
    ),
    (
        "“{0}” is a folder on another disk. Folders can be moved only within the same disk; move it in the Finder or File Explorer instead",
        "„{0}“ ist ein Ordner auf einem anderen Laufwerk. Ordner lassen sich nur innerhalb desselben Laufwerks bewegen; bewegen Sie ihn stattdessen im Finder oder Explorer",
    ),
    (
        "couldn't move it to the trash: {0}",
        "Konnte nicht in den Papierkorb gelegt werden: {0}",
    ),
    (
        "a source itself can't be moved to the trash",
        "Ein Bibliotheksordner selbst kann nicht in den Papierkorb gelegt werden",
    ),
    // MARK: - Folders
    (
        "Rupert can't open “{0}”: {1}",
        "Rupert kann „{0}“ nicht öffnen: {1}",
    ),
    (
        "folder bookmarks exist only on macOS",
        "Ordner-Lesezeichen gibt es nur unter macOS",
    ),
    (
        "couldn't remember access to the folder",
        "Der Zugriff auf den Ordner konnte nicht gespeichert werden",
    ),
    (
        "the folder's bookmark is damaged",
        "Das Lesezeichen des Ordners ist beschädigt",
    ),
    (
        "the folder can't be found",
        "Der Ordner wurde nicht gefunden",
    ),
    (
        "the folder's bookmark has no path",
        "Das Lesezeichen des Ordners hat keinen Pfad",
    ),
    // MARK: - Scanning and text recognition
    ("scanner: {0}", "Scanner: {0}"),
    (
        "{0} isn't reachable ({1})",
        "{0} ist nicht erreichbar ({1})",
    ),
    (
        "{0} answered {1} (is it an eSCL scanner?)",
        "{0} hat mit {1} geantwortet (ist es ein eSCL-Scanner?)",
    ),
    ("bad capabilities: {0}", "Fehlerhafte Geräteangaben: {0}"),
    (
        "the scanner reported no flatbed or feeder",
        "Der Scanner meldet weder Vorlagenglas noch Einzug",
    ),
    (
        "this scanner doesn't have that input",
        "Dieser Scanner hat diese Zufuhr nicht",
    ),
    (
        "couldn't start the scan ({0})",
        "Der Scan konnte nicht gestartet werden ({0})",
    ),
    (
        "the scanner is busy, or the feeder is empty",
        "Der Scanner ist beschäftigt, oder der Einzug ist leer",
    ),
    (
        "the scanner refused the scan ({0})",
        "Der Scanner hat den Scan abgelehnt ({0})",
    ),
    (
        "the scanner didn't say where the scan is",
        "Der Scanner hat nicht mitgeteilt, wo der Scan liegt",
    ),
    (
        "lost the scanner ({0})",
        "Die Verbindung zum Scanner ist abgebrochen ({0})",
    ),
    ("the scan failed ({0})", "Der Scan ist fehlgeschlagen ({0})"),
    (
        "no pages were scanned (is the feeder empty?)",
        "Es wurden keine Seiten gescannt (ist der Einzug leer?)",
    ),
    ("the scan has no pages", "Der Scan hat keine Seiten"),
    (
        "a scanned page is unreadable: {0}",
        "Eine gescannte Seite ist nicht lesbar: {0}",
    ),
    (
        "OCR failed: {0}",
        "Die Texterkennung ist fehlgeschlagen: {0}",
    ),
    ("tesseract failed", "Tesseract ist fehlgeschlagen"),
    (
        "no ScanSnap Home library in {0} ({1} is missing)",
        "Keine ScanSnap Home-Bibliothek in {0} ({1} fehlt)",
    ),
    (
        "this doesn't look like a ScanSnap Home library this version of Rupert can read ({0})",
        "Das scheint keine ScanSnap Home-Bibliothek zu sein, die diese Version von Rupert lesen kann ({0})",
    ),
    ("{0} is missing", "{0} fehlt"),
    // MARK: - Inbox
    (
        "Online-only: open it once to download it, then Rupert will read it.",
        "Nur online: Öffnen Sie es einmal, um es herunterzuladen; dann liest Rupert es.",
    ),
    (
        "Text recognized on {0} of {1} pages ({2})",
        "Text auf {0} von {1} Seiten erkannt ({2})",
    ),
    (
        "The text couldn't be added to the PDF itself ({0}); Rupert keeps it beside the file, so it can still be searched.",
        "Der Text konnte nicht in das PDF selbst eingefügt werden ({0}); Rupert legt ihn neben die Datei, damit sie trotzdem durchsuchbar ist.",
    ),
    (
        "Text recognition worked, but the text couldn't be saved: {0}; {1}",
        "Die Texterkennung hat funktioniert, aber der Text konnte nicht gespeichert werden: {0}; {1}",
    ),
    (
        "Text recognition failed: {0}",
        "Die Texterkennung ist fehlgeschlagen: {0}",
    ),
    (
        "Couldn't save the recognized text: {0}",
        "Der erkannte Text konnte nicht gespeichert werden: {0}",
    ),
    ("Text recognized ({0})", "Text erkannt ({0})"),
    (
        "No text to suggest from.",
        "Kein Text für Vorschläge vorhanden.",
    ),
    (
        "set up the inbox first: scans are saved there",
        "Richten Sie zuerst den Eingang ein: Dort werden die Scans gespeichert",
    ),
    (
        "nothing to import; review the import first",
        "Nichts zu importieren; prüfen Sie zuerst den Import",
    ),
    // MARK: - Local AI and Ask Rupert
    (
        "This iPhone can't run Apple Intelligence.",
        "Dieses iPhone kann Apple Intelligence nicht ausführen.",
    ),
    (
        "Apple Intelligence is turned off. Turn it on in Settings ▸ Apple Intelligence & Siri.",
        "Apple Intelligence ist ausgeschaltet. Schalten Sie es unter Einstellungen ▸ Apple Intelligence & Siri ein.",
    ),
    (
        "Apple Intelligence needs iOS 26 or later.",
        "Apple Intelligence erfordert iOS 26 oder neuer.",
    ),
    (
        "This Mac can't run Apple Intelligence.",
        "Dieser Mac kann Apple Intelligence nicht ausführen.",
    ),
    (
        "Apple Intelligence is turned off. Turn it on in System Settings ▸ Apple Intelligence & Siri.",
        "Apple Intelligence ist ausgeschaltet. Schalten Sie es unter Systemeinstellungen ▸ Apple Intelligence & Siri ein.",
    ),
    (
        "Apple Intelligence is still getting ready (downloading its model).",
        "Apple Intelligence wird noch vorbereitet (das Modell wird geladen).",
    ),
    (
        "Apple Intelligence needs macOS 26 or later.",
        "Apple Intelligence erfordert macOS 26 oder neuer.",
    ),
    (
        "Apple Intelligence isn't available here.",
        "Apple Intelligence ist hier nicht verfügbar.",
    ),
    (
        "Apple Intelligence couldn't answer: {0}",
        "Apple Intelligence konnte nicht antworten: {0}",
    ),
    (
        "The document is too long for Apple Intelligence.",
        "Das Dokument ist zu lang für Apple Intelligence.",
    ),
    (
        "Apple Intelligence declined to answer for this document.",
        "Apple Intelligence hat eine Antwort zu diesem Dokument abgelehnt.",
    ),
    (
        "Apple Intelligence doesn't support this document's language.",
        "Apple Intelligence unterstützt die Sprache dieses Dokuments nicht.",
    ),
    (
        "Apple Intelligence is busy. Try again in a moment.",
        "Apple Intelligence ist beschäftigt. Versuchen Sie es gleich noch einmal.",
    ),
    (
        "The Local AI server has no chat model",
        "Der Server für lokale KI hat kein Chat-Modell",
    ),
    (
        "The Local AI server answered oddly: {0}",
        "Der Server für lokale KI hat unerwartet geantwortet: {0}",
    ),
    (
        "The Local AI server isn't reachable ({0})",
        "Der Server für lokale KI ist nicht erreichbar ({0})",
    ),
    (
        "the model didn't answer with JSON",
        "Das Modell hat nicht mit JSON geantwortet",
    ),
    (
        "the model's answer didn't parse: {0}",
        "Die Antwort des Modells war nicht lesbar: {0}",
    ),
    ("assistant: {0}", "Assistent: {0}"),
    (
        "not signed in or no access ({0})",
        "Nicht angemeldet oder kein Zugriff ({0})",
    ),
    (
        "rate limited, try again shortly ({0})",
        "Zu viele Anfragen; versuchen Sie es gleich noch einmal ({0})",
    ),
    (
        "the request ended unexpectedly",
        "Die Anfrage wurde unerwartet beendet",
    ),
    (
        "The model declined to answer that.",
        "Das Modell hat diese Antwort abgelehnt.",
    ),
    (
        "\n\n(The answer was cut off.)",
        "\n\n(Die Antwort wurde abgeschnitten.)",
    ),
    (
        "the model sent no answer",
        "Das Modell hat keine Antwort gesendet",
    ),
    (
        "the answer stopped early ({0})",
        "Die Antwort endete vorzeitig ({0})",
    ),
    (
        "the ChatGPT sign-in has expired; sign in again in Settings ▸ Assistant",
        "Die ChatGPT-Anmeldung ist abgelaufen; melden Sie sich unter Einstellungen ▸ Assistent erneut an",
    ),
    ("Searching documents", "Dokumente werden durchsucht"),
    ("Searching for “{0}”", "Suche nach „{0}“"),
    ("Looking up tags {0}", "Tags {0} werden nachgeschlagen"),
    (
        "Searching for “{0}” tagged {1}",
        "Suche nach „{0}“ mit dem Tag {1}",
    ),
    ("Looking at tags", "Tags werden angesehen"),
    ("Looking at the library", "Die Bibliothek wird angesehen"),
    ("Opening {0}", "{0} wird geöffnet"),
    ("Reading {0}", "{0} wird gelesen"),
    ("Showing folder {0}", "Ordner {0} wird angezeigt"),
    ("Showing {0}", "{0} wird angezeigt"),
    ("Showing results", "Ergebnisse werden angezeigt"),
    (
        "This provider needs no key.",
        "Dieser Anbieter braucht keinen Schlüssel.",
    ),
    (
        "The ant command isn't installed (see Settings ▸ Assistant).",
        "Der Befehl ant ist nicht installiert (siehe Einstellungen ▸ Assistent).",
    ),
    (
        "Not signed in to Anthropic: use Sign in… in Settings ▸ Assistant.",
        "Nicht bei Anthropic angemeldet: Verwenden Sie „Anmelden …“ unter Einstellungen ▸ Assistent.",
    ),
    (
        "The ant command isn't installed.",
        "Der Befehl ant ist nicht installiert.",
    ),
    (
        "Signing in didn't finish.",
        "Die Anmeldung wurde nicht abgeschlossen.",
    ),
    (
        "Sign in with ChatGPT in Settings ▸ Assistant.",
        "Melden Sie sich unter Einstellungen ▸ Assistent mit ChatGPT an.",
    ),
    (
        "Signing in took too long; please try again.",
        "Die Anmeldung hat zu lange gedauert; bitte versuchen Sie es erneut.",
    ),
    ("Signed in", "Angemeldet"),
    (
        "Rupert can now use your ChatGPT plan. You can close this tab.",
        "Rupert kann jetzt Ihr ChatGPT-Abo nutzen. Sie können diesen Tab schließen.",
    ),
    ("Not signed in", "Nicht angemeldet"),
    (
        "Local AI isn't running, or has no model.",
        "Die lokale KI läuft nicht oder hat kein Modell.",
    ),
    (
        "The Local AI server has no model.",
        "Der Server für lokale KI hat kein Modell.",
    ),
    (
        "Add an Anthropic API key in Settings ▸ Assistant.",
        "Fügen Sie unter Einstellungen ▸ Assistent einen API-Schlüssel von Anthropic hinzu.",
    ),
    (
        "Your ChatGPT plan lists no models for other apps.",
        "Ihr ChatGPT-Abo bietet keine Modelle für andere Apps an.",
    ),
    (
        "Choose a model in Settings ▸ Assistant.",
        "Wählen Sie unter Einstellungen ▸ Assistent ein Modell aus.",
    ),
    (
        "Add an API key in Settings ▸ Assistant.",
        "Fügen Sie unter Einstellungen ▸ Assistent einen API-Schlüssel hinzu.",
    ),
    ("ChatGPT plan · {0}", "ChatGPT-Abo · {0}"),
    ("ChatGPT plan", "ChatGPT-Abo"),
    ("Cloud · {0}", "Cloud · {0}"),
    ("Local AI", "Lokale KI"),
    ("Local AI on {0}", "Lokale KI auf {0}"),
    ("AI at {0}", "KI bei {0}"),
    (
        "The model ran out of room before answering (thinking models can). Choose a model that doesn't think, or thinks less.",
        "Dem Modell ging vor der Antwort der Platz aus (das kann bei Modellen passieren, die nachdenken). Wählen Sie ein Modell, das nicht oder weniger nachdenkt.",
    ),
    (
        "Suggestions only use a Local AI server on this computer or your local network.",
        "Vorschläge verwenden nur einen Server für lokale KI auf diesem Computer oder in Ihrem lokalen Netzwerk.",
    ),
    ("Cloud (no model chosen)", "Cloud (kein Modell gewählt)"),
    (
        "I couldn't finish that in a reasonable number of steps.",
        "Das konnte ich nicht in einer vernünftigen Anzahl von Schritten erledigen.",
    ),
    (
        "your ChatGPT plan's limit for Rupert is used up for now; try again later, or raise Rupert's limit in ChatGPT's settings",
        "Das Limit Ihres ChatGPT-Abos für Rupert ist vorerst aufgebraucht; versuchen Sie es später erneut oder erhöhen Sie Ruperts Limit in den ChatGPT-Einstellungen",
    ),
    (
        "ChatGPT plan usage isn't available right now; try again later",
        "Die Nutzung des ChatGPT-Abos ist gerade nicht verfügbar; versuchen Sie es später erneut",
    ),
    (
        "this ChatGPT account or plan can't be used by other apps (signing in again won't change that)",
        "Dieses ChatGPT-Konto oder -Abo kann nicht von anderen Apps genutzt werden (eine erneute Anmeldung ändert daran nichts)",
    ),
    // MARK: - Signing in (ChatGPT, OneDrive, Google Drive)
    (
        "the sign-in answer didn't match; please try again",
        "Die Antwort der Anmeldung passt nicht; bitte versuchen Sie es erneut",
    ),
    (
        "the sign-in page returned no code",
        "Die Anmeldeseite hat keinen Code zurückgegeben",
    ),
    (
        "the sign-in returned another app registration",
        "Die Anmeldung hat die Registrierung einer anderen App zurückgegeben",
    ),
    (
        "registering Rupert with ChatGPT didn't finish",
        "Die Registrierung von Rupert bei ChatGPT wurde nicht abgeschlossen",
    ),
    (
        "the sign-in returned no ID token",
        "Die Anmeldung hat kein ID-Token zurückgegeben",
    ),
    (
        "the sign-in came from an unexpected issuer",
        "Die Anmeldung kam von einem unerwarteten Aussteller",
    ),
    (
        "the sign-in was meant for another app",
        "Die Anmeldung war für eine andere App bestimmt",
    ),
    (
        "the sign-in has expired; please try again",
        "Die Anmeldung ist abgelaufen; bitte versuchen Sie es erneut",
    ),
    (
        "the sign-in named no account",
        "Die Anmeldung hat kein Konto genannt",
    ),
    (
        "the refreshed sign-in belongs to another account",
        "Die erneuerte Anmeldung gehört zu einem anderen Konto",
    ),
    (
        "the sign-in returned no refresh token",
        "Die Anmeldung hat kein Aktualisierungstoken zurückgegeben",
    ),
    ("the ID token is malformed", "Das ID-Token ist fehlerhaft"),
    (
        "listing models failed (HTTP {0})",
        "Die Modelle konnten nicht abgerufen werden (HTTP {0})",
    ),
    (
        "signed out, please sign in again",
        "Abgemeldet; bitte melden Sie sich erneut an",
    ),
    ("OneDrive: {0}", "OneDrive: {0}"),
    (
        "OneDrive isn't set up in this build of Rupert.",
        "OneDrive ist in dieser Version von Rupert nicht eingerichtet.",
    ),
    (
        "Sign in to OneDrive first (Settings).",
        "Melden Sie sich zuerst bei OneDrive an (Einstellungen).",
    ),
    (
        "Microsoft didn't allow staying signed in.",
        "Microsoft hat das Angemeldetbleiben nicht erlaubt.",
    ),
    (
        "Google Drive isn't set up in this build of Rupert.",
        "Google Drive ist in dieser Version von Rupert nicht eingerichtet.",
    ),
    (
        "Sign in to Google Drive first (Settings).",
        "Melden Sie sich zuerst bei Google Drive an (Einstellungen).",
    ),
    (
        "Google didn't allow staying signed in.",
        "Google hat das Angemeldetbleiben nicht erlaubt.",
    ),
    (
        "Google doesn't recognize this copy of Rupert, so it can't sign in. Please tell us at contact@rupertlibrarian.com.",
        "Google erkennt diese Kopie von Rupert nicht, daher ist keine Anmeldung möglich. Bitte melden Sie es an contact@rupertlibrarian.com.",
    ),
    (
        "Couldn't reach Google. Check the internet connection and try again.",
        "Google ist nicht erreichbar. Prüfen Sie die Internetverbindung und versuchen Sie es erneut.",
    ),
    (
        "Google didn't let Rupert sign in: {0} (code {1}).",
        "Google hat die Anmeldung von Rupert abgelehnt: {0} (Code {1}).",
    ),
    (
        "“{0}” is already in the library.",
        "„{0}“ ist bereits in der Bibliothek.",
    ),
    (
        "“{0}” isn't in Google Drive any more.",
        "„{0}“ ist nicht mehr in Google Drive.",
    ),
    // MARK: - iPhone
    (
        "That's the inbox; choose another inbox in Settings instead.",
        "Das ist der Eingang; wählen Sie stattdessen in den Einstellungen einen anderen Eingang.",
    ),
    (
        "Choose the inbox folder first.",
        "Wählen Sie zuerst den Ordner für den Eingang.",
    ),
    // MARK: - Desktop
    ("Not a Markdown file.", "Keine Markdown-Datei."),
    ("Rupert Help", "Rupert-Hilfe"),
    // MARK: - macOS menu
    ("About {0}", "Über {0}"),
    ("Check for Updates…", "Nach Updates suchen …"),
    ("Settings…", "Einstellungen …"),
    ("Services", "Dienste"),
    ("Hide {0}", "{0} ausblenden"),
    ("Hide Others", "Andere ausblenden"),
    ("Show All", "Alle einblenden"),
    ("Quit {0}", "{0} beenden"),
    ("File", "Ablage"),
    ("Scan…", "Scannen …"),
    (
        "Import from ScanSnap Home…",
        "Aus ScanSnap Home importieren …",
    ),
    ("Close Window", "Fenster schließen"),
    ("Edit", "Bearbeiten"),
    ("Undo", "Widerrufen"),
    ("Redo", "Wiederholen"),
    ("Cut", "Ausschneiden"),
    ("Copy", "Kopieren"),
    ("Paste", "Einsetzen"),
    ("Select All", "Alles auswählen"),
    ("View", "Darstellung"),
    ("Enter Full Screen", "Vollbildmodus aktivieren"),
    ("Window", "Fenster"),
    ("Minimize", "Im Dock ablegen"),
    ("Zoom", "Zoomen"),
    ("Help", "Hilfe"),
];
