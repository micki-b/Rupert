// Rupert's phone services on Android (the Rust side is mobile/plugin/src/lib.rs):
// Google's document scanner, web sign-in in a Custom Tab, Google's own Drive
// authorization, a Keystore-protected keychain, and opening documents in
// another app, and documents shared from other apps (kept for Rust to send
// to the inbox). The Files-picker calls are iOS-only for now.

package io.github.micki_b.rupert.phone

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.ClipData
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Canvas
import android.graphics.ColorMatrix
import android.graphics.ColorMatrixColorFilter
import android.graphics.Paint
import android.net.Uri
import android.provider.OpenableColumns
import android.util.Log
import android.os.Handler
import android.os.Looper
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.MimeTypeMap
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.activity.result.ActivityResult
import androidx.activity.result.IntentSenderRequest
import androidx.browser.customtabs.CustomTabsIntent
import androidx.core.content.FileProvider
import androidx.core.content.IntentCompat
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.common.api.ApiException
import com.google.android.gms.common.api.CommonStatusCodes
import com.google.android.gms.common.api.Scope
import com.google.mlkit.vision.documentscanner.GmsDocumentScannerOptions
import com.google.mlkit.vision.documentscanner.GmsDocumentScanning
import com.google.mlkit.vision.documentscanner.GmsDocumentScanningResult
import java.io.File
import org.json.JSONObject
import java.io.FileOutputStream
import java.security.KeyStore
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import kotlin.math.max
import kotlin.math.roundToInt

@InvokeArg
class ShareArgs {
  lateinit var path: String
  var subject: String = ""
}

@InvokeArg
class ScanArgs {
  var mode: String = "gray"
  var dpi: Int = 300
}

@InvokeArg
class SignInArgs {
  lateinit var url: String
  lateinit var callbackScheme: String
}

@InvokeArg
class TokenArgs {
  lateinit var scope: String
}

@InvokeArg
class UrlArgs {
  lateinit var url: String
}

@InvokeArg
class KeyArgs {
  lateinit var key: String
  var value: String? = null
}

@InvokeArg
class PathArgs {
  lateinit var path: String
}

@TauriPlugin
class RupertPlugin(private val activity: Activity) : Plugin(activity) {

  // MARK: - Screen edges

  private var webView: WebView? = null
  private var navInset = 0f

  /** Android's web view doesn't report the navigation bar (gesture handle or
   * buttons) as a safe area, so the page gets it as `--nav-inset` (CSS px). */
  override fun load(webView: WebView) {
    this.webView = webView
    // Read the insets without taking over their handling (the web view uses
    // it for the status bar): on layout changes, and once the page is there.
    webView.addOnLayoutChangeListener { _, _, _, _, _, _, _, _, _ -> updateNavInset() }
    Handler(Looper.getMainLooper()).postDelayed({ updateNavInset() }, 1500)
    // Opened by sharing a document with Rupert, or by the scan link.
    takeShared(activity.intent)
    takeScanLink(activity.intent)
  }

  private fun updateNavInset() {
    val view = webView ?: return
    val insets = ViewCompat.getRootWindowInsets(view) ?: return
    val bottom = insets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom
    navInset = bottom / activity.resources.displayMetrics.density
    applyNavInset()
  }

  private fun applyNavInset() {
    webView?.evaluateJavascript(
      "document.documentElement.style.setProperty('--nav-inset', '${navInset}px')", null)
  }

  // MARK: - Scanning

  private var scanMode = "gray"
  private var scanDpi = 300

  /** Google's document scanner; answers `{pages: [paths]}` (empty if cancelled). */
  @Command
  fun scan(invoke: Invoke) {
    val args = invoke.parseArgs(ScanArgs::class.java)
    scanMode = args.mode
    scanDpi = args.dpi
    val options = GmsDocumentScannerOptions.Builder()
      .setGalleryImportAllowed(true)
      .setResultFormats(GmsDocumentScannerOptions.RESULT_FORMAT_JPEG)
      .setScannerMode(GmsDocumentScannerOptions.SCANNER_MODE_FULL)
      .build()
    GmsDocumentScanning.getClient(options).getStartScanIntent(activity)
      .addOnSuccessListener { sender ->
        startIntentSenderForResult(invoke, IntentSenderRequest.Builder(sender).build(), "scanDone")
      }
      .addOnFailureListener { invoke.reject(it.message ?: "The scanner isn't available on this phone.") }
  }

  @ActivityCallback
  fun scanDone(invoke: Invoke, result: ActivityResult) {
    val pages = JSArray()
    val scan = GmsDocumentScanningResult.fromActivityResultIntent(result.data)
    if (result.resultCode != Activity.RESULT_OK || scan == null) {
      invoke.resolve(JSObject().put("pages", pages))
      return
    }
    // Pages go to a folder of their own; Rust removes it when done.
    Thread {
      try {
        val dir = File(activity.cacheDir, "scan-${System.currentTimeMillis()}").apply { mkdirs() }
        scan.pages?.forEachIndexed { i, page -> pages.put(savePage(page.imageUri, dir, i).absolutePath) }
        invoke.resolve(JSObject().put("pages", pages))
      } catch (e: Exception) {
        invoke.reject(e.message ?: "Couldn't save the scan.")
      }
    }.start()
  }

  /** One page at the chosen resolution (the long side as 11 inches, never
   * enlarged): color or grayscale as JPEG, black and white as a PNG. */
  private fun savePage(uri: Uri, dir: File, index: Int): File {
    val source = activity.contentResolver.openInputStream(uri).use { BitmapFactory.decodeStream(it) }
      ?: throw IllegalStateException("Couldn't read a scanned page.")
    val longSide = (scanDpi * 11.0).roundToInt()
    val scale = minOf(1.0, longSide.toDouble() / max(source.width, source.height))
    val w = max(1, (source.width * scale).roundToInt())
    val h = max(1, (source.height * scale).roundToInt())
    val page = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888)
    val paint = Paint(Paint.FILTER_BITMAP_FLAG)
    if (scanMode != "color") {
      paint.colorFilter = ColorMatrixColorFilter(ColorMatrix().apply { setSaturation(0f) })
    }
    Canvas(page).drawBitmap(Bitmap.createScaledBitmap(source, w, h, true), 0f, 0f, paint)
    if (scanMode == "bw") {
      val px = IntArray(w * h)
      page.getPixels(px, 0, w, 0, 0, w, h)
      for (i in px.indices) {
        val gray = px[i] and 0xff
        px[i] = if (gray < 128) 0xff000000.toInt() else 0xffffffff.toInt()
      }
      page.setPixels(px, 0, w, 0, 0, w, h)
      val file = File(dir, "page-${index + 1}.png")
      FileOutputStream(file).use { page.compress(Bitmap.CompressFormat.PNG, 100, it) }
      return file
    }
    val file = File(dir, "page-${index + 1}.jpg")
    FileOutputStream(file).use { page.compress(Bitmap.CompressFormat.JPEG, 85, it) }
    return file
  }

  // MARK: - Sign-in

  private var signIn: Invoke? = null
  private var signInScheme: String? = null
  private var leftForBrowser = false

  /** Shows a sign-in page in a Custom Tab; answers `{url}` with the redirect,
   * or `{url: null}` when the person comes back without signing in. */
  @Command
  fun signIn(invoke: Invoke) {
    val args = invoke.parseArgs(SignInArgs::class.java)
    signIn?.resolve(JSObject().put("url", JSONObject.NULL))
    signIn = invoke
    signInScheme = args.callbackScheme.lowercase()
    leftForBrowser = false
    try {
      CustomTabsIntent.Builder().build().launchUrl(activity, Uri.parse(args.url))
    } catch (e: ActivityNotFoundException) {
      signIn = null
      invoke.reject("There's no web browser to sign in with.")
    }
  }

  override fun onNewIntent(intent: Intent) {
    takeShared(intent)
    takeScanLink(intent)
    val data = intent.data ?: return
    val pending = signIn ?: return
    if (data.scheme?.lowercase() == signInScheme) {
      signIn = null
      pending.resolve(JSObject().put("url", data.toString()))
    }
  }

  override fun onPause() {
    if (signIn != null) leftForBrowser = true
  }

  override fun onResume() {
    updateNavInset()
    // Back from the browser: the redirect arrives just before this, through
    // onNewIntent. If it hasn't come in a moment, the sign-in was cancelled.
    if (signIn == null || !leftForBrowser) return
    Handler(Looper.getMainLooper()).postDelayed({
      signIn?.let {
        signIn = null
        it.resolve(JSObject().put("url", JSONObject.NULL))
      }
    }, 800)
  }

  // MARK: - The scan link

  /** Opened by the desktop app's Scan QR code (#124): an App Link to
   * https://rupertlibrarian.com/scan/, or rupert://scan from that page. */
  @Volatile private var scanLink = false

  private fun takeScanLink(intent: Intent?) {
    if (intent?.action != Intent.ACTION_VIEW) return
    val data = intent.data ?: return
    val scheme = data.scheme?.lowercase()
    val host = data.host?.lowercase()
    val path = data.path ?: ""
    val isScan = (scheme == "https" && host == "rupertlibrarian.com" && path.trimEnd('/') == "/scan") ||
      (scheme == "rupert" && host == "scan")
    if (!isScan) return
    // Handled once, even if the activity is created again.
    intent.action = null
    scanLink = true
  }

  /** Whether the scan link opened Rupert since the last time it was asked. */
  @Command
  fun takeScanLink(invoke: Invoke) {
    val was = scanLink
    scanLink = false
    invoke.resolve(JSObject().put("scan", was))
  }

  // MARK: - Sharing from other apps

  /** Documents shared with Rupert wait here (`<id>/<name>`) until Rust sends
   * them to the inbox (mobile/src/share.rs). */
  private val sharedDir: File get() = File(activity.cacheDir, "shared")
  private val sharedLock = Object()
  /** Copies under way; `sharedFolders` waits for them. */
  private var copying = 0

  /** Keeps what a share intent brought, copying it on a background thread
   * (the sending app's permission to read it lasts while Rupert is open). */
  private fun takeShared(intent: Intent?) {
    if (intent == null) return
    val uris: List<Uri> = when (intent.action) {
      Intent.ACTION_SEND ->
        listOfNotNull(IntentCompat.getParcelableExtra(intent, Intent.EXTRA_STREAM, Uri::class.java))
      Intent.ACTION_SEND_MULTIPLE ->
        IntentCompat.getParcelableArrayListExtra(intent, Intent.EXTRA_STREAM, Uri::class.java)
          ?: emptyList()
      else -> return
    }
    // Handled once, even if the activity is created again.
    intent.action = null
    if (uris.isEmpty()) return
    synchronized(sharedLock) { copying++ }
    Thread {
      try {
        for (uri in uris) keepShared(uri)
      } finally {
        synchronized(sharedLock) {
          copying--
          sharedLock.notifyAll()
        }
      }
    }.start()
  }

  /** Copies one shared document into its own folder, keeping its name. */
  private fun keepShared(uri: Uri) {
    val resolver = activity.contentResolver
    var name: String? = null
    try {
      resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
        if (c.moveToFirst()) name = c.getString(0)
      }
    } catch (e: Exception) {
      // Some apps don't answer; the name then comes from the address.
    }
    var fileName = (name ?: uri.lastPathSegment ?: "Shared").substringAfterLast('/')
    if (!fileName.contains('.')) {
      val ext = resolver.getType(uri)?.let { MimeTypeMap.getSingleton().getExtensionFromMimeType(it) }
      if (ext != null) fileName += ".$ext"
    }
    val dir = File(sharedDir, UUID.randomUUID().toString())
    try {
      dir.mkdirs()
      resolver.openInputStream(uri)?.use { input ->
        FileOutputStream(File(dir, fileName)).use { input.copyTo(it) }
      }
    } catch (e: Exception) {
      Log.w("Rupert", "couldn't keep a shared document: $e")
      dir.deleteRecursively()
    }
  }

  /** Where shared documents wait: `{pending}`, once any copy under way is done. */
  @Command
  fun sharedFolders(invoke: Invoke) {
    Thread {
      synchronized(sharedLock) {
        val until = System.currentTimeMillis() + 120_000
        while (copying > 0 && System.currentTimeMillis() < until) sharedLock.wait(1_000)
      }
      sharedDir.mkdirs()
      invoke.resolve(JSObject().put("pending", sharedDir.path).put("settings", JSONObject.NULL))
    }.start()
  }

  // MARK: - Google Drive

  /** An access token for `scope` from Google Play services: `{accessToken}`,
   * null if the person declined, or `{refused: {code, status}}` when Google
   * refused (an unregistered signing key, no network…), for Rupert to
   * explain (#53). Asks only the first time. */
  @Command
  fun googleToken(invoke: Invoke) {
    val args = invoke.parseArgs(TokenArgs::class.java)
    val request = AuthorizationRequest.builder().setRequestedScopes(listOf(Scope(args.scope))).build()
    Identity.getAuthorizationClient(activity).authorize(request)
      .addOnSuccessListener { result ->
        val pending = result.pendingIntent
        if (result.hasResolution() && pending != null) {
          startIntentSenderForResult(invoke, IntentSenderRequest.Builder(pending.intentSender).build(), "tokenDone")
        } else {
          invoke.resolve(JSObject().put("accessToken", result.accessToken))
        }
      }
      .addOnFailureListener { invoke.resolve(refused(it)) }
  }

  /** Google's sheet closed. Not RESULT_OK means declined, or refused by
   * Google: the result's data says which. */
  @ActivityCallback
  fun tokenDone(invoke: Invoke, result: ActivityResult) {
    val data = result.data
    if (result.resultCode != Activity.RESULT_OK && data == null) {
      invoke.resolve(JSObject().put("accessToken", JSONObject.NULL))
      return
    }
    try {
      val auth = Identity.getAuthorizationClient(activity).getAuthorizationResultFromIntent(data)
      invoke.resolve(JSObject().put("accessToken", auth.accessToken ?: JSONObject.NULL))
    } catch (e: ApiException) {
      // 12501: Google Sign-In's own "cancelled".
      if (e.statusCode == CommonStatusCodes.CANCELED || e.statusCode == 12501) {
        invoke.resolve(JSObject().put("accessToken", JSONObject.NULL))
      } else {
        invoke.resolve(refused(e))
      }
    } catch (e: Exception) {
      invoke.resolve(refused(e))
    }
  }

  private fun refused(e: Exception): JSObject {
    Log.w("Rupert", "Google sign-in refused", e)
    val code = (e as? ApiException)?.statusCode ?: CommonStatusCodes.INTERNAL_ERROR
    val status = (e as? ApiException)?.status?.statusMessage
      ?: e.message
      ?: CommonStatusCodes.getStatusCodeString(code)
    return JSObject()
      .put("accessToken", JSONObject.NULL)
      .put("refused", JSObject().put("code", code).put("status", status))
  }

  // MARK: - Keychain

  private val prefs by lazy { activity.getSharedPreferences("rupert-keychain", Context.MODE_PRIVATE) }

  /** An AES key that never leaves the Android Keystore. */
  private fun secretKey(): SecretKey {
    val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    (store.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
    val spec = KeyGenParameterSpec.Builder(KEY_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
      .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
      .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
      .build()
    return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply { init(spec) }.generateKey()
  }

  @Command
  fun keychainGet(invoke: Invoke) {
    val args = invoke.parseArgs(KeyArgs::class.java)
    val stored = prefs.getString(args.key, null)
    val value = stored?.let {
      try {
        val (iv, data) = it.split(":").map { part -> Base64.decode(part, Base64.NO_WRAP) }
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, secretKey(), GCMParameterSpec(128, iv))
        String(cipher.doFinal(data), Charsets.UTF_8)
      } catch (e: Exception) {
        null // e.g. restored from a backup onto another phone: sign in again
      }
    }
    invoke.resolve(JSObject().put("value", value))
  }

  @Command
  fun keychainSet(invoke: Invoke) {
    val args = invoke.parseArgs(KeyArgs::class.java)
    try {
      val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.ENCRYPT_MODE, secretKey())
      val data = cipher.doFinal((args.value ?: "").toByteArray(Charsets.UTF_8))
      val stored = Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" + Base64.encodeToString(data, Base64.NO_WRAP)
      prefs.edit().putString(args.key, stored).apply()
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject(e.message ?: "Couldn't save to the keychain.")
    }
  }

  @Command
  fun keychainDelete(invoke: Invoke) {
    val args = invoke.parseArgs(KeyArgs::class.java)
    prefs.edit().remove(args.key).apply()
    invoke.resolve()
  }

  // MARK: - Opening documents

  /** Opens a downloaded document (in the app's cache) in another app. */
  @Command
  fun preview(invoke: Invoke) {
    val args = invoke.parseArgs(PathArgs::class.java)
    val file = File(args.path)
    try {
      val uri = FileProvider.getUriForFile(activity, activity.packageName + ".fileprovider", file)
      val mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase())
        ?: "application/octet-stream"
      val view = Intent(Intent.ACTION_VIEW).setDataAndType(uri, mime).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      activity.startActivity(Intent.createChooser(view, null))
      invoke.resolve()
    } catch (e: ActivityNotFoundException) {
      invoke.reject("No app on this phone can open this file.")
    } catch (e: Exception) {
      invoke.reject(e.message ?: "Couldn't open the file.")
    }
  }

  /** The share chooser for a (downloaded) document, in the app's cache (#69). */
  @Command
  fun share(invoke: Invoke) {
    val args = invoke.parseArgs(ShareArgs::class.java)
    val file = File(args.path)
    try {
      val uri = FileProvider.getUriForFile(activity, activity.packageName + ".fileprovider", file)
      val mime = MimeTypeMap.getSingleton().getMimeTypeFromExtension(file.extension.lowercase())
        ?: "application/octet-stream"
      val send = Intent(Intent.ACTION_SEND)
        .setType(mime)
        .putExtra(Intent.EXTRA_STREAM, uri)
        .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      if (args.subject.isNotEmpty()) send.putExtra(Intent.EXTRA_SUBJECT, args.subject)
      // The grant reaches the app chosen in the chooser through the clip.
      send.clipData = ClipData.newRawUri(file.name, uri)
      activity.startActivity(Intent.createChooser(send, null))
      invoke.resolve()
    } catch (e: ActivityNotFoundException) {
      invoke.reject("No app on this phone can take this file.")
    } catch (e: Exception) {
      invoke.reject(e.message ?: "Couldn't share the file.")
    }
  }

  /** Opens a web page (Rupert's privacy policy) in the phone's browser. */
  @Command
  fun openUrl(invoke: Invoke) {
    val args = invoke.parseArgs(UrlArgs::class.java)
    try {
      activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(args.url)))
      invoke.resolve()
    } catch (e: ActivityNotFoundException) {
      invoke.reject("No browser on this phone can open ${args.url}.")
    }
  }

  // MARK: - iOS only for now

  @Command fun pickFolder(invoke: Invoke) = notYet(invoke)
  @Command fun resolveBookmarks(invoke: Invoke) = notYet(invoke)
  @Command fun fetchFolder(invoke: Invoke) = notYet(invoke)
  @Command fun download(invoke: Invoke) = notYet(invoke)
  @Command fun place(invoke: Invoke) = notYet(invoke)
  @Command fun writeFile(invoke: Invoke) = notYet(invoke)
  @Command fun removeFile(invoke: Invoke) = notYet(invoke)
  @Command fun thumbnail(invoke: Invoke) = notYet(invoke)

  private fun notYet(invoke: Invoke) =
    invoke.reject("Folders on the phone aren't available on Android yet; use Google Drive or OneDrive.")

  private companion object {
    const val KEY_ALIAS = "rupert-keychain"
  }
}
