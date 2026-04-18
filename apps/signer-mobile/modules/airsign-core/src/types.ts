// Shared types for the AirSignCore module — mirrors IAirSignCore from the original stub.

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

export interface IAirSignCore {
  generateKeypair(): Promise<Keypair>;
  deleteKeypair(id: string): Promise<void>;
  listKeypairIds(): Promise<string[]>;
  getPublicKey(id: string): Promise<Keypair>;
  signTransaction(id: string, txBase64: string): Promise<SignResult>;
  signMessage(id: string, messageBase64: string): Promise<SignResult>;
  inspectTransaction(txBase64: string): Promise<InspectResult>;
  fountainEncode(payloadBase64: string, targetFrames: number): Promise<FountainEncodeResult>;
  fountainDecodeAdd(sessionId: string, frameBase64: string, totalBlocks: number): Promise<FountainDecodeResult>;
  fountainDecodeReset(sessionId: string): Promise<void>;
}