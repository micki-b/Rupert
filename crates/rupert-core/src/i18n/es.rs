//! Spanish (Rioplatense: "vos", as in Argentina) messages: English → Spanish.
//! Words shared with the screens' catalog (`src/lib/i18n/es.ts`): carpeta,
//! biblioteca, bandeja de entrada, etiqueta, solo en línea, iniciar sesión,
//! Configuración ▸ Asistente.

pub(super) static MESSAGES: &[(&str, &str)] = &[
    // MARK: - Errors (error.rs)
    (
        "couldn't read or write settings: {0}",
        "no se pudo leer ni guardar la configuración: {0}",
    ),
    (
        "file watching failed: {0}",
        "falló la vigilancia de archivos: {0}",
    ),
    ("not a folder: {0}", "no es una carpeta: {0}"),
    (
        "this folder is already a source",
        "esta carpeta ya está en la biblioteca",
    ),
    (
        "this folder overlaps the source \"{0}\"; sources can't be nested",
        "esta carpeta se superpone con «{0}»; las carpetas de la biblioteca no pueden estar una dentro de otra",
    ),
    ("unknown source", "carpeta de la biblioteca desconocida"),
    ("invalid path: {0}", "ruta no válida: {0}"),
    ("search index: {0}", "índice de búsqueda: {0}"),
    ("PDF problem: {0}", "problema con el PDF: {0}"),
    ("stopped", "detenido"),
    // MARK: - Documents and their details
    (
        "the PDF is encrypted, so details are kept in a sidecar file",
        "el PDF está cifrado, así que los detalles se guardan en un archivo aparte",
    ),
    (
        "the PDF has signature permissions, so details are kept in a sidecar file",
        "el PDF tiene permisos de firma, así que los detalles se guardan en un archivo aparte",
    ),
    (
        "the PDF is digitally signed, so details are kept in a sidecar file",
        "el PDF tiene firma digital, así que los detalles se guardan en un archivo aparte",
    ),
    (
        "the PDF is encrypted, so Rupert won't add a text layer to it",
        "el PDF está cifrado, así que Rupert no le agrega una capa de texto",
    ),
    (
        "the PDF has signature permissions, so Rupert won't add a text layer to it",
        "el PDF tiene permisos de firma, así que Rupert no le agrega una capa de texto",
    ),
    (
        "the PDF is digitally signed, so Rupert won't add a text layer to it",
        "el PDF tiene firma digital, así que Rupert no le agrega una capa de texto",
    ),
    (
        "the file changed while saving; please try again",
        "el archivo cambió mientras se guardaba; probá de nuevo",
    ),
    (
        "the file changed while recognizing its text; please try again",
        "el archivo cambió mientras se reconocía su texto; probá de nuevo",
    ),
    (
        "Some details are kept beside the file until it's next saved in Rupert",
        "Algunos detalles se guardan junto al archivo hasta la próxima vez que se guarde en Rupert",
    ),
    (
        "couldn't read the PDF ({0}); details are kept in a sidecar",
        "no se pudo leer el PDF ({0}); los detalles se guardan en un archivo aparte",
    ),
    (
        "document date must be YYYY-MM-DD, got \"{0}\"",
        "la fecha del documento tiene que ser AAAA-MM-DD, no «{0}»",
    ),
    ("choose at least one tag", "elegí al menos una etiqueta"),
    (
        "the new tag name is empty",
        "falta el nombre nuevo de la etiqueta",
    ),
    // MARK: - Moving and deleting
    ("the file name is empty", "falta el nombre del archivo"),
    (
        "its details file couldn't be moved ({0}), so it was left where it was",
        "no se pudo mover su archivo de detalles ({0}), así que quedó donde estaba",
    ),
    (
        "its details file couldn't be moved ({0}), and moving the document back failed too ({1}): it is at {2}, its details file still in {3}",
        "no se pudo mover su archivo de detalles ({0}), y tampoco se pudo devolver el documento ({1}): está en {2}, y su archivo de detalles sigue en {3}",
    ),
    ("copying the file failed", "falló la copia del archivo"),
    (
        "it's online-only, and moving it out of its cloud folder would download it. Download it first (open it once), then move it",
        "está solo en línea, y sacarlo de su carpeta en la nube lo descargaría. Descargalo primero (abrilo una vez) y después movelo",
    ),
    (
        "a source itself can't be moved",
        "una carpeta de la biblioteca no se puede mover",
    ),
    (
        "“{0}” can't be moved into itself",
        "«{0}» no se puede mover dentro de sí misma",
    ),
    (
        "it's a hidden or system file",
        "es un archivo oculto o del sistema",
    ),
    (
        "it isn't a kind of document Rupert shows in this source. To add it, turn on \
             “Show all file types” for the source first",
        "no es un tipo de documento que Rupert muestre en esta fuente. Para agregarlo, activá \
             primero «Mostrar todos los tipos de archivo» en la fuente",
    ),
    ("Can't move “{0}”: {1}.", "No se puede mover «{0}»: {1}."),
    (
        "“{0}” is a folder on another disk. Folders can be moved only within the same disk; move it in the Finder or File Explorer instead",
        "«{0}» es una carpeta en otro disco. Las carpetas solo se pueden mover dentro del mismo disco; movela con el Finder o el Explorador de archivos",
    ),
    (
        "couldn't move it to the trash: {0}",
        "no se pudo mandar a la papelera: {0}",
    ),
    (
        "a source itself can't be moved to the trash",
        "una carpeta de la biblioteca no se puede mandar a la papelera",
    ),
    // MARK: - Folders
    (
        "Rupert can't open “{0}”: {1}",
        "Rupert no puede abrir «{0}»: {1}",
    ),
    (
        "folder bookmarks exist only on macOS",
        "los marcadores de carpetas solo existen en macOS",
    ),
    (
        "couldn't remember access to the folder",
        "no se pudo recordar el acceso a la carpeta",
    ),
    (
        "the folder's bookmark is damaged",
        "el marcador de la carpeta está dañado",
    ),
    ("the folder can't be found", "no se encuentra la carpeta"),
    (
        "the folder's bookmark has no path",
        "el marcador de la carpeta no tiene ruta",
    ),
    // MARK: - Scanning and text recognition
    ("scanner: {0}", "escáner: {0}"),
    (
        "{0} isn't reachable ({1})",
        "no se puede conectar con {0} ({1})",
    ),
    (
        "{0} answered {1} (is it an eSCL scanner?)",
        "{0} respondió {1} (¿es un escáner eSCL?)",
    ),
    ("bad capabilities: {0}", "funciones mal informadas: {0}"),
    (
        "the scanner reported no flatbed or feeder",
        "el escáner no informó ni cama plana ni alimentador",
    ),
    (
        "this scanner doesn't have that input",
        "este escáner no tiene esa entrada",
    ),
    (
        "couldn't start the scan ({0})",
        "no se pudo empezar a escanear ({0})",
    ),
    (
        "the scanner is busy, or the feeder is empty",
        "el escáner está ocupado, o el alimentador está vacío",
    ),
    (
        "the scanner refused the scan ({0})",
        "el escáner rechazó el escaneo ({0})",
    ),
    (
        "the scanner didn't say where the scan is",
        "el escáner no dijo dónde está el escaneo",
    ),
    (
        "lost the scanner ({0})",
        "se perdió la conexión con el escáner ({0})",
    ),
    ("the scan failed ({0})", "falló el escaneo ({0})"),
    (
        "no pages were scanned (is the feeder empty?)",
        "no se escaneó ninguna página (¿está vacío el alimentador?)",
    ),
    ("the scan has no pages", "el escaneo no tiene páginas"),
    (
        "a scanned page is unreadable: {0}",
        "no se puede leer una página escaneada: {0}",
    ),
    ("OCR failed: {0}", "falló el reconocimiento de texto: {0}"),
    ("tesseract failed", "falló tesseract"),
    (
        "no ScanSnap Home library in {0} ({1} is missing)",
        "no hay una biblioteca de ScanSnap Home en {0} (falta {1})",
    ),
    (
        "this doesn't look like a ScanSnap Home library this version of Rupert can read ({0})",
        "no parece una biblioteca de ScanSnap Home que esta versión de Rupert pueda leer ({0})",
    ),
    ("{0} is missing", "falta {0}"),
    // MARK: - Inbox
    (
        "Online-only: open it once to download it, then Rupert will read it.",
        "Solo en línea: abrilo una vez para descargarlo y Rupert lo va a leer.",
    ),
    (
        "Text recognized on {0} of {1} pages ({2})",
        "Texto reconocido en {0} de {1} páginas ({2})",
    ),
    (
        "The text couldn't be added to the PDF itself ({0}); Rupert keeps it beside the file, so it can still be searched.",
        "El texto no se pudo agregar al PDF ({0}); Rupert lo guarda junto al archivo, así que igual se puede buscar.",
    ),
    (
        "Text recognition worked, but the text couldn't be saved: {0}; {1}",
        "El reconocimiento de texto funcionó, pero no se pudo guardar el texto: {0}; {1}",
    ),
    (
        "Text recognition failed: {0}",
        "Falló el reconocimiento de texto: {0}",
    ),
    (
        "Couldn't save the recognized text: {0}",
        "No se pudo guardar el texto reconocido: {0}",
    ),
    ("Text recognized ({0})", "Texto reconocido ({0})"),
    (
        "No text to suggest from.",
        "No hay texto para hacer sugerencias.",
    ),
    (
        "set up the inbox first: scans are saved there",
        "primero configurá la bandeja de entrada: ahí se guardan los escaneos",
    ),
    (
        "nothing to import; review the import first",
        "no hay nada para importar; revisá la importación primero",
    ),
    // MARK: - Local AI and Ask Rupert
    (
        "This iPhone can't run Apple Intelligence.",
        "Este iPhone no puede usar Apple Intelligence.",
    ),
    (
        "Apple Intelligence is turned off. Turn it on in Settings ▸ Apple Intelligence & Siri.",
        "Apple Intelligence está desactivado. Activalo en Configuración ▸ Apple Intelligence y Siri.",
    ),
    (
        "Apple Intelligence needs iOS 26 or later.",
        "Apple Intelligence necesita iOS 26 o posterior.",
    ),
    (
        "This Mac can't run Apple Intelligence.",
        "Esta Mac no puede usar Apple Intelligence.",
    ),
    (
        "Apple Intelligence is turned off. Turn it on in System Settings ▸ Apple Intelligence & Siri.",
        "Apple Intelligence está desactivado. Activalo en Configuración del Sistema ▸ Apple Intelligence y Siri.",
    ),
    (
        "Apple Intelligence is still getting ready (downloading its model).",
        "Apple Intelligence todavía se está preparando (está descargando su modelo).",
    ),
    (
        "Apple Intelligence needs macOS 26 or later.",
        "Apple Intelligence necesita macOS 26 o posterior.",
    ),
    (
        "Apple Intelligence isn't available here.",
        "Apple Intelligence no está disponible acá.",
    ),
    (
        "Apple Intelligence couldn't answer: {0}",
        "Apple Intelligence no pudo responder: {0}",
    ),
    (
        "The document is too long for Apple Intelligence.",
        "El documento es demasiado largo para Apple Intelligence.",
    ),
    (
        "Apple Intelligence declined to answer for this document.",
        "Apple Intelligence no quiso responder sobre este documento.",
    ),
    (
        "Apple Intelligence doesn't support this document's language.",
        "Apple Intelligence no admite el idioma de este documento.",
    ),
    (
        "Apple Intelligence is busy. Try again in a moment.",
        "Apple Intelligence está ocupado. Probá de nuevo en un momento.",
    ),
    (
        "The Local AI server has no chat model",
        "El servidor de IA local no tiene un modelo de chat",
    ),
    (
        "The Local AI server answered oddly: {0}",
        "El servidor de IA local respondió algo raro: {0}",
    ),
    (
        "The Local AI server isn't reachable ({0})",
        "No se puede conectar con el servidor de IA local ({0})",
    ),
    (
        "the model didn't answer with JSON",
        "el modelo no respondió con JSON",
    ),
    (
        "the model's answer didn't parse: {0}",
        "no se pudo interpretar la respuesta del modelo: {0}",
    ),
    ("assistant: {0}", "asistente: {0}"),
    (
        "not signed in or no access ({0})",
        "no iniciaste sesión o no tenés acceso ({0})",
    ),
    (
        "rate limited, try again shortly ({0})",
        "demasiadas solicitudes; probá de nuevo en un rato ({0})",
    ),
    (
        "the request ended unexpectedly",
        "la solicitud terminó de golpe",
    ),
    (
        "The model declined to answer that.",
        "El modelo prefirió no responder eso.",
    ),
    (
        "\n\n(The answer was cut off.)",
        "\n\n(La respuesta quedó cortada.)",
    ),
    (
        "the model sent no answer",
        "el modelo no mandó ninguna respuesta",
    ),
    (
        "the answer stopped early ({0})",
        "la respuesta se cortó antes de tiempo ({0})",
    ),
    (
        "the ChatGPT sign-in has expired; sign in again in Settings ▸ Assistant",
        "la sesión de ChatGPT venció; volvé a iniciar sesión en Configuración ▸ Asistente",
    ),
    ("Searching documents", "Buscando documentos"),
    ("Searching for “{0}”", "Buscando «{0}»"),
    ("Looking up tags {0}", "Buscando las etiquetas {0}"),
    (
        "Searching for “{0}” tagged {1}",
        "Buscando «{0}» con la etiqueta {1}",
    ),
    ("Looking at tags", "Mirando las etiquetas"),
    ("Looking at the library", "Mirando la biblioteca"),
    ("Opening {0}", "Abriendo {0}"),
    ("Reading {0}", "Leyendo {0}"),
    ("Showing folder {0}", "Mostrando la carpeta {0}"),
    ("Showing {0}", "Mostrando {0}"),
    ("Showing results", "Mostrando los resultados"),
    (
        "This provider needs no key.",
        "Este servicio no necesita clave.",
    ),
    (
        "The ant command isn't installed (see Settings ▸ Assistant).",
        "El comando ant no está instalado (mirá Configuración ▸ Asistente).",
    ),
    (
        "Not signed in to Anthropic: use Sign in… in Settings ▸ Assistant.",
        "No iniciaste sesión en Anthropic: usá Iniciar sesión… en Configuración ▸ Asistente.",
    ),
    (
        "The ant command isn't installed.",
        "El comando ant no está instalado.",
    ),
    (
        "Signing in didn't finish.",
        "El inicio de sesión no terminó.",
    ),
    (
        "Sign in with ChatGPT in Settings ▸ Assistant.",
        "Iniciá sesión con ChatGPT en Configuración ▸ Asistente.",
    ),
    (
        "Signing in took too long; please try again.",
        "El inicio de sesión tardó demasiado; probá de nuevo.",
    ),
    ("Signed in", "Sesión iniciada"),
    (
        "Rupert can now use your ChatGPT plan. You can close this tab.",
        "Rupert ya puede usar tu plan de ChatGPT. Podés cerrar esta pestaña.",
    ),
    ("Not signed in", "No se inició sesión"),
    (
        "Local AI isn't running, or has no model.",
        "La IA local no está funcionando, o no tiene ningún modelo.",
    ),
    (
        "The Local AI server has no model.",
        "El servidor de IA local no tiene ningún modelo.",
    ),
    (
        "Add an Anthropic API key in Settings ▸ Assistant.",
        "Agregá una clave de API de Anthropic en Configuración ▸ Asistente.",
    ),
    (
        "Your ChatGPT plan lists no models for other apps.",
        "Tu plan de ChatGPT no ofrece modelos para otras apps.",
    ),
    (
        "Choose a model in Settings ▸ Assistant.",
        "Elegí un modelo en Configuración ▸ Asistente.",
    ),
    (
        "Add an API key in Settings ▸ Assistant.",
        "Agregá una clave de API en Configuración ▸ Asistente.",
    ),
    ("ChatGPT plan · {0}", "Plan de ChatGPT · {0}"),
    ("ChatGPT plan", "Plan de ChatGPT"),
    ("Cloud · {0}", "Nube · {0}"),
    ("Local AI", "IA local"),
    ("Local AI on {0}", "IA local en {0}"),
    ("AI at {0}", "IA en {0}"),
    (
        "The model ran out of room before answering (thinking models can). Choose a model that doesn't think, or thinks less.",
        "El modelo se quedó sin espacio antes de responder (les pasa a los modelos que razonan). Elegí un modelo que no razone, o que razone menos.",
    ),
    (
        "Suggestions only use a Local AI server on this computer or your local network.",
        "Las sugerencias solo usan un servidor de IA local en esta computadora o en tu red local.",
    ),
    ("Cloud (no model chosen)", "Nube (sin modelo elegido)"),
    (
        "I couldn't finish that in a reasonable number of steps.",
        "No pude terminar eso en una cantidad razonable de pasos.",
    ),
    (
        "your ChatGPT plan's limit for Rupert is used up for now; try again later, or raise Rupert's limit in ChatGPT's settings",
        "por ahora se agotó el límite de tu plan de ChatGPT para Rupert; probá más tarde, o subí el límite de Rupert en la configuración de ChatGPT",
    ),
    (
        "ChatGPT plan usage isn't available right now; try again later",
        "el uso del plan de ChatGPT no está disponible en este momento; probá más tarde",
    ),
    (
        "this ChatGPT account or plan can't be used by other apps (signing in again won't change that)",
        "esta cuenta o este plan de ChatGPT no se puede usar desde otras apps (volver a iniciar sesión no lo cambia)",
    ),
    // MARK: - Signing in (ChatGPT, OneDrive, Google Drive)
    (
        "the sign-in answer didn't match; please try again",
        "la respuesta del inicio de sesión no coincide; probá de nuevo",
    ),
    (
        "the sign-in page returned no code",
        "la página de inicio de sesión no devolvió ningún código",
    ),
    (
        "the sign-in returned another app registration",
        "el inicio de sesión devolvió el registro de otra app",
    ),
    (
        "registering Rupert with ChatGPT didn't finish",
        "no terminó el registro de Rupert en ChatGPT",
    ),
    (
        "the sign-in returned no ID token",
        "el inicio de sesión no devolvió un token de identidad",
    ),
    (
        "the sign-in came from an unexpected issuer",
        "el inicio de sesión vino de un emisor inesperado",
    ),
    (
        "the sign-in was meant for another app",
        "el inicio de sesión era para otra app",
    ),
    (
        "the sign-in has expired; please try again",
        "el inicio de sesión venció; probá de nuevo",
    ),
    (
        "the sign-in named no account",
        "el inicio de sesión no indicó ninguna cuenta",
    ),
    (
        "the refreshed sign-in belongs to another account",
        "la sesión renovada pertenece a otra cuenta",
    ),
    (
        "the sign-in returned no refresh token",
        "el inicio de sesión no devolvió un token de renovación",
    ),
    (
        "the ID token is malformed",
        "el token de identidad está mal formado",
    ),
    (
        "listing models failed (HTTP {0})",
        "no se pudieron listar los modelos (HTTP {0})",
    ),
    (
        "signed out, please sign in again",
        "se cerró la sesión; volvé a iniciar sesión",
    ),
    ("OneDrive: {0}", "OneDrive: {0}"),
    (
        "OneDrive isn't set up in this build of Rupert.",
        "OneDrive no está configurado en esta versión de Rupert.",
    ),
    (
        "Sign in to OneDrive first (Settings).",
        "Primero iniciá sesión en OneDrive (Configuración).",
    ),
    (
        "Microsoft didn't allow staying signed in.",
        "Microsoft no permitió mantener la sesión iniciada.",
    ),
    (
        "Google Drive isn't set up in this build of Rupert.",
        "Google Drive no está configurado en esta versión de Rupert.",
    ),
    (
        "Sign in to Google Drive first (Settings).",
        "Primero iniciá sesión en Google Drive (Configuración).",
    ),
    (
        "Google didn't allow staying signed in.",
        "Google no permitió mantener la sesión iniciada.",
    ),
    (
        "Google doesn't recognize this copy of Rupert, so it can't sign in. Please tell us at contact@rupertlibrarian.com.",
        "Google no reconoce esta copia de Rupert, así que no puede iniciar sesión. Avisanos a contact@rupertlibrarian.com.",
    ),
    (
        "Couldn't reach Google. Check the internet connection and try again.",
        "No se pudo conectar con Google. Revisá la conexión a internet y probá de nuevo.",
    ),
    (
        "Google didn't let Rupert sign in: {0} (code {1}).",
        "Google no dejó que Rupert inicie sesión: {0} (código {1}).",
    ),
    (
        "“{0}” is already in the library.",
        "«{0}» ya está en la biblioteca.",
    ),
    (
        "“{0}” isn't in Google Drive any more.",
        "«{0}» ya no está en Google Drive.",
    ),
    // MARK: - iPhone
    (
        "That's the inbox; choose another inbox in Settings instead.",
        "Esa es la bandeja de entrada; elegí otra bandeja en Configuración.",
    ),
    (
        "Choose the inbox folder first.",
        "Primero elegí la carpeta de la bandeja de entrada.",
    ),
    // MARK: - Desktop
    ("Not a Markdown file.", "No es un archivo Markdown."),
    ("Rupert Help", "Ayuda de Rupert"),
    // MARK: - macOS menu
    ("About {0}", "Acerca de {0}"),
    ("Check for Updates…", "Buscar actualizaciones…"),
    ("Settings…", "Configuración…"),
    ("Services", "Servicios"),
    ("Hide {0}", "Ocultar {0}"),
    ("Hide Others", "Ocultar otros"),
    ("Show All", "Mostrar todo"),
    ("Quit {0}", "Salir de {0}"),
    ("File", "Archivo"),
    ("Scan…", "Escanear…"),
    ("Import from ScanSnap Home…", "Importar de ScanSnap Home…"),
    ("Close Window", "Cerrar ventana"),
    ("Edit", "Edición"),
    ("Undo", "Deshacer"),
    ("Redo", "Rehacer"),
    ("Cut", "Cortar"),
    ("Copy", "Copiar"),
    ("Paste", "Pegar"),
    ("Select All", "Seleccionar todo"),
    ("View", "Visualización"),
    ("Enter Full Screen", "Entrar en pantalla completa"),
    ("Window", "Ventana"),
    ("Minimize", "Minimizar"),
    ("Zoom", "Zoom"),
    ("Help", "Ayuda"),
];
