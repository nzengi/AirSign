/**
 * @airsign/wallet-adapter — public API
 *
 * Implements @solana/wallet-adapter-base (BaseSignerWalletAdapter) and
 * @wallet-standard/core (Wallet) so AirSign works with every Solana dApp
 * that uses wallet-adapter-react, as well as Phantom / Solflare native
 * Wallet Standard detection — no dApp code changes required.
 *
 * Quick start:
 *
 *   // 1. Add AirSignProvider once near your app root:
 *   import { AirSignProvider } from "@airsign/wallet-adapter";
 *   <AirSignProvider wasmUrl="/wasm/afterimage_wasm_bg.wasm">
 *     <App />
 *   </AirSignProvider>
 *
 *   // 2. Register the adapter alongside other wallets:
 *   import { AirSignWalletAdapter } from "@airsign/wallet-adapter";
 *   import { WalletProvider } from "@solana/wallet-adapter-react";
 *   const wallets = [new AirSignWalletAdapter(), ...otherAdapters];
 *   <WalletProvider wallets={wallets}><App /></WalletProvider>
 */

// ─── Adapter (class + config type) ────────────────────────────────────────────
export {
  AirSignWalletAdapter,
  AirSignWalletName,
  // Bridge helpers — used internally by AirSignProvider; exported for advanced use
  resolveAirSignRequest,
  getPendingRequest,
} from "./adapter.js";

export type {
  AirSignWalletAdapterConfig,
  AirSignRequest,
  AirSignResponse,
} from "./adapter.js";

// ─── Provider + Modal ─────────────────────────────────────────────────────────
export { AirSignProvider } from "./AirSignModal.js";
export type { AirSignProviderProps } from "./AirSignModal.js";