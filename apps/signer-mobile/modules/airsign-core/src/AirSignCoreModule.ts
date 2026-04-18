/**
 * AirSignCoreModule — JavaScript/TypeScript implementation of IAirSignCore.
 *
 * Cryptographic primitives:
 *   - Ed25519 key generation & signing: tweetnacl (same library used by
 *     @solana/web3.js — well-audited, constant-time).
 *   - Key persistence: expo-secure-store (iOS Keychain / Android Keystore).
 *   - Randomness: expo-crypto getRandomBytesAsync.
 *   - Fountain codes: pure-TS implementation matching afterimage-core's
 *     LT-code scheme (same block size, same degree distribution).
 *
 * Upgrade path (JSI / Rust FFI):
 *   When the Xcode / Gradle native build is wired up, replace this module
 *   with the Swift/Kotlin bridge in AirSignCoreModule.swift / .kt which
 *   calls the afterimage-wasm binary via WKWebView JSContext. The public
 *   API (IAirSignCore) stays identical — no callers need to change.
 */

import * as SecureStore from "expo-secure-store";
import * as ExpoCrypto from "expo-crypto";
import nacl from "tweetnacl";
import {
  Keypair,
  SignResult,
  InspectResult,
  InspectedInstruction,
  InspectedAccount,
  FountainEncodeResult,
  FountainDecodeResult,
  IAirSignCore,
} from "./types";

// ── Constants ────────────────────────────────────────────────────────────────

const KEYPAIR_INDEX_KEY = "__airsign_keypair_index__";
const KEYPAIR_PREFIX = "__airsign_kp_";
const FOUNTAIN_BLOCK_SIZE = 256; // bytes per fountain symbol
const FOUNTAIN_OVERHEAD = 1.5; // generate 1.5× as many frames as needed

// ── Base58 encoder/decoder (Bitcoin/Solana alphabet) ─────────────────────────

const B58_ALPHABET =
  "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

function base58Encode(bytes: Uint8Array): string {
  const digits: number[] = [];
  for (const byte of bytes) {
    let carry = byte;
    for (let j = 0; j < digits.length; j++) {
      carry += digits[j] << 8;
      digits[j] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = Math.floor(carry / 58);
    }
  }
  const leading = bytes.findIndex((b) => b !== 0);
  const prefix = leading === -1 ? "" : "1".repeat(leading === -1 ? bytes.length : leading);
  return (
    prefix + digits.reverse().map((d) => B58_ALPHABET[d]).join("")
  );
}

function base58Decode(str: string): Uint8Array {
  const table: Record<string, number> = {};
  for (let i = 0; i < B58_ALPHABET.length; i++) table[B58_ALPHABET[i]] = i;
  const bytes: number[] = [];
  for (const c of str) {
    let carry = table[c];
    if (carry === undefined) throw new Error(`Invalid base58 char: ${c}`);
    for (let j = 0; j < bytes.length; j++) {
      carry += bytes[j] * 58;
      bytes[j] = carry & 0xff;
      carry >>= 8;
    }
    while (carry > 0) {
      bytes.push(carry & 0xff);
      carry >>= 8;
    }
  }
  const leading = str.split("").findIndex((c) => c !== "1");
  const prefix = leading === -1 ? 0 : leading;
  const result = new Uint8Array(prefix + bytes.length);
  bytes.reverse().forEach((b, i) => (result[prefix + i] = b));
  return result;
}

// ── Hex helpers ───────────────────────────────────────────────────────────────

function toHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

function fromHex(hex: string): Uint8Array {
  const result = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    result[i / 2] = parseInt(hex.slice(i, i + 2), 16);
  }
  return result;
}

// ── Secure keychain helpers ───────────────────────────────────────────────────

async function getKeypairIndex(): Promise<string[]> {
  const raw = await SecureStore.getItemAsync(KEYPAIR_INDEX_KEY);
  if (!raw) return [];
  try {
    return JSON.parse(raw) as string[];
  } catch {
    return [];
  }
}

async function saveKeypairIndex(ids: string[]): Promise<void> {
  await SecureStore.setItemAsync(KEYPAIR_INDEX_KEY, JSON.stringify(ids));
}

// ── Solana transaction parser ─────────────────────────────────────────────────

/** Minimal Solana transaction deserializer (legacy format, no versioned tx). */
function parseSolanaTransaction(txBytes: Uint8Array): {
  numRequiredSignatures: number;
  numReadonlySignedAccounts: number;
  numReadonlyUnsignedAccounts: number;
  accountKeys: Uint8Array[];
  recentBlockhash: Uint8Array;
  instructions: Array<{
    programIdIndex: number;
    accountIndices: number[];
    data: Uint8Array;
  }>;
} {
  let offset = 0;

  function readByte(): number {
    return txBytes[offset++];
  }

  function readBytes(n: number): Uint8Array {
    const slice = txBytes.slice(offset, offset + n);
    offset += n;
    return slice;
  }

  function readCompactU16(): number {
    let val = 0;
    let shift = 0;
    while (true) {
      const byte = readByte();
      val |= (byte & 0x7f) << shift;
      if ((byte & 0x80) === 0) break;
      shift += 7;
    }
    return val;
  }

  // Skip signatures (compact-u16 count × 64 bytes each)
  const sigCount = readCompactU16();
  offset += sigCount * 64;

  // Message header
  const numRequiredSignatures = readByte();
  const numReadonlySignedAccounts = readByte();
  const numReadonlyUnsignedAccounts = readByte();

  // Account keys
  const numAccounts = readCompactU16();
  const accountKeys: Uint8Array[] = [];
  for (let i = 0; i < numAccounts; i++) {
    accountKeys.push(readBytes(32));
  }

  // Recent blockhash
  const recentBlockhash = readBytes(32);

  // Instructions
  const numInstructions = readCompactU16();
  const instructions = [];
  for (let i = 0; i < numInstructions; i++) {
    const programIdIndex = readByte();
    const numAccts = readCompactU16();
    const accountIndices: number[] = [];
    for (let j = 0; j < numAccts; j++) accountIndices.push(readByte());
    const dataLen = readCompactU16();
    const data = readBytes(dataLen);
    instructions.push({ programIdIndex, accountIndices, data });
  }

  return {
    numRequiredSignatures,
    numReadonlySignedAccounts,
    numReadonlyUnsignedAccounts,
    accountKeys,
    recentBlockhash,
    instructions,
  };
}

// Known program IDs (base58) → human-readable names
const KNOWN_PROGRAMS: Record<string, string> = {
  "11111111111111111111111111111111": "System Program",
  TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA: "Token Program",
  ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bRS: "Associated Token Program",
  ComputeBudget111111111111111111111111111111: "Compute Budget",
  SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf: "Squads v4",
  BPFLoaderUpgradeab1e11111111111111111111111: "BPF Loader (Upgradeable)",
  Vote111111111111111111111111111111111111111h: "Vote Program",
  Stake11111111111111111111111111111111111111: "Stake Program",
};

function classifyInstruction(
  programName: string,
  data: Uint8Array
): { name: string; flags: string[] } {
  const flags: string[] = [];

  if (programName === "System Program") {
    const ix = data[0] | (data[1] << 8) | (data[2] << 16) | (data[3] << 24);
    const names: Record<number, string> = {
      0: "CreateAccount",
      1: "Assign",
      2: "Transfer",
      3: "CreateAccountWithSeed",
      8: "Allocate",
    };
    if (ix === 2) flags.push("TRANSFER");
    return { name: names[ix] ?? `SystemIx(${ix})`, flags };
  }

  if (programName === "Token Program") {
    const ixType = data[0];
    const names: Record<number, string> = {
      0: "InitializeMint",
      1: "InitializeAccount",
      3: "Transfer",
      7: "MintTo",
      8: "Burn",
      9: "CloseAccount",
    };
    if (ixType === 3 || ixType === 7) flags.push("TOKEN_MOVEMENT");
    return { name: names[ixType] ?? `TokenIx(${ixType})`, flags };
  }

  if (programName === "BPF Loader (Upgradeable)") {
    flags.push("PROGRAM_UPGRADE");
    return { name: "Upgrade", flags };
  }

  if (programName === "Squads v4") {
    flags.push("MULTISIG");
    return { name: "SquadsInstruction", flags };
  }

  return { name: "Unknown", flags: ["UNKNOWN_PROGRAM"] };
}

function computeRiskLevel(
  instructions: InspectedInstruction[]
): "safe" | "warn" | "critical" {
  const flags = instructions.flatMap((ix) => ix.flags);
  if (flags.includes("PROGRAM_UPGRADE")) return "critical";
  if (flags.includes("TOKEN_MOVEMENT") || flags.includes("TRANSFER"))
    return "warn";
  if (flags.includes("UNKNOWN_PROGRAM")) return "warn";
  return "safe";
}

// ── LT Fountain Codes ─────────────────────────────────────────────────────────

/**
 * Robust Soliton degree distribution (simplified).
 * Returns a degree d for a given block count k.
 */
function solitonDegree(k: number, rng: () => number): number {
  const p = rng();
  if (p < 1 / k) return 1;
  // Simplified: use ideal soliton (robust soliton adds c·ln(k/delta)·sqrt(k) spike)
  const d = Math.round(1 / p);
  return Math.min(Math.max(d, 1), k);
}

/** XOR two equal-length Uint8Arrays in-place into dst. */
function xorInto(dst: Uint8Array, src: Uint8Array): void {
  for (let i = 0; i < dst.length; i++) dst[i] ^= src[i];
}

/** Seeded PRNG (xorshift32) — matches Rust fountain.rs seed derivation. */
function makePrng(seed: number): () => number {
  let state = seed >>> 0;
  return function () {
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    state = state >>> 0;
    return state / 0x100000000;
  };
}

/**
 * Encode a payload into fountain frames.
 * Each frame is: [4-byte seed LE][4-byte degree LE][block XOR data]
 */
function fountainEncode(payload: Uint8Array, targetFrames: number): string[] {
  const k = Math.ceil(payload.length / FOUNTAIN_BLOCK_SIZE);
  // Pad payload to k × FOUNTAIN_BLOCK_SIZE
  const padded = new Uint8Array(k * FOUNTAIN_BLOCK_SIZE);
  padded.set(payload);

  const frames: string[] = [];
  // Prepend 4-byte LE length to first block so decoder knows original length
  const lengthHeader = new Uint8Array(4);
  new DataView(lengthHeader.buffer).setUint32(0, payload.length, true);
  // Embed in padded[0..4]
  padded.set(lengthHeader, 0);

  for (let i = 0; i < targetFrames; i++) {
    // Each frame gets a unique seed derived from index
    const seed = (i * 2654435761 + 1) >>> 0; // golden-ratio hash
    const rng = makePrng(seed);
    const degree = solitonDegree(k, rng);

    // Pick `degree` distinct source blocks
    const chosen = new Set<number>();
    while (chosen.size < degree) {
      chosen.add(Math.floor(rng() * k));
    }

    // XOR chosen blocks together
    const symbol = new Uint8Array(FOUNTAIN_BLOCK_SIZE);
    for (const idx of chosen) {
      xorInto(symbol, padded.slice(idx * FOUNTAIN_BLOCK_SIZE, (idx + 1) * FOUNTAIN_BLOCK_SIZE));
    }

    // Frame = [4B seed][4B degree][symbol]
    const frame = new Uint8Array(8 + FOUNTAIN_BLOCK_SIZE);
    new DataView(frame.buffer).setUint32(0, seed, true);
    new DataView(frame.buffer).setUint32(4, degree, true);
    frame.set(symbol, 8);

    frames.push(Buffer.from(frame).toString("base64"));
  }

  return frames;
}

/**
 * Incremental fountain decoder state.
 * Stored in memory; callers feed frames one at a time via fountainDecodeAdd.
 */
interface DecoderState {
  k: number;
  blocks: (Uint8Array | null)[];
  edges: Array<[Set<number>, Uint8Array]>; // [remaining indices, XOR data]
  resolved: number;
}

const decoderStates = new Map<string, DecoderState>();

function fountainDecodeFrame(
  sessionId: string,
  frameB64: string,
  totalBlocks: number
): FountainDecodeResult {
  // Get or create decoder state
  if (!decoderStates.has(sessionId)) {
    decoderStates.set(sessionId, {
      k: totalBlocks,
      blocks: new Array(totalBlocks).fill(null),
      edges: [],
      resolved: 0,
    });
  }
  const state = decoderStates.get(sessionId)!;

  const frameBytes = Buffer.from(frameB64, "base64");
  const view = new DataView(frameBytes.buffer, frameBytes.byteOffset);
  const seed = view.getUint32(0, true);
  const degree = view.getUint32(4, true);
  const symbol = new Uint8Array(frameBytes.buffer, frameBytes.byteOffset + 8, FOUNTAIN_BLOCK_SIZE);

  // Reconstruct which blocks this frame covers
  const rng = makePrng(seed);
  solitonDegree(state.k, rng); // consume the same calls as encoder
  const chosen = new Set<number>();
  while (chosen.size < degree) {
    chosen.add(Math.floor(rng() * state.k));
  }

  // XOR out already-resolved blocks
  const xorData = new Uint8Array(symbol);
  const remaining = new Set<number>();
  for (const idx of chosen) {
    if (state.blocks[idx] !== null) {
      xorInto(xorData, state.blocks[idx]!);
    } else {
      remaining.add(idx);
    }
  }

  if (remaining.size === 0) {
    // All blocks already known
  } else if (remaining.size === 1) {
    // Degree-1: directly recover a block
    const [idx] = remaining;
    state.blocks[idx] = new Uint8Array(xorData);
    state.resolved++;
    // Propagate: try to reduce other edges
    propagate(state, idx);
  } else {
    state.edges.push([remaining, xorData]);
  }

  if (state.resolved === state.k) {
    // Reconstruct payload
    const fullData = new Uint8Array(state.k * FOUNTAIN_BLOCK_SIZE);
    for (let i = 0; i < state.k; i++) {
      fullData.set(state.blocks[i]!, i * FOUNTAIN_BLOCK_SIZE);
    }
    // Read original length from header
    const origLen = new DataView(fullData.buffer).getUint32(0, true);
    const payload = fullData.slice(0, origLen);
    // Clean up state
    decoderStates.delete(sessionId);
    return {
      complete: true,
      payload: Buffer.from(payload).toString("base64"),
    };
  }

  return { complete: false };
}

function propagate(state: DecoderState, resolvedIdx: number): void {
  const newEdges: Array<[Set<number>, Uint8Array]> = [];
  for (const [rem, data] of state.edges) {
    if (rem.has(resolvedIdx)) {
      rem.delete(resolvedIdx);
      xorInto(data, state.blocks[resolvedIdx]!);
      if (rem.size === 0) {
        // Nothing to recover (already fully XOR'd)
      } else if (rem.size === 1) {
        const [idx] = rem;
        if (state.blocks[idx] === null) {
          state.blocks[idx] = new Uint8Array(data);
          state.resolved++;
          propagate(state, idx);
        }
      } else {
        newEdges.push([rem, data]);
      }
    } else {
      newEdges.push([rem, data]);
    }
  }
  state.edges = newEdges;
}

// ── Main module implementation ────────────────────────────────────────────────

const AirSignCoreModuleImpl: IAirSignCore = {
  // ── Key management ──────────────────────────────────────────────────────────

  async generateKeypair(): Promise<Keypair> {
    // Generate 32 random bytes using platform CSPRNG
    const randomBytes = await ExpoCrypto.getRandomBytesAsync(32);
    const secretKey = new Uint8Array(randomBytes);

    // nacl.sign.keyPair.fromSeed produces a 64-byte secret (seed+pubkey) and 32-byte pubkey
    const pair = nacl.sign.keyPair.fromSeed(secretKey);

    const pubkeyHex = toHex(pair.publicKey);
    const pubkeyBase58 = base58Encode(pair.publicKey);

    // Generate a unique ID for this keypair
    const idBytes = await ExpoCrypto.getRandomBytesAsync(16);
    const id = toHex(new Uint8Array(idBytes));

    // Persist: store secret key (seed) in secure store
    // Key: __airsign_kp_<id>  Value: hex-encoded 32-byte seed
    await SecureStore.setItemAsync(
      `${KEYPAIR_PREFIX}${id}`,
      JSON.stringify({ seed: toHex(secretKey), pubkeyHex, pubkeyBase58 })
    );

    // Update index
    const index = await getKeypairIndex();
    index.push(id);
    await saveKeypairIndex(index);

    return { id, pubkeyHex, pubkeyBase58 };
  },

  async deleteKeypair(id: string): Promise<void> {
    await SecureStore.deleteItemAsync(`${KEYPAIR_PREFIX}${id}`);
    const index = await getKeypairIndex();
    const updated = index.filter((i) => i !== id);
    await saveKeypairIndex(updated);
  },

  async listKeypairIds(): Promise<string[]> {
    return getKeypairIndex();
  },

  async getPublicKey(id: string): Promise<Keypair> {
    const raw = await SecureStore.getItemAsync(`${KEYPAIR_PREFIX}${id}`);
    if (!raw) throw new Error(`Keypair not found: ${id}`);
    const { seed: _seed, pubkeyHex, pubkeyBase58 } = JSON.parse(raw);
    return { id, pubkeyHex, pubkeyBase58 };
  },

  // ── Signing ──────────────────────────────────────────────────────────────────

  async signTransaction(id: string, txBase64: string): Promise<SignResult> {
    const raw = await SecureStore.getItemAsync(`${KEYPAIR_PREFIX}${id}`);
    if (!raw) throw new Error(`Keypair not found: ${id}`);
    const { seed } = JSON.parse(raw);

    const seedBytes = fromHex(seed);
    const pair = nacl.sign.keyPair.fromSeed(seedBytes);

    // txBase64 is the raw transaction message bytes (not full tx)
    const message = Buffer.from(txBase64, "base64");

    // nacl.sign.detached produces a 64-byte Ed25519 signature
    const sig = nacl.sign.detached(message, pair.secretKey);

    return {
      signatureHex: toHex(sig),
      signatureBase58: base58Encode(sig),
    };
  },

  async signMessage(id: string, messageBase64: string): Promise<SignResult> {
    // Same as signTransaction — Ed25519 over arbitrary bytes
    return this.signTransaction(id, messageBase64);
  },

  // ── Transaction inspection ────────────────────────────────────────────────────

  async inspectTransaction(txBase64: string): Promise<InspectResult> {
    const txBytes = Buffer.from(txBase64, "base64") as unknown as Uint8Array;

    const parsed = parseSolanaTransaction(new Uint8Array(txBytes));

    // Fee payer is account[0]
    const feePayer = base58Encode(parsed.accountKeys[0]);
    const recentBlockhash = base58Encode(parsed.recentBlockhash);

    // Approximate fee: 5000 lamports base + 5000 per instruction (rough estimate)
    const feeLamports = 5000 + parsed.instructions.length * 5000;

    const instructions: InspectedInstruction[] = parsed.instructions.map((ix) => {
      const programKey = base58Encode(parsed.accountKeys[ix.programIdIndex]);
      const programName = KNOWN_PROGRAMS[programKey] ?? "Unknown Program";
      const { name, flags } = classifyInstruction(programName, ix.data);

      const accounts: InspectedAccount[] = ix.accountIndices.map((accIdx, i) => {
        const isSignerIdx = accIdx < parsed.numRequiredSignatures;
        const isWritable =
          accIdx < parsed.numRequiredSignatures - parsed.numReadonlySignedAccounts ||
          (accIdx >= parsed.numRequiredSignatures &&
            accIdx <
              parsed.accountKeys.length - parsed.numReadonlyUnsignedAccounts);

        return {
          label: `account_${i}`,
          pubkey: base58Encode(parsed.accountKeys[accIdx]),
          isSigner: isSignerIdx,
          isWritable,
        };
      });

      return {
        programId: programKey,
        name,
        flags,
        accounts,
        dataHex: toHex(ix.data),
      };
    });

    const riskLevel = computeRiskLevel(instructions);

    return { feePayer, recentBlockhash, feeLamports, riskLevel, instructions };
  },

  // ── Fountain codes ────────────────────────────────────────────────────────────

  async fountainEncode(
    payloadBase64: string,
    targetFrames: number
  ): Promise<FountainEncodeResult> {
    const payload = Buffer.from(payloadBase64, "base64") as unknown as Uint8Array;
    const frames = fountainEncode(new Uint8Array(payload), targetFrames);
    return { frames, totalFrames: frames.length };
  },

  async fountainDecodeAdd(
    sessionId: string,
    frameBase64: string,
    totalBlocks: number
  ): Promise<FountainDecodeResult> {
    return fountainDecodeFrame(sessionId, frameBase64, totalBlocks);
  },

  async fountainDecodeReset(sessionId: string): Promise<void> {
    decoderStates.delete(sessionId);
  },
};

export default AirSignCoreModuleImpl;