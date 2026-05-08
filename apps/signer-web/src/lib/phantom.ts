/**
 * Minimal Phantom wallet adapter — only what the AirSign demo needs.
 *
 * Phantom injects `window.phantom.solana` (or older `window.solana`) with a
 * tiny EIP-1193-ish API. We use it to:
 *   - connect / disconnect
 *   - read the public key
 *   - sign a Solana transaction (via signTransaction — the *correct* path for
 *     transfers; Phantom's signMessage refuses bytes that look like a tx)
 *   - sign arbitrary message bytes (only used for non-Solana payloads)
 *
 * If the user wants the air-gap story, they should NOT sign with Phantom —
 * Phantom is by definition an online wallet. We expose signing here only as a
 * convenience for users who ask "can I use my own wallet?" and accept that
 * the signing is happening online.
 */

import { Transaction, VersionedTransaction } from "@solana/web3.js";
import { base58Encode } from "./solana-tx.js";

interface PhantomProvider {
  isPhantom?: boolean;
  publicKey?: { toBytes(): Uint8Array; toString(): string } | null;
  connect(opts?: { onlyIfTrusted?: boolean }): Promise<{ publicKey: { toBytes(): Uint8Array; toString(): string } }>;
  disconnect(): Promise<void>;
  signMessage?(message: Uint8Array, encoding?: "utf8"): Promise<{ signature: Uint8Array; publicKey?: unknown }>;
  signTransaction?<T extends Transaction | VersionedTransaction>(tx: T): Promise<T>;
  on?(event: string, handler: (...a: unknown[]) => void): void;
}

function getProvider(): PhantomProvider | null {
  if (typeof window === "undefined") return null;
  const phantom = (window as unknown as { phantom?: { solana?: PhantomProvider } }).phantom;
  if (phantom?.solana?.isPhantom) return phantom.solana;
  const solana = (window as unknown as { solana?: PhantomProvider }).solana;
  if (solana?.isPhantom) return solana;
  return null;
}

export function isPhantomInstalled(): boolean {
  return getProvider() !== null;
}

export interface SignedTx {
  /** 64-byte ed25519 signature for our pubkey. */
  signature: Uint8Array;
  /** Full wire-format transaction (sigs + message) — broadcast this directly.
   *  IMPORTANT: callers MUST use these bytes rather than rebuilding a tx from
   *  the original message buffer; web3.js may re-serialize the message and
   *  Phantom signs the *re-serialized* form, so any byte mismatch on broadcast
   *  triggers "Transaction did not pass signature verification". */
  serialized: Uint8Array;
}

export interface PhantomConnection {
  pubkey: Uint8Array;
  pubkeyB58: string;
  /** Sign a Solana legacy Transaction. Returns sig + serialized full tx. */
  signTransaction(tx: Transaction): Promise<SignedTx>;
  /** Sign arbitrary bytes via Phantom's signMessage. Phantom rejects bytes that look like a tx — use signTransaction for transfers. */
  signMessageBytes(msg: Uint8Array): Promise<Uint8Array>;
  disconnect(): Promise<void>;
}

export async function connectPhantom(): Promise<PhantomConnection> {
  const provider = getProvider();
  if (!provider) {
    throw new Error(
      "Phantom is not installed. Install from https://phantom.app, then reload this page.",
    );
  }
  const { publicKey } = await provider.connect();
  const pubkey = publicKey.toBytes();
  const pubkeyB58 = base58Encode(pubkey);
  return {
    pubkey,
    pubkeyB58,
    async signTransaction(tx: Transaction): Promise<SignedTx> {
      if (!provider.signTransaction) throw new Error("Phantom signTransaction unavailable");
      const signed = await provider.signTransaction(tx);
      const sigEntry = signed.signatures.find(
        (s) => s.publicKey.toBase58() === pubkeyB58,
      );
      if (!sigEntry?.signature) throw new Error("Phantom returned tx without our signature");
      const serialized = signed.serialize({ requireAllSignatures: false, verifySignatures: false });
      return {
        signature: new Uint8Array(sigEntry.signature),
        serialized: new Uint8Array(serialized),
      };
    },
    async signMessageBytes(msg: Uint8Array): Promise<Uint8Array> {
      if (!provider.signMessage) throw new Error("Phantom signMessage unavailable");
      const out = await provider.signMessage(msg);
      return out.signature;
    },
    async disconnect(): Promise<void> {
      try { await provider.disconnect(); } catch { /* ignore */ }
    },
  };
}
