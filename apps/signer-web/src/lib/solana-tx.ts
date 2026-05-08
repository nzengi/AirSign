/**
 * Minimal Solana legacy-transaction helpers used by the demo flow.
 *
 * Avoids pulling in @solana/web3.js — we only need a SystemProgram::Transfer
 * builder, a base58 codec, and a couple of read-only RPC calls
 * (getLatestBlockhash, getBalance). Everything here matches the on-wire
 * format documented at
 * https://docs.solana.com/developing/programming-model/transactions.
 */

const BASE58_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/* ── Base58 codec ────────────────────────────────────────────────────────── */
export function base58Encode(bytes: Uint8Array): string {
  let num = 0n;
  for (const b of bytes) num = num * 256n + BigInt(b);
  let out = "";
  while (num > 0n) {
    out = BASE58_ALPHABET[Number(num % 58n)] + out;
    num /= 58n;
  }
  for (const b of bytes) {
    if (b !== 0) break;
    out = "1" + out;
  }
  return out;
}

export function base58Decode(s: string): Uint8Array {
  let num = 0n;
  let leadingZeros = 0;
  let leadingDone = false;
  for (const ch of s) {
    if (!leadingDone) {
      if (ch === "1") leadingZeros++;
      else leadingDone = true;
    }
    const idx = BASE58_ALPHABET.indexOf(ch);
    if (idx < 0) throw new Error(`invalid base58 character: ${ch}`);
    num = num * 58n + BigInt(idx);
  }
  const bytes: number[] = [];
  while (num > 0n) {
    bytes.unshift(Number(num & 0xffn));
    num >>= 8n;
  }
  for (let i = 0; i < leadingZeros; i++) bytes.unshift(0);
  return new Uint8Array(bytes);
}

/* ── Compact-u16 ("shortvec") encoding used by Solana ────────────────────── */
function encodeShortVec(n: number): Uint8Array {
  if (n < 0) throw new Error("shortvec: negative");
  if (n < 0x80) return new Uint8Array([n]);
  if (n < 0x4000) return new Uint8Array([(n & 0x7f) | 0x80, (n >> 7) & 0x7f]);
  return new Uint8Array([(n & 0x7f) | 0x80, ((n >> 7) & 0x7f) | 0x80, (n >> 14) & 0x3]);
}

/* ── System Program constants ────────────────────────────────────────────── */
const SYSTEM_PROGRAM_ID = new Uint8Array(32); // 32 zero bytes — base58 = "11111…111"

/* ── Transfer message builder ───────────────────────────────────────────── */
export interface TransferMessage {
  /** Serialized message bytes (the thing the signer must sign). */
  messageBytes: Uint8Array;
  /** Human-readable summary for the UI. */
  summary: {
    from: string;       // base58
    to: string;         // base58
    lamports: bigint;
    blockhash: string;  // base58
  };
}

/**
 * Build a v-legacy SystemProgram::Transfer **message** (not a full
 * transaction — caller must concatenate signatures in front).
 *
 * Layout:
 *   [header: 3 bytes]
 *   [num_account_keys (shortvec)]
 *   [account_keys: 32 * N]
 *   [recent_blockhash: 32]
 *   [num_instructions (shortvec)]
 *   [instruction: prog_idx, num_accts (shortvec), accts, data_len (shortvec), data]
 */
export function buildTransferMessage(
  fromPubkey: Uint8Array,
  toPubkey: Uint8Array,
  lamports: bigint,
  recentBlockhashB58: string,
): TransferMessage {
  if (fromPubkey.length !== 32) throw new Error("from pubkey must be 32 bytes");
  if (toPubkey.length !== 32)   throw new Error("to pubkey must be 32 bytes");
  const blockhash = base58Decode(recentBlockhashB58);
  if (blockhash.length !== 32) {
    throw new Error(`blockhash must decode to 32 bytes, got ${blockhash.length}`);
  }

  const accountKeys = [fromPubkey, toPubkey, SYSTEM_PROGRAM_ID];
  const header = new Uint8Array([
    1, // num_required_signatures
    0, // num_readonly_signed_accounts
    1, // num_readonly_unsigned_accounts (system program)
  ]);

  // Instruction: System::Transfer { lamports }
  // Discriminator = 2 (u32 LE), then lamports (u64 LE)
  const instrData = new Uint8Array(12);
  const dv = new DataView(instrData.buffer);
  dv.setUint32(0, 2, true);            // Transfer
  dv.setBigUint64(4, lamports, true);

  const instrAccounts = new Uint8Array([0, 1]); // from (signer/writable), to (writable)

  const parts: Uint8Array[] = [
    header,
    encodeShortVec(accountKeys.length),
    ...accountKeys,
    blockhash,
    encodeShortVec(1), // num_instructions
    new Uint8Array([2]), // program_id_index → SYSTEM_PROGRAM_ID at slot 2
    encodeShortVec(instrAccounts.length),
    instrAccounts,
    encodeShortVec(instrData.length),
    instrData,
  ];

  const total = parts.reduce((sum, p) => sum + p.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const p of parts) { out.set(p, offset); offset += p.length; }

  return {
    messageBytes: out,
    summary: {
      from: base58Encode(fromPubkey),
      to:   base58Encode(toPubkey),
      lamports,
      blockhash: recentBlockhashB58,
    },
  };
}

/**
 * Wrap a signed message into a full Solana transaction (single signer):
 *   [shortvec(1)] [signature: 64] [messageBytes]
 */
export function buildSignedTransaction(
  signature: Uint8Array,
  messageBytes: Uint8Array,
): Uint8Array {
  if (signature.length !== 64) throw new Error("signature must be 64 bytes");
  const sigCount = encodeShortVec(1);
  const out = new Uint8Array(sigCount.length + 64 + messageBytes.length);
  out.set(sigCount, 0);
  out.set(signature, sigCount.length);
  out.set(messageBytes, sigCount.length + 64);
  return out;
}

/* ── RPC helpers ─────────────────────────────────────────────────────────── */
export async function fetchLatestBlockhash(rpcUrl: string): Promise<string> {
  const res = await fetch(rpcUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      jsonrpc: "2.0", id: 1, method: "getLatestBlockhash",
      params: [{ commitment: "finalized" }],
    }),
  });
  if (!res.ok) throw new Error(`RPC HTTP ${res.status}`);
  const json = (await res.json()) as {
    result?: { value: { blockhash: string } };
    error?: { message: string };
  };
  if (json.error) throw new Error(json.error.message);
  const bh = json.result?.value?.blockhash;
  if (!bh) throw new Error("RPC returned no blockhash");
  return bh;
}

export const DEVNET_RPC_URL = "https://api.devnet.solana.com";

/** Solana RPC: getBalance returning lamports, or null on RPC error. */
export async function getBalanceLamports(
  pubkeyB58: string,
  rpcUrl: string,
): Promise<number | null> {
  try {
    const res = await fetch(rpcUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0", id: 1, method: "getBalance",
        params: [pubkeyB58, { commitment: "confirmed" }],
      }),
    });
    if (!res.ok) return null;
    const json = (await res.json()) as { result?: { value: number } };
    return typeof json.result?.value === "number" ? json.result.value : null;
  } catch {
    return null;
  }
}


/* ── Base64 helpers (browser) ───────────────────────────────────────────── */
export function bytesToBase64(bytes: Uint8Array): string {
  let bin = "";
  for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
  return btoa(bin);
}

export function base64ToBytes(s: string): Uint8Array {
  const bin = atob(s);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}
