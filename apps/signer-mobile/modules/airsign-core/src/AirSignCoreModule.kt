/**
 * AirSignCoreModule.kt — Android v2.1 WebView bridge for AirSignCore.
 *
 * v2.1 changes vs v2:
 *   · JsBridge.saveSecret(id, secretHex) @JavascriptInterface — called by the
 *     bridge HTML immediately after generating a keypair, before the Promise
 *     resolves. The secret is persisted to Android Keystore + SharedPreferences
 *     without ever appearing in a Promise result on the RN side.
 *   · onReady(): after WASM init, all stored {id, secretHex} pairs are read
 *     from the Keystore and passed to AirSign.restoreKeypairs() so the WASM
 *     memory cache is warm before any sign call arrives (no per-call loadKeypair
 *     round-trip required after app restart).
 *
 * Architecture:
 *   ┌─────────────────────┐   evaluateJavascript()
 *   │ Expo AsyncFunction  │ ──────────────────────▶ WebView
 *   │ (JS thread)         │                         (airsign_bridge.html)
 *   │                     │ ◀── @JavascriptInterface callbacks
 *   └─────────────────────┘
 *
 * @JavascriptInterface callbacks (JS → native):
 *   onReady()                    → isReady = true; drain queue; restoreKeypairs
 *   saveSecret(id, secretHex)    → saveToKeystore(id, secretHex)
 *   postResult(callId, payload)  → resolve/reject pending Promise
 *
 * Key storage:
 *   Android Keystore AES-256-GCM key per keypair id (TEE / StrongBox).
 *   Ciphertext stored in SharedPreferences.
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
import org.json.JSONArray
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
      // The bridge HTML calls AirSignBridge.saveSecret() automatically before
      // posting the result, so we just forward {id, pubkeyHex, pubkeyBase58}.
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
    // Cache is warm after restoreKeypairs on startup; ensureLoaded is a safety
    // net for the rare case where the WebView was recreated mid-session.

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
    val wv  = WebView(ctx)

    wv.settings.apply {
      javaScriptEnabled    = true
      allowFileAccess      = true
      allowContentAccess   = false
      blockNetworkLoads    = true
      cacheMode            = WebSettings.LOAD_NO_CACHE
    }
    WebView.setWebContentsDebuggingEnabled(false)

    wv.webViewClient = object : WebViewClient() {
      override fun shouldOverrideUrlLoading(
        view: WebView?,
        request: android.webkit.WebResourceRequest?
      ): Boolean = request?.isForMainFrame == true && request.url?.scheme != "file"
    }

    wv.addJavascriptInterface(JsBridge(), "AirSignBridge")
    webView = wv
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
              val esc = arg
                .replace("\\", "\\\\")
                .replace("\"", "\\\"")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
              append("\"$esc\"")
            }
            is Int  -> append(arg)
            else    -> append("null")
          }
        }
      }
      mainHandler.post { wv.evaluateJavascript("AirSign.$method($jsArgs)", null) }
    }

    synchronized(readyLock) {
      if (isReady) dispatch.run() else readyQueue.add(dispatch)
    }
  }

  // ── Ensure keypair warm (safety net for cold cache) ───────────────────────────

  private fun ensureLoaded(id: String, then: () -> Unit) {
    val secretHex  = loadSecretFromKeystore(id) ?: run { then(); return }
    val wv         = webView                    ?: run { then(); return }
    val loadId     = UUID.randomUUID().toString()
    val escapedHex = secretHex.replace("\\", "\\\\").replace("\"", "\\\"")
    val js         = "AirSign.loadKeypair(\"$loadId\",\"$id\",\"$escapedHex\")"
    mainHandler.post { wv.evaluateJavascript(js) { _ -> then() } }
  }

  // ── @JavascriptInterface ──────────────────────────────────────────────────────

  inner class JsBridge {

    /**
     * Called by airsign_bridge.html when WASM is initialised.
     * Drains the pending call queue and bulk-restores all stored keypairs
     * into the WASM memory cache via AirSign.restoreKeypairs().
     */
    @JavascriptInterface
    fun onReady() {
      synchronized(readyLock) {
        isReady = true
        val queued = readyQueue.toList()
        readyQueue.clear()
        queued.forEach { it.run() }
      }
      // Build JSON array of {id, secretHex} pairs from the Keystore.
      val ids   = listKeystoreIds()
      val array = JSONArray()
      for (id in ids) {
        val secretHex = loadSecretFromKeystore(id) ?: continue
        array.put(JSONObject().put("id", id).put("secretHex", secretHex))
      }
      if (array.length() == 0) return

      val wv = webView ?: return
      // Escape the JSON for safe embedding into the JS string argument.
      val json    = array.toString()
      val escaped = json.replace("\\", "\\\\").replace("\"", "\\\"")
      val callId  = UUID.randomUUID().toString()
      val js      = "AirSign.restoreKeypairs(\"$callId\",\"$escaped\")"
      mainHandler.post { wv.evaluateJavascript(js, null) }
    }

    /**
     * Called by airsign_bridge.html immediately after generating a keypair,
     * BEFORE the Promise result is posted. Saves the secret to the Keystore.
     *
     * @param id        keypair UUID
     * @param secretHex Ed25519 secret key as lowercase hex string
     */
    @JavascriptInterface
    fun saveSecret(id: String, secretHex: String) {
      saveToKeystore(id, secretHex)
    }

    /**
     * Called by airsign_bridge.html with the JSON result of every async call.
     *
     * @param callId  opaque UUID matching the pending Promise
     * @param payload JSON string: { callId, ok, result | error }
     */
    @JavascriptInterface
    fun postResult(callId: String, payload: String) {
      val promise = pending.remove(callId) ?: return
      try {
        val json = JSONObject(payload)
        val ok   = json.getBoolean("ok")
        if (ok) {
          val result = if (json.isNull("result")) null else json.get("result")
          promise.resolve(if (result is JSONObject) jsonToMap(result) else result)
        } else {
          promise.reject("WASM_ERROR", json.optString("error", "unknown"), null)
        }
      } catch (e: Exception) {
        promise.reject("PARSE_ERROR", e.message, e)
      }
    }

    private fun jsonToMap(obj: JSONObject): Map<String, Any?> =
      obj.keys().asSequence().associateWith { key ->
        if (obj.isNull(key)) null else obj.get(key)
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
    (ks.getEntry(alias, null) as? KeyStore.SecretKeyEntry)?.let { return it.secretKey }

    val keyGen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
    keyGen.init(
      KeyGenParameterSpec.Builder(
        alias,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
      )
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .build()
    )
    return keyGen.generateKey()
  }

  private fun saveToKeystore(id: String, secretHex: String) {
    val ctx    = appContext.reactContext ?: return
    val key    = getOrCreateAesKey(id)
    val cipher = Cipher.getInstance("AES/GCM/NoPadding").also { it.init(Cipher.ENCRYPT_MODE, key) }
    val iv         = cipher.iv
    val ciphertext = cipher.doFinal(secretHex.toByteArray(Charsets.UTF_8))
    val blob       = Base64.encodeToString(iv + ciphertext, Base64.NO_WRAP)
    ctx.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE).edit().putString(id, blob).apply()
  }

  private fun loadSecretFromKeystore(id: String): String? = runCatching {
    val ctx  = appContext.reactContext ?: return null
    val blob = ctx.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE).getString(id, null) ?: return null
    val raw        = Base64.decode(blob, Base64.NO_WRAP)
    val iv         = raw.sliceArray(0 until GCM_IV_LENGTH)
    val ciphertext = raw.sliceArray(GCM_IV_LENGTH until raw.size)
    val key    = getOrCreateAesKey(id)
    val cipher = Cipher.getInstance("AES/GCM/NoPadding")
      .also { it.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_LENGTH, iv)) }
    cipher.doFinal(ciphertext).toString(Charsets.UTF_8)
  }.getOrNull()

  private fun deleteFromKeystore(id: String) {
    appContext.reactContext
      ?.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
      ?.edit()?.remove(id)?.apply()
    runCatching {
      KeyStore.getInstance(KEYSTORE_PROVIDER).also { it.load(null) }.deleteEntry("$KEY_ALIAS_PREFIX$id")
    }
  }

  private fun listKeystoreIds(): List<String> =
    appContext.reactContext
      ?.getSharedPreferences(PREF_NAME, Context.MODE_PRIVATE)
      ?.all?.keys?.toList()
      ?: emptyList()
}