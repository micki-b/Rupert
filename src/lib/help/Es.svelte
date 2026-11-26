<script lang="ts" module>
  // La ayuda de Rupert en español (rioplatense). Tiene las mismas secciones
  // (y los mismos id) que En.svelte; mantenelas a la par entre sí y con los
  // textos de la app (src/lib/i18n/es.ts).
  export const sections: { id: string; title: string; sub?: boolean }[] = [
    { id: "start", title: "Primeros pasos" },
    { id: "library", title: "Tu biblioteca" },
    { id: "finding", title: "Encontrar documentos" },
    { id: "details", title: "Títulos, etiquetas y fechas" },
    { id: "tags", title: "Administrar etiquetas" },
    { id: "inbox", title: "La bandeja de entrada" },
    { id: "scanning", title: "Escanear" },
    { id: "scansnap", title: "Venir de ScanSnap Home" },
    { id: "assistant", title: "Preguntale a Rupert" },
    { id: "ai-models", title: "Configurar modelos de IA" },
    { id: "local-ai", title: "IA local (en esta computadora)", sub: true },
    { id: "claude", title: "Claude", sub: true },
    { id: "chatgpt-plan", title: "ChatGPT con tu plan", sub: true },
    { id: "openai", title: "Servicios compatibles con OpenAI", sub: true },
    { id: "ai-trouble", title: "Cuando el asistente no funciona", sub: true },
    { id: "iphone", title: "La app del iPhone" },
    { id: "android", title: "La app de Android" },
    { id: "storage", title: "Dónde guarda Rupert las cosas" },
    { id: "keys", title: "Atajos de teclado" },
    { id: "about", title: "Acerca de Rupert" },
  ];
</script>

<script lang="ts">
  let { mac, windows, cmd, version, repo: REPO }: { mac: boolean; windows: boolean; cmd: string; version: string; repo: string } =
    $props();
  const linux = $derived(!mac && !windows);
  const fileManager = $derived(mac ? "el Finder" : windows ? "el Explorador de archivos" : "tu administrador de archivos");
  const reveal = $derived(mac ? "Mostrar en el Finder" : windows ? "Mostrar en el Explorador" : "Mostrar en Archivos");
  const keychain = $derived(
    mac
      ? "el Llavero de macOS"
      : windows
        ? "el Administrador de credenciales de Windows"
        : "el llavero del sistema",
  );
  const trash = $derived(windows ? "Papelera de reciclaje" : "Papelera");
</script>

<!-- MARK: Primeros pasos -->
<section id="start">
  <h1>Rupert, el bibliotecario de tus documentos</h1>
  <p>
    Rupert lleva la cuenta de tus documentos escaneados y otros archivos en las carpetas que ya usás: en esta
    computadora, en iCloud Drive, OneDrive o Google Drive. No importa ni copia nada. Tus carpetas siguen siendo la
    biblioteca, y Rupert les suma búsqueda, etiquetas, fechas, favoritos y una bandeja de entrada para los escaneos nuevos.
  </p>
  <h3>Para empezar</h3>
  <ol>
    <li>
      <strong>Agregá una carpeta.</strong> Hacé clic en <b>+</b> al lado de <b>Carpetas</b> en la barra lateral y elegí
      una carpeta, por ejemplo tu carpeta Documentos, o una carpeta de OneDrive o Google Drive. Se incluyen sus
      subcarpetas. Agregá todas las que quieras.
    </li>
    <li>
      <strong>Esperá la primera pasada.</strong> Rupert lee cada documento una vez para poder buscar en su texto. La
      barra de estado de abajo muestra el avance. Mientras tanto podés seguir trabajando.
    </li>
    <li>
      <strong>Configurá la bandeja de entrada</strong> (opcional). En <b>Configuración</b>, elegí la carpeta donde llegan
      los escaneos nuevos. Rupert reconoce su texto y sugiere cómo archivarlos. Mirá
      <a href="#inbox">La bandeja de entrada</a>.
    </li>
    <li>
      <strong>Configurá la IA local</strong> (opcional) para las sugerencias de archivo y el asistente, con LM Studio,
      Ollama o una app parecida. Mirá <a href="#local-ai">IA local</a>.
    </li>
  </ol>
  <div class="note">
    Todo lo que hagas en {fileManager} o en tu app de sincronización aparece en Rupert, y lo que hacés en Rupert es un
    cambio común y corriente en los archivos. Nada queda encerrado en Rupert.
  </div>
</section>

<!-- MARK: Biblioteca -->
<section id="library">
  <h2>Tu biblioteca</h2>
  <p>
    La barra lateral muestra tus <b>Carpetas</b>, las que agregaste. Hacé clic en una carpeta para ver sus documentos, y
    en un documento para ver sus detalles a la derecha. Con doble clic (o <b>Abrir</b>) se abre en su app de siempre;
    <b>{reveal}</b> lo muestra en {fileManager}.
  </p>
  <p>Los dos botones arriba de las carpetas cambian la barra lateral entre tus <b>carpetas</b> y tus <b>etiquetas</b>.</p>
  <p>
    La <b>Configuración</b> ({mac ? "⌘," : "Ctrl+,"}, o el botón <b>Configuración</b> abajo a la izquierda) se abre en su
    propia ventana, con sus partes a la izquierda: <b>General</b>, <b>Biblioteca</b>, <b>Bandeja de entrada</b>,
    <b>Reconocimiento de texto</b>, <b>IA local</b> y <b>Asistente</b>. Escribí en el campo de búsqueda de arriba para
    encontrar un ajuste. <b>Biblioteca</b> muestra tus fuentes con sus opciones, las mismas que las del <b>⋯</b> al lado
    de cada una; <b>+</b> agrega una carpeta. Los cambios se aplican en el acto.
  </p>
  <p>
    Para darle más o menos lugar a la barra lateral, a los <b>Detalles</b> o a la vista previa y <b>Archivarlo</b> de la
    bandeja de entrada, arrastrá el borde entre las columnas. Doble clic en el borde para volver al ancho de siempre.
    Rupert recuerda cada ancho, y el centro siempre conserva algo de lugar. Con el teclado, apretá {mac ? "Opción-Tab" : "Tab"} hasta que el borde
    quede seleccionado y usá las flechas (con Mayúsculas, pasos más grandes).
  </p>
  <p>
    Los <b>archivos Markdown</b> (<code>.md</code>) se abren en el lector propio de Rupert, con títulos, listas, tablas e
    imágenes. Los enlaces a otros archivos Markdown también se abren ahí (<b>Atrás</b> vuelve), los enlaces web se abren
    en tu navegador y <b>Abrir con la app predeterminada</b> le pasa el archivo a tu editor de siempre. Los archivos
    Markdown que están solo en línea se abren en tu app de siempre, que los descarga.
  </p>
  <h3 id="views">Vistas</h3>
  <p>Los tres botones a la derecha de la ruta de la carpeta cambian cómo se muestran las carpetas, para todas:</p>
  <ul>
    <li><b>Lista</b>: nombres, tipos, tamaños y fechas.</li>
    <li>
      <b>Íconos grandes</b>: una imagen de la primera página de cada documento (el comienzo del texto en los archivos
      Markdown). Usá las flechas del teclado para moverte entre ellos.
    </li>
    <li>
      <b>Galería</b>: el documento elegido en grande, con una tira de todos los documentos abajo. <kbd>←</kbd> y
      <kbd>→</kbd> pasan al documento anterior o siguiente; <kbd>↑</kbd> y <kbd>↓</kbd> (o los botones de flecha sobre
      la página) pasan sus páginas. Hacé clic en la página para abrirla en el <a href="#viewer">visor de páginas</a>.
    </li>
  </ul>
  <p>
    <b>Ordenar:</b> en la lista, hacé clic en el título de una columna (Nombre, Tamaño, Fecha, Modificado, y Tipo cuando
    hay lugar) para ordenar por ella; otro clic invierte el orden. <b>Fecha</b> es la fecha del documento (la de su
    archivo si no tiene). En íconos grandes y en la galería, elegí el orden en <b>Ordenar</b>, al lado de los botones de
    vista. Las carpetas quedan arriba, y el orden vale para todas las carpetas.
  </p>
  <h3 id="viewer">El visor de páginas</h3>
  <p>
    Hacé clic en una página (en la galería, o en la vista previa de la bandeja de entrada) para verla en grande. Acercá y
    alejá con <b>+</b> y <b>−</b>, {mac ? "⌘+ y ⌘−" : "Ctrl++ y Ctrl+−"}, pellizcando en el trackpad o con {mac ? "⌘" : "Ctrl"}
    y la rueda; con doble clic acercás a un punto y con otro ves la página entera. <b>Página entera</b>
    ({mac ? "⌘0" : "Ctrl+0"}) y <b>Ancho de página</b> la ajustan a la ventana. Arrastrá la página, o desplazate, para
    moverte por ella. Pasá páginas con las flechas, <kbd>←</kbd> <kbd>→</kbd>, Re Pág y Av Pág, o escribí un número de
    página. <b>Rotar</b> gira un escaneo de costado solo para verlo; el archivo no cambia. <b>Abrir con la app
    predeterminada</b> lo abre en tu app de siempre. Escape, o un clic afuera, cierra el visor.
  </p>
  <p>
    También podés seleccionar un documento en cualquier vista, o en los resultados de búsqueda, y apretar
    <kbd>Espacio</kbd>. Ahí <kbd>↑</kbd> y <kbd>↓</kbd> pasan al documento anterior o siguiente, y <kbd>Espacio</kbd> o
    Escape cierran el visor. Un documento solo en línea muestra la imagen chica que Rupert guardó (ver abajo); no se
    descarga nada hasta que lo abrís.
  </p>
  <p>
    Desde los resultados de búsqueda, el visor abre en la primera página con una coincidencia y marca las palabras que
    buscaste. <b>3 de 7</b> cuenta las coincidencias; las flechas al lado (o {mac ? "⌘G y ⇧⌘G" : "Ctrl+G y Mayúsculas+Ctrl+G"}) pasan a la siguiente o a la anterior. Para buscar otras palabras, apretá {cmd}F en el visor, o
    hacé clic en la lupa, y escribilas; Return pasa a la siguiente coincidencia. Las palabras se encuentran en los PDF que
    están en esta computadora, también en escaneos cuyo texto reconoció Rupert. En los documentos solo en línea no se
    puede buscar así, porque habría que descargarlos.
  </p>
  <p>
    Las imágenes se guardan en una caché en esta computadora y se vuelven a hacer cuando cambia un documento. Rupert
    nunca lee un archivo que está solo en línea para mostrarlo, porque eso lo descargaría. En cambio, mientras un PDF de
    una carpeta de iCloud Drive, OneDrive o Google Drive está en esta computadora, Rupert guarda una imagen chica de su
    primera página, así sigue mostrando una cuando el archivo pasa a estar solo en línea. Los documentos solo en línea sin
    imagen muestran su ícono. En los dos casos conservan la nube, y abrir uno lo descarga.
  </p>
  <h3 id="selecting">Seleccionar varios</h3>
  <p>
    Hacé clic en un documento y después {mac ? "⌘-clic" : "Ctrl-clic"} en otros para sumarlos (o para sacar uno), o
    Mayúsculas-clic para seleccionar todo lo que hay entre medio. <kbd>{cmd}A</kbd> selecciona todo lo de la carpeta o de
    los resultados de búsqueda, y Mayúsculas con <kbd>↑</kbd> <kbd>↓</kbd> (<kbd>←</kbd> <kbd>→</kbd> en íconos grandes)
    amplía la selección desde el teclado. Funciona en la lista, los íconos grandes, la tira de la galería y los resultados
    de búsqueda.
  </p>
  <p>
    Con varios seleccionados, <b>Detalles</b> muestra cuántos son, el tamaño total y las etiquetas que tienen en común,
    con lo que podés hacer con todos. Hacé clic derecho en uno de ellos para lo mismo en un menú:
  </p>
  <ul>
    <li>
      <b>Mover a…</b> una carpeta para todos, o arrastralos a una carpeta. Arrastrados fuera de la ventana, las otras apps
      reciben copias de todos.
    </li>
    <li>
      <b>Agregar etiquetas…</b> y <b>Quitar etiquetas…</b>, que se guardan donde cada documento tiene sus detalles, como en
      <a href="#tags">Administrar etiquetas</a>. Los PDF solo en línea que quedarían sin etiquetas se descargan, después de
      preguntar.
    </li>
    <li><b>Marcar con estrella</b> o quitarla.</li>
    <li>
      {#if !linux}<b>Compartir…</b> y {/if}<b>Enviar por correo…</b>: {#if !linux}una sola vez, o {/if}un solo correo con
      todos adjuntos.
    </li>
    <li><b>Cambiar el nombre de documentos…</b> con solo estos marcados (en una carpeta, no en resultados de búsqueda).</li>
    <li><b>{windows ? "Eliminar" : "Trasladar a la Papelera"}</b>, con una sola pregunta para todos.</li>
  </ul>
  <p>
    Los lotes largos muestran su avance abajo en la ventana y después un resumen, con lo que no haya funcionado.
  </p>
  <h3 id="export">Exportar un conjunto de documentos</h3>
  <p>
    Para entregar varios documentos juntos (los papeles de impuestos del año para el contador, todo lo de un auto que
    vendés), elegí <b>Exportar…</b>: para lo seleccionado (en Detalles o en el menú del clic derecho), para una carpeta
    con todo lo que tiene (clic derecho sobre ella) o para los resultados de búsqueda (el botón arriba de ellos, por
    ejemplo la etiqueta Impuestos y el año pasado).
  </p>
  <p>
    Rupert copia los documentos en un <b>archivo zip</b> o una <b>carpeta</b>, donde elijas en el diálogo de guardar, y
    agrega una lista: <b>index.csv</b> para una planilla e <b>index.html</b> para leer en un navegador, con el título, la
    fecha, las etiquetas y la carpeta original de cada documento. Las copias mantienen sus carpetas y sus nombres, y las
    notas van si marcás <b>Incluir las notas</b>. Los originales quedan como están. Los documentos solo en línea hay que
    descargarlos para copiarlos; el diálogo dice cuántos son. Después, <b>{reveal}</b> muestra el resultado y <b>Compartir…</b> envía el zip.
  </p>
  <h3 id="dragging-out">Arrastrar documentos hacia afuera</h3>
  <p>
    Arrastrá un documento (o una carpeta, o un escaneo de la bandeja de entrada) fuera de la ventana de Rupert hasta
    Mail, Mensajes, la zona para subir archivos de un sitio web, {fileManager} o el escritorio, y esa app recibe una
    <em>copia</em> del archivo, con su nombre. Tu biblioteca conserva el archivo. Los archivos que están solo en línea los
    descarga la app donde los soltás. Si soltás un documento en una de las
    carpetas de Rupert (se resalta mientras estás encima), se mueve ahí, como siempre.
  </p>
  <h3 id="sending">Enviar documentos</h3>
  <p>
    Hacé clic derecho en un documento y elegí {#if linux}<b>Enviar por correo…</b>{:else}<b>Compartir…</b> o
      <b>Enviar por correo…</b>{/if}, o usá los botones junto a {reveal} en los detalles y junto a Abrir en la bandeja de entrada.
    {#if mac}<b>Compartir…</b> abre el menú Compartir de la Mac: Mail, Mensajes, AirDrop, Notas y las apps que hayas
      agregado.
    {:else if windows}<b>Compartir…</b> abre la ventana Compartir de Windows.{/if}
    <b>Enviar por correo…</b> empieza un mensaje nuevo en tu programa de correo con el documento adjunto y su título
    como asunto.
  </p>
  <p>
    Rupert guarda tus notas dentro de cada PDF, así que un PDF se envía como una copia <b>sin sus notas</b> (el título,
    la fecha y las etiquetas sí van). Para enviarlas también, activá <b>Incluir mis notas al enviar un PDF</b> en
    Configuración ▸ Biblioteca. Un escaneo de la bandeja de entrada sale con el nombre con el que se archivaría, no con el
    del escáner. Un documento solo en línea se descarga primero; abajo en la ventana se ve, con Cancelar.
  </p>
  <h3 id="renaming">Renombrar</h3>
  <p>
    Hacé clic derecho en un documento o una carpeta y elegí <b>Renombrar…</b>, o seleccionalo y apretá
    {mac ? "Return" : "F2"}{mac ? " (⌘O o ⌘↓ lo abre)" : ""}. Escribí el nombre nuevo ahí mismo y apretá
    {mac ? "Return" : "Enter"}; Escape lo deja como estaba. Solo queda seleccionada la parte antes de la extensión, y si
    no escribís la extensión, se conserva la anterior. Se renombra en el disco en el acto, junto con su archivo oculto de
    detalles, y los archivos que están solo en línea se renombran sin descargarlos.
  </p>
  <p>
    Rupert nunca reemplaza nada: un nombre que ya existe en la carpeta no se acepta. Tampoco los nombres que algunas
    computadoras no pueden usar, porque tus carpetas pueden sincronizarse entre una Mac, Windows y Linux: nombres con
    <code>/ \ : * ? " &lt; &gt; |</code>, nombres que empiezan con un punto o terminan con un punto o un espacio, y nombres
    que Windows se reserva, como CON o NUL. Si renombrás la carpeta de la bandeja de entrada (o una que la contiene), la
    configuración de la bandeja la sigue. Los resultados de búsqueda no se pueden renombrar: abrí primero la carpeta.
  </p>
  <h3 id="renaming-documents">Cambiar el nombre de muchos documentos</h3>
  <p>
    Hacé clic derecho en una carpeta y elegí <b>Cambiar el nombre de documentos…</b> para darles nombres nuevos a sus
    documentos de una vez; desde un documento, el mismo comando muestra su carpeta con solo ese documento marcado. Una
    tabla muestra cada nombre actual y su nombre nuevo, con lo que cambia resaltado, y al lado la primera página, el
    título, la fecha y las etiquetas del documento seleccionado. Desmarcá los que no quieras cambiar. No cambia nada hasta
    que hagas clic en <b>Cambiar</b>.
  </p>
  <p>
    Los nombres nuevos salen de la fecha y el título de cada documento, según tu formato de nombre (Configuración ▸
    Bandeja de entrada). Hacé clic en un nombre nuevo para cambiarlo y Rupert nombra los demás <b>de la misma manera</b>:
    enseguida si la regla está en la fecha y el título (la fecha al final, otro formato de fecha, una palabra adelante);
    si no, con la IA local, que sigue tu ejemplo. Los nombres que escribiste quedan marcados como <b>Tuyo</b> y no
    cambian. Con IA local también podés describir los nombres («aseguradora - póliza - año») y hacer clic en <b>Proponer
    con IA</b>; las propuestas llegan de a una y <b>Detener</b> conserva las que ya llegaron. La IA local solo lee lo que
    Rupert ya indexó, así que los archivos solo en línea no se descargan y su texto nunca va a un servicio en la nube.
    <b>Deshacer</b> en el texto de ayuda vuelve a las propuestas anteriores.
  </p>
  <p>
    Rupert avisa de los problemas antes de cambiar nada: a un nombre ocupado se le agrega " (2)", y los nombres no
    permitidos o sin cambios quedan afuera. La extensión y los datos de cada documento se mantienen; marcá <b>Usar también
    el nombre nuevo como título de cada documento</b> para cambiar también los títulos. Después, <b>Deshacer</b> abajo en
    la ventana ({mac ? "o Edición ▸ Deshacer, ⌘Z" : "o Ctrl+Z"}) les devuelve a todos sus nombres (y títulos) anteriores,
    hasta que hagas otra cosa.
  </p>
  <h3 id="moving">Mover</h3>
  <p>
    Arrastrá un documento o una carpeta sobre una carpeta, en la barra lateral o en la vista de la carpeta, o hacé clic
    derecho y elegí <b>Mover a…</b> para elegir una carpeta. Se mueve en el disco en el acto, también a otra de tus
    carpetas, y el archivo oculto de detalles del documento se va con él, así que su título, sus etiquetas y sus notas lo
    acompañan. Si ya hay un documento con ese nombre, Rupert le agrega " (2)" en vez de reemplazar nada.
  </p>
  <p>
    Mover nunca descarga nada. Por eso un archivo solo en línea se mueve solo dentro de su propia carpeta en la nube
    (por ejemplo, de una carpeta de OneDrive a otra); para moverlo a otro lado, abrilo una vez para que esté en esta
    computadora y después movelo. Las carpetas se mueven solo dentro del mismo disco. La carpeta que tiene la
    <a href="#inbox">bandeja de entrada</a> no se puede mover hasta que elijas otra bandeja en
    <b>Configuración ▸ Bandeja de entrada</b>.
  </p>
  <h3 id="adding">Agregar archivos</h3>
  <p>
    Arrastrá archivos desde {fileManager} u otra app sobre una carpeta de Rupert: la carpeta abierta, o una carpeta de
    la barra lateral o de la vista de la carpeta. Rupert los copia ahí y abre esa carpeta; los originales quedan donde
    estaban. Cada copia conserva su nombre y su fecha, con " (2)" agregado si el nombre ya existe. Si en cambio los
    soltás sobre <b>Bandeja de entrada</b> en la barra lateral, Rupert los lee y te sugiere dónde archivarlos, como con
    un escaneo nuevo.
  </p>
  <p>
    Solo se pueden soltar archivos, no carpetas. Una fuente muestra solo documentos salvo que actives
    <b>Mostrar todos los tipos de archivo</b>, así que los demás tipos de archivo se rechazan con un aviso que explica
    por qué.
  </p>
  <h3 id="deleting">Borrar</h3>
  <p>
    Hacé clic derecho en un documento o una carpeta y elegí <b>{windows ? "Eliminar" : "Trasladar a la Papelera"}</b>, o
    seleccionalo y apretá <kbd>{mac ? "⌘⌫" : "Supr"}</kbd>. Va a la {trash}, así que lo podés recuperar de ahí. El
    archivo oculto de detalles del documento se va con él. Los documentos se van en el acto; para una carpeta, Rupert
    pregunta primero, porque se va todo lo que tiene adentro.
  </p>
  <p>
    Rupert no abre los archivos solo en línea para borrarlos; el borrado se sincroniza como cualquier otro, y iCloud
    Drive, OneDrive y Google Drive además guardan los archivos borrados por un tiempo en sus sitios web. Las carpetas que
    agregaste a la biblioteca no se pueden borrar así; usá <b>Quitar de Rupert…</b> en el menú <b>⋯</b> de la carpeta
    (eso deja la carpeta en el disco).
  </p>
  <h3 id="undo">Deshacer</h3>
  <p>
    Justo después de que Rupert cambia algo, un aviso abajo en la ventana dice qué, con <b>Deshacer</b>. Lo mismo hace
    {#if mac}<b>Edición ▸ Deshacer</b> (<kbd>⌘Z</kbd>), que nombra el cambio{:else}<kbd>Ctrl+Z</kbd>{/if}. Deshace el último
    de estos: mover documentos o carpetas (arrastrando o con <b>Mover a…</b>), cambiar el nombre de uno o de muchos,
    archivar un escaneo de la bandeja de entrada (vuelve a la bandeja con su nombre anterior; los detalles que recibió
    quedan en él) y cambiar etiquetas (en el administrador de etiquetas o con <b>Agregar etiquetas…</b> / <b>Quitar
    etiquetas…</b>). Rupert solo deshace lo que sigue como lo dejó: un archivo que se movió o cambió desde entonces, o un
    nombre que volvió a estar ocupado, queda como está, y el aviso lo dice. Deshacer alcanza solo al último cambio, hasta
    que cierres Rupert. Lo que mandaste a la {trash} se recupera desde ahí.
  </p>
  <h3>Opciones de las carpetas</h3>
  <p>Hacé clic en <b>⋯</b> al lado de una carpeta:</p>
  <ul>
    <li>
      <b>Mostrar todos los tipos de archivo</b>: de entrada Rupert muestra solo documentos (PDF, imágenes, archivos de
      Office, texto). Activalo para ver todos los archivos.
    </li>
    <li>
      <b>Descargar los archivos solo en línea para indexarlos</b> (carpetas de iCloud Drive, OneDrive y Google Drive):
      mirá <a href="#online-only">archivos solo en línea</a> más abajo.
    </li>
    <li><b>Renombrar…</b> cambia el nombre que se muestra en Rupert, no el nombre de la carpeta en el disco.</li>
    <li><b>Quitar de Rupert…</b> deja de mostrar la carpeta. La carpeta y sus archivos quedan exactamente donde están.</li>
  </ul>
  <h3 id="online-only">Archivos solo en línea</h3>
  <p>
    iCloud Drive, OneDrive y Google Drive pueden dejar archivos solo en la nube, para ahorrar espacio. Rupert los muestra
    con un ícono de nube <span class="cloud">☁</span> y nunca los abre por su cuenta, porque abrir uno lo descarga. Igual
    los podés encontrar por el nombre y por los detalles que Rupert conoce, y al abrir uno se descarga como siempre.
    Hacé clic en la nube para descargar solo ese archivo: Rupert lee entonces su texto, y el archivo queda en la
    computadora.
  </p>
  <p>
    En Windows, Google Drive configurado en <b>Transmitir archivos</b> no les dice a otras apps qué archivos están en la
    computadora, así que Rupert trata todos los archivos de su unidad <b>Google Drive (G:)</b> como solo en línea. Los
    títulos, las etiquetas y el texto vienen entonces de Rupert en otra computadora (a través del catálogo compartido), o
    de <b>Descargar los archivos solo en línea para indexarlos</b>. Con Google Drive en <b>Duplicar archivos</b>, todo
    está en la computadora y se lee como siempre.
  </p>
  <p>
    Para buscar en el <em>texto</em> de los documentos solo en línea, activá <b>Descargar los archivos solo en línea para
    indexarlos</b> en esa carpeta. Rupert los descarga de a poco, lee su texto, guarda una imagen chica de la
    primera página de cada PDF y libera el espacio de nuevo. Los PDF cuyo texto ya se conoce también se descargan una vez si
    todavía no tienen imagen en esta computadora. Los archivos
    no cambian. En una carpeta grande puede tardar horas; los archivos de más de 100 MB se saltean. Lo que Rupert aprende
    se guarda en la carpeta misma, así tus otras computadoras (y la app del iPhone) pueden buscar en esos documentos sin
    volver a descargarlos.
  </p>
  <p>
    Para poner al día una carpeta entera en el momento, hacé clic derecho sobre ella (en la barra lateral o en la vista de
    la carpeta, o usá el menú <b>⋯</b> de una fuente) y elegí <b>Indexar esta carpeta</b>. Rupert vuelve a leer todos
    los documentos de esa carpeta y sus subcarpetas, y descarga los que están solo en línea para leerlos y guardar una
    imagen; después libera el espacio de nuevo. Funciona aunque <b>Descargar los archivos solo en línea para
    indexarlos</b> esté apagado.
  </p>
  <p>
    Mientras Rupert indexa o descarga, abajo a la derecha de la ventana se ve cuánto le falta, con <b>Detener</b>.
    Detener deja terminar el archivo en curso y frena; lo hecho se conserva. <b>Reanudar</b>, o el próximo inicio de
    Rupert, sigue desde ahí.
  </p>
</section>

<!-- MARK: Encontrar -->
<section id="finding">
  <h2>Encontrar documentos</h2>
  <h3 id="quick-open">Apertura rápida</h3>
  <p>
    Apretá <kbd>{cmd}K</kbd>{mac ? " (o elegí Archivo ▸ Apertura rápida…)" : ""} y escribí algunas letras de un nombre
    para ir directo a una carpeta, un documento (por su título o nombre de archivo), una etiqueta o un comando como
    Bandeja de entrada, Administrar etiquetas o Configuración. Antes de que escribas, te ofrece los últimos documentos
    que abriste o miraste y tus carpetas favoritas. <kbd>↑</kbd> <kbd>↓</kbd> eligen y <kbd>Return</kbd> te lleva: una
    carpeta se abre, un documento aparece seleccionado en su carpeta, una etiqueta muestra sus documentos.
    <kbd>{mac ? "⌘↩" : "Ctrl+Enter"}</kbd> abre un documento en su app. <kbd>Esc</kbd> la cierra.
  </p>
  <h3>Buscar</h3>
  <p>
    Escribí en el campo de búsqueda de arriba ({cmd}F). Rupert busca en los títulos, los nombres de archivo, las
    etiquetas, las notas y el texto de los documentos, en todas las carpetas. Tienen que coincidir todas las palabras que
    escribas; la última puede ser el comienzo de una palabra, así que <kbd>impul</kbd> encuentra “impulsor”. Los acentos
    no importan. Apretá <kbd>Esc</kbd> para borrar la búsqueda.
  </p>
  <p>
    Abajo del campo de búsqueda podés acotar por <b>fecha</b> (este año, el año pasado o las fechas que elijas: la fecha
    del documento, o la de su archivo), por <b>tipo</b> (PDF, imagen, Office, texto, correo) y por <b>lugar</b>: en todos
    lados, en la fuente de la carpeta abierta o en la carpeta abierta con sus subcarpetas. Funcionan también sin
    palabras: elegí <b>El año pasado</b> y <b>PDF</b> para ver todos los PDF del año pasado. <b>Más relevantes</b> y
    <b>Más nuevos</b>, arriba de los resultados, cambian el orden. <kbd>Esc</kbd> borra las palabras y los filtros.
  </p>
  <p>En los resultados, <b>Mostrar en Rupert</b> (en los detalles) te lleva a la carpeta del documento.</p>
  <h3>Etiquetas</h3>
  <p>
    Cambiá la barra lateral a <b>Etiquetas</b>. Sin nada escrito muestra las etiquetas que más usás; escribí para achicar
    la lista, o hacé clic en <b>Mostrar las …</b> debajo. Hacé clic en varias etiquetas para elegirlas, y elegí en
    <b>Coincidir</b>:
  </p>
  <ul>
    <li><b>Alguna</b>: documentos con al menos una de las etiquetas elegidas.</li>
    <li>
      <b>Todas</b>: documentos con todas las etiquetas elegidas. La lista ofrece entonces solo las etiquetas que van
      junto con ellas.
    </li>
  </ul>
  <p>
    El filtro de etiquetas se combina con el texto de la búsqueda y con Favoritos. Hacé clic en la × de una etiqueta
    arriba de la lista para quitarla.
  </p>
  <h3>Favoritos</h3>
  <p>
    Hacé clic en la ☆ de un documento para marcarlo como favorito. Aparece cuando apuntás a un documento en la lista,
    en los íconos o en los resultados de búsqueda, y siempre en sus detalles y en la galería; los favoritos conservan
    la ★. Hacé clic en la ★ para quitarla. Quitarle la estrella a un PDF solo en línea lo descarga, porque la estrella
    se guarda dentro del archivo. Marcá una carpeta con la ☆ al lado de su ruta, arriba de la lista de documentos.
    <b>Favoritos</b> en la barra lateral muestra los dos. Los favoritos se guardan con los archivos, así que
    aparecen en tus otras computadoras y en el iPhone.
  </p>
</section>

<!-- MARK: Detalles -->
<section id="details">
  <h2>Títulos, etiquetas y fechas</h2>
  <p>Elegí un documento para ver y editar sus detalles a la derecha:</p>
  <ul>
    <li><b>Título</b>: un nombre fácil de leer, independiente del nombre del archivo.</li>
    <li>
      <b>Fecha del documento</b>: la fecha que figura en el papel (la de una factura, la de una carta), no la del
      escaneo.
    </li>
    <li><b>Etiquetas</b>: todas las que quieras. Escribí y apretá Enter o una coma para agregar una.</li>
    <li><b>Notas</b>: lo que quieras recordar sobre el documento.</li>
  </ul>
  <p>
    Hacé clic en <b>Guardar</b> para conservar los cambios, o en <b>Deshacer cambios</b> para descartarlos. Se guardan
    dentro del PDF mismo, en el lugar estándar que también leen otros programas (Acrobat, Vista Previa y muchos más). Los
    archivos que no pueden guardar detalles, como los documentos de Word o las fotos, los guardan en un archivo oculto
    chiquito al lado. El panel de detalles dice cuál de los dos.
  </p>
  <p>
    En un PDF solo en línea, hacé clic en <b>Descargar y mostrar los detalles</b> para leer lo que tiene guardado adentro.
    Guardar cambios en un archivo solo en línea lo descarga primero.
  </p>
</section>

<!-- MARK: Etiquetas -->
<section id="tags">
  <h2>Administrar etiquetas</h2>
  <p>
    Con la barra lateral en <b>Etiquetas</b>, hacé clic en <b>Administrar etiquetas…</b>. Elegí una etiqueta para
    <b>renombrarla</b>, varias para <b>unirlas</b> en una, o usá <b>Borrar…</b> para sacar etiquetas de todos los
    documentos. Rupert muestra cuántos documentos van a cambiar antes de hacer nada, y cambia cada documento ahí donde
    estén guardados sus detalles.
  </p>
  <p>
    Los documentos solo en línea no se descargan para esto cuando se puede evitar: el cambio se anota al lado y pasa al PDF
    la próxima vez que se guarde. Si un cambio dejara un PDF solo en línea sin ninguna etiqueta, Rupert pregunta antes de
    descargarlo.
  </p>
</section>

<!-- MARK: Bandeja de entrada -->
<section id="inbox">
  <h2>La bandeja de entrada</h2>
  <p>
    La bandeja de entrada es una carpeta donde llegan los escaneos nuevos: la carpeta donde guarda el programa de tu
    escáner, o la que reciben los escaneos de Rupert y del iPhone. Tiene que estar dentro de una de tus carpetas. Elegila
    en <b>Configuración ▸ Bandeja de entrada</b>. Para agregar archivos de tu computadora, arrastralos sobre
    <b>Bandeja de entrada</b> en la barra lateral o a la bandeja abierta.
  </p>
  <p>Con cada archivo nuevo, Rupert:</p>
  <ol>
    <li>
      <b>Reconoce el texto</b> si es un escaneo sin texto, y le agrega al PDF una capa de texto invisible, para que
      cualquier app de PDF pueda buscar en él. Esto pasa en esta computadora ({mac
        ? "con el reconocimiento de texto que trae Apple"
        : "con Tesseract, si está instalado"}). Activalo o desactivalo en <b>Configuración ▸ Reconocimiento de texto</b>.
    </li>
    <li>
      <b>Sugiere título, fecha, etiquetas y carpeta</b> con la <a href="#local-ai">IA local</a> (LM Studio, Ollama o una
      app parecida), si está funcionando. Rupert sugiere carpetas donde ya hay documentos parecidos.
    </li>
    <li>Te espera. Nada se mueve hasta que lo digas.</li>
  </ol>
  <p>
    Hacé clic en <b>Bandeja de entrada</b> en la barra lateral para revisar. Mirá la sugerencia, cambiá lo que quieras y
    hacé clic en <b>Archivar</b> ({cmd}{mac ? "↩" : "Enter"}). Rupert escribe los detalles en el archivo, le cambia el
    nombre y lo mueve a la carpeta. Las dos flechas en círculo junto al tacho rojo (<b>Sugerir de nuevo</b>) le piden a la IA local una sugerencia
    nueva; giran mientras trabaja.
  </p>
  <p>
    Los escaneos que esperan ocupan el centro de la ventana, con los títulos que sugiere Rupert. Buscalos (por nombre,
    título sugerido, etiquetas, carpeta o su texto) y cambiá entre <b>Lista</b>, <b>Íconos grandes</b> y <b>Galería</b>
    como en una carpeta; Rupert recuerda la vista de la bandeja de entrada. La página del escaneo elegido está arriba de
    la columna derecha, sobre el formulario, en todas las vistas: pasá sus páginas con las flechas sobre la página o con
    <kbd>Re Pág</kbd> y <kbd>Av Pág</kbd>, y hacé clic en ella para abrir el <a href="#viewer">visor de páginas</a>. En la
    galería, la página también se ve grande en el centro.
  </p>
  <p>
    <b>Carpeta</b> muestra la ruta completa adonde va el documento, con la carpeta misma en negrita (el nombre de la fuente
    no aparece si tenés una sola). Para elegir otra, hacé clic y escribí cualquier parte del nombre de una carpeta: primero
    aparecen las sugeridas y después todas las que coinciden, cada una con dónde está. Usá las flechas y
    {mac ? "Return" : "Enter"}, o hacé clic en una. El botón de carpeta al lado del campo, o <b>Ver todas las carpetas…</b>
    al final de la lista, abre todas tus carpetas como un árbol; ahí, <b>Carpeta nueva…</b> crea una carpeta dentro de la
    seleccionada y la elige. Las sugerencias debajo del campo muestran lo justo de cada ruta para distinguirlas.
  </p>
  <p>
    En la lista de la bandeja, las rutas largas muestran las dos últimas carpetas, como “…/Sol y Mar/Purchase”. Apoyá el
    puntero sobre una carpeta, o sobre cualquier campo que no muestre todo su texto, para verlo completo.
  </p>
  <p>
    Para deshacerte de un escaneo, hacé clic en el tacho rojo (<b>{windows ? "Eliminar…" : "Trasladar a la Papelera…"}</b>) debajo del
    formulario, hacé clic derecho sobre él en la lista, o apretá <kbd>{mac ? "⌘⌫" : "Supr"}</kbd>. Rupert pregunta
    primero, porque puede que un escaneo nuevo todavía no esté guardado en ningún otro lado; va a la {trash}, de donde lo
    podés recuperar.
  </p>
  <p>
    El nombre del archivo sigue el formato de <b>Configuración ▸ Bandeja de entrada</b>, en <b>Nombres de archivo</b>: se completan
    <code>{"{date}"}</code> (la fecha) y <code>{"{title}"}</code> (el título), así que el formato predeterminado
    <code>{"{date} {title}"}</code> da nombres como “2026-04-29 Renovación del seguro del auto.pdf”. Si ya existe un
    archivo con ese nombre, Rupert le agrega “(2)”; nunca sobrescribe.
  </p>
  {#if !mac}
    <div class="note">
      En Windows y Linux, el reconocimiento de texto usa el programa gratuito <b>Tesseract</b>. Si la Configuración dice
      que no se encontró ningún motor, instalá Tesseract y volvé a abrir Rupert.
    </div>
  {/if}
</section>

<!-- MARK: Escanear -->
<section id="scanning">
  <h2>Escanear</h2>
  <h3 id="scan-with-phone">Con el teléfono</h3>
  {#if mac}
    <p>
      <b>Con un iPhone o iPad cerca</b> (Cámara de Continuidad) no hace falta ninguna app: hacé clic en <b>Escanear…</b>
      en la bandeja de entrada y después en <b>Escanear con el iPhone…</b>, o elegí <b>Archivo ▸ Importar desde el iPhone o
      iPad</b>, y elegí <b>Escanear documentos</b> debajo de tu iPhone. Escaneá las páginas en el iPhone y tocá
      <b>Guardar</b>: el PDF va directo a la bandeja de entrada. El iPhone tiene que tener la misma cuenta de Apple que la
      Mac, con Wi-Fi y Bluetooth activados.
    </p>
  {/if}
  <p>
    <b>Con la app de Rupert</b> en el iPhone o en Android, la cámara escanea papeles a la misma bandeja de entrada. Hacé
    clic en <b>Escanear…</b> en la bandeja de entrada y apuntá la cámara del teléfono al código: abre la pestaña Escanear
    de Rupert, o te muestra dónde conseguir Rupert si todavía no está en el teléfono. En la pestaña Escanear, elegí la
    carpeta de la bandeja de entrada que indica la ventana. El teléfono solo llega a una bandeja de entrada en iCloud
    Drive (iPhone), OneDrive o Google Drive, no a una carpeta de esta computadora. Mirá
    <a href="#iphone">La app del iPhone</a> y <a href="#android">La app de Android</a>.
  </p>
  <h3>ScanSnap y otros programas de escáner</h3>
  <p>
    Cualquier app de escáner que guarde en una carpeta funciona con Rupert. Para un ScanSnap, creá un perfil de ScanSnap
    Home que guarde los PDF en la carpeta de la bandeja de entrada de Rupert. Rupert levanta cada escaneo nuevo de ahí. Un
    escáner de red sin programa propio funciona igual: {#if mac}en <b>Captura de Imagen</b>, elegí la carpeta de la bandeja
      de entrada en <b>Escanear a</b>{:else if windows}en la app <b>Escáner de Windows</b>, elegí la carpeta de la bandeja de entrada
      como destino de los escaneos{/if}.
  </p>
  <h3 id="import-into-inbox">Importar muchos archivos</h3>
  <p>
    Para traer un montón de archivos que ya tenés (escaneos viejos, una carpeta de resúmenes), hacé clic en
    <b>Importar…</b> en la bandeja de entrada{#if mac} o elegí <b>Archivo ▸ Importar a la bandeja de entrada…</b>
      (⇧⌘I){/if}. Soltá archivos o carpetas en la ventana, o usá <b>Agregar archivos…</b> y <b>Agregar carpeta…</b>; las
    carpetas vienen con todo lo que tienen adentro. La ventana muestra cuántos documentos trae cada uno; hacé clic en
    <b>Importar</b>. Rupert los copia a la bandeja de entrada (los originales quedan donde están, con sus nombres y fechas) y
    muestra el avance arriba de la bandeja, con <b>Cancelar</b>; cada documento aparece cuando llega, y Rupert lo lee y
    sugiere dónde archivarlo, como un escaneo. Un documento cuyo contenido ya está en tu biblioteca no se copia de nuevo: el
    resumen dice cuántos, y <b>Mostrar</b> los lista con dónde están (hacé clic en uno para verlo ahí). Los archivos que no
    son documentos quedan afuera.
  </p>
  <h3 id="print-to-rupert">Imprimir en Rupert</h3>
  <p>
    Muchos documentos nunca pasan por un escáner: una factura en el navegador, la página de un resumen, una confirmación
    de reserva.
    {#if mac}
      Activá <b>Agregar “Guardar en Rupert” al menú PDF del cuadro Imprimir</b> en <b>Configuración ▸ Bandeja de
      entrada</b>. Después, en cualquier app, elegí <b>Archivo ▸ Imprimir</b>, abrí el menú <b>PDF</b> de abajo y elegí
      <b>Guardar en Rupert</b>: el PDF va a la bandeja de entrada, donde Rupert lo lee y sugiere dónde archivarlo, como un
      escaneo. (En la edición de la Mac App Store, al activarlo te pregunta dónde guardar la entrada: elegí la carpeta
      <b>PDF Services</b> de tu carpeta Biblioteca y creala si no está.)
    {:else if windows}
      Activá <b>Agregar “Bandeja de entrada de Rupert” al menú Enviar a del Explorador de archivos</b> en
      <b>Configuración ▸ Bandeja de entrada</b>. Después hacé clic derecho en archivos en el Explorador y elegí <b>Enviar a
      ▸ Bandeja de entrada de Rupert</b> para ponerlos en la bandeja de entrada. Para traer lo que imprimís, elegí la
      impresora <b>Microsoft Print to PDF</b> y guardá en la carpeta de la bandeja de entrada.
    {:else}
      Imprimí a un archivo PDF y guardalo en la carpeta de la bandeja de entrada: Rupert lo lee y sugiere dónde archivarlo,
      como un escaneo.
    {/if}
  </p>
</section>

<!-- MARK: ScanSnap Home -->
<section id="scansnap">
  <h2>Venir de ScanSnap Home</h2>
  <p>
    Si usabas ScanSnap Home para ordenar tus escaneos, Rupert puede traer lo que sabe y no está en los archivos: fechas de
    los documentos, favoritos, datos de los recibos y su texto reconocido. Elegí <b>Importar de ScanSnap Home…</b> al
    final de la barra lateral{mac ? " o en el menú Archivo" : ""}.
  </p>
  <p>
    Rupert primero muestra lo que encontró y lo que va a cambiar, incluidas las carpetas de ScanSnap que todavía no están
    en tu biblioteca (<b>Agregar como carpeta</b>). No se sobrescribe nada de lo que pusiste en Rupert, los archivos solo
    en línea no se descargan, y si lo volvés a hacer no cambia nada.
  </p>
</section>

<!-- MARK: Asistente -->
<section id="assistant">
  <h2>Preguntale a Rupert</h2>
  <p>
    Hacé clic en el botón de chat de la esquina inferior derecha ({cmd}J) para pedirle documentos a Rupert con tus
    palabras. Él busca, lee y muestra lo que encuentra en la ventana principal. Por ejemplo:
  </p>
  <ul>
    <li>«Buscá la factura del impulsor de la primavera pasada»</li>
    <li>«¿Cuál es la franquicia del seguro del barco?»</li>
    <li>«Mostrame todo lo que tenga las etiquetas Taxes y 2025»</li>
    <li>«Abrí la carpeta Boat»</li>
  </ul>
  <p>
    El búho muestra lo que está haciendo: pensar, buscar, leer un documento o abrir algo. Una línea debajo de la
    conversación dice el paso actual. Preguntá en el idioma que quieras: Rupert contesta en el mismo.
  </p>
  <ul>
    <li>
      <b>Detener</b> (el botón ■, o <kbd>Esc</kbd>) corta una respuesta en el acto. Tu pregunta vuelve al cuadro para que
      la puedas cambiar.
    </li>
    <li><b>Nueva conversación</b> (la flecha circular) empieza de cero.</li>
    <li>El engranaje abre la configuración del asistente.</li>
  </ul>
  <p>
    Rupert puede mirar tu biblioteca, pero <b>nunca cambia</b> nada: no renombra, no mueve, no etiqueta ni borra. Solo lee
    el texto que Rupert ya conoce, así que nunca descarga archivos solo en línea.
  </p>
  <p>
    Cuando una respuesta sale de un documento, Rupert nombra la página, por ejemplo <b>Contrato de alquiler 2024, p. 3</b>.
    Hacé clic ahí para ver esa página en el visor, con el pasaje marcado. Los documentos indexados antes de Rupert 0.9
    tienen sus páginas cuando Rupert los vuelve a leer, cosa que hace solo después de la actualización.
  </p>
  <p>
    De entrada funciona con la IA local, en esta computadora. En su lugar podés elegir Claude u otro servicio en la nube:
    mirá <a href="#ai-models">Configurar modelos de IA</a>.
  </p>
  <p class="tip">Cuando Rupert habla español, el búho se pone la camiseta albiceleste.</p>
</section>

<!-- MARK: Modelos de IA -->
<section id="ai-models">
  <h2>Configurar modelos de IA</h2>
  <p>Rupert usa IA en dos lugares, que se configuran por separado:</p>
  <table>
    <thead><tr><th>Función</th><th>Dónde funciona</th><th>Configuración</th></tr></thead>
    <tbody>
      <tr>
        <td>Sugerencias de la bandeja de entrada (título, fecha, etiquetas, carpeta)</td>
        <td>IA local, siempre (en esta computadora o en tu red local)</td>
        <td><b>Configuración ▸ IA local</b>, y <b>Sugerencias</b> en <b>Configuración ▸ Bandeja de entrada</b></td>
      </tr>
      <tr>
        <td>Preguntale a Rupert (el chat)</td>
        <td>IA local de entrada, o un modelo en la nube que elijas</td>
        <td><b>Configuración ▸ Asistente</b></td>
      </tr>
    </tbody>
  </table>
  <p>
    El reconocimiento de texto no usa servicios de IA: funciona en esta computadora. En <b>Configuración ▸ Asistente</b>,
    el menú <b>Modelo</b> elige un tipo de conexión:
  </p>
  <ul>
    <li>
      <a href="#local-ai"><b>IA local</b></a> (LM Studio, Ollama o una app parecida): privado y gratis, necesita una
      computadora potente.
    </li>
    <li>
      <a href="#claude"><b>Claude (Anthropic, en la nube)</b></a>: con una clave de API o con tu cuenta de Anthropic (los
      créditos mensuales de un plan Max, Ultra o Team lo pueden pagar).
    </li>
    <li>
      <a href="#chatgpt-plan"><b>ChatGPT, con tu plan (en la nube)</b></a>: iniciás sesión con ChatGPT; sin clave de API,
      usa tu plan de ChatGPT.
    </li>
    <li>
      <a href="#openai"><b>Servicio compatible con OpenAI (en la nube)</b></a>: OpenAI u otro servicio que funcione igual.
    </li>
  </ul>
  <div class="note cloud-note">
    <b>☁ Sobre la privacidad.</b> Con un modelo en la nube, tus preguntas y lo que mira el asistente (títulos, etiquetas,
    nombres de carpetas y el texto de los documentos que lee) se mandan a ese servicio. Siempre que se usa un modelo en la
    nube, el panel del chat muestra <span class="cloud">☁</span> y el nombre del modelo debajo de “Preguntale a Rupert”.
    Las sugerencias de la bandeja de entrada y el reconocimiento de texto nunca usan un servicio en la nube.
  </div>
</section>

<!-- MARK: IA local -->
<section id="local-ai">
  <h3 class="sub-h">IA local (en esta computadora)</h3>
  <p>
    La IA local ejecuta un modelo en tu propia computadora, así que nada va a internet. Rupert funciona con cualquier app
    que ofrezca un servidor compatible con OpenAI. Está probado con <a href="https://lmstudio.ai">LM Studio</a> y
    <a href="https://ollama.com">Ollama</a>; otras, como <code>llama-server</code> de llama.cpp o Jan, también deberían
    funcionar. La IA local anda mejor en una Mac con Apple silicon y mucha memoria, o en una PC con una placa de video
    reciente.
  </p>
  {#if mac}
    <p>
      <b>Con Apple Intelligence</b> (una Mac con Apple silicon y macOS 26 o posterior): no hay nada que instalar.
      Activá Apple Intelligence en <b>Configuración del Sistema ▸ Apple Intelligence y Siri</b>, y en Rupert elegí
      <b>Apple Intelligence</b> en <b>Configuración ▸ IA local ▸ Servidor</b> (en una Mac así, Rupert empieza con él). Responde en segundos y nada sale de la
      Mac. Eso sí, es un modelo chico: uno más grande en LM Studio u Ollama archiva y responde mejor.
    </p>
  {/if}
  <p><b>Con LM Studio:</b></p>
  <ol>
    <li>Instalá LM Studio desde <a href="https://lmstudio.ai">lmstudio.ai</a> y abrilo.</li>
    <li>
      Descargá un modelo desde la búsqueda de LM Studio. Para el asistente, elegí uno que admita <b>uso de herramientas</b>
      (LM Studio los marca); por ejemplo un modelo Qwen reciente. Los modelos más grandes responden mejor pero son más
      lentos.
    </li>
    <li>
      Iniciá el servidor local: en la pestaña <b>Developer</b> de LM Studio, encendé el servidor. Cargá tu modelo ahí
      también.
    </li>
  </ol>
  <p><b>Con Ollama:</b></p>
  <ol>
    <li>Instalá Ollama desde <a href="https://ollama.com">ollama.com</a> y abrilo. Queda funcionando en segundo plano.</li>
    <li>
      Descargá un modelo que admita <b>herramientas</b> (la biblioteca de modelos de Ollama los marca), por ejemplo en una
      terminal: <code>ollama pull qwen3:4b</code>.
    </li>
  </ol>
  <p><b>Después, en Rupert:</b></p>
  <ol>
    <li>
      Abrí <b>Configuración ▸ IA local</b>. En <b>Servidor</b>, elegí LM Studio u Ollama, y se completa la
      <b>Dirección</b> (<code>http://localhost:1234</code> o <code>http://localhost:11434</code>); para otro servidor,
      escribí su dirección. Tiene que decir <span class="ok">Conectado</span>.
    </li>
    <li>
      En <b>Modelo</b>, elegí tu modelo. “Automático” toma el primer modelo que lista el servidor: con LM Studio es el
      modelo cargado, pero Ollama lista todos los modelos que descargaste, así que ahí elegí uno.
    </li>
    <li>
      En <b>Configuración ▸ Asistente</b>, elegí <b>IA local</b>. El asistente usa el mismo servidor y el mismo modelo que
      las sugerencias de la bandeja de entrada. Los cambios en Configuración se aplican enseguida; hacé clic en
      <b>Listo</b> cuando termines.
    </li>
  </ol>
  <p class="tip">
    Los modelos locales se toman su tiempo: una pregunta puede tardar uno o dos minutos, casi todo pensando. Mientras
    tanto podés seguir usando Rupert, y frenar la respuesta cuando quieras.
  </p>
  <p class="tip">
    Si las respuestas se cortan o el asistente pierde el hilo de lo que leyó, dale más espacio al modelo (su largo de
    contexto): en la configuración de Ollama, o al cargar el modelo en LM Studio.
  </p>
  <p class="tip">
    El servidor también puede funcionar en otra computadora de tu casa: dejá que sirva en la red local (la opción de LM
    Studio para servir en la red local, o “Expose Ollama to the network” en Ollama), y escribí la dirección de esa
    computadora, por ejemplo <code>http://192.168.1.20:11434</code>. Hacelo solo en una red de confianza: estos servidores
    no tienen contraseña. Configuración te dice adónde va el texto de los documentos. Una dirección fuera de tu red de casa
    cuenta como un servicio en la nube: Preguntale a Rupert muestra <span class="cloud">☁</span>, y las sugerencias de la
    bandeja de entrada no la usan.
  </p>
</section>

<!-- MARK: Claude -->
<section id="claude">
  <h3 class="sub-h">Claude</h3>
  <p>
    Claude, de Anthropic, responde rápido y bien. Es un servicio pago en la nube, que se cobra por uso a través de una
    cuenta de desarrollador de Anthropic (la Claude Console). Esa cuenta es aparte de una suscripción a claude.ai:
    Anthropic no deja que otras apps usen el consumo propio de una suscripción, así que Rupert no puede funcionar
    directamente con tu plan de Claude.
  </p>
  <div class="note" id="claude-credits">
    <b>¿Tenés un plan Max, Ultra o Team?</b> Incluye créditos mensuales para la Claude Console (USD 100 por mes con Max,
    USD 200 con Ultra, hasta USD 500 compartidos con Team), que Rupert puede gastar:
    <ol>
      <li>
        En <a href="https://claude.ai">claude.ai</a>, abrí <b>Settings ▸ Billing</b> (Team: <b>Organization settings ▸
        Billing</b>) y, en <b>API credits</b>, vinculá una organización de la Claude Console. Podés crear una ahí. Elegí
        con cuidado: el vínculo no se puede cambiar después sin el soporte de Anthropic.
      </li>
      <li>Iniciá sesión en Claude más abajo con esa organización (opción 1 o 2).</li>
    </ol>
    Los créditos se usan primero, vencen a fin de cada mes y no se acumulan. Cuando se terminan, las consultas se cortan
    hasta el mes siguiente (salvo que también hayas comprado créditos); nunca se le cobra nada a tu plan. Los planes Free y
    Pro no los incluyen. Los suscriptores nuevos los pueden pedir después de 7 días.
  </div>
  <p>En <b>Configuración ▸ Asistente</b>, elegí <b>Claude (Anthropic, en la nube)</b>. Después elegí cómo iniciar sesión.</p>

  <h4>Opción 1: clave de API</h4>
  <ol>
    <li>
      Iniciá sesión o creá una cuenta en <a href="https://platform.claude.com">platform.claude.com</a>, y cargá créditos
      en Billing.
    </li>
    <li>
      Creá una clave en <a href="https://platform.claude.com/settings/keys">platform.claude.com/settings/keys</a> y
      copiala.
    </li>
    <li>
      En Rupert, elegí <b>Clave de API</b> en Inicio de sesión, pegá la clave y hacé clic en <b>Guardar clave</b>.
    </li>
  </ol>
  <p>
    La clave se guarda en {keychain}, nunca en el archivo de configuración de Rupert. <b>Quitar</b> la borra de ahí.
    Podés revocar la clave en la Claude Console cuando quieras.
  </p>

  <h4>Opción 2: tu cuenta de Anthropic, sin clave</h4>
  <p>
    Rupert puede usar tu cuenta de Anthropic a través de la herramienta de línea de comandos de Anthropic,
    <code>ant</code>, así no manejás ninguna clave.
  </p>
  <ol>
    <li>
      Instalá <code>ant</code>:
      {#if mac}
        en la Terminal, ejecutá <code>brew install anthropics/tap/ant</code> (necesita
        <a href="https://brew.sh">Homebrew</a>).
      {:else if windows}
        descargalo de <a href="https://github.com/anthropics/anthropic-cli/releases">su página de versiones</a> y poné
        <code>ant.exe</code> en una carpeta de tu PATH.
      {:else}
        seguí los pasos de <a href="https://platform.claude.com/docs/en/cli-sdks-libraries/cli/quickstart"
          >la guía rápida de la CLI de Anthropic</a
        >.
      {/if}
    </li>
    <li>
      En Rupert, elegí <b>Cuenta de Anthropic, con el comando <code>ant</code></b> en Inicio de sesión, y hacé clic en
      <b>Iniciar sesión…</b>. Se abre tu navegador; iniciá sesión y aprobá. La línea de la cuenta pasa a decir “Sesión
      iniciada”.
    </li>
  </ol>
  <p>
    El uso se cobra a la organización de la Console que elegiste al iniciar sesión. La sesión dura algunas semanas;
    cuando vence, el asistente lo avisa y <b>Volver a iniciar sesión…</b> la renueva.
  </p>

  <h4>Qué modelo de Claude</h4>
  <p>
    Dejá <b>Modelo de Claude</b> vacío para usar <code>claude-opus-5-5</code>, el más capaz. Para respuestas más rápidas y
    baratas, escribí <code>claude-sonnet-5-5</code> o <code>claude-haiku-5-5</code>.
  </p>
</section>

<!-- MARK: Plan de ChatGPT -->
<section id="chatgpt-plan">
  <h3 class="sub-h">ChatGPT con tu plan</h3>
  <p>
    Si pagás ChatGPT, Rupert puede usar tu plan en vez de una clave de API: iniciás sesión con tu cuenta de ChatGPT, y las
    preguntas de Rupert cuentan para los límites de tu plan. OpenAI lo llama “Sign in with ChatGPT”. Es una vista previa,
    así que OpenAI todavía puede cambiar cómo funciona y qué planes abarca.
  </p>
  <ol>
    <li>En <b>Configuración ▸ Asistente</b>, elegí <b>ChatGPT, con tu plan (en la nube)</b>.</li>
    <li>
      Hacé clic en <b>Iniciar sesión con ChatGPT…</b>. Tu navegador abre la página de inicio de sesión de OpenAI; iniciá
      sesión y permití que Rupert use tu plan de ChatGPT. La página dice entonces que la podés cerrar, y la Configuración
      muestra tu cuenta.
    </li>
    <li>Elegí un <b>Modelo</b>: la lista muestra lo que tu plan ofrece a otras apps.</li>
  </ol>
  <p>
    Rupert guarda la sesión en {keychain}. No ve tus conversaciones de ChatGPT, y nada de lo que manda queda guardado en
    OpenAI para después. En la configuración de ChatGPT podés ver a Rupert entre las apps que usan tu plan, limitar cuánto
    puede usar por semana, o quitarlo. <b>Cerrar sesión</b> en Rupert olvida la sesión en esta computadora.
  </p>
  <p class="tip">
    Cuando se agota la parte de tu plan para Rupert, el asistente lo avisa; vuelve a funcionar cuando se renueva tu
    límite. No se cobra nada aparte de tu plan.
  </p>
</section>

<!-- MARK: Compatibles con OpenAI -->
<section id="openai">
  <h3 class="sub-h">Servicios compatibles con OpenAI</h3>
  <p>
    Muchos servicios de IA aceptan pedidos igual que OpenAI: OpenAI mismo, y otros como OpenRouter, Groq o Mistral. En
    <b>Configuración ▸ Asistente</b>, elegí <b>Servicio compatible con OpenAI (en la nube)</b> y completá:
  </p>
  <dl>
    <dt>Servicio</dt>
    <dd>
      La dirección de la API del servicio. Dejala vacía para OpenAI (<code>https://api.openai.com/v1</code>). Los otros
      publican la suya, por ejemplo <code>https://openrouter.ai/api/v1</code>, <code>https://api.groq.com/openai/v1</code>
      o <code>https://api.mistral.ai/v1</code>.
    </dd>
    <dt>Modelo</dt>
    <dd>
      El nombre del modelo tal como lo muestra el servicio. Elegí uno que admita uso de herramientas (también llamado
      “function calling”).
    </dd>
    <dt>Clave de API</dt>
    <dd>
      Creá una en el sitio del servicio (para OpenAI:
      <a href="https://platform.openai.com/api-keys">platform.openai.com/api-keys</a>), pegala y hacé clic en
      <b>Guardar clave</b>. Se guarda en {keychain}.
    </dd>
  </dl>
  <p>Abrí el chat: su encabezado muestra el modelo con <span class="cloud">☁</span>.</p>
</section>

<!-- MARK: Problemas -->
<section id="ai-trouble">
  <h3 class="sub-h">Cuando el asistente no funciona</h3>
  <table>
    <thead><tr><th>Mensaje o síntoma</th><th>Qué hacer</th></tr></thead>
    <tbody>
      <tr>
        <td>“La IA local no está funcionando, o no tiene ningún modelo.”</td>
        <td>
          Abrí LM Studio u Ollama, iniciá su servidor y fijate que tenga un modelo. Configuración ▸ IA local tiene que decir
          Conectado.
        </td>
      </tr>
      <tr>
        <td>“El modelo se quedó sin espacio antes de responder…”</td>
        <td>Un modelo que razona gastó su espacio pensando. Elegí un modelo que no razone, o que razone menos.</td>
      </tr>
      <tr>
        <td>“Agregá una clave de API de Anthropic…” o “Agregá una clave de API…”</td>
        <td>Pegá una clave en Configuración ▸ Asistente y hacé clic en Guardar clave.</td>
      </tr>
      <tr>
        <td>“El comando ant no está instalado.”</td>
        <td>
          Instalalo (mirá <a href="#claude">Claude</a>) y volvé a abrir la Configuración. Rupert lo busca en tu PATH y en
          las carpetas de instalación de siempre.
        </td>
      </tr>
      <tr>
        <td>“No iniciaste sesión en Anthropic”</td>
        <td>Hacé clic en Iniciar sesión… en Configuración ▸ Asistente.</td>
      </tr>
      <tr>
        <td>“se agotó el límite de tu plan de ChatGPT para Rupert”</td>
        <td>Esperá a que se renueve el límite, o subí el límite de Rupert en la configuración de ChatGPT.</td>
      </tr>
      <tr>
        <td>“esta cuenta o este plan de ChatGPT no se puede usar desde otras apps”</td>
        <td>Tu plan o tu espacio de trabajo no lo permite. Elegí otro modelo en Configuración ▸ Asistente.</td>
      </tr>
      <tr>
        <td>“la sesión de ChatGPT venció”</td>
        <td>Hacé clic en Volver a iniciar sesión… en Configuración ▸ Asistente.</td>
      </tr>
      <tr>
        <td>“no iniciaste sesión o no tenés acceso”</td>
        <td>La clave se revocó o está mal escrita, o venció la sesión. Guardá una clave nueva, o volvé a iniciar sesión.</td>
      </tr>
      <tr>
        <td>Un mensaje sobre el saldo de créditos</td>
        <td>
          Cargá créditos en la cuenta, en el sitio del servicio. Con un plan Max, Ultra o Team de Claude, fijate que sus
          créditos mensuales estén vinculados a la organización con la que iniciaste sesión (mirá
          <a href="#claude-credits">Claude</a>).
        </td>
      </tr>
      <tr>
        <td>“demasiadas solicitudes; probá de nuevo en un rato”</td>
        <td>El servicio está ocupado o llegaste a su límite. Esperá un minuto y volvé a preguntar.</td>
      </tr>
      <tr>
        <td>Responde sin buscar nada, o “No pude terminar eso…”</td>
        <td>Puede que el modelo no maneje bien las herramientas. Probá un modelo más grande, o uno marcado para uso de herramientas.</td>
      </tr>
      <tr>
        <td>Respuestas muy lentas</td>
        <td>Es normal con modelos locales grandes. Elegí un modelo más chico en Configuración ▸ IA local, o un modelo en la nube.</td>
      </tr>
    </tbody>
  </table>
</section>

<!-- MARK: iPhone -->
<section id="iphone">
  <h2>La app del iPhone</h2>
  <p>Rupert en el iPhone muestra la misma biblioteca, así podés buscar cosas lejos de la computadora.</p>
  <ul>
    <li>
      <b>Agregá carpetas</b> de iCloud Drive o En mi iPhone con el selector de Archivos: las mismas carpetas que usa Rupert
      en la computadora. Para <b>OneDrive</b>, en cambio, iniciá sesión con tu cuenta de Microsoft en la app (el selector
      de Archivos no puede compartir carpetas de OneDrive con otras apps). <b>Google Drive</b> funciona igual (como vista
      previa): la app de Google Drive tampoco comparte sus carpetas con otras apps, así que iniciá sesión con tu cuenta de
      Google en <b>Configuración ▸ Google Drive</b>. Google no revisó Rupert, así que su página de inicio de sesión avisa
      que la app no está verificada; tocá <b>Configuración avanzada</b> y después <b>Ir a Rupert</b>.
    </li>
    <li>
      <b>Buscá y recorré</b> títulos, etiquetas y texto. El texto sale de lo que Rupert leyó en tu computadora, así que
      dejá que termine ahí su primera pasada.
    </li>
    <li><b>Abrí</b> un documento para descargarlo y verlo, y después compartirlo o imprimirlo.</li>
    <li>
      <b>Compartí</b> un documento (junto a Abrir) para enviarlo con Mail, Mensajes, AirDrop u otra app. Como en la
      computadora, un PDF va como una copia sin tus notas, salvo que Configuración ▸ Compartir las incluya.
    </li>
    <li>
      <b>Vistas</b>: arriba de una carpeta, cambiá entre lista, íconos grandes y galería (un documento grande y los
      demás en una tira abajo; deslizá para recorrerlos). Rupert recuerda la vista de cada carpeta. Las imágenes vienen
      de OneDrive o Google Drive, que las hacen sin descargar el documento; en carpetas de Archivos, el iPhone las hace
      para los documentos que tiene guardados. Los documentos solo en línea muestran su ícono.
    </li>
    <li>
      <b>Escaneá</b> con la cámara. Elegí color, grises o blanco y negro, y la resolución, en la pestaña Escanear. Los
      escaneos se guardan en la carpeta de bandeja de entrada que elijas, que tendría que ser la bandeja de la
      computadora, así Rupert ahí los puede reconocer y archivar. La <b>Bandeja de entrada</b> está siempre arriba de todo
      en la pestaña Biblioteca, así ves tus escaneos hasta que se archiven, aunque la carpeta de la bandeja no sea una de
      tus carpetas de la biblioteca.
    </li>
    <li>
      <b>Mandá documentos a la bandeja de entrada</b> desde otras apps: en Mail, Archivos, Fotos o Safari, tocá
      Compartir y elegí <b>Rupert</b>. El documento se guarda en la bandeja de entrada en el momento, con su propio
      nombre, sin abrir Rupert. Si en ese momento no se puede guardar (por ejemplo, sin conexión), Rupert lo envía la
      próxima vez que abras la app. Rupert acepta solo documentos: PDF, imágenes, archivos de Office, texto y correos.
      Si Rupert no aparece en la fila de apps, buscalo en <b>Más</b>.
    </li>
    <li><b>Marcá carpetas con la estrella</b> para sumarlas a Favoritos en todos tus dispositivos.</li>
    <li>
      <b>Preguntale a Rupert</b> en la pestaña <b>Preguntar</b>, en un iPhone con Apple Intelligence (iOS 26 o posterior,
      con Apple Intelligence activado en la configuración de iOS). Funciona en el iPhone y nada sale de él. Busca y lee
      como en la computadora; los documentos, carpetas y resultados que encontró aparecen como botones debajo de su respuesta. En otros iPhone la pestaña no
      aparece.
    </li>
    <li>
      <b>Idioma</b>: la app usa el idioma del iPhone, o el que elijas en <b>Configuración ▸ Idioma</b> (o en la
      configuración de iOS para Rupert).
    </li>
  </ul>
  <p>
    Su versión y número de compilación están al final de su <b>Configuración</b>, en <b>Acerca de</b>, junto con un
    enlace a la política de privacidad.
  </p>
  <p>
    La app del iPhone no cambia tus documentos: solo Rupert en la computadora edita detalles y archiva escaneos.
  </p>
</section>

<!-- MARK: Android -->
<section id="android">
  <h2>La app de Android</h2>
  <p>Rupert para Android (en marcha) muestra la misma biblioteca en un teléfono Android, como la app del iPhone.</p>
  <ul>
    <li>
      <b>Agregá carpetas</b> de <b>Google Drive</b> u <b>OneDrive</b>: iniciá sesión en <b>Configuración</b> y después
      agregá las mismas carpetas que usa Rupert en tu computadora. iCloud Drive no existe en Android, y todavía no se
      pueden usar carpetas del propio teléfono.
    </li>
    <li>
      <b>Buscá, recorré y abrí</b> documentos. Al abrir uno, se descarga y se muestra en otra app, como tu visor de PDF.
    </li>
    <li>
      <b>Vistas</b>: lista, íconos grandes o galería, como en el iPhone, con imágenes de Google Drive o OneDrive.
    </li>
    <li>
      <b>Compartí</b> un documento (junto a Abrir) para enviarlo con Gmail, Mensajes u otra app; un PDF va sin tus notas,
      salvo que Configuración ▸ Compartir las incluya.
    </li>
    <li>
      <b>Escaneá</b> con el escáner de documentos de Google: en la pestaña Escanear, elegí la carpeta de la bandeja de
      entrada en Google Drive u OneDrive. Las páginas se guardan ahí como un solo PDF.
    </li>
    <li>
      <b>Mandá documentos a la bandeja de entrada</b> desde otras apps: tocá Compartir y elegí <b>Rupert</b>. Rupert se
      abre y envía el documento a la bandeja de entrada en Google Drive o OneDrive, con su propio nombre.
    </li>
    <li><b>Marcá carpetas con la estrella</b> y elegí el <b>Idioma</b> en Configuración, como en el iPhone.</li>
    <li>
      El gesto de volver cierra lo que esté abierto y después sube una carpeta. Preguntale a Rupert todavía no está en
      Android.
    </li>
  </ul>
  <p>La app de Android no cambia tus documentos: solo Rupert en la computadora edita detalles y archiva escaneos.</p>
</section>

<!-- MARK: Almacenamiento -->
<section id="storage">
  <h2>Dónde guarda Rupert las cosas</h2>
  <ul>
    <li>
      <b>Tus documentos</b> se quedan en tus carpetas. Rupert renombra o mueve un archivo solo cuando se lo pedís: cuando
      lo archivás desde la bandeja de entrada, o lo movés a otra carpeta.
    </li>
    <li>
      <b>Títulos, etiquetas, fechas, notas y favoritos</b> se guardan dentro de los PDF, o en una carpeta oculta
      <code>.rupert</code> al lado de los archivos que no los pueden guardar. Viajan con los archivos a tus otras
      computadoras.
    </li>
    <li>
      <b>El índice de búsqueda</b> es una caché en esta computadora. Rupert lo vuelve a armar a partir de tus archivos
      cuando hace falta, así que no se pierde nada si se borra.
    </li>
    <li>
      <b>El catálogo compartido</b>: en las carpetas de iCloud Drive, OneDrive y Google Drive, una carpeta oculta
      <code>.rupert/catalog</code> guarda el texto que Rupert leyó, para que tus otros dispositivos puedan buscar en
      documentos solo en línea.
    </li>
    <li><b>Las claves de API</b> se guardan en {keychain}.</li>
    <li>
      <b>El idioma</b> no cambia nada en el disco: las carpetas y los archivos que crea Rupert (como <code>.rupert</code>)
      se llaman igual en todos los idiomas, porque los comparten dispositivos que pueden estar en idiomas distintos.
    </li>
  </ul>
</section>

<!-- MARK: Atajos -->
<section id="keys">
  <h2>Atajos de teclado</h2>
  <table class="keys">
    <tbody>
      <tr><td><kbd>{cmd}K</kbd></td><td>Apertura rápida: ir a una carpeta, un documento, una etiqueta o un comando</td></tr>
      <tr><td><kbd>{cmd}F</kbd></td><td>Buscar; en el visor de páginas, buscar en el documento ({cmd}G: siguiente coincidencia)</td></tr>
      <tr><td><kbd>Espacio</kbd></td><td>Ver el documento seleccionado en el visor de páginas; ahí <kbd>↑</kbd> <kbd>↓</kbd> pasan al anterior o al siguiente</td></tr>
      <tr>
        <td><kbd>{mac ? "⌘⌫" : "Supr"}</kbd></td>
        <td>Mandar lo seleccionado a la {trash}</td>
      </tr>
      <tr><td><kbd>{cmd}A</kbd></td><td>Seleccionar todo lo de la carpeta o de los resultados de búsqueda</td></tr>
      <tr><td><kbd>←</kbd> <kbd>→</kbd></td><td>Íconos grandes y galería: documento anterior o siguiente</td></tr>
      <tr><td><kbd>↑</kbd> <kbd>↓</kbd></td><td>Galería: página anterior o siguiente. Lista y resultados de búsqueda: documento anterior o siguiente (con Mayúsculas, también lo selecciona)</td></tr>
      <tr><td><kbd>Esc</kbd></td><td>Borrar la búsqueda; en el chat, cortar la respuesta o cerrar el panel</td></tr>
      <tr><td><kbd>{cmd}J</kbd></td><td>Abrir o cerrar Preguntale a Rupert</td></tr>
      <tr><td><kbd>Enter</kbd></td><td>En el chat: enviar (<kbd>Mayús+Enter</kbd> empieza una línea nueva)</td></tr>
      <tr><td><kbd>{cmd}{mac ? "↩" : "Enter"}</kbd></td><td>En la bandeja de entrada: archivar el documento</td></tr>
      {#if mac}<tr><td><kbd>⌘,</kbd></td><td>Configuración</td></tr>{/if}
      <tr><td><kbd>F1</kbd>{#if mac} o <kbd>⇧⌘/</kbd>{/if}</td><td>Esta ayuda</td></tr>
    </tbody>
  </table>
</section>
<!-- MARK: Acerca de -->
<section id="about">
  <h2>Acerca de Rupert</h2>
  <p>
    Rupert{version ? ` ${version}` : ""}, el bibliotecario de tus documentos escaneados. Libre y de código abierto con la
    <a href="{REPO}/blob/main/LICENSE">licencia MIT</a>, © 2026 Micki Braun.
  </p>
  <ul>
    <li>
      <b>Idioma:</b> Rupert usa el idioma de tu computadora, o el que elijas en <b>Configuración ▸ General</b>. Habla
      inglés, español rioplatense y alemán.
    </li>
    <li>
      <b>Actualizaciones:</b> Rupert busca una versión nueva una vez por día (lo desactivás en
      <b>Configuración ▸ General</b>). Cuando sale una, aparece <b>Actualizar a …</b> en la barra de estado; hacé
      clic para instalarla, y Rupert se reinicia.{#if mac}
        O elegí <b>Rupert ▸ Buscar actualizaciones…</b>.{/if}
    </li>
    <li>
      <b>Pedir ayuda:</b> mencioná la versión y la edición que se ven abajo a la izquierda de la ventana (por ejemplo
      “Rupert 0.3.0 · GitHub”). <b>GitHub</b> quiere decir instalado desde GitHub (se actualiza solo); <b>App Store</b>
      quiere decir la edición de la Mac App Store (la actualiza la App Store).
    </li>
    <li><a href={REPO}>Código fuente, versiones y reportes de problemas</a> en GitHub</li>
    <li>
      <a href="{REPO}/blob/main/PRIVACY.md">Política de privacidad</a> (en inglés): Rupert no recopila nada sobre vos;
      ahí están las funciones opcionales en la nube con exactamente lo que mandan.
    </li>
    <li>
      <a href="{REPO}/blob/main/THIRD_PARTY_NOTICES.md">Avisos de terceros</a> (en inglés): los componentes de código
      abierto con los que está hecho Rupert, y sus licencias (también incluidos con la app).
    </li>
  </ul>
</section>
