# AirSign Security Audit — Scope Document

**Version:** 6.0.0  
**Date:** 2026-04-18  
**Prepared for:** Independent security auditor  
**Repository:** https://github.com/nzengi/AirSign

---

## 1. Engagement Overview

AirSign is a zero-network air-gap signing system for Solana. Private keys never touch an internet-connected process. The online machine and the offline signer communicate exclusively through an optical, one-way QR-code channel ("fountain"). The system also supports FROST RFC 9591 threshold signatures, Pedersen DKG without a trusted dealer, and Squads v4 on-chain multisig instruction building.

This document defines the scope, objectives, and exclusions for an independent security audit.

---

## 2. Crates In Scope

| Crate | Path | Lines of Rust | Primary Concern |
|---|---|---|---|
| `afterimage-core` | `crates/afterimage-core/` | ~1 200 | Fountain codes, ChaCha20-Poly1305, Argon2id, session state machines |
| `afterimage-wasm` | `crates/afterimage-wasm/` | ~900 | wasm-bindgen FFI boundary, WASM memory safety, JS-visible API surface |
| `afterimage-solana` | `crates/afterimage-solana/` | ~1 800 | Solana transaction handling, Ledger APDU, key store, inspector, preflight |
| `afterimage-squads` | `crates/afterimage-squads/` | ~700 | Squads v4 PDA derivation, Anchor discriminator, Borsh serialisation |
| `afterimage-frost` | `crates/afterimage-frost/` | ~500 | FROST RFC 9591 — dealer, participant, aggregator |
| `afterimage-dkg` | `crates/afterimage-dkg/` | ~400 | Pedersen DKG — 3-phase protocol over Ed25519 |
| `afterimage-cli` | `crates/afterimage-cli/` | ~600 | CLI entry-point, keypair loading, password handling |

### Out of Scope (Rust)

- `afterimage-optical` — camera/display I/O; no cryptographic logic
- Solana core runtime and validator
- Squads v4 on-chain program (separately audited by Squads team)
- Third-party RPC providers

### TypeScript / React (Secondary Scope)

| Package | Path | Primary Concern |
|---|---|---|
| `@airsign/react` | `packages/react/` | Hook state machine correctness, WASM module origin validation |
| `signer-web` | `apps/signer-web/` | CSP headers, `__airsign_wasm__` global writability, key material in browser memory |
| `signer-mobile` | `apps/signer-mobile/` | Airplane mode enforcement, Expo SecureStore usage, network access prevention |

---

## 3. Audit Objectives

### Priority 1 — Cryptographic Correctness

- Verify ChaCha20-Poly1305 tag authentication occurs **before** any plaintext is consumed
- Verify Argon2id parameters cannot be reduced below the specified minimums by user input
- Verify FROST partial signature verification rejects invalid signers before aggregation
- Verify Ed25519 signing uses deterministic nonce (RFC 8032 §5.1.6)
- Verify DKG share verification catches malicious dealers (Feldman VSS commitment check)
- Verify nonce uniqueness across sessions (96-bit OS CSPRNG, no counter-based nonce)

### Priority 2 — Protocol Security

- Verify fountain decoder does not accept data after completion (buffer overread)
- Verify frame index and `total_frames` are bounds-checked before memory allocation
- Verify session reset fully wipes nonce state and key material
- Verify the `session_id` prevents cross-session replay attacks

### Priority 3 — On-Chain Safety

- Verify Squads v4 PDA derivation seeds exactly match the on-chain program
- Verify Anchor discriminator computation (`SHA-256("global:<name>")[0..8]`) is correct and unique
- Verify `proposal_approve_ix` sets `signer=true, writable=false` on the approver account
- Verify transaction index 0 is rejected (Squads v4 uses 1-based indices)

### Priority 4 — WASM / Browser

- Verify WASM module is loaded from a trusted origin or bundled hash
- Verify `__airsign_wasm__` global is not writable from external scripts
- Verify `WasmKeyStore.delete()` zeroes memory before dropping
- Verify no private key material is serialised to `localStorage` or `sessionStorage`

### Priority 5 — Supply Chain

- Verify `Cargo.lock` is committed and all dependency hashes are reproducible
- Verify no `unsafe` blocks exist outside `crates/afterimage-wasm/src/lib.rs`
- Verify `cargo audit` passes with no known vulnerabilities

---

## 4. Known `unsafe` Blocks

The only permitted `unsafe` code is in the WASM FFI boundary:

```
crates/afterimage-wasm/src/lib.rs
```

All other crates must be `#![forbid(unsafe_code)]`. Auditor should verify this holds.

---

## 5. Test Coverage Summary

See [`TEST_COVERAGE.md`](TEST_COVERAGE.md) for per-crate test counts and feature matrix.

---

## 6. Engagement Logistics

| Item | Detail |
|---|---|
| Estimated effort | 3–4 auditor-weeks |
| Preferred firms | Trail of Bits, Zellic, OtterSec, Halborn, Neodyme |
| Report format | PDF + Markdown findings, severity: Critical / High / Medium / Low / Informational |
| Remediation window | 30 days from draft report |
| Public disclosure | 90 days after final report or upon fix release, whichever comes first |
| Contact | security@airsign.io (see SECURITY.md) |