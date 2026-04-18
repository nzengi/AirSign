/**
 * AirSignCore — production entry point.
 *
 * Delegates to the `modules/airsign-core` Expo module which provides:
 *   - Real Ed25519 key generation via tweetnacl (same crypto as @solana/web3.js)
 *   - Key persistence via expo-secure-store (iOS Keychain / Android Keystore)
 *   - Secure randomness via expo-crypto (platform CSPRNG)
 *   - Solana transaction parsing and risk classification
 *   - Fountain-code QR encode/decode matching afterimage-core's LT-code scheme
 *
 * v2 upgrade path:
 *   The Swift/Kotlin native layer in modules/airsign-core/src/ will bridge
 *   to the afterimage-wasm Rust binary via JavaScriptCore (iOS) /
 *   WebView (Android), providing direct Secure Enclave access and
 *   eliminating the JS thread hop. The API below stays identical.
 *
 * Usage:
 *   import AirSignCore from "@/src/native/AirSignCore";
 *   const { id, pubkeyBase58 } = await AirSignCore.generateKeypair();
 *   const { signatureBase58 } = await AirSignCore.signTransaction(id, txBase64);
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