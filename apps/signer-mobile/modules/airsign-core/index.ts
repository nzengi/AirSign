/**
 * airsign-core module entry point — v2.2
 *
 * Platform-aware implementation selector:
 *
 *   Native build (iOS / Android)
 *     Attempts to load the Expo native module "AirSignCore" which bridges to
 *     the afterimage-wasm Rust binary via WKWebView (iOS) / WebView (Android).
 *     Secrets are generated and held in WASM linear memory; persisted to
 *     iOS Keychain or Android Keystore (TEE-backed AES-256-GCM).
 *
 *   Fallback (Expo Go, web, simulator without native build)
 *     Pure-TypeScript implementation backed by tweetnacl + expo-secure-store.
 *     API surface is identical — all callers are unaffected.
 *
 * The IAirSignCore interface is the single contract both implementations fulfil.
 */

import type { IAirSignCore } from "./src/types";

// ── Try to load the Expo native module ───────────────────────────────────────
// requireOptionalNativeModule returns null when the native module is absent
// (Expo Go, web renderer, or a JS-only build) instead of throwing.

function loadNativeImpl(): IAirSignCore | null {
  try {
    // expo-modules-core is always present in Expo SDK ≥ 46.
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { requireOptionalNativeModule } = require("expo-modules-core");
    const native = requireOptionalNativeModule("AirSignCore") as IAirSignCore | null;
    if (!native) return null;

    // Sanity-check: verify at least one expected function is present.
    if (typeof native.generateKeypair !== "function") return null;

    return native;
  } catch {
    return null;
  }
}

// ── Resolve the concrete implementation ──────────────────────────────────────

// Dynamic import of the TS fallback is deferred so that in native builds the
// tweetnacl / expo-secure-store bundle is not loaded unnecessarily.
function loadTsImpl(): IAirSignCore {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  return require("./src/AirSignCoreModule").default as IAirSignCore;
}

const nativeImpl = loadNativeImpl();

/**
 * The active AirSignCore implementation.
 *
 * On a real native build this will be the WASM-backed Expo native module
 * (hardware key isolation, Rust Ed25519).  On Expo Go or web it is the
 * pure-TypeScript tweetnacl fallback with identical API behaviour.
 */
const AirSignCore: IAirSignCore = nativeImpl ?? loadTsImpl();

export default AirSignCore;

export type {
  IAirSignCore,
  Keypair,
  SignResult,
  InspectResult,
  InspectedInstruction,
  InspectedAccount,
  FountainEncodeResult,
  FountainDecodeResult,
} from "./src/types";