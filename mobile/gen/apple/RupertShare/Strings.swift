// The share sheet's text, in Rupert's language (the app's setting, else the
// phone's). Spanish is Rioplatense ("vos"), German formal ("Sie").

import Foundation

struct Strings {
  let saving: String
  let savedOne: (_ name: String, _ inbox: String) -> String
  let savedMany: (_ count: Int, _ inbox: String) -> String
  /// Some or all couldn't be saved now; the app sends them later.
  let kept: (_ why: String) -> String
  let noInbox: String
  let notDocuments: String
  let nothing: String
  let inboxUnreachable: String
  let signIn: (_ service: String) -> String
  let notSetUp: (_ service: String) -> String
  let done: String

  static func forLanguage(_ code: String?) -> Strings {
    let preferred = code ?? Locale.preferredLanguages.first.map { String($0.prefix(2)) } ?? "en"
    switch preferred {
    case "es": return spanish
    case "de": return german
    default: return english
    }
  }

  static let english = Strings(
    saving: "Saving to Rupert's inbox…",
    savedOne: { name, inbox in "Saved “\(name)” in \(inbox)." },
    savedMany: { count, inbox in "Saved \(count) documents in \(inbox)." },
    kept: { why in
      "Couldn't save it now: \(why) Rupert will send it to the inbox the next time you open the app."
    },
    noInbox:
      "Rupert has no inbox yet. Choose one in Rupert (Settings ▸ Inbox for scans); Rupert sends this there then.",
    notDocuments:
      "Rupert's inbox takes documents only: PDFs, pictures, Office files, text and emails.",
    nothing: "There's nothing here Rupert can save.",
    inboxUnreachable: "Rupert can't reach the inbox folder. Choose it again in Rupert (Settings).",
    signIn: { service in "Sign in to \(service) again in Rupert (Settings)." },
    notSetUp: { service in "\(service) isn't set up in this build of Rupert." },
    done: "Done")

  static let spanish = Strings(
    saving: "Guardando en la bandeja de entrada de Rupert…",
    savedOne: { name, inbox in "Se guardó «\(name)» en \(inbox)." },
    savedMany: { count, inbox in "Se guardaron \(count) documentos en \(inbox)." },
    kept: { why in
      "No se pudo guardar ahora: \(why) Rupert lo va a enviar a la bandeja de entrada la próxima vez que abras la app."
    },
    noInbox:
      "Rupert todavía no tiene bandeja de entrada. Elegila en Rupert (Configuración ▸ Bandeja de entrada para escaneos); Rupert va a enviar esto ahí.",
    notDocuments:
      "La bandeja de entrada de Rupert acepta solo documentos: PDF, imágenes, archivos de Office, texto y correos.",
    nothing: "No hay nada que Rupert pueda guardar.",
    inboxUnreachable:
      "Rupert no puede llegar a la carpeta de la bandeja de entrada. Elegila de nuevo en Rupert (Configuración).",
    signIn: { service in "Volvé a iniciar sesión en \(service) en Rupert (Configuración)." },
    notSetUp: { service in "\(service) no está configurado en esta versión de Rupert." },
    done: "Listo")

  static let german = Strings(
    saving: "Wird im Eingang von Rupert gespeichert …",
    savedOne: { name, inbox in "„\(name)“ wurde in \(inbox) gespeichert." },
    savedMany: { count, inbox in "\(count) Dokumente wurden in \(inbox) gespeichert." },
    kept: { why in
      "Jetzt nicht gespeichert: \(why) Rupert sendet es an den Eingang, sobald Sie die App das nächste Mal öffnen."
    },
    noInbox:
      "Rupert hat noch keinen Eingang. Wählen Sie ihn in Rupert (Einstellungen ▸ Eingang für Scans); Rupert sendet dies dann dorthin.",
    notDocuments:
      "Der Eingang von Rupert nimmt nur Dokumente an: PDFs, Bilder, Office-Dateien, Text und E-Mails.",
    nothing: "Hier ist nichts, was Rupert speichern kann.",
    inboxUnreachable:
      "Rupert erreicht den Ordner des Eingangs nicht. Wählen Sie ihn in Rupert erneut (Einstellungen).",
    signIn: { service in "Melden Sie sich in Rupert erneut bei \(service) an (Einstellungen)." },
    notSetUp: { service in "\(service) ist in dieser Version von Rupert nicht eingerichtet." },
    done: "Fertig")

  /// The words for why a document couldn't be saved.
  func reason(_ error: Error) -> String {
    switch error {
    case ShareError.noAppGroup, ShareError.inboxUnreachable: return inboxUnreachable
    case ShareError.noInbox: return noInbox
    case ShareError.signIn(let service): return signIn(service)
    case ShareError.notSetUp(let service): return notSetUp(service)
    case ShareError.service(let message): return message.hasSuffix(".") ? message : message + "."
    default:
      let message = error.localizedDescription
      return message.hasSuffix(".") ? message : message + "."
    }
  }
}
