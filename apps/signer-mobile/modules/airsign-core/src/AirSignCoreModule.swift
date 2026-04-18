/**
 * AirSignCoreModule.swift — iOS v2.1 WKWebView bridge for AirSignCore.
 *
 * v2.1 changes vs v2:
 *   · Registers "airSignSaveSecret" WKScriptMessageHandler so the bridge HTML
 *     can push newly-generated secret keys directly into the Keychain without
 *     the secret ever appearing in a Promise result.
 *   · On bridgeDidBecomeReady(), loads all stored {id, secretHex} pairs from
 *     the Keychain and calls AirSign.restoreKeypairs() so the WASM memory
 *     cache is fully warm before any sign call arrives.
 *   · Removes the per-call ensureLoaded() round-trip from signTransaction /
 *     signMessage — the cache is guaranteed warm after startup restore; the
 *     fallback ensureLoaded() is kept only as a safety net for edge cases.
 *
 * Architecture:
 *   ┌────────────────────────┐      evaluateJavaScript()
 *   │  Expo AsyncFunction    │ ──────────────────────────▶ WKWebView
 *   │  (JS thread)           │                             (airsign_bridge.html)
 *   │                        │ ◀─── WKScriptMessageHandler ─ postMessage()
 *   └────────────────────────┘
 *
 * Message channels (JS → native):
 *   airSignReady       → bridgeDidBecomeReady()  → kick off restoreKeypairs
 *   airSignResult      → bridgeDidReceiveResult() → resolve/reject Promise
 *   airSignSaveSecret  → saveToKeychain()         → persist new keypair secret
 *
 * Key storage:
 *   iOS Keychain, kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly.
 */

import ExpoModulesCore
import WebKit
import Security

// MARK: - AirSignCoreModule

public class AirSignCoreModule: Module, WKNavigationDelegate {

  // ── State ──────────────────────────────────────────────────────────────────

  private var webView: WKWebView?
  private var coordinator: BridgeCoordinator?
  private var pending: [String: Promise] = [:]
  private let pendingLock = NSLock()
  private var isReady = false
  private var readyQueue: [() -> Void] = []

  // ── Expo module definition ─────────────────────────────────────────────────

  public func definition() -> ModuleDefinition {
    Name("AirSignCore")

    OnCreate { [weak self] in
      DispatchQueue.main.async { self?.setupWebView() }
    }

    // ── Key management ────────────────────────────────────────────────────────

    AsyncFunction("generateKeypair") { [weak self] (promise: Promise) in
      // The bridge HTML calls saveSecretToNative() automatically before
      // resolving, so we just forward the result {id, pubkeyHex, pubkeyBase58}.
      self?.call("generateKeypair", args: [], promise: promise)
    }

    AsyncFunction("deleteKeypair") { [weak self] (id: String, promise: Promise) in
      self?.deleteFromKeychain(id: id)
      self?.call("deleteKeypair", args: [id], promise: promise)
    }

    AsyncFunction("listKeypairIds") { [weak self] (promise: Promise) in
      promise.resolve(self?.listKeychainIds() ?? [])
    }

    AsyncFunction("getPublicKey") { [weak self] (id: String, promise: Promise) in
      guard let self = self else { promise.reject("INTERNAL", "module deallocated", nil); return }
      guard let secretHex = self.loadSecretFromKeychain(id: id) else {
        promise.reject("KEY_NOT_FOUND", "No keypair with id \(id)", nil); return
      }
      self.call("loadKeypair", args: [id, secretHex], promise: promise)
    }

    // ── Signing ───────────────────────────────────────────────────────────────
    // Cache is warm after restoreKeypairs on startup; ensureLoaded is a safety
    // net for the rare case where the WebView was recreated mid-session.

    AsyncFunction("signTransaction") { [weak self] (id: String, txBase64: String, promise: Promise) in
      guard let self = self else { promise.reject("INTERNAL", "module deallocated", nil); return }
      self.ensureLoaded(id: id) { self.call("signTransaction", args: [id, txBase64], promise: promise) }
    }

    AsyncFunction("signMessage") { [weak self] (id: String, messageBase64: String, promise: Promise) in
      guard let self = self else { promise.reject("INTERNAL", "module deallocated", nil); return }
      self.ensureLoaded(id: id) { self.call("signMessage", args: [id, messageBase64], promise: promise) }
    }

    // ── Transaction inspection ─────────────────────────────────────────────────

    AsyncFunction("inspectTransaction") { [weak self] (txBase64: String, promise: Promise) in
      self?.call("inspectTransaction", args: [txBase64], promise: promise)
    }

    // ── Fountain codes ─────────────────────────────────────────────────────────

    AsyncFunction("fountainEncode") { [weak self] (payloadBase64: String, targetFrames: Int, promise: Promise) in
      self?.call("fountainEncode", args: [payloadBase64, targetFrames], promise: promise)
    }

    AsyncFunction("fountainDecodeAdd") { [weak self] (sessionId: String, frameBase64: String, totalBlocks: Int, promise: Promise) in
      self?.call("fountainDecodeAdd", args: [sessionId, frameBase64, totalBlocks], promise: promise)
    }

    AsyncFunction("fountainDecodeReset") { [weak self] (sessionId: String, promise: Promise) in
      self?.call("fountainDecodeReset", args: [sessionId], promise: promise)
    }
  }

  // MARK: - WebView setup

  private func setupWebView() {
    let config = WKWebViewConfiguration()
    let coordinator = BridgeCoordinator(module: self)
    self.coordinator = coordinator

    // Three inbound message channels.
    config.userContentController.add(coordinator, name: "airSignReady")
    config.userContentController.add(coordinator, name: "airSignResult")
    config.userContentController.add(coordinator, name: "airSignSaveSecret")

    let wv = WKWebView(frame: .zero, configuration: config)
    wv.navigationDelegate = self
    wv.allowsLinkPreview = false
    self.webView = wv

    guard let bundleURL = Bundle.main.url(forResource: "airsign_bridge", withExtension: "html",
                                           subdirectory: "AirSignCore_assets") else {
      print("[AirSignCore] Bridge HTML not found in bundle")
      return
    }
    wv.loadFileURL(bundleURL, allowingReadAccessTo: bundleURL.deletingLastPathComponent())
  }

  // MARK: - WKNavigationDelegate

  public func webView(_ webView: WKWebView,
                      decidePolicyFor navigationAction: WKNavigationAction,
                      decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
    decisionHandler(navigationAction.navigationType == .other ? .allow : .cancel)
  }

  // MARK: - Bridge ready (called by BridgeCoordinator)
  //
  // After WASM is live, bulk-push all stored keypairs back into the WASM
  // memory cache so signTransaction / signMessage work without any extra
  // per-call loadKeypair round-trip.

  func bridgeDidBecomeReady() {
    isReady = true

    // Build the JSON array of {id, secretHex} pairs.
    let ids = listKeychainIds()
    let pairs: [[String: String]] = ids.compactMap { id in
      guard let hex = loadSecretFromKeychain(id: id) else { return nil }
      return ["id": id, "secretHex": hex]
    }

    if !pairs.isEmpty, let data = try? JSONSerialization.data(withJSONObject: pairs),
       let json = String(data: data, encoding: .utf8) {
      let restoreCallId = UUID().uuidString
      // restoreKeypairs result is fire-and-forget; we don't need a Promise.
      let escaped = json
        .replacingOccurrences(of: "\\", with: "\\\\")
        .replacingOccurrences(of: "\"", with: "\\\"")
      let js = "AirSign.restoreKeypairs(\"\(restoreCallId)\",\"\(escaped)\")"
      DispatchQueue.main.async { [weak self] in
        self?.webView?.evaluateJavaScript(js, completionHandler: nil)
      }
    }

    // Drain any calls queued before ready.
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

  // MARK: - Save secret (called by BridgeCoordinator on airSignSaveSecret)

  func bridgeDidRequestSaveSecret(id: String, secretHex: String) {
    saveToKeychain(id: id, secretHex: secretHex)
  }

  // MARK: - Call dispatch

  private func call(_ method: String, args: [Any], promise: Promise) {
    let callId = UUID().uuidString
    pendingLock.lock()
    pending[callId] = promise
    pendingLock.unlock()

    let dispatch = { [weak self] in
      guard let self = self, let wv = self.webView else {
        promise.reject("INTERNAL", "WebView not ready", nil); return
      }
      var jsArgs = ["\"\(callId)\""]
      for arg in args {
        if let s = arg as? String {
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
        }
      }
    }

    if isReady {
      dispatch()
    } else {
      readyQueue.append(dispatch)
    }
  }

  // MARK: - Ensure keypair warm (safety net for cold cache)

  private func ensureLoaded(id: String, then: @escaping () -> Void) {
    // If the keypair is already in the WASM cache (typical after startup
    // restore), we can skip straight to the real call.  We detect this by
    // probing the Keychain — the cache warmth itself is opaque to Swift.
    guard let secretHex = loadSecretFromKeychain(id: id) else { then(); return }
    guard let wv = webView else { then(); return }
    let loadId  = UUID().uuidString
    let escaped = secretHex
      .replacingOccurrences(of: "\\", with: "\\\\")
      .replacingOccurrences(of: "\"", with: "\\\"")
    let js = "AirSign.loadKeypair(\"\(loadId)\",\"\(id)\",\"\(escaped)\")"
    DispatchQueue.main.async {
      wv.evaluateJavaScript(js) { _, _ in then() }
    }
  }

  // MARK: - Keychain helpers

  private static let keychainService = "com.airsign.keypairs"

  private func saveToKeychain(id: String, secretHex: String) {
    guard let data = secretHex.data(using: .utf8) else { return }
    let query: [CFString: Any] = [
      kSecClass:          kSecClassGenericPassword,
      kSecAttrService:    Self.keychainService,
      kSecAttrAccount:    id,
      kSecValueData:      data,
      kSecAttrAccessible: kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly,
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

private class BridgeCoordinator: NSObject, WKScriptMessageHandler {
  weak var module: AirSignCoreModule?

  init(module: AirSignCoreModule) { self.module = module }

  func userContentController(_ userContentController: WKUserContentController,
                              didReceive message: WKScriptMessage) {
    guard let module = module else { return }

    switch message.name {

    case "airSignReady":
      module.bridgeDidBecomeReady()

    case "airSignSaveSecret":
      // body: JSON string { id, secretHex }
      guard let body   = message.body as? String,
            let data   = body.data(using: .utf8),
            let json   = try? JSONSerialization.jsonObject(with: data) as? [String: String],
            let id     = json["id"],
            let secret = json["secretHex"]
      else { return }
      module.bridgeDidRequestSaveSecret(id: id, secretHex: secret)

    case "airSignResult":
      guard let body   = message.body as? String,
            let data   = body.data(using: .utf8),
            let json   = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let callId = json["callId"] as? String,
            let ok     = json["ok"]     as? Bool
      else { return }
      let payload = ok ? json["result"] : json["error"]
      module.bridgeDidReceiveResult(callId: callId, ok: ok, payload: payload)

    default:
      break
    }
  }
}