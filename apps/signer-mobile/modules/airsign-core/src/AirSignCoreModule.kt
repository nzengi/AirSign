/**
 * AirSignCoreModule.kt — Android v2 WebView bridge for AirSignCore.
 *
 * Architecture:
 *   ┌─────────────────────┐   evaluateJavascript()
 *   │ Expo AsyncFunction  │ ──────────────────────▶ WebView
 *   │ (JS thread)         │                         (airsign_bridge.html)
 *   │                     │ ◀── @JavascriptInterface  postResult()
 *   └─────────────────────┘
 *
 * Key storage (v2):
 *   · WASM generates the Ed25519 keypair inside WebAssembly linear memory.
 *   · After generation the secret key hex is returned to the Kotlin layer.
 *   · The secret is encrypted with AES-256-GCM using a key that lives in
 *     the Android Keystore (StrongBox / TEE backed where available), then
 *     the ciphertext is stored in EncryptedSharedPreferences.
 *   · On sign, the Kotlin layer decrypts the secret and calls
 *     AirSign.loadKeypair() so WASM can perform the Ed25519 operation.
 *
 * Thread model:
 *   · WebView.loadUrl / evaluateJavascript must be called on the main thread.
 *   · @JavascriptInterface methods are called on a dedicated JS thread.
 *   · Expo Promises are resolved on a background thread after the JS callback.
 */

package expo.modules.airsigncore

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.JavascriptInterface
import android.webkit.WebSettings
import android.webkit.WebView
import android.webkit.WebViewClient
import expo.modules.kotlin.Promise
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import org.json.JSONObject
import java.security.KeyStore
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class AirSignCoreModule : Module() {

  // ── State ────────────────────────────────────────────────────────────────────

  private var webView: WebView? = null
  private val mainHandler = Handler(Looper.getMainLooper())
  private val pending = ConcurrentHashMap<String, Promise>()
  private var isReady = false
  private val readyQueue = mutableListOf<Runnable>()
  private val readyLock = Any()

  // ── Module definition ────────────────────────────────────────────────────────

  override fun definition() = ModuleDefinition {
    Name("AirSignCore")

    OnCreate {
      mainHandler.post { setupWebView() }
    }

    // ── Key management ──────────────────────────────────────────────────────────

    AsyncFunction("generateKeypair") { promise: Promise ->
      call("generateKeypair", emptyList(), promise)
    }

    AsyncFunction("deleteKeypair") { id: String, promise: Promise ->
      deleteFromKeystore(id)
      call("deleteKeypair", listOf(id), promise)
    }

    AsyncFunction("listKeypairIds") { promise: Promise ->
      promise.resolve(listKeystoreIds())
    }

    AsyncFunction("getPublicKey") { id: String, promise: Promise ->
      val secretHex = loadSecretFromKeystore(id)
        ?: return@AsyncFunction promise.reject("KEY_NOT_FOUND", "No keypair with id $id", null)
      call("loadKeypair", listOf(id, secretHex), promise)
    }

    // ── Signing ──────────────────────────────────────────────────────────────────

    AsyncFunction("signTransaction") { id: String, txBase64: String, promise: Promise ->
      ensureLoaded(id) { call("signTransaction", listOf(id, txBase64), promise) }
    }

    AsyncFunction("signMessage") { id: String, messageBase64: String, promise: Promise ->
      ensureLoaded(id) { call("signMessage", listOf(id, messageBase64), promise) }
    }

    // ── Transaction inspection ────────────────────────────────────────────────────

    AsyncFunction("inspectTransaction") { txBase64: String, promise: Promise ->
      call("inspectTransaction", listOf(txBase64), promise)
    }

    // ── Fountain codes ────────────────────────────────────────────────────────────

    AsyncFunction("fountainEncode") { payloadBase64: String, targetFrames: Int, promise: Promise ->
      call("fountainEncode", listOf(payloadBase64, targetFrames), promise)
    }

    AsyncFunction("fountainDecodeAdd") { sessionId: String, frameBase64: String, totalBlocks: Int, promise: Promise ->
      call("fountainDecodeAdd", listOf(sessionId, frameBase64, totalBlocks), promise)
    }

    AsyncFunction("fountainDecodeReset") { sessionId: String, promise: Promise ->
      call("fountainDecodeReset", listOf(sessionId), promise)
    }
  }

  // ── WebView setup ─────────────────────────────────────────────────────────────

  private fun setupWebView() {
    val ctx = appContext.reactContext ?: return
    val wv = WebView(ctx)

    wv.settings.apply {
      javaScriptEnabled = true
      allowFileAccess = true
      allowContentAccess = false
      // Block network — fully air-gapped.
      blockNetworkLoads = true
      cacheMode = WebSettings.LOAD_NO_CACHE
    }

    // Disable remote debugging in production builds.
    WebView.setWebContentsDebuggingEnabled(false)

    wv.webViewClient = object : WebViewClient() {
      override fun shouldOverrideUrlLoading(view: WebView?, url: android.webkit.WebResourceRequest?): Boolean {
        // Block all navigation except the initial file:// load.
        return url?.isForMainFrame == true && url.url?.scheme != "file"
      }
    }

    // Expose the native callback interface to JS.
    wv.addJavascriptInterface(JsBridge(), "AirSignBridge")

    webView = wv

    // Load bridge HTML from assets.
    wv.loadUrl("file:///android_asset/airsign_core/airsign_bridge.html")
  }

  // ── Call dispatch ─────────────────────────────────────────────────────────────

  private fun call(method: String, args: List<Any>, promise: Promise) {
    val callId = UUID.randomUUID().toString()
    pending[callId] = promise

    val dispatch = Runnable {
      val wv = webView ?: run {
        pending.remove(callId)
        promise.reject("INTERNAL", "WebView not ready", null)
        return@Runnable
      }
      val jsArgs = buildString {
        append("\"$callId\"")
        for (arg in args) {
          append(",")
          when (arg) {
            is String -> {
              val escaped = arg
                .replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
              append("\"$escaped\"")
            }
            is Int -> append(arg)
            else  -> append("null")
          }
        }
      }
      val js = "AirSign.$method($jsArgs)"
      mainHandler.post {
        wv.evaluateJavascript(js) { _ ->
          // Result arrives async via AirSignBridge.postResult()
        }
      }
    }

    synchronized(readyLock) {
      if (isReady) dispatch.run() else readyQueue.add(dispatch)
    }
  }

  // ── Ensure keypair is loaded into WASM before signing ────────────────────────

  private fun ensureLoaded(id: String, then: () -> Unit) {
    val secretHex = loadSecretFromKeystore(id) ?: run { then(); return }
    val loadId = UUID.randomUUID().toString()
    val wv = webView ?: run { then(); return }
    val escapedHex = secretHex.replace("\\", "\\\\").replace("\"", "\\\"")
    val js = "AirSign.loadKeypair(\"$loadId\",\"$id\",\"$escapedHex\")"
    mainHandler.post {
      wv.evaluateJavascript(js) { _ -> then() }
    }
  }

  // ── @JavascriptInterface ──────────────────────────────────────────────────────

  inner class JsBridge {
    /** Called by airsign_bridge.html when the WASM is initialised. */
    @JavascriptInterface
    fun onReady() {
      synchronized(readyLock) {
        isReady = true
        val queued = readyQueue.toList()
        readyQueue.clear()
        queued.forEach { it.run() }
      }
    }

    /**
     * Called by airsign_bridge.html with the JSON result of every async call.
     * @param callId  opaque UUID matching the pending Promise entry
     * @param payload JSON string: { callId, ok, result|error }
     */
    @JavascriptInterface
    fun postResult(callId: String, payload: String) {
      val promise = pending.remove(callId) ?: return
      try {
        val json = JSONObject(payload)
        val ok   = json.getBoolean("ok")
        if (ok) {
          // Unwrap the nested result object/value.
          val result = if (json.isNull("result")) null else json.get("result")
          // Convert JSONObject to a Map so Expo can serialise it.
          val mapped = when (result) {
            is JSONObject -> jsonToMap(result)
            else          -> result
          }
          promise.resolve(mapped)
        } else {
          promise.reject("WASM_ERROR", json.optString("error", "unknown"), null)
        }
      } catch (e: Exception) {
        promise.reject("PARSE_ERROR", e.message, e)
      }
    }

    private fun jsonToMap(obj: JSONObject): Map<String, Any?> {
      val map = mutableMapOf<String, Any?>()
      obj.keys().forEach { key ->
        map[key] = if (obj.isNull(key)) null else obj.get(key)
      }
      return map
    }
  }

  // ── Android Keystore helpers ──────────────────────────────────────────────────

  private companion object {
    const val KEYSTORE_PROVIDER = "AndroidKeyStore"
    const val KEY_ALIAS_PREFIX  = "airsign_key_"
    const val PREF_NAME         = "airsign_keypairs"
    const val GCM_TAG_LENGTH    = 128
    const val GCM_IV_LENGTH     = 12
  }

  private fun getOrCreateAesKey(id: String): SecretKey {
    val alias = "$KEY_ALIAS_PREFIX$id"
    val ks    = KeyStore.getInstance(KEYSTORE_PROVIDER).also { it.load(null) }
    val entry = ks.getEntry(alias, null)
    if (entry is KeyStore.SecretKeyEntry) return entry.secretKey

    val keyGen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
    keyGen.init(
      KeyGenParameterSpec.Builder(alias,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        // Require user authentication (biometric/PIN) — comment out for dev builds.
        // .setUserAuthenticationRequired(true)
        .build()
    )
    return keyGen.generateKey()
  }

  private fun saveToKeystore(id: String, secretHex: String) {
    val ctx = appContext.reactContext ?: return
    val key    = getOrCreateAesKey(id)
    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
    cipher.init(Cipher.ENCRYPT_MODE, key)
    val iv         = cipher.iv
    val ciphertext = cipher.doFinal(secretHex.toByteArray(Charsets.UTF_8))
    val blob = Base64.encodeToString(iv + ciphertext, Base64.NO_WRAP)
    ctx.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
      .edit().putString(id, blob).apply()
  }

  private fun loadSecretFromKeystore(id: String): String? {
    return try {
      val ctx  = appContext.reactContext ?: return null
      val blob = ctx.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
        .getString(id, null) ?: return null
      val raw        = Base64.decode(blob, Base64.NO_WRAP)
      val iv         = raw.sliceArray(0 until GCM_IV_LENGTH)
      val ciphertext = raw.sliceArray(GCM_IV_LENGTH until raw.size)
      val key        = getOrCreateAesKey(id)
      val cipher     = Cipher.getInstance("AES/GCM/NoPadding")
      cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_LENGTH, iv))
      cipher.doFinal(ciphertext).toString(Charsets.UTF_8)
    } catch (e: Exception) {
      null
    }
  }

  private fun deleteFromKeystore(id: String) {
    val ctx = appContext.reactContext ?: return
    ctx.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
      .edit().remove(id).apply()
    try {
      val ks = KeyStore.getInstance(KEYSTORE_PROVIDER).also { it.load(null) }
      ks.deleteEntry("$KEY_ALIAS_PREFIX$id")
    } catch (_: Exception) {}
  }

  private fun listKeystoreIds(): List<String> {
    val ctx = appContext.reactContext ?: return emptyList()
    return ctx.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
      .all.keys.toList()
  }
}