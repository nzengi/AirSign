# AirSign

**Air-gapped, fountain-coded, encrypted transaction signing for Solana.**

> **v5.0.0** — WASM bindings (Squads, Broadcaster, KeyStore), 135 tests across all crates, `docs/SECURITY_MODEL.md` (STRIDE + RFC-style crypto spec)

AirSign lets you sign Solana transactions on a device that has *never* touched the internet — using nothing but a webcam and QR codes. It is also the first open-source implementation of M-of-N multi-signature orchestration over an air-gap channel.

---

## Table of Contents

- [Why AirSign](#why-airsign)
- [Architecture](#architecture)
- [Security Model](#security-model) · [Full Security Doc](docs/SECURITY_MODEL.md)
- [Features](#features)
- [Workspace Layout](#workspace-layout)
- [Quick Start](#quick-start)
- [Protocol](#protocol)
- [M-of-N Multisig](#m-of-n-multisig)
- [FROST Threshold Signatures](#frost-threshold-signatures)
- [Trustless DKG](#trustless-dkg)
- [Squads v4 Multisig](#squads-v4-multisig)
- [Broadcast & Faucet](#broadcast--faucet)
- [Ledger Hardware Wallet](#ledger-hardware-wallet)
- [Cryptography](#cryptography)
- [CI & Testing](#ci--testing)
- [Roadmap](#roadmap)
- [Contributing](#contributing)
- [License](#license)

---

## Why AirSign

Hot wallets holding the private key on an online machine are the single largest attack surface in Solana key management. AirSign moves the private key onto a permanently offline device — an old laptop, a Raspberry Pi, even a phone in airplane mode — and communicates over an optical channel (QR codes) that carries no writable interface.

Compared to hardware wallets alone, AirSign adds:

| Capability | Hardware Wallet | AirSign |
|---|---|---|
| Air-gap (no USB/BT) | ✗ | ✓ |
| M-of-N multisig | vendor-dependent | ✓ built-in |
| Open-source crypto | partial | ✓ fully auditable |
| Fountain-coded channel | ✗ | ✓ (survives frame loss) |
| WASM browser support | via adapter | ✓ native |
| Rust + TypeScript SDK | ✗ | ✓ |

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  ONLINE MACHINE                       │
│                                                      │
│  ┌────────────┐   unsigned tx    ┌────────────────┐  │
│  │ dApp / CLI │ ───────────────▶ │  AirSign Send  │  │
│  └────────────┘                  │  (fountain enc)│  │
│                                  └───────┬────────┘  │
│                                          │ QR stream  │
└──────────────────────────────────────────┼───────────┘
                                           │  ▲
                              webcam/screen │  │ webcam
                                           ▼  │
┌──────────────────────────────────────────┼───────────┐
│                 AIR-GAPPED MACHINE        │           │
│                                          │           │
│  ┌────────────────┐  ┌───────────────────┴────────┐  │
│  │  AirSign Sign  │  │  AirSign Receive            │  │
│  │  (decrypt +    │  │  (fountain decode +         │  │
│  │   Ed25519 sign)│  │   display signed QR stream) │  │
│  └───────┬────────┘  └────────────────────────────┘  │
│          │ signed tx                                  │
└──────────┼────────────────────────────────────────────┘
           │ QR stream
           ▼
┌──────────────────────────────────────────────────────┐
│                  ONLINE MACHINE                       │
│                                                      │
│  ┌────────────────┐                                  │
│  │ AirSign Receive│ ──▶ broadcast to Solana RPC      │
│  └────────────────┘                                  │
└──────────────────────────────────────────────────────┘
```

### Component Map

```
AirSign/
├── crates/
│   ├── afterimage-core/      # Pure Rust: fountain codes, AEAD crypto, protocol
│   ├── afterimage-wasm/      # wasm-bindgen bindings for browser/Node.js
│   │   └── tests/e2e.rs      # 29 native integration tests
│   ├── afterimage-solana/    # Solana-specific: wallet, Ledger, multisig, preflight
│   ├── afterimage-squads/    # Squads v4 offline instruction builder (64 tests)
│   ├── afterimage-frost/     # FROST RFC 9591 threshold signing (19 tests)
│   ├── afterimage-dkg/       # Pedersen DKG without trusted dealer (23 tests)
│   └── afterimage-cli/       # CLI entry-point (send / sign / receive / keygen)
├── packages/
│   └── react/                # @airsign/react — hooks + components for dApps (19 TS tests)
├── apps/
│   └── signer-web/           # Vite + React demo web app (8 tabs)
└── docs/
    └── SECURITY_MODEL.md     # STRIDE threat model, RFC-style crypto spec, audit checklist
```

---

## Security Model

### Threat model

AirSign assumes:

1. **The online machine may be fully compromised.** An attacker with root access cannot extract the private key because it never exists on the online machine.
2. **The QR optical channel is passive.** An attacker who records every QR frame learns only the ciphertext; without the shared password the plaintext is computationally inaccessible.
3. **The air-gapped machine is trusted.** Physical security of the offline device is the user's responsibility.

### Cryptographic guarantees

| Layer | Primitive | Parameters |
|---|---|---|
| Key derivation | Argon2id | m=65536 KiB, t=3, p=1 |
| Encryption | ChaCha20-Poly1305 | 256-bit key, 96-bit nonce |
| Signing | Ed25519 (RFC 8032) | deterministic, constant-time |
| Threshold signing | FROST Ed25519 (RFC 9591) | t-of-n, n ≤ 255 |
| Session nonce | OS CSPRNG (getrandom) | 96-bit — collision prob < 2⁻⁸⁰ |

### What AirSign does NOT do

- It does not provide a secure enclave or TEE; the private key exists in RAM on the air-gapped machine.
- It does not protect against a malicious QR frame that exploits a vulnerability in the QR decoder library.
- It does not enforce rate-limiting on password attempts; operators should use a strong, unique password per session.

**→ Full STRIDE threat model, RFC-style session protocol spec, Squads v4 security notes, and audit checklist: [`docs/SECURITY_MODEL.md`](docs/SECURITY_MODEL.md)**

---

## Features

- **Fountain-coded QR stream** — LT (Luby Transform) codes allow the receiver to decode even if up to ~30 % of frames are missed or corrupted. No frame ordering required.
- **End-to-end AEAD encryption** — ChaCha20-Poly1305 with Argon2id key derivation.
- **M-of-N multisig orchestration** — Coordinate signatures from multiple air-gapped devices sequentially. Each round is independently authenticated with a shared session nonce.
- **Ledger hardware wallet support** — Send signing requests to a Ledger Nano via APDU over the AirSign optical channel.
- **Solana transaction preflight** — Simulate a transaction against devnet/mainnet before signing to prevent signing invalid or malicious transactions.
- **Transaction inspector** — Decode and display Solana transaction instructions in human-readable form before the signer approves.
- **Browser-native WASM** — The entire crypto stack compiles to `wasm32-unknown-unknown`; no Node.js required.
- **React SDK** — `@airsign/react` ships `useSendSession`, `useRecvSession`, `<QrAnimator />`, `<QrScanner />`, and `<TransactionReview />` for easy dApp integration.

---

## Workspace Layout

| Crate / Package | Language | Description |
|---|---|---|
| `afterimage-core` | Rust | Fountain codes, ChaCha20-Poly1305, Argon2id, session state machines |
| `afterimage-wasm` | Rust + wasm-bindgen | Browser/Node.js bindings: `WasmSendSession`, `WasmRecvSession`, `WasmSquads`, `WasmBroadcaster`, `WasmKeyStore`, `WasmFrost*`, `WasmDkg*` |
| `afterimage-solana` | Rust | Solana wallet, Ledger APDU, transaction preflight, inspector, broadcaster, multisig |
| `afterimage-squads` | Rust | Squads v4 offline instruction builder — PDA derivation, all proposal instructions, adapter |
| `afterimage-frost` | Rust | FROST RFC 9591 threshold signatures — dealer, participant, aggregator |
| `afterimage-dkg` | Rust | Pedersen DKG without trusted dealer — 3-phase protocol over Ed25519 |
| `afterimage-cli` | Rust | `airsign send/sign/receive/keygen/multisig/squads/broadcast/airdrop/run/inspect/key/ledger` |
| `@airsign/react` | TypeScript | React hooks (`useSendSession`, `useRecvSession`) and components (`QrAnimator`, `QrScanner`, `TransactionReview`) |
| `signer-web` | TypeScript + Vite | Demo web app (8 tabs: Send · Sign · Receive · Multisig · FROST · DKG · Squads · KeyStore) |

---

## Quick Start

### Prerequisites

- Rust (stable + `wasm32-unknown-unknown` target)
- `wasm-pack`
- Node.js ≥ 18, pnpm

```bash
# Clone
git clone https://github.com/nzengi/AirSign.git
cd AirSign

# Build WASM
wasm-pack build crates/afterimage-wasm --target web --out-dir apps/signer-web/public/wasm

# Install JS deps
pnpm install

# Start dev server
pnpm --filter signer-web dev
```

Open `http://localhost:5173` — four tabs: **Prepare & Send**, **Air-gap Sign**, **Receive & Broadcast**, **M-of-N Multisig**.

### CLI

```bash
# Build CLI
cargo build -p afterimage-cli --release

# Generate a keypair (stored encrypted on the air-gapped machine)
./target/release/airsign keygen --output ~/.airsign/keypair.json

# On the ONLINE machine — prepare and display the QR stream
./target/release/airsign send --tx <BASE64_TX> --password <PASSWORD>

# On the AIR-GAPPED machine — scan and sign
./target/release/airsign sign --password <PASSWORD>

# On the ONLINE machine — receive the signed transaction and broadcast
./target/release/airsign receive --password <PASSWORD> --rpc https://api.devnet.solana.com
```

---

## Protocol

### Frame format (v2)

```
[ version: u8 ][ session_id: [u8; 16] ][ frame_index: u32 ]
[ total_frames: u32 ][ dropout_symbols: u32 ]
[ ciphertext: variable ][ poly1305_tag: [u8; 16] ]
```

- `version` — protocol version, currently `2`.
- `session_id` — 16-byte random nonce, stable for the lifetime of one send/receive session.
- `frame_index` / `total_frames` — fountain code metadata.
- `dropout_symbols` — LT-code degree for this frame.
- `ciphertext` — ChaCha20-encrypted payload fragment.
- `poly1305_tag` — authentication tag covering all preceding fields and the ciphertext.

Frames are serialised to binary and then Base45-encoded for QR data encoding (alphanumeric mode, higher density than Base64).

### Key derivation

```
salt   = random_bytes(32)             # included in session header frame
key    = Argon2id(password, salt,
                  m=65536, t=3, p=1)  # 32-byte ChaCha20 key
nonce  = random_bytes(12)             # per-frame, derived from frame_index + session_id
```

---

## M-of-N Multisig

AirSign implements sequential M-of-N signing without any on-chain program. Each round:

1. The **orchestrator** (online machine) produces a `MultiSignRequest` JSON containing:
   - Session nonce (anti-replay)
   - Round number and expected signer pubkey
   - The unsigned transaction bytes (Base64)
   - Human-readable description for the signer to review

2. The **signer** (air-gapped machine) receives the request via QR stream, verifies the nonce and round, signs the transaction bytes with their Ed25519 key, and returns a `MultiSignResponse` JSON.

3. The orchestrator verifies the response signature against the expected pubkey, stores the partial signature, and advances to the next round.

4. Once M signatures are collected, the orchestrator outputs a `PartialSignatures` JSON ready for `@solana/web3.js`:

```ts
for (const { signer_pubkey, signature_b64 } of partialSigs) {
  transaction.addSignature(
    new PublicKey(signer_pubkey),
    Buffer.from(signature_b64, "base64")
  );
}
await connection.sendRawTransaction(transaction.serialize());
```

### Example: 2-of-3 treasury multisig

```
Signers: Alice (A), Bob (B), Carol (C)
Threshold: 2

Round 1 → Request sent to A → A signs → Response accepted
Round 2 → Request sent to B → B signs → Response accepted
Threshold met (2/3) → PartialSignatures output
```

Carol's key is never contacted. The session nonce prevents an attacker from replaying a valid response from a previous session.

---

## FROST Threshold Signatures

AirSign v2.0 adds native support for **FROST** (Flexible Round-Optimized Schnorr Threshold Signatures, [RFC 9591](https://www.rfc-editor.org/rfc/rfc9591)) over Ed25519 — the same curve used by Solana.

### Why FROST vs. sequential multisig

| Property | AirSign Sequential Multisig | FROST |
|---|---|---|
| On-chain signature size | M × 64 bytes | **64 bytes** (single sig) |
| On-chain program required | No (native Ed25519 multisig) | **No** |
| Private key ever assembled | Never | **Never** |
| Signing rounds | M sequential rounds | **2 rounds (parallel)** |
| Nonce replay protection | Session nonce | **Built-in (FROST spec)** |
| Threshold enforcement | Coordinator enforces | **Cryptographic guarantee** |

The resulting FROST signature is **indistinguishable from a regular Ed25519 signature** — it can be placed into a Solana transaction's signature field and broadcast directly, with no on-chain program changes.

### Architecture

```
┌─────────────────────────────────────────────────┐
│              DEALER (one-time, trusted)          │
│  generate_setup(n, t) → N key shares + pubkey   │
│  Each share transmitted to participant via QR   │
└──────────────┬──────────────────────────────────┘
               │  key_package[i]    pubkey_package
               ▼                         │
┌──────────────────────┐   ┌─────────────▼──────────────┐
│   PARTICIPANT i      │   │       AGGREGATOR            │
│  round1_commit()     │──▶│  add_commitment(r1_i)       │
│    → nonces (priv)   │   │  build_signing_package(msg) │
│    → commitment (pub)│   └──────────┬─────────────────┘
│                      │◀────────────┘ signing_package
│  round2_sign(nonces, │
│    signing_pkg)      │──▶ aggregator.add_share(r2_i)
│    → share (pub)     │
└──────────────────────┘
                            aggregator.aggregate()
                            → final Ed25519 signature ✓
```

### Crate: `afterimage-frost`

```rust
use afterimage_frost::{dealer, participant, aggregator};

// 1. Trusted dealer generates key shares
let setup = dealer::generate_setup(3, 2)?;  // 2-of-3

// 2. Round 1 — each participant commits
let r1_1 = participant::round1_commit(&setup.key_packages[0], 1)?;
let r1_2 = participant::round1_commit(&setup.key_packages[1], 2)?;

// 3. Aggregator builds signing package
let pkg = aggregator::build_signing_package(&[r1_1.clone(), r1_2.clone()], b"tx bytes")?;

// 4. Round 2 — participants sign
let r2_1 = participant::round2_sign(&setup.key_packages[0], &r1_1.nonces_json, &pkg, 1)?;
let r2_2 = participant::round2_sign(&setup.key_packages[1], &r1_2.nonces_json, &pkg, 2)?;

// 5. Aggregate → final sig
let result = aggregator::aggregate(&pkg, &[r2_1, r2_2], &setup.pubkey_package, 2, 3)?;
println!("sig: {}", result.signature_hex);  // 128 hex chars = 64-byte Ed25519 sig
```

### WASM API (browser / `@airsign/react`)

```ts
// Step 1 — dealer
const dealer = WasmFrostDealer.generate(3, 2);
const setup  = JSON.parse(dealer.setup_json());

// Step 2 — Round 1
const p1 = new WasmFrostParticipant(setup.key_packages[0], 1);
const r1  = JSON.parse(p1.round1());

// Step 3 — aggregator
const agg = new WasmFrostAggregator(setup.pubkey_package, 2, 3);
agg.add_commitment(JSON.stringify(r1));
const pkg = agg.build_signing_package(hexEncode("transfer 1 SOL"));

// Step 4 — Round 2
const r2 = JSON.parse(p1.round2(r1.nonces_json, pkg));

// Step 5 — aggregate
agg.add_share(JSON.stringify(r2));
const result = JSON.parse(agg.aggregate(pkg));
// result.signature_hex — ready to insert into Solana transaction
```

The **❄️ FROST Threshold** tab in the signer-web app (`apps/signer-web`) provides a guided, step-by-step in-browser demo of the full flow.

### Security notes

- **Threshold enforcement is cryptographic** — fewer than `t` honest participants cannot produce a valid signature, regardless of what the aggregator claims.
- **Nonces are ephemeral** — each Round-1 call generates fresh nonces from OS randomness; nonce reuse is impossible through the API.
- **The aggregator is untrusted** — it never sees private key material; it only combines public commitments and public shares.
- **`t = 1` is rejected** — `frost-ed25519` requires `min_signers ≥ 2`; the crate validates this before calling the library.

---

## Trustless DKG

AirSign v2.1 extends the FROST stack with a **Distributed Key Generation (DKG)** protocol that eliminates the need for a trusted dealer entirely.

### Why DKG?

The FROST trusted-dealer model requires one party to generate all key shares and then destroy their copy of the master secret. If that party is dishonest or compromised before deletion, the whole threshold is bypassed. DKG distributes key generation across all participants — no single party ever holds the full secret.

| Property | Trusted Dealer (FROST v2.0) | Trustless DKG (v2.1) |
|---|---|---|
| Single point of failure | The dealer | **None** |
| Secret ever assembled | Once (dealer) | **Never** |
| Rounds | 1 (offline) | **3 rounds** |
| Verifiable secret sharing | No | **Yes (Feldman VSS)** |
| Group pubkey determined by | Dealer | **All participants jointly** |

### Crate: `afterimage-dkg`

```rust
use afterimage_dkg::participant::{dkg_round1, dkg_round2, dkg_finish};

let r1_1 = dkg_round1(1, 3, 2)?;  // id=1, n=3, t=2
let r1_2 = dkg_round1(2, 3, 2)?;
let r1_3 = dkg_round1(3, 3, 2)?;
let all_r1 = vec![r1_1.clone(), r1_2.clone(), r1_3.clone()];

let r2_1 = dkg_round2(&r1_1, &all_r1)?;
let r2_2 = dkg_round2(&r1_2, &all_r1)?;
let r2_3 = dkg_round2(&r1_3, &all_r1)?;
let all_r2 = vec![r2_1.clone(), r2_2.clone(), r2_3.clone()];

let out = dkg_finish(&r1_1, &r2_1, &all_r1, &all_r2)?;
// out.group_pubkey_hex  -- same for all participants
// out.key_package_json  -- private key share; compatible with WasmFrostParticipant
```

### WASM API

```ts
const p1 = new WasmDkgParticipant(1, 3, 2);
const p2 = new WasmDkgParticipant(2, 3, 2);
const p3 = new WasmDkgParticipant(3, 3, 2);

const r1 = [p1.round1(), p2.round1(), p3.round1()].map(JSON.parse);
const allR1 = JSON.stringify(r1);
const r2 = [p1.round2(allR1), p2.round2(allR1), p3.round2(allR1)].map(JSON.parse);
const allR2 = JSON.stringify(r2);

const out = JSON.parse(p1.finish(allR1, allR2));
// out.group_pubkey_hex -- Solana-compatible Ed25519 public key
// out.key_package_json -- pass to WasmFrostParticipant for threshold signing
```

The **Trustless DKG** tab in the signer-web app runs a full multi-participant DKG session inside the browser with no server required.

---

## Broadcast & Faucet

Once the signed response is back on the online machine, AirSign can submit it directly to any Solana cluster — no third-party wallet required.

### `airsign broadcast` — submit a signed response

```bash
# Devnet (default)
airsign broadcast sign_response.json

# Mainnet-beta
airsign broadcast sign_response.json --cluster mainnet

# Custom RPC
airsign broadcast sign_response.json --cluster https://my-rpc.example.com
```

On success the transaction signature is printed to **stdout** (scriptable) and an Explorer deep-link is printed to **stderr**:

```
5Xg1…mBpQ
[airsign] ✓ confirmed on devnet
https://explorer.solana.com/tx/5Xg1…mBpQ?cluster=devnet
```

### `airsign airdrop` — fund an address from the public faucet

Devnet and testnet only. Mainnet requests are rejected immediately.

```bash
airsign airdrop --to 4wTQ…               # 1 SOL on devnet (default)
airsign airdrop --to 4wTQ… --amount 2    # 2 SOL on devnet
airsign airdrop --to 4wTQ… --cluster testnet  # testnet faucet
```

### `airsign run` — end-to-end single-command pipeline

Loads a Solana CLI keypair file, builds a SOL transfer, signs locally, fetches a fresh blockhash, and broadcasts — useful for demos and testing without the QR air-gap.

```bash
airsign run \
  --keypair ~/.config/solana/id.json \
  --to 9xRz… \
  --amount 0.01 \
  --cluster devnet
```

Output:

```
[airsign] run: 4wTQ… → 9xRz… | 0.01 SOL (10000000 lamports) | cluster: devnet
[airsign] ✓ transaction built and signed locally
[airsign] broadcasting to https://api.devnet.solana.com…
5Xg1…mBpQ
[airsign] ✓ confirmed!
  Explorer : https://explorer.solana.com/tx/5Xg1…mBpQ?cluster=devnet
  Solscan  : https://solscan.io/tx/5Xg1…mBpQ?cluster=devnet
```

### Web UI — ReceivePage (Broadcast & Faucet)

The signer-web `Receive & Broadcast` tab now includes:

| Feature | Description |
|---|---|
| Cluster selector | Devnet / Testnet / Mainnet-beta / Custom RPC |
| Balance display | Live SOL balance for the signer pubkey |
| 💧 Airdrop panel | Request up to 2 SOL from the public faucet (devnet/testnet only) |
| 🚀 Broadcast | One-click `sendTransaction`, shows signature + Explorer/Solscan links |
| 📋 Copy sig | Copy transaction signature to clipboard |
| CLI snippet | Equivalent `airsign broadcast` / `airsign airdrop` command |

---

## Ledger Hardware Wallet

AirSign can route signing requests to a Ledger Nano S/X connected to the air-gapped machine. The `afterimage-solana` crate constructs the correct APDU sequence for the Solana Ledger app and presents the signed response in the standard AirSign `MultiSignResponse` format.

```bash
airsign sign --ledger --hd-path "44'/501'/0'/0'"
```

---

## Cryptography

| Primitive | Library | Purpose |
|---|---|---|
| ChaCha20-Poly1305 | `chacha20poly1305` (RustCrypto) | Frame encryption + authentication |
| Argon2id | `argon2` (RustCrypto) | Password-based key derivation |
| Ed25519 | `ed25519-dalek` v2 | Transaction signing and verification |
| BLAKE2b | `blake2` (RustCrypto) | Session ID derivation, integrity checks |
| PBKDF2-HMAC-SHA256 | `pbkdf2` (RustCrypto) | Keystore encryption (legacy compat) |
| `getrandom` | `getrandom` | OS-backed CSPRNG for nonces and salts |

All cryptographic dependencies are part of the **RustCrypto** organisation, which maintains a consistent standard of `#![no_std]` support, constant-time implementations, and community audits.

---

## CI & Testing

```
cargo test --workspace          # all unit tests
cargo test -p afterimage-wasm   # 15 e2e integration tests (native)
wasm-pack test --headless --chrome crates/afterimage-wasm  # browser tests
pnpm --filter @airsign/react test  # TypeScript hook tests (vitest)
```

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs:

- `cargo check --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --check`
- `wasm-pack build` (smoke test)
- `pnpm test` (TypeScript)

All checks must pass before merging to `main`.

---

## Squads v4 Multisig

AirSign v3.0 integrates natively with the [Squads v4](https://squads.so) on-chain multisig program (`SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`) via the `afterimage-squads` crate.  All instruction building is **fully offline** — no RPC call is needed until the final broadcast step.

### Derive PDAs

```bash
airsign squads pda --create-key 4wTQ…
# → { "multisig_pda": "…", "vault_pda": "…", "bump": 255 }
```

### Create a 2-of-3 multisig

```bash
airsign squads create \
  --create-key 4wTQ… \
  --members Alice…,voter:Bob…,Carol… \
  --threshold 2 \
  --memo "Team treasury"
# → InstructionResult JSON (program_id, accounts[], data_b64)
```

Member permission prefixes:

| Prefix | On-chain permissions |
|---|---|
| _(none)_ | Full — Proposer + Voter + Executor |
| `voter:` | Voter only |
| `initiator:` | Initiator (Proposer) only |
| `executor:` | Executor only |

### Approve a proposal via QR air-gap

```bash
# Online machine: build payload + transmit via QR
airsign squads approve \
  --multisig SQDS… --tx-index 7 --approver Alice… \
  | airsign send -

# Air-gapped machine: receive + sign
airsign recv response.json
airsign sign response.json --keypair keychain:alice-key
```

### Config transactions

```bash
# Add a member
airsign squads add-member \
  --multisig SQDS… --creator Alice… --tx-index 5 --member voter:Dave…

# Remove a member
airsign squads remove-member \
  --multisig SQDS… --creator Alice… --tx-index 6 --member Bob…

# Change threshold
airsign squads change-threshold \
  --multisig SQDS… --creator Alice… --tx-index 7 --threshold 3
```

### Crate: `afterimage-squads`

```rust
use afterimage_squads::{
    multisig::{create_multisig_json, derive_pda_info},
    types::{Member, MultisigConfig},
};

let config = MultisigConfig {
    create_key: "4wTQ…".into(),
    members: vec![
        Member::full("Alice…"),
        Member::voter("Bob…"),
        Member::full("Carol…"),
    ],
    threshold: 2,
    time_lock: 0,
    memo: Some("Team treasury".into()),
};
let ix_json = create_multisig_json(&config)?;
// ix_json.program_id  → "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf"
// ix_json.data_b64    → Anchor-discriminated Borsh payload (ready to sign)
```

### Web UI

Open **tab 7 "🏛️ Squads v4"** in the signer web app for a full guided form with one-click CLI command copy, JSON preview, and a collapsible instruction reference table.

---

## Roadmap

- [x] **Squads v4 integration** — `afterimage-squads` crate + `airsign squads` CLI — see [Squads v4 Multisig](#squads-v4-multisig).
- [x] **FROST threshold signatures** — FROST RFC 9591, 2-round parallel threshold signing.
- [x] **Trustless DKG** — FROST RFC 9591 distributed key generation without a trusted dealer.
- [ ] **NFC channel** — alternative to QR codes for devices with NFC capability.
- [ ] **Mobile companion app** — React Native app that acts as the air-gapped signer (airplane mode enforced).
- [ ] **Formal security audit** — independent review of the cryptographic protocol and Rust implementation.
- [ ] **Devnet faucet integration** — one-click airdrop + sign + broadcast demo for new users.

---

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.

For security vulnerabilities, see [SECURITY.md](SECURITY.md) — do **not** open a public issue.

---

## License

Apache 2.0 — see [LICENSE](LICENSE).

---

*AirSign is not affiliated with Solana Labs or the Solana Foundation. Use at your own risk. Always verify transaction contents on the air-gapped device before signing.*