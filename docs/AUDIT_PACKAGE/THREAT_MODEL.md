# AirSign Threat Model

**Version:** 6.0.0  
**Date:** 2026-04-18  
**Methodology:** STRIDE

---

## 1. System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                     TRUST BOUNDARY                              │
│  ┌──────────────────────┐          ┌──────────────────────────┐ │
│  │   ONLINE MACHINE     │  QR-only │   OFFLINE SIGNER         │ │
│  │  (signer-web / CLI)  │◄────────►│  (signer-mobile / CLI)   │ │
│  │                      │  optical │                          │ │
│  │  • Build unsigned tx │  channel │  • Decrypt tx payload    │ │
│  │  • Encrypt + display │          │  • Display tx for review │ │
│  │  • Receive signed tx │          │  • Sign with keypair     │ │
│  │  • Broadcast to RPC  │          │  • Encrypt + display     │ │
│  └──────────────────────┘          └──────────────────────────┘ │
│           │                                                      │
│           ▼                                                      │
│   Solana RPC / Squads v4 program                                 │
└─────────────────────────────────────────────────────────────────┘
```

**Trust boundaries:**
- The online machine is assumed to be potentially fully compromised (attacker has root)
- The offline device is assumed to be physically secure (operational security responsibility of operator)
- The optical channel is one-way and unauthenticated at the channel level; authentication is via AEAD

---

## 2. STRIDE Analysis

### 2.1 Spoofing

| ID | Threat | Attack Scenario | Mitigation | Residual Risk |
|---|---|---|---|---|
| S-01 | Spoof online machine | Attacker presents a malicious unsigned transaction to the offline signer | Transaction Inspector parses and displays all instruction details before signing. Operator must confirm. | Low — requires operator to not read the display |
| S-02 | Spoof signed response | Attacker replays a previous signed transaction as a "new" response | Each session uses a fresh session_id + random nonce. AEAD tag covers the ciphertext; without K, forging a tag is infeasible. | Very Low |
| S-03 | Spoof Squads multisig member | Attacker adds their pubkey to a create_multisig instruction | Transaction Inspector flags all signers. Operator verifies pubkeys on the display. | Low — requires operator inattention |

### 2.2 Tampering

| ID | Threat | Attack Scenario | Mitigation | Residual Risk |
|---|---|---|---|---|
| T-01 | Tamper with QR frame in transit | Attacker with a camera between the two devices modifies QR content | ChaCha20-Poly1305 tag authentication — any bit flip in the ciphertext or nonce causes AEAD failure and frame is discarded | Very Low |
| T-02 | Tamper with metadata frame | Attacker substitutes the salt in the plaintext metadata frame | Causes KDF to derive K' ≠ K; all subsequent AEAD tags fail; session is disrupted but no secret is revealed (DoS only) | Low — see KI-002 |
| T-03 | Tamper with Squads PDA seeds | Attacker supplies a forged create_key to derive a different multisig PDA | PDA derivation is deterministic; operator must verify the displayed multisig address on-chain before approving | Medium — UX-dependent |
| T-04 | Tamper with Rust dependencies | Supply chain attack on a crate used by afterimage-core | cargo audit + cargo deny in CI; Cargo.lock pins all hashes | Medium — residual supply chain risk |

### 2.3 Repudiation

| ID | Threat | Attack Scenario | Mitigation | Residual Risk |
|---|---|---|---|---|
| R-01 | Deny signing a transaction | Signer claims they did not sign a transaction | Ed25519 signature is deterministic and verifiable on-chain; the signature is public record | Very Low |
| R-02 | Deny initiating a FROST signing session | A FROST coordinator claims a different message was signed | FROST signing package binds to the specific message; partial signatures are verifiable per participant | Low |

### 2.4 Information Disclosure

| ID | Threat | Attack Scenario | Mitigation | Residual Risk |
|---|---|---|---|---|
| I-01 | Key extraction from online machine | Attacker compromises online machine and extracts private key | Private key never exists on the online machine. Only the encrypted fountain payload is present. | None — by design |
| I-02 | Key extraction from QR channel | Attacker captures all QR frames and attempts to decrypt | All payloads are ChaCha20-Poly1305 encrypted. Without the password, decryption is infeasible (128-bit security). | Very Low |
| I-03 | Key extraction from browser memory | JS heap snapshot reveals key material | Key material is only in WASM linear memory on the offline device; WASM heap is not accessible from the JS heap without explicit export | Low |
| I-04 | Key extraction from mobile app | Memory forensic on the offline phone | Expo SecureStore uses iOS Keychain / Android Keystore hardware-backed storage; key is only in memory during active signing | Low — requires physical device access |
| I-05 | Password exposure via process list | Password passed as CLI argument is visible in ps output | airsign CLI reads password from stdin or environment variable, not command-line arguments | Low |

### 2.5 Denial of Service

| ID | Threat | Attack Scenario | Mitigation | Residual Risk |
|---|---|---|---|---|
| D-01 | Metadata frame substitution | Attacker substitutes salt → wrong K → all frames fail | Session fails; operator retries. No data loss — the unsigned transaction is reconstructed. | Low — availability impact only |
| D-02 | Fountain frame flooding | Attacker sends thousands of malformed frames | Frames with invalid AEAD tags are silently discarded; decoder state is unchanged | Very Low |
| D-03 | Large payload allocation | Malicious metadata frame sets total_frames=MAX_U32 | No upper bound on allocation (KI-006). Fix: cap total_frames. | Medium — see KI-006 |
| D-04 | RPC DoS | Attacker floods the RPC endpoint used by the online machine | Online machine uses user-supplied RPC; operator selects a reliable provider | Low — operational |

### 2.6 Elevation of Privilege

| ID | Threat | Attack Scenario | Mitigation | Residual Risk |
|---|---|---|---|---|
| E-01 | FROST share reconstruction | t-1 colluding participants attempt to reconstruct the group secret | FROST (RFC 9591) is secure against t-1 colluders; requires t shares to reconstruct | Very Low |
| E-02 | DKG malicious dealer | A participant sends an invalid share to trigger abort and social-engineer a retry | Feldman VSS commitments allow per-participant verification; invalid shares are identified by identifier | Low |
| E-03 | Squads proposal escalation | Attacker gains access to a member keypair and approves a malicious proposal | Squads v4 threshold enforcement is on-chain; approval requires threshold members. AirSign only builds instructions; it does not bypass on-chain checks. | Very Low |
| E-04 | WASM module substitution | Attacker replaces afterimage_wasm.js with a malicious version | WASM is served from the same origin or bundled. CSP `script-src 'self'` prevents external script injection. | Low — CSP-dependent |

---

## 3. Attack Trees

### 3.1 Primary Attack: Sign a Malicious Transaction

```
Goal: Get the offline signer to sign a transaction the operator did not intend

OR
├── Compromise the offline device (physical access / malware) [HIGH effort, out of scope]
├── Trick the operator into approving a malicious transaction
│   OR
│   ├── Build a crafted unsigned tx that looks benign but has malicious instructions
│   │   → Mitigated by TransactionInspector risk flags + operator review
│   └── Display a benign tx on the online machine but send a different tx via QR
│       → Impossible: the QR payload IS the encrypted tx; display and payload are identical
└── Break ChaCha20-Poly1305 without K
    → Computationally infeasible (128-bit authentication tag)
```

### 3.2 Secondary Attack: Extract Private Key

```
Goal: Obtain the Ed25519 private key

OR
├── Compromise online machine → key not present there [none]
├── Intercept QR channel → encrypted with K, infeasible without password [very low]
├── Compromise offline device physically [out of scope]
├── Brute-force password
│   → Argon2id: ~2 attempts/second on RTX 4090
│   → 5-word Diceware: 64 bits entropy → 10^11 years
└── WASM/JS memory leak → key only in WASM linear memory, not JS heap [low]
```

---

## 4. Security Properties Guaranteed

| Property | Guarantee | Mechanism |
|---|---|---|
| Air-gap | Online machine cannot sign | Private key only on offline device |
| Integrity | Malformed/tampered frames rejected | ChaCha20-Poly1305 AEAD |
| Confidentiality | Payload unreadable without password | ChaCha20-Poly1305(Argon2id(password)) |
| Authenticity | Forged transactions rejected | AEAD tag + operator review |
| Non-repudiation | Signatures are on-chain fact | Ed25519 deterministic signing |
| Threshold security | t-1 colluders cannot sign | FROST RFC 9591 |
| Forward secrecy | Per-session fresh salt | Argon2id fresh salt per session (partial — see KI-005) |

---

## 5. Operational Security Requirements

These are outside AirSign's software scope but must be followed by operators:

1. The offline device must never connect to any network
2. The offline device must boot from a verified OS image (ideally read-only media)
3. The password must be a minimum 5-word Diceware passphrase or equivalent entropy
4. FROST key shares must be distributed across devices in different physical locations
5. Operators must read and confirm the full transaction display before signing
6. The offline device's display must be shielded from cameras not controlled by the operator