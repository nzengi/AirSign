/**
 * Shared TypeScript types for the @airsign/react package.
 *
 * These mirror the structures produced by the afterimage-wasm WASM module
 * and the afterimage-solana TransactionInspector.
 */

// ─── WASM module interface ────────────────────────────────────────────────────
// These describe the shape of the wasm-bindgen-generated JS classes.
// The actual WASM module is loaded separately via `initWasm()`.

export interface AirSignWasm {
  WasmSendSession: new (
    data: Uint8Array,
    filename: string,
    password: string,
  ) => WasmSendSession;
  WasmRecvSession: new (password: string) => WasmRecvSession;
  recommended_frames(size: number): number;
  encode_base64(data: Uint8Array): string;
  decode_base64(s: string): Uint8Array;
  encode_hex(data: Uint8Array): string;
  decode_hex(s: string): Uint8Array;
  version(): string;
  default: () => Promise<AirSignWasm>;
}

export interface WasmSendSession {
  has_next(): boolean;
  next_frame(): Uint8Array | null;
  progress(): number;
  frame_index(): number;
  total_frames(): number;
  droplet_count(): number;
  recommended_droplet_count(): number;
  set_limit(limit: number): void;
  free(): void;
}

export interface WasmRecvSession {
  ingest_frame(frame: Uint8Array): boolean;
  is_complete(): boolean;
  progress(): number;
  received_count(): bigint;
  filename(): string | undefined;
  original_size(): number | undefined;
  protocol_version(): number | undefined;
  get_data(): Uint8Array;
  free(): void;
}

// ─── Session state ────────────────────────────────────────────────────────────

/** State returned by `useSendSession`. */
export interface SendSessionState {
  /** 0.0 – 1.0 fraction of recommended frames emitted. */
  progress: number;
  /** Number of frames emitted so far. */
  frameIndex: number;
  /** Recommended total frame count. */
  totalFrames: number;
  /** Whether the send loop is currently running. */
  isRunning: boolean;
  /** Whether the recommended frame count has been fully transmitted. */
  isDone: boolean;
  /** Start (or resume) the QR animation loop. */
  start: () => void;
  /** Pause the QR animation loop. */
  stop: () => void;
  /** Reset to the beginning — re-creates the send session. */
  reset: () => void;
  /** Error message, if any. */
  error: string | null;
}

/** State returned by `useRecvSession`. */
export interface RecvSessionState {
  /** 0.0 – 1.0 fraction of required droplets received. */
  progress: number;
  /** Number of raw frames ingested so far. */
  receivedCount: number;
  /** Whether decoding is complete and data is ready. */
  isComplete: boolean;
  /** Decoded plaintext bytes (only populated when `isComplete` is true). */
  data: Uint8Array | null;
  /** Filename from the metadata frame, if available. */
  filename: string | undefined;
  /** Original file size in bytes, if available. */
  originalSize: number | undefined;
  /** Feed a raw QR-decoded payload into the session. */
  ingest: (frame: Uint8Array) => void;
  /** Reset — starts a fresh receive session. */
  reset: () => void;
  /** Error message, if any. */
  error: string | null;
}

// ─── Transaction inspector types ─────────────────────────────────────────────

export type RiskSeverity = "HIGH" | "MEDIUM" | "LOW";

export interface RiskFlag {
  code: string;
  severity: RiskSeverity;
  message: string;
}

export type InstructionKind =
  | "SystemTransfer"
  | "TokenTransfer"
  | "TokenMintTo"
  | "TokenBurn"
  | "TokenSetAuthority"
  | "AtaCreate"
  | "Memo"
  | "Unknown";

export interface InstructionInfo {
  kind: InstructionKind;
  programId: string;
  /** Human-readable summary line. */
  summary: string;
  /** Key-value pairs extracted from the instruction (amounts, addresses, etc.). */
  fields: Record<string, string>;
}

export interface TransactionSummary {
  instructions: InstructionInfo[];
  riskFlags: RiskFlag[];
  hasHighRisk: boolean;
  /** Number of signatures present (0 = unsigned). */
  signatureCount: number;
}

// ─── QrAnimator props ─────────────────────────────────────────────────────────

export interface QrAnimatorProps {
  /** Raw transaction bytes or arbitrary plaintext to transmit. */
  data: Uint8Array;
  /** Logical filename embedded in the metadata frame. */
  filename?: string;
  /** Shared Argon2id password known to both sides. */
  password: string;
  /** Frames per second for the QR animation. @default 8 */
  fps?: number;
  /** QR module size in pixels. @default 4 */
  qrScale?: number;
  /** QR error correction level. @default 'M' */
  errorCorrectionLevel?: "L" | "M" | "Q" | "H";
  /** Called when the recommended frame count has been transmitted. */
  onComplete?: () => void;
  /** Called on each frame with the current progress (0–1). */
  onProgress?: (progress: number) => void;
  /** Additional CSS class applied to the wrapper div. */
  className?: string;
}

// ─── QrScanner props ──────────────────────────────────────────────────────────

export interface QrScannerProps {
  /** Shared Argon2id password. */
  password: string;
  /** Camera device ID (undefined = system default). */
  deviceId?: string;
  /** Called when a complete payload has been decoded. */
  onComplete: (data: Uint8Array, filename?: string) => void;
  /** Called on each ingested frame with the current progress (0–1). */
  onProgress?: (progress: number) => void;
  /** Called if an error occurs during scanning or decryption. */
  onError?: (error: string) => void;
  /** Additional CSS class applied to the video wrapper div. */
  className?: string;
}

// ─── TransactionReview props ──────────────────────────────────────────────────

export interface TransactionReviewProps {
  summary: TransactionSummary;
  /** Show the raw instruction fields. @default false */
  showFields?: boolean;
  /** Additional CSS class applied to the wrapper div. */
  className?: string;
}