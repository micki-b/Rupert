<script lang="ts" module>
  // Die Rupert-Hilfe auf Deutsch (förmlich, „Sie“). Gleiche Abschnitte (und
  // ids) wie En.svelte; halten Sie beide und die Texte der App
  // (src/lib/i18n/de.ts) im Einklang.
  export const sections: { id: string; title: string; sub?: boolean }[] = [
    { id: "start", title: "Erste Schritte" },
    { id: "library", title: "Ihre Bibliothek" },
    { id: "finding", title: "Dokumente finden" },
    { id: "details", title: "Titel, Tags und Daten" },
    { id: "tags", title: "Tags verwalten" },
    { id: "inbox", title: "Der Eingang" },
    { id: "scanning", title: "Scannen" },
    { id: "scansnap", title: "Umstieg von ScanSnap Home" },
    { id: "assistant", title: "Rupert fragen" },
    { id: "ai-models", title: "KI-Modelle einrichten" },
    { id: "local-ai", title: "Lokale KI (auf diesem Computer)", sub: true },
    { id: "claude", title: "Claude", sub: true },
    { id: "chatgpt-plan", title: "ChatGPT mit Ihrem Abo", sub: true },
    { id: "openai", title: "OpenAI-kompatible Dienste", sub: true },
    { id: "ai-trouble", title: "Wenn der Assistent nicht funktioniert", sub: true },
    { id: "iphone", title: "Die iPhone-App" },
    { id: "storage", title: "Wo Rupert was speichert" },
    { id: "keys", title: "Tastenkürzel" },
    { id: "about", title: "Über Rupert" },
  ];
</script>

<script lang="ts">
  let { mac, windows, cmd, version, repo: REPO }: { mac: boolean; windows: boolean; cmd: string; version: string; repo: string } =
    $props();
  const fileManager = $derived(mac ? "im Finder" : windows ? "im Explorer" : "in Ihrem Dateimanager");
  const reveal = $derived(mac ? "Im Finder zeigen" : windows ? "Im Explorer zeigen" : "In Dateien zeigen");
  const keychain = $derived(
    mac ? "im macOS-Schlüsselbund" : windows ? "in der Windows-Anmeldeinformationsverwaltung" : "im Schlüsselbund des Systems",
  );
  const deleteKey = $derived(mac ? "⌘⌫" : "Entf");
</script>

<!-- MARK: Erste Schritte -->
<section id="start">
  <h1>Rupert, der Bibliothekar für Ihre Dokumente</h1>
  <p>
    Rupert behält den Überblick über Ihre gescannten Dokumente und andere Dateien in den Ordnern, die Sie schon
    verwenden: auf diesem Computer, in iCloud Drive, OneDrive oder Google Drive. Es wird nichts importiert oder kopiert.
    Ihre Ordner bleiben die Bibliothek, und Rupert ergänzt Suche, Tags, Daten, Favoriten und einen Eingang für neue Scans.
  </p>
  <h3>Die ersten Schritte</h3>
  <ol>
    <li>
      <strong>Fügen Sie einen Ordner hinzu.</strong> Klicken Sie in der Seitenleiste neben <b>Bibliotheksordner</b> auf
      <b>+</b> und wählen Sie einen Ordner, zum Beispiel Ihren Dokumente-Ordner oder einen Ordner in OneDrive oder Google
      Drive. Seine Unterordner gehören dazu. Fügen Sie so viele Ordner hinzu, wie Sie möchten.
    </li>
    <li>
      <strong>Warten Sie den ersten Durchgang ab.</strong> Rupert liest jedes Dokument einmal, damit es seinen Text
      durchsuchen kann. Die Statusleiste unten zeigt den Fortschritt. Sie können währenddessen weiterarbeiten.
    </li>
    <li>
      <strong>Richten Sie den Eingang ein</strong> (optional). Wählen Sie in den <b>Einstellungen</b> den Ordner, in dem
      neue Scans ankommen. Rupert erkennt ihren Text und schlägt vor, wie sie abgelegt werden. Siehe
      <a href="#inbox">Der Eingang</a>.
    </li>
    <li>
      <strong>Richten Sie lokale KI ein</strong> (optional), für Ablagevorschläge und den Assistenten, mit LM Studio,
      Ollama oder einer ähnlichen App. Siehe <a href="#local-ai">Lokale KI</a>.
    </li>
  </ol>
  <div class="note">
    Alles, was Sie {fileManager} oder in Ihrer Synchronisations-App tun, erscheint in Rupert, und was Sie in Rupert tun,
    ist eine gewöhnliche Änderung an den Dateien. Nichts ist in Rupert eingeschlossen.
  </div>
</section>

<!-- MARK: Bibliothek -->
<section id="library">
  <h2>Ihre Bibliothek</h2>
  <p>
    Die Seitenleiste zeigt Ihre <b>Bibliotheksordner</b>, die Ordner, die Sie hinzugefügt haben. Klicken Sie auf einen
    Ordner, um seine Dokumente zu sehen, und auf ein Dokument, um rechts seine Angaben zu sehen. Ein Doppelklick (oder
    <b>Öffnen</b>) öffnet es in seiner gewohnten App; <b>{reveal}</b> zeigt es {fileManager}.
  </p>
  <p>Die beiden Tasten über den Ordnern schalten die Seitenleiste zwischen Ihren <b>Ordnern</b> und Ihren <b>Tags</b> um.</p>
  <p>
    <b>Markdown-Dateien</b> (<code>.md</code>) öffnen sich in Ruperts eigenem Leser, formatiert mit Überschriften,
    Listen, Tabellen und Bildern. Links zu anderen Markdown-Dateien öffnen sich ebenfalls dort (<b>Zurück</b> kehrt
    zurück), Weblinks öffnen sich in Ihrem Browser, und <b>Mit Standard-App öffnen</b> übergibt die Datei Ihrem gewohnten
    Editor. Markdown-Dateien, die nur online verfügbar sind, öffnen sich in Ihrer gewohnten App, die sie herunterlädt.
  </p>
  <h3 id="views">Darstellungen</h3>
  <p>Die drei Tasten rechts neben dem Ordnerpfad ändern, wie Ordner gezeigt werden, für alle Ordner:</p>
  <ul>
    <li><b>Liste</b>: Namen, Arten, Größen und Daten.</li>
    <li>
      <b>Große Symbole</b>: ein Bild der ersten Seite jedes Dokuments (bei Markdown-Dateien der Anfang des Textes). Mit
      den Pfeiltasten wechseln Sie zwischen ihnen.
    </li>
    <li>
      <b>Galerie</b>: das ausgewählte Dokument groß, darunter ein Streifen mit allen Dokumenten. <kbd>←</kbd> und
      <kbd>→</kbd> wechseln zum vorherigen oder nächsten Dokument; <kbd>↑</kbd> und <kbd>↓</kbd> (oder die Pfeiltasten
      auf der Seite) blättern durch seine Seiten.
    </li>
  </ul>
  <p>
    Dateien, die nur online verfügbar sind, zeigen eine Wolke statt eines Bildes, denn das Anzeigen ihrer Seiten würde
    sie herunterladen. Die Bilder liegen in einem Zwischenspeicher auf diesem Computer und werden neu erstellt, sobald
    sich ein Dokument ändert.
  </p>
  <h3 id="moving">Bewegen</h3>
  <p>
    Ziehen Sie ein Dokument oder einen Ordner auf einen Ordner, in der Seitenleiste oder in der Ordneransicht, oder
    klicken Sie mit der rechten Maustaste darauf und wählen Sie <b>Bewegen nach …</b>, um einen Ordner auszuwählen. Es
    wird sofort auf dem Laufwerk bewegt, auch in einen anderen Ihrer Ordner, und die versteckte Begleitdatei eines
    Dokuments wandert mit, sodass Titel, Tags und Notizen bei ihm bleiben. Gibt es dort schon ein Dokument mit diesem
    Namen, hängt Rupert „ (2)“ an, statt etwas zu ersetzen.
  </p>
  <p>
    Beim Bewegen wird nie etwas heruntergeladen. Eine Datei, die nur online verfügbar ist, lässt sich deshalb nur
    innerhalb ihres eigenen Cloud-Ordners bewegen (zum Beispiel von einem OneDrive-Ordner in einen anderen); um sie
    woandershin zu bewegen, öffnen Sie sie einmal, damit sie auf diesem Computer liegt, und bewegen Sie sie dann. Ordner
    lassen sich nur innerhalb desselben Laufwerks bewegen. Der Ordner mit dem <a href="#inbox">Eingang</a> kann erst
    bewegt werden, wenn Sie unter <b>Einstellungen ▸ Eingang</b> einen anderen Eingang wählen.
  </p>
  <h3 id="deleting">Löschen</h3>
  <p>
    Klicken Sie mit der rechten Maustaste auf ein Dokument oder einen Ordner und wählen Sie
    <b>{windows ? "Löschen" : "In den Papierkorb legen"}</b>, oder wählen Sie es aus und drücken Sie
    <kbd>{deleteKey}</kbd>. Es landet im Papierkorb, aus dem Sie es zurückholen können. Die versteckte Begleitdatei eines
    Dokuments geht mit. Dokumente gehen sofort; bei einem Ordner fragt Rupert zuerst, weil alles darin mitgeht.
  </p>
  <p>
    Rupert öffnet Dateien, die nur online verfügbar sind, zum Löschen nicht; das Löschen wird wie jedes andere
    synchronisiert, und iCloud Drive, OneDrive und Google Drive bewahren gelöschte Dateien außerdem eine Weile auf ihren
    Websites auf. Ordner, die Sie als Bibliotheksordner hinzugefügt haben, lassen sich so nicht löschen; verwenden Sie
    <b>Aus Rupert entfernen …</b> im Menü <b>⋯</b> des Ordners (der Ordner bleibt dabei auf dem Laufwerk).
  </p>
  <h3>Optionen für Bibliotheksordner</h3>
  <p>Klicken Sie neben einem Bibliotheksordner auf <b>⋯</b>:</p>
  <ul>
    <li>
      <b>Alle Dateitypen zeigen</b>: Standardmäßig zeigt Rupert nur Dokumente (PDFs, Bilder, Office-Dateien, Text).
      Schalten Sie dies ein, um alle Dateien zu sehen.
    </li>
    <li>
      <b>Nur-online-Dateien zum Indexieren laden</b> (Ordner in iCloud Drive, OneDrive und Google Drive): siehe
      <a href="#online-only">Dateien, die nur online verfügbar sind</a> weiter unten.
    </li>
    <li><b>Umbenennen …</b> ändert den Namen, der in Rupert gezeigt wird, nicht den Namen des Ordners auf dem Laufwerk.</li>
    <li>
      <b>Aus Rupert entfernen …</b> zeigt den Ordner nicht mehr. Der Ordner und seine Dateien bleiben genau dort, wo sie
      sind.
    </li>
  </ul>
  <h3 id="online-only">Dateien, die nur online verfügbar sind</h3>
  <p>
    iCloud Drive, OneDrive und Google Drive können Dateien nur in der Cloud behalten, um Platz zu sparen. Rupert zeigt
    sie mit einem Wolkensymbol <span class="cloud">☁</span> und öffnet sie nie von sich aus, denn Öffnen lädt sie
    herunter. Sie finden sie trotzdem über ihren Namen und die Angaben, die Rupert über sie kennt, und beim Öffnen werden
    sie wie gewohnt heruntergeladen.
  </p>
  <p>
    Unter Windows teilt Google Drive in der Einstellung <b>Dateien streamen</b> anderen Apps nicht mit, welche Dateien
    auf dem Computer liegen, daher behandelt Rupert jede Datei auf dem Laufwerk <b>Google Drive (G:)</b> als nur online.
    Titel, Tags und Text kommen dann von Rupert auf einem anderen Computer (über den gemeinsamen Katalog) oder von
    <b>Nur-online-Dateien zum Indexieren laden</b>. Mit <b>Dateien spiegeln</b> liegt alles auf dem Computer und wird
    normal gelesen.
  </p>
  <p>
    Um den <em>Text</em> von Dokumenten zu durchsuchen, die nur online verfügbar sind, schalten Sie für diesen Ordner
    <b>Nur-online-Dateien zum Indexieren laden</b> ein. Rupert lädt sie dann nach und nach herunter, liest ihren Text und
    gibt den Platz wieder frei. An den Dateien ändert sich nichts. Bei einem großen Ordner kann das Stunden dauern;
    Dateien über 100 MB werden übersprungen. Was Rupert dabei erfährt, wird im Ordner selbst gespeichert, sodass Ihre
    anderen Computer (und die iPhone-App) diese Dokumente durchsuchen können, ohne sie erneut herunterzuladen.
  </p>
</section>

<!-- MARK: Finden -->
<section id="finding">
  <h2>Dokumente finden</h2>
  <h3>Suchen</h3>
  <p>
    Tippen Sie in das Suchfeld oben ({cmd}F). Rupert sucht in Titeln, Dateinamen, Tags, Notizen und im Text der
    Dokumente, in allen Bibliotheksordnern. Jedes eingegebene Wort muss vorkommen; das letzte darf ein Wortanfang sein,
    sodass <kbd>impel</kbd> „Impeller“ findet. Akzente und Umlaute spielen keine Rolle. Mit <kbd>Esc</kbd> löschen Sie die
    Suche.
  </p>
  <p>In den Ergebnissen führt <b>In Rupert zeigen</b> (bei den Angaben) zum Ordner des Dokuments.</p>
  <h3>Tags</h3>
  <p>
    Schalten Sie die Seitenleiste auf <b>Tags</b>. Ohne Eingabe zeigt sie Ihre meistverwendeten Tags; tippen Sie, um die
    Liste einzugrenzen, oder klicken Sie darunter auf <b>Alle … anzeigen</b>. Klicken Sie auf Tags, um mehrere
    auszuwählen, und wählen Sie unter <b>Treffer</b>:
  </p>
  <ul>
    <li><b>Eines</b>: Dokumente mit mindestens einem der ausgewählten Tags.</li>
    <li>
      <b>Alle</b>: Dokumente mit allen ausgewählten Tags. Die Liste bietet dann nur noch Tags an, die dazu passen.
    </li>
  </ul>
  <p>
    Der Tag-Filter lässt sich mit dem Suchtext und den Favoriten kombinieren. Das × eines Tags über der Liste entfernt
    es.
  </p>
  <h3>Favoriten</h3>
  <p>
    Markieren Sie ein Dokument mit dem ☆ in seinen Angaben. Markieren Sie einen Ordner mit dem ☆ neben seinem Pfad über
    der Dokumentliste. <b>Favoriten</b> in der Seitenleiste zeigt beides. Favoriten werden mit den Dateien gespeichert und
    erscheinen deshalb auch auf Ihren anderen Computern und auf dem iPhone.
  </p>
</section>

<!-- MARK: Angaben -->
<section id="details">
  <h2>Titel, Tags und Daten</h2>
  <p>Wählen Sie ein Dokument aus, um rechts seine Angaben zu sehen und zu bearbeiten:</p>
  <ul>
    <li><b>Titel</b>: ein lesbarer Name, unabhängig vom Dateinamen.</li>
    <li>
      <b>Dokumentdatum</b>: das Datum auf dem Papier (Rechnungsdatum, Datum eines Briefs), nicht das Datum des Scans.
    </li>
    <li><b>Tags</b>: so viele Sie möchten. Tippen und mit Return oder einem Komma hinzufügen.</li>
    <li><b>Notizen</b>: alles, was Sie sich dazu merken möchten.</li>
  </ul>
  <p>
    Klicken Sie auf <b>Speichern</b>, um die Änderungen zu behalten, oder auf <b>Zurücksetzen</b>, um sie zu verwerfen.
    Sie werden im PDF selbst gespeichert, an der üblichen Stelle, die auch andere Programme (Acrobat, Vorschau und viele
    mehr) lesen. Dateien, die keine Angaben aufnehmen können, etwa Word-Dokumente oder Fotos, bewahren sie in einer
    kleinen versteckten Datei daneben auf. Die Angaben zeigen, welches von beiden zutrifft.
  </p>
  <p>
    Bei einem PDF, das nur online verfügbar ist, klicken Sie auf <b>Laden und Angaben anzeigen</b>, um zu lesen, was darin
    gespeichert ist. Beim Speichern von Änderungen an einer Datei, die nur online verfügbar ist, wird sie zuerst
    heruntergeladen.
  </p>
</section>

<!-- MARK: Tags -->
<section id="tags">
  <h2>Tags verwalten</h2>
  <p>
    Klicken Sie mit der Seitenleiste auf <b>Tags</b> auf <b>Tags verwalten …</b>. Wählen Sie ein Tag, um es
    <b>umzubenennen</b>, mehrere, um sie zu einem <b>zusammenzuführen</b>, oder verwenden Sie <b>Löschen …</b>, um Tags
    aus allen Dokumenten zu entfernen. Rupert zeigt vorher, wie viele Dokumente sich ändern, und ändert jedes Dokument
    dort, wo seine Angaben gespeichert sind.
  </p>
  <p>
    Dokumente, die nur online verfügbar sind, werden dafür nach Möglichkeit nicht heruntergeladen: Die Änderung wird
    neben ihnen vermerkt und kommt ins PDF, wenn es das nächste Mal gespeichert wird. Würde ein PDF, das nur online
    verfügbar ist, durch eine Änderung ganz ohne Tags bleiben, fragt Rupert vor dem Herunterladen.
  </p>
</section>

<!-- MARK: Eingang -->
<section id="inbox">
  <h2>Der Eingang</h2>
  <p>
    Der Eingang ist ein Ordner, in dem neue Scans ankommen: der Ordner, in den Ihre Scannersoftware speichert, oder der,
    in den Scans aus Rupert und vom iPhone gehen. Er muss in einem Ihrer Bibliotheksordner liegen. Wählen Sie ihn unter
    <b>Einstellungen ▸ Eingang</b>.
  </p>
  <p>Mit jeder neuen Datei macht Rupert Folgendes:</p>
  <ol>
    <li>
      <b>Es erkennt den Text</b>, wenn es ein Scan ohne Text ist, und fügt dem PDF eine unsichtbare Textebene hinzu, damit
      jede PDF-App es durchsuchen kann. Das geschieht auf diesem Computer ({mac
        ? "mit Apples eingebauter Texterkennung"
        : "mit Tesseract, falls installiert"}). Ein- und ausschalten unter <b>Einstellungen ▸ Texterkennung</b>.
    </li>
    <li>
      <b>Es schlägt Titel, Datum, Tags und Ordner vor</b>, mit <a href="#local-ai">lokaler KI</a> (LM Studio, Ollama oder
      einer ähnlichen App), sofern sie läuft. Rupert schlägt Ordner vor, in denen bereits ähnliche Dokumente liegen.
    </li>
    <li>Es wartet auf Sie. Nichts wird bewegt, bevor Sie es sagen.</li>
  </ol>
  <p>
    Klicken Sie in der Seitenleiste auf <b>Eingang</b>, um zu prüfen. Sehen Sie sich den Vorschlag an, ändern Sie, was
    Sie möchten, und klicken Sie auf <b>Ablegen</b> ({cmd}{mac ? "↩" : "Enter"}). Rupert schreibt die Angaben in die
    Datei, benennt sie um und bewegt sie in den Ordner. <b>Neu vorschlagen</b> bittet die lokale KI um einen neuen Vorschlag.
  </p>
  <p>
    Um einen Scan stattdessen loszuwerden, klicken Sie unter dem Formular auf
    <b>{windows ? "Löschen …" : "In den Papierkorb legen …"}</b>, klicken Sie in der Liste mit der rechten Maustaste
    darauf oder drücken Sie <kbd>{deleteKey}</kbd>. Rupert fragt zuerst, weil ein neuer Scan vielleicht noch nirgends sonst
    gespeichert ist; er landet im Papierkorb, aus dem Sie ihn zurückholen können.
  </p>
  <p>
    Der Dateiname folgt dem Muster unter <b>Einstellungen ▸ Dateinamen</b>: <code>{"{date}"}</code> (das Datum) und
    <code>{"{title}"}</code> (der Titel) werden ausgefüllt, sodass das Standardmuster <code>{"{date} {title}"}</code>
    Namen wie „2026-04-29 Verlängerung der Kfz-Versicherung.pdf“ ergibt. Gibt es eine Datei mit diesem Namen schon, hängt
    Rupert „(2)“ an; es überschreibt nie.
  </p>
  {#if !mac}
    <div class="note">
      Die Texterkennung unter Windows und Linux verwendet das kostenlose Programm <b>Tesseract</b>. Wenn die
      Einstellungen melden, dass keine Texterkennung gefunden wurde, installieren Sie Tesseract und starten Sie Rupert neu.
    </div>
  {/if}
</section>

<!-- MARK: Scannen -->
<section id="scanning">
  <h2>Scannen</h2>
  <h3>Netzwerkscanner</h3>
  <p>
    Rupert scannt direkt von Scannern, die AirScan (auch eSCL genannt) unterstützen, wie die meisten aktuellen
    Netzwerkscanner und Multifunktionsdrucker. Wählen Sie {#if mac}<b>Ablage ▸ Scannen …</b> oder {/if}<b>Scannen …</b>
    im Eingang. Rupert findet Scanner in Ihrem Netzwerk, oder Sie fügen einen über seine Adresse hinzu. Wählen Sie Quelle
    (Dokumenteneinzug oder Vorlagenglas), Seiten, Farbe, Auflösung und Seitenformat und klicken Sie auf <b>Scannen</b>.
    Das PDF kommt direkt in den Eingang.
  </p>
  <h3>ScanSnap und andere Scannersoftware</h3>
  <p>
    Jede Scanner-App, die in einen Ordner speichert, funktioniert mit Rupert. Legen Sie für einen ScanSnap ein ScanSnap
    Home-Profil an, das PDFs in Ruperts Eingangsordner speichert. Rupert holt jeden neuen Scan von dort ab.
  </p>
  <h3>Mit dem iPhone</h3>
  <p>Die iPhone-App scannt mit der Kamera in denselben Eingang. Siehe <a href="#iphone">Die iPhone-App</a>.</p>
</section>

<!-- MARK: ScanSnap Home -->
<section id="scansnap">
  <h2>Umstieg von ScanSnap Home</h2>
  <p>
    Wenn Sie Ihre Scans mit ScanSnap Home organisiert haben, kann Rupert übernehmen, was es weiß und nicht in den
    Dateien steht: Dokumentdaten, Favoriten, Angaben zu Belegen und den erkannten Text. Wählen Sie
    <b>Aus ScanSnap Home importieren …</b> unten in der Seitenleiste{mac ? " oder im Menü Ablage" : ""}.
  </p>
  <p>
    Rupert zeigt zuerst, was es gefunden hat und was es ändern wird, einschließlich ScanSnap-Ordnern, die noch nicht in
    Ihrer Bibliothek sind (<b>Als Bibliotheksordner hinzufügen</b>). Nichts, was Sie in Rupert festgelegt haben, wird
    überschrieben, Dateien, die nur online verfügbar sind, werden nicht heruntergeladen, und ein erneuter Import ändert
    nichts.
  </p>
</section>

<!-- MARK: Assistent -->
<section id="assistant">
  <h2>Rupert fragen</h2>
  <p>
    Klicken Sie auf die Chat-Taste unten rechts ({cmd}J), um Rupert in eigenen Worten nach Dokumenten zu fragen. Er sucht,
    liest und zeigt, was er findet, im Hauptfenster. Zum Beispiel:
  </p>
  <ul>
    <li>„Finde die Rechnung für den Impeller vom letzten Frühjahr“</li>
    <li>„Wie hoch ist die Selbstbeteiligung der Bootsversicherung?“</li>
    <li>„Zeig alles mit den Tags Taxes und 2025“</li>
    <li>„Öffne den Ordner Boat“</li>
  </ul>
  <p>
    Die Eule zeigt, was er gerade tut: nachdenken, suchen, ein Dokument lesen oder etwas öffnen. Eine Zeile unter dem
    Gespräch nennt den aktuellen Schritt. Fragen Sie in jeder Sprache: Rupert antwortet in derselben.
  </p>
  <ul>
    <li>
      <b>Stoppen</b> (die Taste ■ oder <kbd>Esc</kbd>) beendet eine Antwort sofort. Ihre Frage kommt zurück ins
      Eingabefeld, damit Sie sie ändern können.
    </li>
    <li><b>Neues Gespräch</b> (der kreisförmige Pfeil) beginnt von vorn.</li>
    <li>Das Zahnrad öffnet die Einstellungen des Assistenten.</li>
  </ul>
  <p>
    Rupert kann Ihre Bibliothek ansehen, <b>ändert aber nie</b> etwas: kein Umbenennen, Bewegen, Taggen oder Löschen. Er
    liest nur den Text, den Rupert bereits kennt, und lädt daher nie Dateien herunter, die nur online verfügbar sind.
  </p>
  <p>
    Standardmäßig läuft er mit lokaler KI auf diesem Computer. Stattdessen können Sie Claude oder einen anderen
    Cloud-Dienst wählen: siehe <a href="#ai-models">KI-Modelle einrichten</a>.
  </p>
  <p class="tip">Wenn Rupert Deutsch spricht, trägt die Eule Lederhosen.</p>
</section>

<!-- MARK: KI-Modelle -->
<section id="ai-models">
  <h2>KI-Modelle einrichten</h2>
  <p>Rupert verwendet KI an zwei Stellen, die getrennt eingerichtet werden:</p>
  <table>
    <thead><tr><th>Funktion</th><th>Wo sie läuft</th><th>Einstellungen</th></tr></thead>
    <tbody>
      <tr>
        <td>Vorschläge im Eingang (Titel, Datum, Tags, Ordner)</td>
        <td>Lokale KI, immer (auf diesem Computer oder in Ihrem lokalen Netzwerk)</td>
        <td><b>Einstellungen ▸ Lokale KI</b> und <b>Vorschläge</b></td>
      </tr>
      <tr>
        <td>Rupert fragen (der Chat)</td>
        <td>Standardmäßig lokale KI, oder ein Cloud-Modell Ihrer Wahl</td>
        <td><b>Einstellungen ▸ Assistent</b></td>
      </tr>
    </tbody>
  </table>
  <p>
    Die Texterkennung verwendet überhaupt keine KI-Dienste: Sie läuft auf diesem Computer. Unter
    <b>Einstellungen ▸ Assistent</b> wählt das Menü <b>Modell</b> eine Art von Verbindung:
  </p>
  <ul>
    <li>
      <a href="#local-ai"><b>Lokale KI</b></a> (LM Studio, Ollama oder eine ähnliche App): privat und kostenlos, braucht
      einen leistungsfähigen Computer.
    </li>
    <li>
      <a href="#claude"><b>Claude (Anthropic, Cloud)</b></a>: mit einem API-Schlüssel oder Ihrem Anthropic-Konto (die
      monatlichen Guthaben eines Max-, Ultra- oder Team-Abos können es bezahlen).
    </li>
    <li>
      <a href="#chatgpt-plan"><b>ChatGPT mit Ihrem Abo (Cloud)</b></a>: Anmeldung mit ChatGPT; kein API-Schlüssel, nutzt
      Ihr ChatGPT-Abo.
    </li>
    <li>
      <a href="#openai"><b>OpenAI-kompatibler Dienst (Cloud)</b></a>: OpenAI oder ein anderer Dienst, der genauso
      funktioniert.
    </li>
  </ul>
  <div class="note cloud-note">
    <b>☁ Zum Datenschutz.</b> Mit einem Cloud-Modell werden Ihre Fragen und das, was der Assistent ansieht (Titel, Tags,
    Ordnernamen und der Text gelesener Dokumente), an diesen Dienst gesendet. Sobald ein Cloud-Modell verwendet wird, zeigt
    der Chat <span class="cloud">☁</span> und den Namen des Modells unter „Rupert fragen“. Vorschläge im Eingang und die
    Texterkennung verwenden nie einen Cloud-Dienst.
  </div>
</section>

<!-- MARK: Lokale KI -->
<section id="local-ai">
  <h3 class="sub-h">Lokale KI (auf diesem Computer)</h3>
  <p>
    Lokale KI führt ein Modell auf Ihrem eigenen Computer aus, sodass nichts ins Internet geht. Rupert funktioniert mit
    jeder App, die einen OpenAI-kompatiblen Server anbietet. Getestet ist er mit
    <a href="https://lmstudio.ai">LM Studio</a> und <a href="https://ollama.com">Ollama</a>; andere, etwa
    <code>llama-server</code> von llama.cpp oder Jan, sollten ebenfalls funktionieren. Lokale KI läuft am besten auf einem
    Mac mit Apple-Chip und viel Arbeitsspeicher oder auf einem PC mit einer aktuellen Grafikkarte.
  </p>
  {#if mac}
    <p>
      <b>Mit Apple Intelligence</b> (ein Mac mit Apple-Chip und macOS 26 oder neuer): Es gibt nichts zu installieren.
      Schalten Sie Apple Intelligence unter <b>Systemeinstellungen ▸ Apple Intelligence &amp; Siri</b> ein und wählen
      Sie in Rupert unter <b>Einstellungen ▸ Lokale KI ▸ Server</b> die Option <b>Apple Intelligence</b> (auf einem solchen Mac beginnt Rupert damit). Es antwortet in
      Sekunden, und nichts verlässt den Mac. Es ist allerdings ein kleines Modell: Ein größeres in LM Studio oder Ollama
      legt besser ab und antwortet besser.
    </p>
  {/if}
  <p><b>Mit LM Studio:</b></p>
  <ol>
    <li>Installieren Sie LM Studio von <a href="https://lmstudio.ai">lmstudio.ai</a> und öffnen Sie es.</li>
    <li>
      Laden Sie über die Suche in LM Studio ein Modell. Wählen Sie für den Assistenten eines, das <b>Werkzeuge</b>
      unterstützt (LM Studio kennzeichnet diese), zum Beispiel ein aktuelles Qwen-Modell. Größere Modelle antworten
      besser, sind aber langsamer.
    </li>
    <li>
      Starten Sie den lokalen Server: Schalten Sie ihn im Bereich <b>Developer</b> von LM Studio ein. Laden Sie dort
      auch Ihr Modell.
    </li>
  </ol>
  <p><b>Mit Ollama:</b></p>
  <ol>
    <li>
      Installieren Sie Ollama von <a href="https://ollama.com">ollama.com</a> und öffnen Sie es. Es läuft im Hintergrund.
    </li>
    <li>
      Laden Sie ein Modell, das <b>Werkzeuge</b> unterstützt (die Modellbibliothek von Ollama kennzeichnet diese), zum
      Beispiel in einem Terminal: <code>ollama pull qwen3:4b</code>.
    </li>
  </ol>
  <p><b>Dann in Rupert:</b></p>
  <ol>
    <li>
      Öffnen Sie <b>Einstellungen ▸ Lokale KI</b>. Wählen Sie unter <b>Server</b> LM Studio oder Ollama; damit wird die
      <b>Adresse</b> ausgefüllt (<code>http://localhost:1234</code> oder <code>http://localhost:11434</code>). Für einen
      anderen Server geben Sie seine Adresse ein. Dort sollte <span class="ok">Verbunden</span> stehen.
    </li>
    <li>
      Wählen Sie unter <b>Modell</b> Ihr Modell. „Automatisch“ nimmt das erste Modell, das der Server auflistet: Bei LM
      Studio ist das das geladene Modell, Ollama listet dagegen alle heruntergeladenen Modelle auf. Wählen Sie dort also
      eines aus.
    </li>
    <li>
      Wählen Sie unter <b>Einstellungen ▸ Assistent</b> die Option <b>Lokale KI</b>. Der Assistent verwendet denselben
      Server und dasselbe Modell wie die Vorschläge im Eingang. Änderungen in den Einstellungen gelten sofort; klicken Sie
      auf <b>Fertig</b>, wenn Sie fertig sind.
    </li>
  </ol>
  <p class="tip">
    Lokale Modelle brauchen Zeit: Eine Frage kann ein bis zwei Minuten dauern, meist zum Nachdenken. Sie können Rupert
    währenddessen weiter verwenden und die Antwort jederzeit abbrechen.
  </p>
  <p class="tip">
    Wenn Antworten abbrechen oder der Assistent den Überblick über Gelesenes verliert, geben Sie dem Modell mehr Platz
    (seine Kontextlänge): in den Einstellungen von Ollama oder beim Laden des Modells in LM Studio.
  </p>
  <p class="tip">
    Der Server kann auch auf einem anderen Computer bei Ihnen zu Hause laufen: Lassen Sie ihn im lokalen Netzwerk
    bereitstellen (die entsprechende Option in LM Studio oder „Expose Ollama to the network“ in Ollama) und geben Sie die
    Adresse dieses Computers ein, zum Beispiel <code>http://192.168.1.20:11434</code>. Tun Sie das nur in einem Netzwerk,
    dem Sie vertrauen; diese Server haben kein Passwort. Die Einstellungen zeigen, wohin der Text von Dokumenten geht. Eine
    Adresse außerhalb Ihres Heimnetzwerks gilt als Cloud-Dienst: „Rupert fragen“ zeigt <span class="cloud">☁</span>, und
    Vorschläge im Eingang verwenden sie nicht.
  </p>
</section>

<!-- MARK: Claude -->
<section id="claude">
  <h3 class="sub-h">Claude</h3>
  <p>
    Claude von Anthropic antwortet schnell und gut. Es ist ein kostenpflichtiger Cloud-Dienst, abgerechnet nach Nutzung
    über ein Entwicklerkonto bei Anthropic (die Claude Console). Dieses Konto ist von einem claude.ai-Abo getrennt:
    Anthropic erlaubt anderen Apps nicht, das Kontingent eines Abos zu nutzen, daher kann Rupert nicht direkt mit Ihrem
    Claude-Abo laufen.
  </p>
  <div class="note" id="claude-credits">
    <b>Haben Sie ein Max-, Ultra- oder Team-Abo?</b> Es enthält monatliche Guthaben für die Claude Console (100 USD im
    Monat mit Max, 200 USD mit Ultra, bis zu 500 USD gemeinsam mit Team), die Rupert verbrauchen kann:
    <ol>
      <li>
        Öffnen Sie auf <a href="https://claude.ai">claude.ai</a> <b>Settings ▸ Billing</b> (Team:
        <b>Organization settings ▸ Billing</b>) und verknüpfen Sie unter <b>API credits</b> eine Organisation der Claude
        Console. Sie können dort eine anlegen. Wählen Sie sorgfältig: Die Verknüpfung lässt sich später nur mit Hilfe des
        Anthropic-Supports ändern.
      </li>
      <li>Melden Sie sich unten mit dieser Organisation bei Claude an (Option 1 oder 2).</li>
    </ol>
    Die Guthaben werden zuerst verbraucht, verfallen am Monatsende und werden nicht übertragen. Sind sie aufgebraucht,
    werden Anfragen bis zum nächsten Monat abgelehnt (sofern Sie nicht zusätzlich Guthaben gekauft haben); Ihrem Abo wird
    nie etwas berechnet. Free- und Pro-Abos enthalten sie nicht. Neue Abonnenten können sie nach 7 Tagen anfordern.
  </div>
  <p>Wählen Sie unter <b>Einstellungen ▸ Assistent</b> <b>Claude (Anthropic, Cloud)</b>. Wählen Sie dann die Anmeldung.</p>

  <h4>Option 1: API-Schlüssel</h4>
  <ol>
    <li>
      Melden Sie sich auf <a href="https://platform.claude.com">platform.claude.com</a> an oder legen Sie ein Konto an und
      laden Sie unter Billing Guthaben auf.
    </li>
    <li>
      Erstellen Sie einen Schlüssel unter
      <a href="https://platform.claude.com/settings/keys">platform.claude.com/settings/keys</a> und kopieren Sie ihn.
    </li>
    <li>
      Wählen Sie in Rupert unter Anmeldung <b>API-Schlüssel</b>, fügen Sie den Schlüssel ein und klicken Sie auf
      <b>Schlüssel speichern</b>.
    </li>
  </ol>
  <p>
    Der Schlüssel wird {keychain} gespeichert, nie in Ruperts Einstellungsdatei. <b>Entfernen</b> löscht ihn dort. Sie
    können den Schlüssel jederzeit in der Claude Console widerrufen.
  </p>

  <h4>Option 2: Ihr Anthropic-Konto, ohne Schlüssel</h4>
  <p>
    Rupert kann Ihr Anthropic-Konto über Anthropics Befehlszeilenwerkzeug <code>ant</code> nutzen, sodass Sie gar keinen
    Schlüssel verwalten.
  </p>
  <ol>
    <li>
      Installieren Sie <code>ant</code>:
      {#if mac}
        Führen Sie im Terminal <code>brew install anthropics/tap/ant</code> aus (dafür ist
        <a href="https://brew.sh">Homebrew</a> nötig).
      {:else if windows}
        Laden Sie es von <a href="https://github.com/anthropics/anthropic-cli/releases">seiner Release-Seite</a> und legen
        Sie <code>ant.exe</code> in einen Ordner in Ihrem PATH.
      {:else}
        Folgen Sie den Schritten im
        <a href="https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart">Schnellstart zur Anthropic-CLI</a>.
      {/if}
    </li>
    <li>
      Wählen Sie in Rupert unter Anmeldung <b>Anthropic-Konto, über den Befehl <code>ant</code></b> und klicken Sie dann
      auf <b>Anmelden …</b>. Ihr Browser öffnet sich; melden Sie sich an und bestätigen Sie. Die Kontozeile zeigt dann
      „Angemeldet“.
    </li>
  </ol>
  <p>
    Die Nutzung wird der Console-Organisation berechnet, die Sie bei der Anmeldung gewählt haben. Die Anmeldung hält
    einige Wochen; läuft sie ab, meldet der Assistent das, und <b>Erneut anmelden …</b> erneuert sie.
  </p>

  <h4>Welches Claude-Modell</h4>
  <p>
    Lassen Sie <b>Claude-Modell</b> leer, um <code>claude-opus-5-5</code> zu verwenden, das leistungsfähigste. Für
    schnellere, günstigere Antworten tragen Sie <code>claude-sonnet-5-5</code> oder <code>claude-haiku-5-5</code> ein.
  </p>
</section>

<!-- MARK: ChatGPT-Abo -->
<section id="chatgpt-plan">
  <h3 class="sub-h">ChatGPT mit Ihrem Abo</h3>
  <p>
    Wenn Sie für ChatGPT bezahlen, kann Rupert Ihr Abo statt eines API-Schlüssels nutzen: Sie melden sich mit Ihrem
    ChatGPT-Konto an, und Ruperts Fragen zählen zu den Limits Ihres Abos. OpenAI nennt das „Sign in with ChatGPT“. Es ist
    eine Vorschau, daher kann OpenAI noch ändern, wie es funktioniert und welche Abos es umfasst.
  </p>
  <ol>
    <li>Wählen Sie unter <b>Einstellungen ▸ Assistent</b> <b>ChatGPT mit Ihrem Abo (Cloud)</b>.</li>
    <li>
      Klicken Sie auf <b>Mit ChatGPT anmelden …</b>. Ihr Browser öffnet die Anmeldeseite von OpenAI; melden Sie sich an
      und erlauben Sie Rupert, Ihr ChatGPT-Abo zu nutzen. Die Seite sagt dann, dass Sie sie schließen können, und die
      Einstellungen zeigen Ihr Konto.
    </li>
    <li>Wählen Sie ein <b>Modell</b>: Die Liste zeigt, was Ihr Abo anderen Apps anbietet.</li>
  </ol>
  <p>
    Rupert speichert die Anmeldung {keychain}. Es sieht Ihre ChatGPT-Unterhaltungen nicht, und nichts, was es sendet,
    wird bei OpenAI für später gespeichert. In den ChatGPT-Einstellungen sehen Sie Rupert unter den Apps, die Ihr Abo
    nutzen, können begrenzen, wie viel es pro Woche verbrauchen darf, oder es entfernen. <b>Abmelden</b> in Rupert
    vergisst die Anmeldung auf diesem Computer.
  </p>
  <p class="tip">
    Ist Ruperts Anteil an Ihrem Abo aufgebraucht, meldet der Assistent das; er funktioniert wieder, sobald Ihr Limit
    zurückgesetzt wird. Über Ihr Abo hinaus wird nichts berechnet.
  </p>
</section>

<!-- MARK: OpenAI-kompatibel -->
<section id="openai">
  <h3 class="sub-h">OpenAI-kompatible Dienste</h3>
  <p>
    Viele KI-Dienste nehmen Anfragen so entgegen wie OpenAI: OpenAI selbst und andere wie OpenRouter, Groq oder Mistral.
    Wählen Sie unter <b>Einstellungen ▸ Assistent</b> <b>OpenAI-kompatibler Dienst (Cloud)</b> und füllen Sie aus:
  </p>
  <dl>
    <dt>Dienst</dt>
    <dd>
      Die API-Adresse des Dienstes. Für OpenAI leer lassen (<code>https://api.openai.com/v1</code>). Andere
      veröffentlichen ihre eigene, zum Beispiel <code>https://openrouter.ai/api/v1</code>,
      <code>https://api.groq.com/openai/v1</code> oder <code>https://api.mistral.ai/v1</code>.
    </dd>
    <dt>Modell</dt>
    <dd>
      Der Name des Modells genau so, wie der Dienst ihn nennt. Wählen Sie eines, das Werkzeuge unterstützt (auch „function
      calling“ genannt).
    </dd>
    <dt>API-Schlüssel</dt>
    <dd>
      Erstellen Sie einen auf der Website des Dienstes (für OpenAI:
      <a href="https://platform.openai.com/api-keys">platform.openai.com/api-keys</a>), fügen Sie ihn ein und klicken Sie
      auf <b>Schlüssel speichern</b>. Er wird {keychain} gespeichert.
    </dd>
  </dl>
  <p>Öffnen Sie den Chat: Seine Kopfzeile zeigt das Modell mit <span class="cloud">☁</span>.</p>
</section>

<!-- MARK: Probleme -->
<section id="ai-trouble">
  <h3 class="sub-h">Wenn der Assistent nicht funktioniert</h3>
  <table>
    <thead><tr><th>Meldung oder Anzeichen</th><th>Was zu tun ist</th></tr></thead>
    <tbody>
      <tr>
        <td>„Die lokale KI läuft nicht oder hat kein Modell.“</td>
        <td>
          Öffnen Sie LM Studio oder Ollama, starten Sie den Server und sorgen Sie dafür, dass er ein Modell hat. Unter
          Einstellungen ▸ Lokale KI sollte Verbunden stehen.
        </td>
      </tr>
      <tr>
        <td>„Dem Modell ging vor der Antwort der Platz aus …“</td>
        <td>
          Ein Modell, das nachdenkt, hat seinen Platz mit Nachdenken verbraucht. Wählen Sie ein Modell, das nicht oder
          weniger nachdenkt.
        </td>
      </tr>
      <tr>
        <td>„Fügen Sie … einen API-Schlüssel von Anthropic hinzu“ oder „… einen API-Schlüssel hinzu“</td>
        <td>Fügen Sie unter Einstellungen ▸ Assistent einen Schlüssel ein und klicken Sie auf Schlüssel speichern.</td>
      </tr>
      <tr>
        <td>„Der Befehl ant ist nicht installiert.“</td>
        <td>
          Installieren Sie ihn (siehe <a href="#claude">Claude</a>) und öffnen Sie die Einstellungen erneut. Rupert sucht
          ihn in Ihrem PATH und in den üblichen Installationsordnern.
        </td>
      </tr>
      <tr>
        <td>„Nicht bei Anthropic angemeldet“</td>
        <td>Klicken Sie unter Einstellungen ▸ Assistent auf Anmelden ….</td>
      </tr>
      <tr>
        <td>„Das Limit Ihres ChatGPT-Abos für Rupert ist vorerst aufgebraucht“</td>
        <td>Warten Sie, bis das Limit zurückgesetzt wird, oder erhöhen Sie Ruperts Limit in den ChatGPT-Einstellungen.</td>
      </tr>
      <tr>
        <td>„Dieses ChatGPT-Konto oder -Abo kann nicht von anderen Apps genutzt werden“</td>
        <td>Ihr Abo oder Arbeitsbereich erlaubt es nicht. Wählen Sie unter Einstellungen ▸ Assistent ein anderes Modell.</td>
      </tr>
      <tr>
        <td>„Die ChatGPT-Anmeldung ist abgelaufen“</td>
        <td>Klicken Sie unter Einstellungen ▸ Assistent auf Erneut anmelden ….</td>
      </tr>
      <tr>
        <td>„Nicht angemeldet oder kein Zugriff“</td>
        <td>
          Der Schlüssel wurde widerrufen oder falsch eingegeben, oder die Anmeldung ist abgelaufen. Speichern Sie einen neuen
          Schlüssel oder melden Sie sich erneut an.
        </td>
      </tr>
      <tr>
        <td>Eine Meldung zu Ihrem Guthaben</td>
        <td>
          Laden Sie auf der Website des Dienstes Guthaben auf. Prüfen Sie bei einem Claude-Abo Max, Ultra oder Team, ob
          seine monatlichen Guthaben mit der Organisation verknüpft sind, mit der Sie sich angemeldet haben (siehe
          <a href="#claude-credits">Claude</a>).
        </td>
      </tr>
      <tr>
        <td>„Zu viele Anfragen; versuchen Sie es gleich noch einmal“</td>
        <td>Der Dienst ist ausgelastet oder Sie haben sein Limit erreicht. Warten Sie eine Minute und fragen Sie erneut.</td>
      </tr>
      <tr>
        <td>Antworten ohne Nachschlagen, oder „Das konnte ich nicht … erledigen“</td>
        <td>
          Das Modell kommt vielleicht nicht gut mit Werkzeugen zurecht. Versuchen Sie ein größeres Modell oder eines, das für
          Werkzeuge gekennzeichnet ist.
        </td>
      </tr>
      <tr>
        <td>Sehr langsame Antworten</td>
        <td>Bei großen lokalen Modellen normal. Wählen Sie unter Einstellungen ▸ Lokale KI ein kleineres Modell oder ein Cloud-Modell.</td>
      </tr>
    </tbody>
  </table>
</section>

<!-- MARK: iPhone -->
<section id="iphone">
  <h2>Die iPhone-App</h2>
  <p>Rupert auf dem iPhone zeigt dieselbe Bibliothek, damit Sie auch unterwegs etwas nachsehen können.</p>
  <ul>
    <li>
      <b>Fügen Sie Ordner</b> aus iCloud Drive oder „Auf meinem iPhone“ mit der Auswahl von Dateien hinzu: dieselben Ordner,
      die Rupert auf dem Computer verwendet. Für <b>OneDrive</b> melden Sie sich stattdessen in der App mit Ihrem
      Microsoft-Konto an (die Auswahl von Dateien kann OneDrive-Ordner nicht mit anderen Apps teilen). <b>Google Drive</b>
      funktioniert genauso (als Vorschau): Auch die Google Drive-App teilt ihre Ordner nicht mit anderen Apps, daher melden
      Sie sich unter <b>Einstellungen ▸ Google Drive</b> mit Ihrem Google-Konto an. Google hat Rupert nicht geprüft, daher
      warnt die Anmeldeseite, dass die App nicht überprüft ist; tippen Sie auf <b>Erweitert</b> und dann auf
      <b>Weiter zu Rupert</b>.
    </li>
    <li>
      <b>Suchen und stöbern</b> Sie in Titeln, Tags und Text. Der Text stammt aus dem, was Rupert auf Ihrem Computer gelesen
      hat; lassen Sie den ersten Durchgang dort also zu Ende laufen.
    </li>
    <li><b>Öffnen</b> Sie ein Dokument, um es zu laden und anzusehen, und teilen oder drucken Sie es dann.</li>
    <li>
      <b>Scannen</b> Sie mit der Kamera. Farbe, Graustufen oder Schwarzweiß und die Auflösung wählen Sie im Bereich
      Scannen. Scans werden im Eingangsordner Ihrer Wahl gespeichert, der der Eingang des Computers sein sollte, damit
      Rupert dort sie erkennen und ablegen kann. Der <b>Eingang</b> steht immer oben in der Bibliothek, damit Sie Ihre
      Scans sehen, bis sie abgelegt sind, auch wenn der Eingangsordner keiner Ihrer Bibliotheksordner ist.
    </li>
    <li><b>Markieren Sie Ordner mit dem Stern</b>, um sie auf allen Geräten zu den Favoriten hinzuzufügen.</li>
    <li>
      <b>Sprache</b>: Die App verwendet die Sprache des iPhone oder die unter <b>Einstellungen ▸ Sprache</b> gewählte
      (oder in den iOS-Einstellungen für Rupert).
    </li>
  </ul>
  <p>Version und Build-Nummer stehen unten in ihren <b>Einstellungen</b> unter <b>Über</b>.</p>
  <p>
    Die iPhone-App ändert Ihre Dokumente nicht: Nur Rupert auf dem Computer bearbeitet Angaben und legt Scans ab.
  </p>
</section>

<!-- MARK: Speicherorte -->
<section id="storage">
  <h2>Wo Rupert was speichert</h2>
  <ul>
    <li>
      <b>Ihre Dokumente</b> bleiben in Ihren Ordnern. Rupert benennt eine Datei nur um oder bewegt sie, wenn Sie es
      möchten: wenn Sie sie aus dem Eingang ablegen oder in einen anderen Ordner bewegen.
    </li>
    <li>
      <b>Titel, Tags, Daten, Notizen und Favoriten</b> werden in PDFs gespeichert oder in einem versteckten Ordner
      <code>.rupert</code> neben Dateien, die sie nicht aufnehmen können. Sie wandern mit den Dateien auf Ihre anderen
      Computer.
    </li>
    <li>
      <b>Der Suchindex</b> ist ein Zwischenspeicher auf diesem Computer. Rupert baut ihn bei Bedarf aus Ihren Dateien neu
      auf, daher geht nichts verloren, wenn er gelöscht wird.
    </li>
    <li>
      <b>Der gemeinsame Katalog</b>: In Ordnern in iCloud Drive, OneDrive und Google Drive enthält ein versteckter Ordner
      <code>.rupert/catalog</code> den Text, den Rupert gelesen hat, damit Ihre anderen Geräte Dokumente durchsuchen können,
      die nur online verfügbar sind.
    </li>
    <li><b>API-Schlüssel</b> werden {keychain} gespeichert.</li>
    <li>
      <b>Die Sprache</b> ändert nichts auf dem Laufwerk: Ordner und Dateien, die Rupert anlegt (wie <code>.rupert</code>),
      heißen in jeder Sprache gleich, weil Geräte mit unterschiedlichen Sprachen sie gemeinsam nutzen.
    </li>
  </ul>
</section>

<!-- MARK: Tastenkürzel -->
<section id="keys">
  <h2>Tastenkürzel</h2>
  <table class="keys">
    <tbody>
      <tr><td><kbd>{cmd}F</kbd></td><td>Suchen</td></tr>
      <tr><td><kbd>{deleteKey}</kbd></td><td>Das ausgewählte Dokument oder den Ordner in den Papierkorb legen</td></tr>
      <tr><td><kbd>←</kbd> <kbd>→</kbd></td><td>Große Symbole und Galerie: vorheriges oder nächstes Dokument</td></tr>
      <tr><td><kbd>↑</kbd> <kbd>↓</kbd></td><td>Galerie: vorherige oder nächste Seite</td></tr>
      <tr><td><kbd>Esc</kbd></td><td>Suche löschen; im Chat die Antwort stoppen oder den Bereich schließen</td></tr>
      <tr><td><kbd>{cmd}J</kbd></td><td>Rupert fragen öffnen oder schließen</td></tr>
      <tr><td><kbd>Return</kbd></td><td>Im Chat: senden (<kbd>Umschalt+Return</kbd> beginnt eine neue Zeile)</td></tr>
      <tr><td><kbd>{cmd}{mac ? "↩" : "Enter"}</kbd></td><td>Im Eingang: das Dokument ablegen</td></tr>
      {#if mac}<tr><td><kbd>⌘,</kbd></td><td>Einstellungen</td></tr>{/if}
      <tr><td><kbd>F1</kbd>{#if mac} oder <kbd>⇧⌘/</kbd>{/if}</td><td>Diese Hilfe</td></tr>
    </tbody>
  </table>
</section>
<!-- MARK: Über -->
<section id="about">
  <h2>Über Rupert</h2>
  <p>
    Rupert{version ? ` ${version}` : ""}, der Bibliothekar für Ihre gescannten Dokumente. Frei und quelloffen unter der
    <a href="{REPO}/blob/main/LICENSE">MIT-Lizenz</a>, © 2026 Micki Braun.
  </p>
  <ul>
    <li>
      <b>Sprache:</b> Rupert verwendet die Sprache Ihres Computers oder die unter <b>Einstellungen ▸ Sprache</b> gewählte.
      Er spricht Englisch, rioplatensisches Spanisch und Deutsch.
    </li>
    <li>
      <b>Updates:</b> Rupert sucht einmal am Tag nach einer neuen Version (abschaltbar unter
      <b>Einstellungen ▸ Updates</b>). Gibt es eine, erscheint <b>Auf … aktualisieren</b> in der Statusleiste; ein Klick
      installiert sie, und Rupert startet neu.{#if mac}
        Oder wählen Sie <b>Rupert ▸ Nach Updates suchen …</b>.{/if}
    </li>
    <li>
      <b>Hilfe anfragen:</b> Nennen Sie die Version und Edition, die unten links im Fenster stehen (zum Beispiel „Rupert
      0.3.0 · GitHub“). <b>GitHub</b> heißt von GitHub installiert (aktualisiert sich selbst); <b>App Store</b> heißt die
      Edition aus dem Mac App Store (der App Store aktualisiert sie).
    </li>
    <li><a href={REPO}>Quellcode, Versionen und Problemmeldungen</a> auf GitHub</li>
    <li>
      <a href="{REPO}/blob/main/PRIVACY.md">Datenschutzerklärung</a> (auf Englisch): Rupert sammelt nichts über Sie;
      optionale Cloud-Funktionen sind dort mit genau dem aufgeführt, was sie senden.
    </li>
    <li>
      <a href="{REPO}/blob/main/THIRD_PARTY_NOTICES.md">Hinweise zu Drittanbietern</a> (auf Englisch): die
      Open-Source-Komponenten, aus denen Rupert gebaut ist, und ihre Lizenzen (auch der App beigelegt).
    </li>
  </ul>
</section>
