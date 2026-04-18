/**
 * AirSignCore — TypeScript interface for the Expo Native Module.
 *
 * The actual native implementation is provided by the
 * `afterimage-wasm` Rust crate compiled to a React Native JSI module
 * (or, for the initial MVP, loaded as a WASM blob via a JS bridge).
 *
 * Until the native build is wired up, all functions throw a
 * NotImplementedError so callers fail fast rather than silently.
 *
 * Usage:
 *   import AirSignCore from "@/src/native/AirSignCore";
 *   const { pubkey, id } = await AirSignCore.generateKeypair();
 */

class NotImplementedError extends Error {
  constructor(fn: string) {
    super(
      `AirSignCore.${fn} is not yet implemented. ` +
        "Build and link the afterimage-wasm native module first."
    );
    this.name = "NotImplementedError";
  }
}

// ── Types ────────────────────────────────────────────────────────────────────

export interface Keypair {
  /** Unique opaque identifier stored in the secure keychain */
  id: string;
  /** Hex-encoded 32-byte Ed25519 public key */
  pubkeyHex: string;
  /** Base58-encoded Solana address derived from the public key */
  pubkeyBase58: string;
}

export interface SignResult {
  /** Hex-encoded 64-byte Ed25519 signature */
  signatureHex: string;
  /** Base58-encoded signature (Solana wire format) */
  signatureBase58: string;
}

export interface InspectResult {
  feePayer: string;
  recentBlockhash: string;
  feeLamports: number;
  riskLevel: "safe" | "warn" | "critical";
  instructions: InspectedInstruction[];
}

export interface InspectedInstruction {
  programId: string;
  name: string;
  flags: string[];
  accounts: InspectedAccount[];
  dataHex: string;
}

export interface InspectedAccount {
  label: string;
  pubkey: string;
  isSigner: boolean;
  isWritable: boolean;
}

export interface FountainEncodeResult {
  /** Array of base64-encoded fountain frames */
  frames: string[];
  totalFrames: number;
}

export interface FountainDecodeResult {
  /** Whether the decoder has accumulated enough frames */
  complete: boolean;
  /** Base64-encoded reconstructed payload (only set when complete === true) */
  payload?: string;
}

// ── Module interface ─────────────────────────────────────────────────────────

export interface IAirSignCore {
  /**
   * Generate a new Ed25519 keypair and persist the private key in the
   * platform secure keychain (iOS Secure Enclave / Android Keystore).
   */
  generateKeypair(): Promise<Keypair>;

  /**
   * Delete a keypair from the secure keychain.
   * Throws if the keypair does not exist.
   */
  deleteKeypair(id: string): Promise<void>;

  /**
   * List all keypair IDs currently stored in the secure keychain.
   */
  listKeypairIds(): Promise<string[]>;

  /**
   * Sign a raw transaction message (32-byte hash or full serialised tx bytes)
   * with the keypair identified by `id`.
   *
   * @param txBase64 - Base64-encoded raw transaction bytes
   * @param keypairId - Opaque keypair identifier returned by generateKeypair()
   */
  signTransaction(txBase64: string, keypairId: string): Promise<SignResult>;

  /**
   * Run the transaction inspector on serialised transaction bytes.
   * Returns human-readable instruction summaries and a risk assessment.
   *
   * @param txBase64 - Base64-encoded raw transaction bytes
   */
  inspectTransaction(txBase64: string): Promise<InspectResult>;

  /**
   * Fountain-encode a payload into multiple QR-scannable frames.
   *
   * @param payloadBase64 - Base64-encoded bytes to encode
   * @param frameSize - Target byte size per frame (default 800)
   */
  fountainEncode(
    payloadBase64: string,
    frameSize?: number
  ): Promise<FountainEncodeResult>;

  /**
   * Feed a scanned QR frame into the fountain decoder.
   * Returns immediately; call until complete === true.
   *
   * @param frameBase64 - Base64-encoded frame data from a scanned QR code
   * @param sessionId - Opaque session identifier (create a new UUID per scan session)
   */
  fountainDecode(
    frameBase64: string,
    sessionId: string
  ): Promise<FountainDecodeResult>;

  /**
   * Discard all state associated with a fountain decode session.
   */
  resetFountainSession(sessionId: string): Promise<void>;
}

// ── Stub implementation ──────────────────────────────────────────────────────

const AirSignCoreStub: IAirSignCore = {
  generateKeypair: () => Promise.reject(new NotImplementedError("generateKeypair")),
  deleteKeypair: () => Promise.reject(new NotImplementedError("deleteKeypair")),
  listKeypairIds: () => Promise.reject(new NotImplementedError("listKeypairIds")),
  signTransaction: () => Promise.reject(new NotImplementedError("signTransaction")),
  inspectTransaction: () => Promise.reject(new NotImplementedError("inspectTransaction")),
  fountainEncode: () => Promise.reject(new NotImplementedError("fountainEncode")),
  fountainDecode: () => Promise.reject(new NotImplementedError("fountainDecode")),
  resetFountainSession: () =>
    Promise.reject(new NotImplementedError("resetFountainSession")),
};

/**
 * Try to load the real native module; fall back to the stub so that
 * the app can still launch in simulators / web previews without the
 * native build present.
 */
// React Native's Metro bundler provides a synchronous `require` at runtime
// even in JSI/Hermes contexts. Declare it so TypeScript is satisfied without
// pulling in the full @types/node package (which conflicts with RN types).
declare function require(module: string): unknown;

let AirSignCore: IAirSignCore;

try {
  const native = require("airsign-core-native") as IAirSignCore;
  AirSignCore = native;
} catch {
  // Native module not linked — use the stub
  AirSignCore = AirSignCoreStub;
}

export default AirSignCore;