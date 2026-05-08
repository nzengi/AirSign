/**
 * @airsign/react — public API barrel
 *
 * Re-exports everything consumers need to build air-gapped Solana signing UIs.
 */

// ─── WASM initialisation ──────────────────────────────────────────────────────
export { initAirSign, isAirSignReady, getAirSignWasm } from "./initAirSign.js";

// ─── Hooks ────────────────────────────────────────────────────────────────────
export { useSendSession } from "./hooks/useSendSession.js";
export type { UseSendSessionOptions } from "./hooks/useSendSession.js";

export { useRecvSession } from "./hooks/useRecvSession.js";
export type { UseRecvSessionOptions } from "./hooks/useRecvSession.js";

// ─── Components ───────────────────────────────────────────────────────────────
export { QrAnimator } from "./components/QrAnimator.js";
export type { QrAnimatorHandle } from "./components/QrAnimator.js";

export { QrScanner } from "./components/QrScanner.js";

export { TransactionReview } from "./components/TransactionReview.js";

// ─── Types ────────────────────────────────────────────────────────────────────
export type {
  // WASM interfaces
  AirSignWasm,
  WasmSendSession,
  WasmRecvSession,
  // Session state
  SendSessionState,
  RecvSessionState,
  // Inspector / transaction types
  RiskSeverity,
  RiskFlag,
  InstructionKind,
  InstructionInfo,
  TransactionSummary,
  // Component props
  QrAnimatorProps,
  QrScannerProps,
  TransactionReviewProps,
} from "./types.js";