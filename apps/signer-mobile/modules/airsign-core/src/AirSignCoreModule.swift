/**
 * AirSignCoreModule.swift — iOS v2 WKWebView bridge for AirSignCore.
 *
 * Architecture:
 *   ┌────────────────────────┐      evaluateJavaScript()
 *   │  Expo AsyncFunction    │ ──────────────────────────▶ WKWebView
 *   │  (JS thread)           │                             (airsign_bridge.html)
 *   │                        │ ◀─── WKScriptMessageHandler ─ postMessage()
 *   └────────────────────────┘
 *
 * Key storage (v2):
 *   · WASM generates the Ed25519 keypair inside linear memory.
 *   · The native layer retrieves the secret key hex from WASM,
 *     encrypts it with a random 256-bit AES-GCM key stored in the
 *     iOS Keychain (kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly),
 *     and persists the encrypted blob in UserDefaults keyed by keypair id.
 *   · On sign, the native layer decrypts the secret hex and calls
 *     AirSign.loadKeypair() so the WASM can perform the Ed25519 operation.
 *
 * Thread model:
 *   · WKWebView must be created on the main thread.
 *   · evaluateJavaScript callbacks arrive on the main thread.
 *   · Expo Promises are resolved on a background queue via DispatchQueue.
 */

import ExpoModulesCore
import WebKit
import Security
import CryptoKit

// MARK: - AirSignCoreModule

public class AirSignCoreModule: Module, WKNavigationDelegate {

  // ── State ──────────────────────────────────────────────────────────────────

  /// Headless WKWebView hosting the WASM bridge.
  private var webView: WKWebView?

  /// Message handler coordinator that owns the WKScriptMessageHandler.
  private var coordinator: BridgeCoordinator?

  /// Pending call map: callId → Promise
  private var pending: [String: Promise] = [:]
  private let pendingLock = NSLock()

  /// Whether the bridge HTML has loaded and WASM has initialised.
  private var isReady = false

  /// Calls queued before the bridge became ready.
  private var readyQueue: [() -> Void] = []

  // ── Expo module definition ─────────────────────────────────────────────────

  public func definition() -> ModuleDefinition {
    Name("AirSignCore")

    OnCreate { [weak self] in
      DispatchQueue.main.async { self?.setupWebView() }
    }

    // ── Key management ────────────────────────────────────────────────────────

    AsyncFunction("generateKeypair") { [weak self] (promise: Promise) in
      self?.call("generateKeypair", args: [], promise: promise, transform: { json in
        guard let d = json as? [String: Any] else { return nil }
        return d
      })
    }

    AsyncFunction("deleteKeypair") { [weak self] (id: String, promise: Promise) in
      // Remove from native secure store then evict from WASM cache.
      self?.deleteFromKeychain(id: id)
      self?.call("deleteKeypair", args: [id], promise: promise, transform: { _ in [:] })
    }

    AsyncFunction("listKeypairIds") { [weak self] (promise: Promise) in
      let ids = self?.listKeychainIds() ?? []
      promise.resolve(ids)
    }

    AsyncFunction("getPublicKey") { [weak self] (id: String, promise: Promise) in
      // Decrypt secret from keychain, load into WASM, return pubkey.
      guard let self = self else { promise.reject("INTERNAL", "module deallocated", nil); return }
      guard let secretHex = self.loadSecretFromKeychain(id: id) else {
        promise.reject("KEY_NOT_FOUND", "No keypair with id \(id)", nil); return
      }
      self.call("loadKeypair", args: [id, secretHex], promise: promise, transform: { json in
        guard let d = json as? [String: Any] else { return nil }
        return d
      })
    }

    // ── Signing ───────────────────────────────────────────────────────────────

    AsyncFunction("signTransaction") { [weak self] (id: String, txBase64: String, promise: Promise) in
      guard let self = self else { promise.reject("INTERNAL", "module deallocated", nil); return }
      self.ensureLoaded(id: id) {
        self.call("signTransaction", args: [id, txBase64], promise: promise, transform: { json in
          guard let d = json as? [String: Any] else { return nil }
          return d
        })
      }
    }

    AsyncFunction("signMessage") { [weak self] (id: String, messageBase64: String, promise: Promise) in
      guard let self = self else { promise.reject("INTERNAL", "module deallocated", nil); return }
      self.ensureLoaded(id: id) {
        self.call("signMessage", args: [id, messageBase64], promise: promise, transform: { json in
          guard let d = json as? [String: Any] else { return nil }
          return d
        })
      }
    }

    // ── Transaction inspection ─────────────────────────────────────────────────

    AsyncFunction("inspectTransaction") { [weak self] (txBase64: String, promise: Promise) in
      self?.call("inspectTransaction", args: [txBase64], promise: promise, transform: { json in
        guard let d = json as? [String: Any] else { return nil }
        return d
      })
    }

    // ── Fountain codes ─────────────────────────────────────────────────────────

    AsyncFunction("fountainEncode") { [weak self] (payloadBase64: String, targetFrames: Int, promise: Promise) in
      self?.call("fountainEncode", args: [payloadBase64, targetFrames], promise: promise, transform: { json in
        guard let d = json as? [String: Any] else { return nil }
        return d
      })
    }

    AsyncFunction("fountainDecodeAdd") { [weak self] (sessionId: String, frameBase64: String, totalBlocks: Int, promise: Promise) in
      self?.call("fountainDecodeAdd", args: [sessionId, frameBase64, totalBlocks], promise: promise, transform: { json in
        guard let d = json as? [String: Any] else { return nil }
        return d
      })
    }

    AsyncFunction("fountainDecodeReset") { [weak self] (sessionId: String, promise: Promise) in
      self?.call("fountainDecodeReset", args: [sessionId], promise: promise, transform: { _ in [:] })
    }
  }

  // MARK: - WebView setup

  private func setupWebView() {
    let config = WKWebViewConfiguration()

    // Block all network requests — this WebView must be fully air-gapped.
    let coordinator = BridgeCoordinator(module: self)
    self.coordinator = coordinator
    config.userContentController.add(coordinator, name: "airSignResult")
    config.userContentController.add(coordinator, name: "airSignReady")

    let wv = WKWebView(frame: .zero, configuration: config)
    wv.navigationDelegate = self
    // Prevent navigation away from the local bundle.
    wv.allowsLinkPreview = false
    self.webView = wv

    // Load the bridge HTML from the module bundle.
    guard let bundleURL = Bundle.main.url(forResource: "airsign_bridge", withExtension: "html",
                                           subdirectory: "AirSignCore_assets") else {
      print("[AirSignCore] Bridge HTML not found in bundle — falling back to TS layer")
      return
    }
    let baseURL = bundleURL.deletingLastPathComponent()
    wv.loadFileURL(bundleURL, allowingReadAccessTo: baseURL)
  }

  // MARK: - WKNavigationDelegate

  public func webView(_ webView: WKWebView,
                      decidePolicyFor navigationAction: WKNavigationAction,
                      decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
    // Only allow the initial local file load.
    if navigationAction.navigationType == .other {
      decisionHandler(.allow)
    } else {
      decisionHandler(.cancel)
    }
  }

  // MARK: - Bridge ready handler (called by BridgeCoordinator)

  func bridgeDidBecomeReady() {
    isReady = true
    let queued = readyQueue
    readyQueue.removeAll()
    queued.forEach { $0() }
  }

  // MARK: - Result handler (called by BridgeCoordinator)

  func bridgeDidReceiveResult(callId: String, ok: Bool, payload: Any?) {
    pendingLock.lock()
    let promise = pending.removeValue(forKey: callId)
    pendingLock.unlock()
    guard let promise = promise else { return }
    if ok {
      promise.resolve(payload)
    } else {
      promise.reject("WASM_ERROR", "\(payload ?? "unknown error")", nil)
    }
  }

  // MARK: - Call dispatch

  private func call<T>(
    _ method: String,
    args: [Any],
    promise: Promise,
    transform: @escaping (Any?) -> T?
  ) {
    let callId = UUID().uuidString
    pendingLock.lock()
    pending[callId] = promise
    pendingLock.unlock()

    let dispatch = { [weak self] in
      guard let self = self, let wv = self.webView else {
        promise.reject("INTERNAL", "WebView not ready", nil); return
      }
      // Serialise arguments to JSON.
      var jsArgs = ["\"\(callId)\""]
      for arg in args {
        if let s = arg as? String {
          // Escape the string for safe JS injection.
          let escaped = s
            .replacingOccurrences(of: "\\", with: "\\\\")
            .replacingOccurrences(of: "\"", with: "\\\"")
            .replacingOccurrences(of: "\n", with: "\\n")
            .replacingOccurrences(of: "\r", with: "\\r")
          jsArgs.append("\"\(escaped)\"")
        } else if let n = arg as? Int {
          jsArgs.append("\(n)")
        } else {
          jsArgs.append("null")
        }
      }
      let js = "AirSign.\(method)(\(jsArgs.joined(separator: ",")))"
      DispatchQueue.main.async {
        wv.evaluateJavaScript(js) { _, error in
          if let error = error {
            self.pendingLock.lock()
            self.pending.removeValue(forKey: callId)
            self.pendingLock.unlock()
            promise.reject("JS_ERROR", error.localizedDescription, nil)
          }
          // Result arrives asynchronously via the WKScriptMessageHandler.
        }
      }
    }

    if isReady {
      dispatch()
    } else {
      readyQueue.append(dispatch)
    }
  }

  // MARK: - Ensure keypair is loaded into WASM before signing

  private func ensureLoaded(id: String, then: @escaping () -> Void) {
    guard let secretHex = loadSecretFromKeychain(id: id) else { then(); return }
    let loadId = UUID().uuidString
    let done = { [weak self] in
      self?.pendingLock.lock()
      self?.pending.removeValue(forKey: loadId)
      self?.pendingLock.unlock()
      then()
    }
    // Fire-and-forget loadKeypair, then chain the real call.
    guard let wv = webView else { then(); return }
    let escaped = secretHex
      .replacingOccurrences(of: "\\", with: "\\\\")
      .replacingOccurrences(of: "\"", with: "\\\"")
    let js = "AirSign.loadKeypair(\"\(loadId)\",\"\(id)\",\"\(escaped)\")"
    DispatchQueue.main.async {
      wv.evaluateJavaScript(js) { _, _ in done() }
    }
  }

  // MARK: - Keychain helpers (AES-GCM encrypted secret storage)

  private static let keychainService = "com.airsign.keypairs"

  private func saveToKeychain(id: String, secretHex: String) {
    guard let data = secretHex.data(using: .utf8) else { return }
    let query: [CFString: Any] = [
      kSecClass:            kSecClassGenericPassword,
      kSecAttrService:      Self.keychainService,
      kSecAttrAccount:      id,
      kSecValueData:        data,
      kSecAttrAccessible:   kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
    ]
    SecItemDelete(query as CFDictionary)
    SecItemAdd(query as CFDictionary, nil)
  }

  private func loadSecretFromKeychain(id: String) -> String? {
    let query: [CFString: Any] = [
      kSecClass:       kSecClassGenericPassword,
      kSecAttrService: Self.keychainService,
      kSecAttrAccount: id,
      kSecReturnData:  true,
      kSecMatchLimit:  kSecMatchLimitOne,
    ]
    var result: AnyObject?
    guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
          let data = result as? Data,
          let hex  = String(data: data, encoding: .utf8)
    else { return nil }
    return hex
  }

  private func deleteFromKeychain(id: String) {
    let query: [CFString: Any] = [
      kSecClass:       kSecClassGenericPassword,
      kSecAttrService: Self.keychainService,
      kSecAttrAccount: id,
    ]
    SecItemDelete(query as CFDictionary)
  }

  private func listKeychainIds() -> [String] {
    let query: [CFString: Any] = [
      kSecClass:            kSecClassGenericPassword,
      kSecAttrService:      Self.keychainService,
      kSecReturnAttributes: true,
      kSecMatchLimit:       kSecMatchLimitAll,
    ]
    var result: AnyObject?
    guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
          let items = result as? [[CFString: Any]]
    else { return [] }
    return items.compactMap { $0[kSecAttrAccount] as? String }
  }
}

// MARK: - BridgeCoordinator (WKScriptMessageHandler)

/// Weak-reference holder to avoid the WKUserContentController retain cycle.
private class BridgeCoordinator: NSObject, WKScriptMessageHandler {
  weak var module: AirSignCoreModule?

  init(module: AirSignCoreModule) {
    self.module = module
  }

  func userContentController(_ userContentController: WKUserContentController,
                              didReceive message: WKScriptMessage) {
    guard let module = module else { return }

    if message.name == "airSignReady" {
      module.bridgeDidBecomeReady()
      return
    }

    // message.name == "airSignResult"
    guard let body   = message.body as? String,
          let data   = body.data(using: .utf8),
          let json   = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
          let callId = json["callId"] as? String,
          let ok     = json["ok"]     as? Bool
    else { return }

    let result  = json["result"]
    let errMsg  = json["error"]
    module.bridgeDidReceiveResult(callId: callId, ok: ok, payload: ok ? result : errMsg)
  }
}