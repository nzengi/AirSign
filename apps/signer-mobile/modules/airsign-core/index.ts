/**
 * airsign-core module entry point.
 *
 * Exports the concrete AirSignCore implementation (TypeScript layer backed by
 * tweetnacl + expo-secure-store) as the default export, conforming to the
 * IAirSignCore interface.
 *
 * The Swift / Kotlin native layers in src/AirSignCoreModule.swift and
 * src/AirSignCoreModule.kt are the v2 upgrade path (JSI bridge to the
 * afterimage-wasm Rust binary). Until that bridge is wired up, this
 * pure-TS implementation is fully functional.
 */

export { default } from "./src/AirSignCoreModule";
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