# AirSign Security Audit Questionnaire

**Version:** 6.0.0  
**Date:** 2026-04-18  
**Purpose:** Pre-engagement information for prospective security auditors (Trail of Bits RFP format)

---

## Section A — Project Overview

**A1. What does AirSign do?**  
AirSign is a zero-network air-gap signing system for Solana. It enables Ed25519 transaction signing on a permanently offline device using an optical QR-code channel. It also provides FROST RFC 9591 threshold signatures, Pedersen DKG without a trusted dealer, and Squads v4 on-chain multisig instruction building — all fully offline.

**A2. Who are the users?**  
- High-value wallet operators (DAOs, treasuries, exchanges) who require air-gap security without a hardware wallet
- Solana protocol teams integrating threshold signing into their key management infrastructure
- Security researchers evaluating air-gap protocols

**A3. What is the intended deployment environment?**  
- **Online machine:** Any OS (macOS, Linux, Windows) or browser (Chrome, Firefox, Safari) — running the send/receive WASM or CLI
- **Offline signer:** Any device that can display and read QR codes — laptop, phone in airplane mode, Raspberry Pi
- **Mobile signer:** React Native / Expo app (`apps/signer-mobile`) running on iOS/Android in airplane mode

---

## Section B — Codebase

**B1. Primary programming languages:**  
Rust (cryptographic core, ~6 000 LOC), TypeScript (React SDK + web app, ~3 000 LOC)

**B2. External dependencies of highest concern:**

| Dependency | Version | Usage | Why High Concern |
|---|---|---|---|
| `frost-ed25519` | 0.7 | FROST signing | Core threshold crypto |
| `ed25519-dalek` | 2.x | Ed25519 signing | Transaction signatures |
| `chacha20poly1305` | 0.10 | AEAD encryption | All session encryption |
| `argon2` | 0.5 | Password KDF | Key derivation |
| `wasm-bindgen` | 0.2 | WASM FFI | JS/Rust boundary |
| `borsh` | 1.x | Serialization | On-chain instruction data |

**B3. Lines of code per crate:**

| Crate | Approx. LOC |
|---|---|
| `afterimage-core` | 1 200 |
| `afterimage-wasm` | 900 |
| `afterimage-solana` | 1 800 |
| `afterimage-squads` | 700 |
| `afterimage-frost` | 500 |
| `afterimage-dkg` | 400 |
| `afterimage-cli` | 600 |

**B4. Is the `Cargo.lock` committed?**  
Yes. All dependency hashes are pinned.

**B5. Are there `unsafe` blocks?**  
Only in `crates/afterimage-wasm/src/lib.rs` (WASM FFI boundary, unavoidable). All other crates have `#![forbid(unsafe_code)]` (auditor should verify).

---

## Section C — Cryptographic Architecture

**C1. What cryptographic operations occur?**  
- Argon2id key derivation (password + salt → 32-byte session key)
- ChaCha20-Poly1305 AEAD encryption/decryption per fountain frame
- Ed25519 signing (transaction signatures)
- FROST Ed25519 threshold signing (2-round protocol, RFC 9591)
- Pedersen DKG (3-round, no trusted dealer)
- SHA-256 (Anchor instruction discriminators)
- OS CSPRNG (nonces, salts, FROST commitment nonces)

**C2. Are there any home-grown cryptographic constructions?**  
No. All cryptographic primitives are from audited RustCrypto or frost-ed25519 crates. The fountain code (LT codes) is home-grown but carries no secret material — it is a loss-tolerant encoding layer only.

**C3. What is the key lifecycle?**  
- Generated: OS CSPRNG on the offline device
- Stored: ChaCha20-Poly1305(Argon2id(password, salt)) encrypted file, or OS keychain
- Used: Ed25519 signing on the offline device only
- Transmitted: Never — the private key never leaves the offline device
- Deleted: `WasmKeyStore.delete()` / `airsign key delete`

**C4. Are there any timing-sensitive operations?**  
- ChaCha20-Poly1305 tag verification uses constant-time comparison (RustCrypto guarantee)
- Ed25519 signing is deterministic and constant-time (`ed25519-dalek` v2 guarantee)
- Argon2id KDF is intentionally slow (~0.5s)

---

## Section D — Threat Model

**D1. What does AirSign protect against?**  
- Full compromise of the online machine (attacker has root access)
- Passive interception of the QR optical channel
- Replay attacks (session nonce + AEAD authentication)
- Forged transactions (offline device re-parses and displays the transaction before signing)
- t-1 colluding FROST participants (threshold guarantee)

**D2. What does AirSign NOT protect against?**  
- Compromise of the offline device itself
- Coercion of the offline device operator
- Physical theft of the offline device
- Rooted mobile device bypassing airplane mode enforcement (KI-010)
- Side-channel attacks via QR camera timing

**D3. Known issues:**  
See [`KNOWN_ISSUES.md`](KNOWN_ISSUES.md). KI-001 (no AAD binding) and KI-006 (fountain decoder allocation) are the highest-priority items for the auditor.

---

## Section E — Previous Audits

**E1. Has AirSign been audited before?**  
No. This is the first engagement.

**E2. Have any security issues been reported?**  
No CVEs or public vulnerability reports exist.

**E3. Bug bounty program?**  
Not yet active. Responsible disclosure process is documented in `SECURITY.md`.

---

## Section F — Engagement Logistics

**F1. Estimated engagement size:**  
3–4 auditor-weeks

**F2. Preferred report format:**  
PDF + Markdown findings. Severity: Critical / High / Medium / Low / Informational. CVSS v3.1 scores where applicable.

**F3. Remediation window:**  
30 days from draft report delivery.

**F4. Public disclosure:**  
90 days after final report, or upon fix release, whichever comes first.

**F5. Point of contact:**  
security@airsign.io — see `SECURITY.md` for PGP key.

**F6. Can you provide access to a live test environment?**  
Yes — devnet deployment and testnet keypairs will be provided for the duration of the engagement.

**F7. Will developers be available for questions?**  
Yes — async (GitHub Discussions / email) and scheduled calls during business hours UTC+3.

---

## Section G — Auditor Checklist

The following items are pre-identified for auditor attention (in priority order):

### Cryptography
- [ ] KI-001: Evaluate whether binding session_id as AAD to ChaCha20-Poly1305 is necessary
- [ ] KI-002: Evaluate whether the unauthenticated metadata frame enables meaningful attacks
- [ ] Verify Argon2id parameters cannot be weakened by user input at any entry point
- [ ] Verify ChaCha20-Poly1305 tag verification happens before any plaintext use
- [ ] Verify FROST partial signature verification rejects invalid signers before aggregation
- [ ] Verify DKG share verification catches malicious dealers (Feldman VSS)
- [ ] Verify Ed25519 nonce is deterministic (RFC 8032 §5.1.6) — no user-supplied randomness
- [ ] Verify nonce uniqueness: no counter-based nonce, each frame uses OS CSPRNG

### Protocol
- [ ] KI-006: Evaluate fountain decoder allocation without `total_frames` cap
- [ ] Verify fountain decoder does not accept data after completion
- [ ] Verify frame index and total_frames are bounds-checked before allocation
- [ ] Verify session reset fully wipes nonce state and key material

### On-Chain / Squads
- [ ] Verify Squads v4 PDA seeds exactly match the on-chain program
- [ ] Verify Anchor discriminators match `SHA-256("global:<name>")[0..8]`
- [ ] Verify `proposal_approve_ix`: approver is `signer=true, writable=false`
- [ ] Verify transaction index 0 is rejected

### WASM / Browser
- [ ] KI-007: Verify no `unsafe` blocks outside `afterimage-wasm/src/lib.rs`
- [ ] Verify `__airsign_wasm__` global is not writable from external scripts
- [ ] Verify `WasmKeyStore.delete()` zeroes heap memory before drop
- [ ] Verify no key material in `localStorage` or `sessionStorage`

### Mobile
- [ ] KI-010: Evaluate airplane mode enforcement on rooted devices
- [ ] KI-011: Verify FROST key package size within Expo SecureStore limits
- [ ] Verify `expo-secure-store` uses hardware-backed keystore on Android

### Supply Chain
- [ ] Verify `Cargo.lock` hashes match published crate checksums
- [ ] Verify `cargo audit` returns 0 advisories
- [ ] Verify `cargo deny check` passes (licenses, duplicates, advisories)
- [ ] Scan for accidental `unsafe` in non-WASM crates