# AirSign Known Issues & Limitations

**Version:** 6.0.0  
**Date:** 2026-04-18  
**Audience:** Security auditors, contributors

This document catalogues all known limitations, unresolved design questions, and intentional out-of-scope items. Each entry includes a risk assessment and proposed mitigation or rationale for acceptance.

---

## 1. Cryptographic Design

### KI-001 — No AAD Binding of session_id to AEAD Tag

**Description:** The session_id and frame_index are present in the plaintext frame header but are not included as Additional Authenticated Data (AAD) in the ChaCha20-Poly1305 AEAD call. An attacker who can manipulate the frame header before it reaches the decoder could potentially replay a ciphertext with a modified session_id.

**Current mitigation:** The session key K is derived from a password shared out-of-band. An attacker without K cannot forge an authenticating tag regardless of the session_id in the header. Cross-session replay is further blocked by the nonce uniqueness property.

**Residual risk:** Low — requires knowledge of K to exploit.

**Proposed fix (v6.1):** Pass `session_id || frame_index` as AAD to `encrypt_in_place_detached`.

**Status:** Open — accepted for v6.0.0, scheduled for v6.1.

---

### KI-002 — Metadata Frame is Unauthenticated

**Description:** The first frame (metadata frame) carries the `salt` used for Argon2id KDF in plaintext, without a MAC. An active attacker on the optical channel could substitute the salt, causing the receiver to derive a different key K' and fail decryption.

**Current mitigation:** A substituted salt causes all subsequent frames to fail AEAD authentication → the session simply fails. No secret material is revealed. The attacker cannot cause the receiver to accept a forged transaction because they cannot produce valid AEAD tags without knowing K.

**Residual risk:** Availability (DoS) — attacker can disrupt a signing session by substituting the salt. Confidentiality and integrity are unaffected.

**Proposed fix (v6.1):** Sign the metadata frame with a pre-shared HMAC key or display a human-readable session fingerprint (e.g., the first 4 bytes of the salt as an emoji grid) for the operator to verify visually.

**Status:** Open — DoS accepted, integrity fix planned for v6.1.

---

### KI-003 — FROST t=2 Minimum Not Enforced in DKG Config Validation

**Description:** `afterimage-dkg` delegates threshold validation to `frost-ed25519`, which rejects t=1 but the error message may not distinguish between "t < 2" and "t > n". The DKG crate's own validation returns `InvalidThreshold` for both.

**Current mitigation:** Tests `dkg_invalid_threshold_zero` and `dkg_invalid_threshold_one` verify both cases return errors.

**Residual risk:** Informational — no security impact, only ergonomics.

**Status:** Open — documentation improvement planned.

---

### KI-004 — Ledger APDU Response Parsing Does Not Validate Signature Length

**Description:** `crates/afterimage-solana/src/ledger_apdu.rs` parses the 64-byte Ed25519 signature from the APDU response by slicing bytes 0..64 without checking the response length first.

**Current mitigation:** A short response from the Ledger app would cause a panic (`index out of bounds`). In practice, the Solana Ledger app always returns exactly 64 bytes on success.

**Residual risk:** Low — panic is non-exploitable (Rust memory safety); only reachable if a malicious or buggy Ledger app returns a short response.

**Proposed fix:** Add `if response.len() < 64 { return Err(LedgerError::InvalidResponse); }` before slicing.

**Status:** Open — fix is trivial, scheduled for next patch.

---

## 2. Protocol Design

### KI-005 — No Forward Secrecy Within a Session

**Description:** If the Argon2id-derived key K is compromised after a session completes, all frames from that session can be decrypted (the session is not forward-secret at the frame level).

**Current mitigation:** K is derived from the user's password, which is not stored. Each session uses a fresh random salt. Once the session is complete, K should be zeroed from memory (verified by Zeroize trait usage on key material in `afterimage-core`).

**Residual risk:** Medium — if a core dump or memory forensic tool captures K during the session window, past frames are decryptable. This is inherent to password-based protocols without a DH exchange.

**Status:** Accepted design limitation. A future version may use an ephemeral X25519 key exchange over the optical channel for per-session PFS.

---

### KI-006 — Fountain Decoder Does Not Enforce Maximum Allocation

**Description:** The fountain decoder allocates a buffer of size `payload_len` derived from the `total_frames` field in the metadata frame. There is no maximum bound on `total_frames`, which could cause a large allocation on a resource-constrained device.

**Residual risk:** Medium — potential DoS on embedded / mobile devices. Not a remote code execution risk.

**Proposed fix:** Cap `total_frames` at a reasonable maximum (e.g., 10 000 frames, corresponding to ~40 MB payload).

**Status:** Open — fix scheduled for v6.1.

---

## 3. Implementation

### KI-007 — `unsafe` in WASM FFI Boundary

**Description:** `crates/afterimage-wasm/src/lib.rs` contains `unsafe` code required by `wasm-bindgen` for JS interop (raw pointer handling in the generated glue code). This is unavoidable for WASM.

**Scope:** All other crates must have `#![forbid(unsafe_code)]`. Auditor should verify this assertion with `grep -r "unsafe" crates/ --exclude-dir=afterimage-wasm`.

**Status:** Accepted — WASM FFI boundary is the only permitted location.

---

### KI-008 — Password Held in Process Memory During KDF

**Description:** The Argon2id password string is held in Rust heap memory during the ~0.5s KDF computation. On UNIX systems, this memory is swappable.

**Proposed mitigation:** Use `mlock` to pin the password buffer during KDF, then zero with `zeroize`. This is planned but not yet implemented.

**Status:** Open — medium priority.

---

### KI-009 — Browser `localStorage` Not Checked at Startup

**Description:** The `signer-web` application does not explicitly verify that no AirSign key material was accidentally written to `localStorage` or `sessionStorage` in a previous (potentially buggy) version.

**Proposed fix:** On startup, scan `localStorage` for any key with a prefix matching `airsign:` and warn the operator.

**Status:** Open — low priority.

---

## 4. Mobile App (apps/signer-mobile)

### KI-010 — Airplane Mode Enforcement is Advisory

**Description:** The `AirplaneModeGuard` component checks network reachability via `expo-network` and blocks the signing UI if a network interface is reachable. However, on rooted/jailbroken devices, a malicious app could bypass this check by manipulating the OS-level API response.

**Residual risk:** Low — users who run the signer on a rooted device are outside the supported threat model. The guard is a UX safety net, not a security boundary.

**Status:** Accepted — documented in SECURITY.md.

---

### KI-011 — Expo SecureStore Has Platform-Specific Limits

**Description:** On iOS, `expo-secure-store` backed by Keychain Services has a per-item size limit of 4 KB. A 64-byte Ed25519 keypair + metadata is well within this limit. However, FROST key shares (which are larger JSON structures) may approach or exceed this limit for large n.

**Proposed fix:** For FROST key packages, store only the 32-byte seed in SecureStore and re-derive the full key package on demand.

**Status:** Open — to be addressed in mobile app v1.0.

---

## 5. Supply Chain

### KI-012 — `cargo audit` Advisory DB Lag

**Description:** The `cargo-audit` advisory database may lag behind public CVE disclosures by hours to days. CI runs `cargo audit` on every push, but a zero-day in a dependency would not be caught until the advisory is published.

**Status:** Accepted — standard limitation of supply chain scanning tools. Mitigated by Dependabot alerts and pinned `Cargo.lock`.

---

## 6. Out of Scope (Not Bugs)

| Item | Reason |
|---|---|
| Solana validator security | Outside project boundary |
| Squads v4 on-chain program correctness | Audited separately by Squads team |
| Physical theft of the air-gap device | Operational security, not software |
| Side-channel attacks via QR camera timing | Physical layer, not software |
| Coercion / rubber-hose attacks | Out of scope for any software system |
| Compromised camera firmware | Hardware trust boundary |