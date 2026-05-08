/**
 * AirSignCore — production entry point (v2.2).
 *
 * Re-exports the platform-aware airsign-core module which now automatically
 * selects the correct implementation at runtime:
 *
 *   Real native build (eas build / local Xcode / Gradle build)
 *     → Expo native module "AirSignCore"
 *       · Swift layer (iOS): WKWebView bridge → afterimage-wasm WASM binary
 *         · Keypairs generated and signed inside WASM linear memory
 *         · Secrets persisted to iOS Keychain (kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly)
 *         · exportPrivateKey() decodes directly in Swift — secret never crosses JS bridge
 *       · Kotlin layer (Android): WebView + @JavascriptInterface bridge → WASM
 *         · Secrets persisted to Android Keystore AES-256-GCM (TEE / StrongBox)
 *         · exportPrivateKey() decodes directly in Kotlin — secret never crosses JS bridge
 *
 *   Expo Go / web / JS-only build (fallback)
 *     → Pure-TypeScript implementation
 *       · Ed25519 via tweetnacl (same crypto as @solana/web3.js)
 *       · Key persistence via expo-secure-store (iOS Keychain / Android Keystore)
 *       · Secure randomness via expo-crypto (platform CSPRNG)
 *
 * The IAirSignCore interface is identical across both paths — no callers need
 * to change when upgrading from the TS fallback to the native WASM bridge.
 *
 * Usage:
 *   import AirSignCore from "@/src/native/AirSignCore";
 *   const { id, pubkeyBase58 } = await AirSignCore.generateKeypair();
 *   const { id, pubkeyBase58 } = await AirSignCore.importKeypair(base58PrivKey);
 *   const base58Seed           = await AirSignCore.exportPrivateKey(id);
 *   const { signatureBase58 }  = await AirSignCore.signTransaction(id, txBase64);
 */

import AirSignCoreImpl from "../../modules/airsign-core";

export type {
  IAirSignCore,
  Keypair,
  SignResult,
  InspectResult,
  InspectedInstruction,
  InspectedAccount,
  FountainEncodeResult,
  FountainDecodeResult,
} from "../../modules/airsign-core";

export default AirSignCoreImpl;