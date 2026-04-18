# AirSign Test Coverage Report

**Version:** 6.0.0  
**Date:** 2026-04-18

---

## 1. Rust Crate Test Summary

| Crate | Tests | Result | Key Areas Covered |
|---|---|---|---|
| `afterimage-core` | 44 | ✅ all pass | Argon2id KDF, ChaCha20-Poly1305 roundtrip, fountain encode/decode, session state machine, `SecurityProfile` presets, nonce freshness |
| `afterimage-wasm` | 29 | ✅ all pass | `WasmSendSession` frame ticking, `WasmRecvSession` ingest + completion, `WasmSquads` PDA + instruction builders, `WasmBroadcaster` URL helpers, `WasmKeyStore` generate/load/delete |
| `afterimage-solana` | 12+ | ✅ all pass | `WatchWallet` / `TransactionBuilder`, `TransactionInspector` all instruction types + risk flags, `PreflightChecker`, `LedgerApdu` HID framing, `KeyStore` lifecycle, `Broadcaster` URL helpers |
| `afterimage-squads` | 64 | ✅ all pass | Discriminator uniqueness + determinism, PDA derivation (multisig, vault, tx, proposal), all instruction builders, member permission bitmask, adapter round-trip |
| `afterimage-frost` | 19 | ✅ all pass | 2-of-2, 2-of-3, 3-of-5 full round-trips, invalid t=1 rejection, nonce uniqueness, share mismatch detection |
| `afterimage-dkg` | 23 | ✅ all pass | 2-of-2, 2-of-3, 3-of-5 DKG round-trips, invalid identifier (0), invalid threshold (t=0, t=1, t>n), malformed packages, FROST signing with DKG-derived keys |
| `afterimage-cli` | 3 | ✅ all pass | `resolve_rpc_url` cluster resolution, `sol_to_lamports` conversion, CLI smoke tests |

**Total Rust tests: 194+ passing, 0 failing**

---

## 2. TypeScript Test Summary

| Package | Tests | Runner | Key Areas Covered |
|---|---|---|---|
| `@airsign/react` | 19 | Vitest + jsdom | `useSendSession` (10 tests): idle state, start/stop/reset, frame advancement, `onComplete`, `onProgress`, missing WASM error, unmount cleanup; `useRecvSession` (9 tests): idle state, ingest progress, completion, `onComplete` with payload + filename, `onProgress`, reset, missing WASM error, no duplicate `onComplete` |

**Total TypeScript tests: 19 passing, 0 failing**

---

## 3. Per-Crate Feature Matrix

### `afterimage-core`

| Feature | Tested | Test Name(s) |
|---|---|---|
| Argon2id with OWASP-2024 params | ✅ | `security_profile_owasp2024` |
| Argon2id with mainnet params | ✅ | `security_profile_mainnet` |
| ChaCha20-Poly1305 encrypt + decrypt | ✅ | `encrypt_decrypt_roundtrip` |
| Wrong key → MAC failure | ✅ | `decrypt_wrong_key_fails` |
| Fountain encode → decode | ✅ | `fountain_roundtrip_*` |
| Session v3 send+recv roundtrip | ✅ | `send_recv_v3_*` |
| SecurityProfile::from_str aliases | ✅ | `security_profile_from_str_*` |
| Argon2Params::meets_mainnet_minimum | ✅ | `argon2_meets_mainnet_minimum` |
| MetadataFrame short slice → correct error | ✅ | `metadata_too_short_before_magic` |

### `afterimage-squads`

| Feature | Tested | Test Name(s) |
|---|---|---|
| Discriminator uniqueness | ✅ | `discriminators_are_distinct` |
| Discriminator determinism | ✅ | `discriminator_is_deterministic` |
| Multisig PDA derivation | ✅ | `derive_multisig_pda_is_deterministic` |
| Vault PDA derivation | ✅ | `derive_vault_pda_is_deterministic` |
| `multisig_create_v2` success | ✅ | `multisig_create_v2_success` |
| Empty members → error | ✅ | `multisig_create_v2_empty_members` |
| threshold=0 → error | ✅ | `multisig_create_v2_zero_threshold` |
| threshold > member count → error | ✅ | `multisig_create_v2_threshold_too_high` |
| Duplicate member → error | ✅ | `multisig_create_v2_duplicate_member` |
| `proposal_approve_ix` | ✅ | `proposal_approve_ix_*` |
| `vault_transaction_execute_ix` | ✅ | `vault_transaction_execute_ix_*` |
| Adapter round-trip | ✅ | `adapter_roundtrip_*` |

### `afterimage-frost`

| Feature | Tested | Test Name(s) |
|---|---|---|
| 2-of-2 threshold signing | ✅ | `frost_2_of_2_roundtrip` |
| 2-of-3 threshold signing | ✅ | `frost_2_of_3_roundtrip` |
| 3-of-5 threshold signing | ✅ | `frost_3_of_5_roundtrip` |
| t=1 rejected | ✅ | `frost_t1_rejected` |
| n < t rejected | ✅ | `frost_n_lt_t_rejected` |
| Nonce uniqueness per participant | ✅ | `frost_nonce_uniqueness` |
| Share mismatch detection | ✅ | `frost_share_mismatch_detected` |
| Output is valid Ed25519 signature | ✅ | `frost_output_is_valid_ed25519` |

### `afterimage-dkg`

| Feature | Tested | Test Name(s) |
|---|---|---|
| 2-of-3 DKG round-trip | ✅ | `dkg_2_of_3_roundtrip` |
| 3-of-5 DKG round-trip | ✅ | `dkg_3_of_5_roundtrip` |
| Identifier 0 rejected | ✅ | `dkg_invalid_identifier_zero` |
| threshold=0 rejected | ✅ | `dkg_invalid_threshold_zero` |
| threshold=1 rejected | ✅ | `dkg_invalid_threshold_one` |
| threshold > n rejected | ✅ | `dkg_threshold_exceeds_n` |
| Group pubkey consistent across participants | ✅ | `dkg_group_pubkey_consistent` |
| DKG keys work with FROST signing | ✅ | `dkg_keys_work_with_frost` |
| Nonce freshness per round1 call | ✅ | `dkg_nonce_freshness` |

### `afterimage-wasm` (e2e.rs)

| Feature | Tested |
|---|---|
| `WasmSendSession` frame generation | ✅ |
| `WasmRecvSession` ingest + decode | ✅ |
| Send→Recv roundtrip (various payload sizes) | ✅ |
| `WasmSquads::derive_pda` | ✅ |
| `WasmSquads::create_multisig` | ✅ |
| `WasmSquads::approve_proposal` | ✅ |
| `WasmSquads::vault_transaction_create` | ✅ |
| `WasmBroadcaster::get_balance` URL | ✅ |
| `WasmBroadcaster::request_airdrop` mainnet guard | ✅ |
| `WasmKeyStore` generate + load roundtrip | ✅ |
| `WasmKeyStore` wrong password → error | ✅ |
| `WasmKeyStore` delete + exists | ✅ |

---

## 4. Coverage Gaps (for auditor attention)

| Area | Gap | Risk |
|---|---|---|
| Fountain decoder overflow | No fuzz test for malformed `total_frames` values | Medium |
| AEAD tag truncation | No test for 15-byte vs 16-byte tag | Low |
| Argon2id CLI flag floor | No test that rejects `--argon2-mem 1024` at CLI level | Medium |
| DKG malicious dealer | No test where a participant sends an invalid share and the DKG aborts cleanly | High |
| WASM memory zeroing | No test verifying `WasmKeyStore.delete()` zeroes heap | Medium |
| Cross-session replay | No test that a frame from session A is rejected by session B | Medium |

---

## 5. How to Run Tests

```bash
# All Rust tests (excluding afterimage-solana which requires live HID)
cargo test --workspace --exclude afterimage-solana 2>&1

# afterimage-solana unit tests only
cargo test -p afterimage-solana --lib 2>&1

# TypeScript tests
cd packages/react && npm test

# WASM e2e (native, no browser needed)
cargo test -p afterimage-wasm 2>&1

# With coverage (requires cargo-llvm-cov)
cargo llvm-cov --workspace --exclude afterimage-solana --html