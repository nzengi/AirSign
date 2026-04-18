# AirSign Security Model

**Version:** 5.0.0  
**Date:** 2026-04-18  
**Status:** Informational — intended for security reviewers and grant evaluators

---

## 1. Overview

AirSign is a **zero-network air-gap signing system** for Solana.  
Private keys never touch an internet-connected process.  
The signing device and the online machine communicate exclusively through an **optical, one-way QR-code channel** (the "fountain").

```
┌─────────────────────┐            ┌──────────────────────────┐
│   ONLINE MACHINE    │            │    OFFLINE SIGNER DEVICE  │
│  (builds unsigned   │──QR codes──▶  (holds private key,      │
│   transactions)     │            │   displays signed result) │
│                     │◀──QR codes─│                           │
└─────────────────────┘            └──────────────────────────┘
         ↕ network                       ✗ no network
```

The channel is **unidirectional per direction** and carries no executable code — only encoded byte payloads.

---

## 2. Cryptographic Primitives

| Layer | Primitive | Parameters | Justification |
|---|---|---|---|
| Signing | Ed25519 | 255-bit curve | Solana native; batch-verifiable |
| FROST threshold signing | Ed25519-FROST (RFC 9591) | t-of-n, n ≤ 255 | Non-interactive partial sigs |
| DKG | Pedersen DKG (FROST §5) | same curve | Distributed key generation, no trusted dealer |
| Session encryption | ChaCha20-Poly1305 | 256-bit key, 96-bit nonce | AEAD; no timing side-channel |
| KDF | Argon2id | m=65536, t=3, p=1 | Memory-hard; resists GPU brute force |
| Nonce | OS CSPRNG (getrandom) | 96-bit | Collision probability < 2⁻⁸⁰ per session |
| Optical encoding | Fountain codes (RaptorQ-inspired) | configurable droplet count | Loss-tolerant; no acknowledgement needed |
| Hash (discriminators) | SHA-256 (first 8 bytes) | — | Anchor-compatible instruction discriminators |

All implementations use audited Rust crates: `curve25519-dalek`, `frost-ed25519`, `chacha20poly1305`, `argon2`, `sha2`, `borsh`.

---

## 3. STRIDE Threat Model

### 3.1 Assets

| Asset | Confidentiality | Integrity | Availability |
|---|---|---|---|
| Ed25519 private key | **Critical** | Critical | High |
| FROST key shares | **Critical** | Critical | High |
| Unsigned transaction payload | Low | **Critical** | Medium |
| Signed transaction | Low | **Critical** | High |
| Session encryption key | Critical | Critical | Medium |

### 3.2 Trust Boundaries

```
TB-1: Online machine ↔ Internet
TB-2: Online machine ↔ Optical channel (QR display)
TB-3: Optical channel ↔ Offline signer camera
TB-4: Offline signer ↔ Hardware (Ledger via APDU / software keystore)
```

### 3.3 STRIDE Analysis

#### S — Spoofing

| Threat | Mitigation |
|---|---|
| Attacker replays a captured QR sequence to make the signer approve a different transaction | Nonce + session ID in every encrypted payload; replay is cryptographically detected |
| Rogue online machine generates a forged "approve" QR | The signer displays the decoded transaction in human-readable form before signing; user must confirm |
| Ledger device impersonation | APDU channel uses Ledger's transport authentication; device attestation via screen confirmation |

#### T — Tampering

| Threat | Mitigation |
|---|---|
| Attacker flips bits in a QR frame during optical transmission | ChaCha20-Poly1305 AEAD; any 1-bit change causes MAC failure and frame is dropped |
| Man-in-the-middle replaces unsigned transaction with a different one | Transaction is parsed and displayed on the offline device before signing |
| Malicious OS on the online machine injects an instruction | Offline signer re-parses the entire `Transaction` from binary; instructions are rendered as human-readable text |

#### R — Repudiation

| Threat | Mitigation |
|---|---|
| Signer denies approving a multisig proposal | On-chain `proposal_approve` instruction carries the signer's Ed25519 public key and signature; immutable on Solana |
| Operator claims a different threshold was used | Squads v4 on-chain multisig records the threshold and all member pubkeys at creation time |

#### I — Information Disclosure

| Threat | Mitigation |
|---|---|
| Attacker intercepts the QR optical channel | Payload is encrypted with ChaCha20-Poly1305; no plaintext is transmitted |
| Private key extracted from the offline keystore file | Keystore is Argon2id-KDF + ChaCha20-Poly1305 encrypted at rest; requires password to decrypt |
| FROST key shares leaked from one participant | Threshold property ensures t-1 shares are computationally useless without the remaining shares |
| Memory scraping on the online machine | Private key never exists on the online machine; only the unsigned transaction is sent |

#### D — Denial of Service

| Threat | Mitigation |
|---|---|
| Attacker floods the optical channel with random QR frames | Fountain decoder rejects frames whose AEAD authentication fails |
| Network outage prevents broadcast | `WasmBroadcaster` / `afterimage-solana::broadcaster` retries with configurable backoff; transaction can be rebroadcast separately |
| Argon2id KDF slowness prevents key derivation | KDF cost is a one-time operation at unlock; not on the signing hot path |

#### E — Elevation of Privilege

| Threat | Mitigation |
|---|---|
| Compromised online machine attempts to read private key | Private key is only ever on the offline device; there is no API surface through which the online machine can request it |
| Malicious WASM module injected into the browser | `initAirSign()` validates the WASM module origin; CSP headers in `signer-web` restrict script sources |
| Squads member forges another member's approval | Squads v4 on-chain program verifies each approver's Ed25519 signature |

---

## 4. Session Protocol (RFC-style)

### 4.1 Notation

```
SEND    = online machine (initiator)
RECV    = offline signer (responder)
K       = session key (256-bit)
N       = nonce (96-bit, random per session)
C       = ciphertext
τ       = Poly1305 authentication tag (128-bit)
||      = concatenation
```

### 4.2 Key Derivation

```
salt     ← CSPRNG(16 bytes)
K        ← Argon2id(password, salt, m=65536, t=3, p=1, len=32)
```

The `salt` is prepended to the first transmitted frame so the receiver can reproduce K.

### 4.3 Frame Encoding

Each "droplet" (fountain frame) has the following layout:

```
┌──────────────────────────────────────────────────────┐
│ frame_index (u32 LE)  │  total_frames (u32 LE)       │
├──────────────────────────────────────────────────────┤
│ nonce (12 bytes)                                     │
├──────────────────────────────────────────────────────┤
│ ciphertext = ChaCha20-Poly1305(K, N, plaintext_shard)│
├──────────────────────────────────────────────────────┤
│ tag (16 bytes, appended by AEAD)                     │
└──────────────────────────────────────────────────────┘
```

The entire frame is then QR-encoded (base45 / binary, configurable).

### 4.4 Receive & Reassembly

1. Camera captures frames in any order.
2. AEAD authentication is verified; failed frames are discarded silently.
3. Fountain decoder accumulates droplets until the original payload can be recovered (rateless codes: any ~1.05 × k distinct droplets suffice).
4. Reassembled plaintext is deserialised as a `solana_sdk::transaction::Transaction`.
5. Transaction is rendered for user confirmation.
6. User approves → Ed25519 signature is produced → signed transaction is fountain-encoded back to the online machine.

### 4.5 Security Properties

| Property | Achieved by |
|---|---|
| Confidentiality | ChaCha20-Poly1305 AEAD |
| Integrity | Poly1305 MAC per frame |
| Authenticity | Session key shared via password; no unauthenticated frame is accepted |
| Forward secrecy | New random nonce per session; past sessions cannot be decrypted if K is later compromised |
| Replay resistance | Nonce is unique per session; frame index prevents intra-session replay |
| Air-gap preservation | Optical channel is physically unidirectional; no TCP/IP socket involved |

---

## 5. FROST Threshold Signing Protocol

AirSign's FROST implementation follows **RFC 9591** (FROST: Flexible Round-Optimized Schnorr Threshold Signatures).

### 5.1 Key Generation (DKG mode)

1. Each participant generates a random polynomial of degree t-1.
2. Participants exchange commitments and secret shares via the coordinator (online machine).
3. Each participant verifies received shares against the published commitments.
4. Any participant who sends an invalid share is identified and excluded.
5. Participants derive their long-term key package without any trusted dealer.

### 5.2 Signing Round

```
Round 1 (Commit):
  Each participant i generates (hiding_nonce_i, binding_nonce_i) ← CSPRNG
  Publishes (hiding_commitment_i, binding_commitment_i)

Round 2 (Sign):
  Coordinator broadcasts signing package = (message, all commitments)
  Each participant computes partial_signature_i using their key share
  Partial signatures are sent to the aggregator

Aggregation:
  Aggregator verifies each partial_signature_i
  Combines into a single (R, s) Schnorr signature
  Verifiable against the group public key using standard Ed25519 verify
```

### 5.3 Security Properties

| Property | Guarantee |
|---|---|
| Unforgeability | t-1 colluding participants cannot produce a valid signature |
| Identifiable abort | Invalid partial signatures are detected and the offending party identified |
| Non-interactivity of aggregation | Aggregator is untrusted; cannot bias the final signature |
| Compatibility | Output signature is a standard Ed25519 signature, verifiable on Solana without any protocol knowledge |

---

## 6. Squads v4 Integration Security

### 6.1 PDA Derivation

All Program Derived Addresses (PDAs) are computed deterministically:

```
multisig_pda = PDA(["multisig", create_key], SQUADS_V4_PROGRAM_ID)
vault_pda    = PDA(["vault", multisig_pda, vault_index_u8], SQUADS_V4_PROGRAM_ID)
tx_pda       = PDA(["multisig_transaction", multisig_pda, tx_index_u64_le], SQUADS_V4_PROGRAM_ID)
proposal_pda = PDA(["multisig_proposal", multisig_pda, tx_index_u64_le], SQUADS_V4_PROGRAM_ID)
```

Derivation is performed in `afterimage-squads::multisig` and verified in unit tests against known vectors.

### 6.2 Instruction Serialisation

Instructions use Anchor's 8-byte discriminator scheme:

```
discriminator = SHA-256("global:<instruction_name>")[0..8]
instruction_data = discriminator || Borsh(args)
```

This is validated in tests (`discriminators_are_distinct`, `discriminator_is_deterministic`).

### 6.3 Member Permission Model

```
INITIATE = 0b001   can propose transactions
VOTE     = 0b010   can approve/reject proposals  
EXECUTE  = 0b100   can execute approved transactions
ALL      = 0b111   full access
```

AirSign enforces threshold ≥ 1 and threshold ≤ member_count at instruction-build time, not only at runtime.

---

## 7. Keystore Security

The software keystore (`afterimage-solana::keystore`) protects private keys at rest:

```
stored_file = {
  salt: [u8; 16],         // random per key
  nonce: [u8; 12],        // random per write
  ciphertext: Vec<u8>,    // ChaCha20-Poly1305(Argon2id(password, salt), nonce, keypair_bytes)
}
```

**Security parameters:**
- Argon2id with m=65536 KiB, t=3 iterations, p=1 lane: ~0.5s on modern hardware, ~256 MiB memory required per attempt
- An attacker with an RTX 4090 can attempt approximately 2 passwords/second — a 5-word passphrase (from a 7776-word list) provides ~64 bits of entropy, requiring ~10¹¹ years to brute-force

---

## 8. Ledger Hardware Wallet Integration

AirSign communicates with Ledger devices via the **APDU (Application Protocol Data Unit)** protocol over USB HID / WebHID.

- Private key never leaves the Ledger secure element.
- The unsigned transaction is sent to the Ledger; the Ledger app parses and displays it on the device screen.
- The user physically presses the Ledger button to confirm.
- The signed transaction is returned; private key remains on-device.

AirSign's APDU layer (`afterimage-solana::ledger_apdu`, `::ledger`) does not extract or store the signing key.

---

## 9. Audit Checklist

The following items should be verified by an independent auditor:

### Cryptography
- [ ] `WasmSendSession` / `WasmRecvSession`: nonce uniqueness across sessions
- [ ] Argon2id parameters are not user-overridable to weaker values
- [ ] ChaCha20-Poly1305 tag verification happens before any plaintext is used
- [ ] FROST partial signature verification rejects invalid signers before aggregation
- [ ] DKG share verification catches malicious dealers
- [ ] Ed25519 signing uses a deterministic nonce (RFC 8032 §5.1.6)

### Protocol
- [ ] Fountain decoder does not accept more data after completion (no double-free / over-read)
- [ ] Frame index and total_frames are bounds-checked before allocation
- [ ] Session reset fully wipes nonce state

### On-chain / Squads
- [ ] `proposal_approve_ix`: approver account is writable=false / signer=true
- [ ] `vault_transaction_execute_ix`: executor account is writable=true / signer=true
- [ ] Transaction index 0 is rejected (Squads v4 indices are 1-based)
- [ ] PDA derivation seeds exactly match Squads v4 on-chain program

### WebAssembly / Browser
- [ ] WASM module is loaded from a pinned URL or bundled hash
- [ ] `__airsign_wasm__` global is not writable from external scripts (CSP + Object.freeze)
- [ ] `WasmKeyStore.delete()` zeroes memory before dropping

### Supply Chain
- [ ] Cargo.lock is committed and all dependency hashes are verified
- [ ] No `unsafe` blocks outside `afterimage-wasm/src/lib.rs` (WASM FFI boundary)
- [ ] CI enforces `cargo audit` on every PR

---

## 10. Known Limitations & Out-of-Scope

| Item | Status |
|---|---|
| Side-channel attacks via QR camera timing | Out of scope (physical layer) |
| Compromised camera firmware on offline device | Out of scope (hardware trust) |
| Coercion / rubber-hose attacks | Out of scope (operational security) |
| Solana validator security | Out of scope (handled by Solana core) |
| Multi-party computation beyond FROST | Not implemented in v5.0.0 |
| Mobile native app | Not implemented; browser-based WASM only |

---

## 11. References

- [RFC 9591](https://www.rfc-editor.org/rfc/rfc9591) — FROST: Flexible Round-Optimized Schnorr Threshold Signatures
- [RFC 8032](https://www.rfc-editor.org/rfc/rfc8032) — Edwards-Curve Digital Signature Algorithm (EdDSA)
- [Argon2 RFC 9106](https://www.rfc-editor.org/rfc/rfc9106) — Argon2 Memory-Hard Function
- [ChaCha20-Poly1305 RFC 8439](https://www.rfc-editor.org/rfc/rfc8439)
- [Squads v4 Protocol](https://github.com/Squads-Protocol/v4) — on-chain multisig program
- [Solana Transaction Format](https://docs.solana.com/developing/programming-model/transactions)
- STRIDE Threat Modelling — Microsoft SDL