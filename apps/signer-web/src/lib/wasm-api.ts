/**
 * Strongly-typed accessor for the AfterImage WASM module.
 *
 * Every page used to do `(globalThis as any).__airsign_wasm__` and call
 * constructors with mismatched argument orders. This module fixes both:
 *  - one typed entry point (`getWasm()`)
 *  - one set of factory helpers that match the Rust constructor signatures
 *    exactly: `(data: &[u8], filename: &str, password: &str)`.
 *
 * Mismatched calls were a silent bug — wasm-bindgen coerced numeric arguments
 * to strings via `toString()`, so Send used a literal "32" as the password
 * while Recv used the user's input → AEAD verification failed → the demo
 * appeared to hang at "Receiving 0%".
 */

export interface WasmSendSession {
  has_next(): boolean;
  next_frame(): Uint8Array | string | null; // wasm-bindgen returns Uint8Array; some build modes stringify
  progress(): number;
  frame_index(): number;
  total_frames(): number;
  droplet_count(): number;
  recommended_droplet_count(): number;
  set_limit(limit: number): void;
  free(): void;
}

export interface WasmRecvSession {
  ingest_frame(frame: Uint8Array | string): boolean;
  is_complete(): boolean;
  progress(): number;
  received_count(): bigint;
  filename(): string | undefined;
  original_size(): number | undefined;
  protocol_version(): number | undefined;
  get_data(): Uint8Array;
  free(): void;
}

export interface WasmKeypair {
  pubkey(): Uint8Array;
  sign(message: Uint8Array): Uint8Array;
  free(): void;
}

interface AirSignWasmModule {
  WasmSendSession: new (
    data: Uint8Array,
    filename: string,
    password: string,
  ) => WasmSendSession;
  WasmRecvSession: new (password: string) => WasmRecvSession;
  WasmKeypair: { generate(): WasmKeypair };
  encode_base64(data: Uint8Array): string;
  decode_base64(s: string): Uint8Array;
  password_entropy_bits(password: string): number;
  password_strength_label(password: string): "weak" | "medium" | "strong" | "paranoid" | string;
  password_is_mainnet_ready(password: string): boolean;
  // Other exports (Multisig, FROST, DKG, Squads) accessed by name when needed
  [key: string]: unknown;
}

export type PasswordStrength = "weak" | "medium" | "strong" | "paranoid";

export interface PasswordAssessment {
  /** Estimated entropy in bits (Shannon-style on character class diversity). */
  bits: number;
  /** Categorical strength label. */
  strength: PasswordStrength;
  /** True iff the password meets the mainnet recommendation (≥60 bits). */
  mainnetReady: boolean;
}

/**
 * Estimate password strength via the WASM backend (Q3 hardening).
 * Returns a graceful fallback if WASM isn't loaded yet.
 */
export function assessPassword(password: string): PasswordAssessment {
  const wasm = getWasm();
  if (!wasm) {
    // Fallback: no WASM yet — degrade gracefully
    return {
      bits: 0,
      strength: "weak",
      mainnetReady: false,
    };
  }
  const bits = wasm.password_entropy_bits(password);
  const label = wasm.password_strength_label(password);
  const known: PasswordStrength[] = ["weak", "medium", "strong", "paranoid"];
  const strength = (known as string[]).includes(label)
    ? (label as PasswordStrength)
    : "weak";
  return {
    bits,
    strength,
    mainnetReady: wasm.password_is_mainnet_ready(password),
  };
}

/**
 * Returns the loaded WASM module, or `null` if it hasn't initialised yet.
 *
 * Pages should treat `null` as "the demo will run in simulation mode" rather
 * than crashing. The actual init happens once at app start in `wasm.ts`.
 */
export function getWasm(): AirSignWasmModule | null {
  return (
    (globalThis as Record<string, unknown>).__airsign_wasm__ as
      | AirSignWasmModule
      | undefined
  ) ?? null;
}

/**
 * Create a send session with the correct constructor argument order.
 *
 * @param data     raw plaintext bytes to transfer
 * @param password Argon2id key-derivation password (shared with the receiver)
 * @param filename optional filename embedded in the metadata frame (default "tx.bin")
 *
 * Throws if the WASM module isn't loaded — pages should catch and fall back.
 */
export function createSendSession(
  data: Uint8Array,
  password: string,
  filename = "tx.bin",
): WasmSendSession {
  const wasm = getWasm();
  if (!wasm) {
    throw new Error("AirSign WASM module not initialised");
  }
  // Rust signature: WasmSendSession::new(data: &[u8], filename: &str, password: &str)
  return new wasm.WasmSendSession(data, filename, password);
}

/**
 * Create a receive session with the shared password.
 */
export function createRecvSession(password: string): WasmRecvSession {
  const wasm = getWasm();
  if (!wasm) {
    throw new Error("AirSign WASM module not initialised");
  }
  return new wasm.WasmRecvSession(password);
}

/**
 * Generate an ephemeral Ed25519 keypair using the WASM-side `getrandom` CSPRNG.
 */
export function generateKeypair(): WasmKeypair {
  const wasm = getWasm();
  if (!wasm) {
    throw new Error("AirSign WASM module not initialised");
  }
  return wasm.WasmKeypair.generate();
}

/**
 * Encode a binary frame as Base64 for QR-code transmission.
 *
 * Why this matters: `next_frame()` returns raw `Uint8Array` (binary), but QR
 * codes traveling through `code.data: string` get UTF-8-decoded by browsers,
 * silently corrupting any byte ≥ 0x80 that's not part of a valid UTF-8 sequence.
 * Wrapping every frame as Base64 makes the payload pure ASCII — survives the
 * camera→string→ingest round-trip without a single bit flipped.
 */
export function encodeFrameForQr(frame: Uint8Array): string {
  const wasm = getWasm();
  if (wasm) return wasm.encode_base64(frame);
  // Fallback: pure-JS Base64 (slower but works without WASM)
  let bin = "";
  for (let i = 0; i < frame.length; i++) bin += String.fromCharCode(frame[i]);
  return btoa(bin);
}

/**
 * Decode a Base64 QR-payload string back to the binary frame the receiver expects.
 */
export function decodeFrameFromQr(qrPayload: string): Uint8Array {
  const wasm = getWasm();
  if (wasm) {
    try {
      return wasm.decode_base64(qrPayload);
    } catch {
      // Not a valid base64 payload — let the receiver decide whether to ignore
      return new TextEncoder().encode(qrPayload);
    }
  }
  // Pure-JS fallback
  try {
    const bin = atob(qrPayload);
    const bytes = new Uint8Array(bin.length);
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
    return bytes;
  } catch {
    return new TextEncoder().encode(qrPayload);
  }
}
