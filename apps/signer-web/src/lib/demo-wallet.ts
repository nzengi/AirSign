/**
 * Imported-keypair adapter — only kept for the SignPage "Imported secret key"
 * mode. Lets a user paste a base58-encoded ed25519 secret (32-byte seed or
 * 64-byte expanded form, like Solana CLI keypairs) and sign locally.
 *
 * The imported key stays in WASM memory for the duration of the page session
 * and is freed via destroy(); never sent over the network.
 */

import { getWasm } from "./wasm-api.js";
import type { WasmKeypair } from "./wasm-api.js";
import { base58Decode, base58Encode } from "./solana-tx.js";

interface WasmKeypairFactory {
  WasmKeypair: {
    from_seed?: (seed: Uint8Array) => WasmKeypair;
    generate(): WasmKeypair;
  };
}

export interface ImportedWallet {
  pubkey: Uint8Array;
  pubkeyB58: string;
  sign(message: Uint8Array): Uint8Array;
  destroy(): void;
}

export function walletFromBase58Secret(b58: string): ImportedWallet {
  const wasm = getWasm() as unknown as WasmKeypairFactory | null;
  if (!wasm) throw new Error("WASM not initialised");
  if (typeof wasm.WasmKeypair.from_seed !== "function") {
    throw new Error("WasmKeypair.from_seed not available — rebuild the WASM module");
  }
  const sk = base58Decode(b58.trim());
  if (sk.length !== 64 && sk.length !== 32) {
    throw new Error(`expected a 32-byte seed or 64-byte ed25519 secret key, got ${sk.length}`);
  }
  const seed = sk.length === 64 ? sk.slice(0, 32) : sk;
  const kp = wasm.WasmKeypair.from_seed(seed);
  const pubkey = kp.pubkey();
  return {
    pubkey,
    pubkeyB58: base58Encode(pubkey),
    sign: (msg: Uint8Array) => kp.sign(msg),
    destroy: () => kp.free(),
  };
}
