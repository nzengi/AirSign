/**
 * AirSignCoreModule.swift — iOS native layer for the AirSignCore Expo Module.
 *
 * Current status: JSI/WKWebView bridge skeleton.
 *   The TypeScript implementation in AirSignCoreModule.ts handles all
 *   cryptographic operations using tweetnacl + expo-secure-store.
 *   This Swift file is the upgrade path to a fully native JSI module
 *   that executes the afterimage-wasm binary via JavaScriptCore,
 *   eliminating the JS thread hop and giving direct SecureEnclave access.
 *
 * Upgrade steps (v2 roadmap):
 *   1. Run `wasm-pack build --target no-modules crates/afterimage-wasm`
 *      to produce `afterimage_wasm_bg.wasm` and a JS glue file.
 *   2. Bundle the .wasm as a resource in the Xcode target.
 *   3. In `init()`, create a WKWebView with a local HTML that loads the WASM.
 *   4. Replace each `callAsync` stub below with a `webView.evaluateJavaScript`
 *      call that serialises arguments as JSON and returns the result.
 *   5. For key storage: swap expo-secure-store for a direct SecureEnclave
 *      CryptoKit call (CryptoKit.P256 or a patched Ed25519 implementation).
 */

import ExpoModulesCore
import JavaScriptCore

public class AirSignCoreModule: Module {

  // JSContext that hosts the WASM + glue JS (lazy-initialised on first call)
  private var jsContext: JSContext?

  public func definition() -> ModuleDefinition {
    Name("AirSignCore")

    // ── Key management ──────────────────────────────────────────────────────

    AsyncFunction("generateKeypair") { (promise: Promise) in
      self.ensureContext()
      // TODO (v2): call wasmGenerateKeypair() via JSContext
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    AsyncFunction("deleteKeypair") { (id: String, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    AsyncFunction("listKeypairIds") { (promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    AsyncFunction("getPublicKey") { (id: String, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    // ── Signing ─────────────────────────────────────────────────────────────

    AsyncFunction("signTransaction") { (id: String, txBase64: String, promise: Promise) in
      self.ensureContext()
      // TODO (v2): self.jsContext?.evaluateScript("AirSign.signTransaction(...)")
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    AsyncFunction("signMessage") { (id: String, messageBase64: String, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    // ── Transaction inspection ───────────────────────────────────────────────

    AsyncFunction("inspectTransaction") { (txBase64: String, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    // ── Fountain codes ───────────────────────────────────────────────────────

    AsyncFunction("fountainEncode") { (payloadBase64: String, targetFrames: Int, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    AsyncFunction("fountainDecodeAdd") { (sessionId: String, frameBase64: String, totalBlocks: Int, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }

    AsyncFunction("fountainDecodeReset") { (sessionId: String, promise: Promise) in
      promise.reject("NOT_IMPLEMENTED_NATIVE",
        "Use the JS implementation. Native JSI bridge is a v2 feature.")
    }
  }

  // ── Private helpers ────────────────────────────────────────────────────────

  private func ensureContext() {
    guard jsContext == nil else { return }
    jsContext = JSContext()
    jsContext?.exceptionHandler = { _, exception in
      print("[AirSignCore] JSContext exception: \(exception?.toString() ?? "unknown")")
    }
    // TODO (v2): load WASM glue script from bundle
    // if let glueURL = Bundle.main.url(forResource: "afterimage_wasm", withExtension: "js"),
    //    let glue = try? String(contentsOf: glueURL) {
    //   jsContext?.evaluateScript(glue)
    // }
  }
}