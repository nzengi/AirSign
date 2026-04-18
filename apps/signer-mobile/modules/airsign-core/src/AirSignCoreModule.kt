/**
 * AirSignCoreModule.kt — Android native layer for the AirSignCore Expo Module.
 *
 * Current status: JSI/WebView bridge skeleton.
 *   The TypeScript implementation in AirSignCoreModule.ts handles all
 *   cryptographic operations using tweetnacl + expo-secure-store.
 *   This Kotlin file is the upgrade path to a fully native module
 *   that executes the afterimage-wasm binary via Android WebView / Jsoup,
 *   eliminating the JS thread hop and giving direct Android Keystore access.
 *
 * Upgrade steps (v2 roadmap):
 *   1. Run `wasm-pack build --target no-modules crates/afterimage-wasm`
 *   2. Copy the .wasm + JS glue into `android/src/main/assets/`
 *   3. In init(), create a headless WebView and load the assets via a
 *      file:// URI pointing to the bundled index.html wrapper.
 *   4. Bridge each function below by calling webView.evaluateJavascript()
 *      and resolving/rejecting the Promise from the callback.
 *   5. For key storage: swap expo-secure-store for Android Keystore
 *      (android.security.keystore.KeyGenParameterSpec with EC/Ed25519 spec).
 */

package expo.modules.airsigncore

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition
import expo.modules.kotlin.Promise

class AirSignCoreModule : Module() {

  override fun definition() = ModuleDefinition {
    Name("AirSignCore")

    // ── Key management ──────────────────────────────────────────────────────

    AsyncFunction("generateKeypair") { promise: Promise ->
      promise.reject(
        "NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.",
        null
      )
    }

    AsyncFunction("deleteKeypair") { _: String, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    AsyncFunction("listKeypairIds") { promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    AsyncFunction("getPublicKey") { _: String, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    // ── Signing ─────────────────────────────────────────────────────────────

    AsyncFunction("signTransaction") { _: String, _: String, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    AsyncFunction("signMessage") { _: String, _: String, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    // ── Transaction inspection ───────────────────────────────────────────────

    AsyncFunction("inspectTransaction") { _: String, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    // ── Fountain codes ───────────────────────────────────────────────────────

    AsyncFunction("fountainEncode") { _: String, _: Int, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    AsyncFunction("fountainDecodeAdd") { _: String, _: String, _: Int, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }

    AsyncFunction("fountainDecodeReset") { _: String, promise: Promise ->
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.", null)
    }
  }
}