# AirSign Cryptographic Specification

**Version:** 6.0.0  
**Date:** 2026-04-18  
**Audience:** Security auditors, cryptographers

---

## 1. Cryptographic Primitives

| Layer | Algorithm | Parameters | RFC / Standard | Rust Crate |
|---|---|---|---|---|
| Symmetric encryption | ChaCha20-Poly1305 | 256-bit key, 96-bit nonce, 128-bit tag | RFC 8439 | `chacha20poly1305` (RustCrypto) |
| Key derivation | Argon2id | m=65536 KiB, t=3, p=1, output=32B | RFC 9106 | `argon2` (RustCrypto) |
| Signing | Ed25519 | 255-bit Edwards curve | RFC 8032 | `ed25519-dalek` v2 |
| Threshold signing | FROST-Ed25519 | t-of-n, min t=2, max n=255 | RFC 9591 | `frost-ed25519` |
| Hash (discriminators) | SHA-256 | first 8 bytes used | FIPS 180-4 | `sha2` (RustCrypto) |
| CSPRNG | OS getrandom | 96-bit nonces, 16-byte salts | — | `getrandom` |
| Serialization | Borsh | — | Borsh spec | `borsh` |

---

## 2. Session Key Derivation

### 2.1 Parameters (non-overridable)

```rust
// crates/afterimage-core/src/crypto.rs
pub const ARGON2_M_COST: u32 = 65536;   // 64 MiB
pub const ARGON2_T_COST: u32 = 3;
pub const ARGON2_P_COST: u32 = 1;
pub const SALT_LEN: usize    = 16;
pub const KEY_LEN: usize     = 32;
```

These constants are embedded in the protocol wire frame and cannot be reduced by user input. The CLI `--argon2-mem` and `--argon2-iter` flags accept values only ≥ the published constants.

### 2.2 Derivation

```
salt     ← CSPRNG(16 bytes)           # fresh per session
K        ← Argon2id(password, salt, m=65536, t=3, p=1, len=32)
```

`salt` is transmitted in the first fountain frame (version byte + session_id + salt, plaintext, unauthenticated). Auditor should verify that no secret material is carried in the metadata frame.

### 2.3 Security Level

- Against GPU brute-force (RTX 4090): ~2 attempts/second
- 5-word Diceware passphrase (7776-word list): ~64 bits entropy → ~10¹¹ years

---

## 3. Frame Encryption

### 3.1 Frame Layout (protocol v2 / v3)

```
[ version:       u8           ]   1 byte
[ session_id:    [u8; 16]     ]  16 bytes
[ frame_index:   u32 LE       ]   4 bytes
[ total_frames:  u32 LE       ]   4 bytes
[ dropout_symbols: u32 LE     ]   4 bytes  (fountain code degree)
[ nonce:         [u8; 12]     ]  12 bytes
[ ciphertext:    variable     ]  ≥ 1 byte
[ poly1305_tag:  [u8; 16]     ]  16 bytes  (AEAD tag, appended by chacha20poly1305)
```

Total overhead per frame: 57 bytes + ciphertext length.

### 3.2 Nonce Generation

```rust
let nonce = Nonce::from_slice(&getrandom_bytes::<12>());
```

Each frame carries an **independent random 12-byte nonce**. There is no counter-based nonce; nonces are not derived from `frame_index`. This means:

- No nonce reuse risk across frames within the same session
- No nonce reuse risk across sessions (statistically: collision probability < 2⁻⁸⁰ per pair)

### 3.3 Authentication

ChaCha20-Poly1305 is an AEAD cipher. The Poly1305 tag covers:
- The nonce (implicit, bound to the key stream)
- The ciphertext

Frames with invalid tags are **silently discarded** by the fountain decoder without exposing error timing.

### 3.4 Additional Authenticated Data (AAD)

Currently no explicit AAD is included. The session_id and frame_index are not bound to the AEAD tag. **Auditor note:** Evaluate whether binding session_id to the AEAD tag (as AAD) would improve session isolation guarantees.

---

## 4. Ed25519 Signing

All Ed25519 operations use `ed25519-dalek` v2, which implements RFC 8032 §5.1.6 deterministic nonce generation (SHA-512 of the private key scalar + message). There is no user-supplied randomness in the signing path.

### Key Storage at Rest

```
stored_file = {
  salt:       [u8; 16],    // random per key, stored plaintext
  nonce:      [u8; 12],    // random per write
  ciphertext: Vec<u8>,     // ChaCha20-Poly1305(Argon2id(password, salt), nonce, keypair_bytes)
}
```

`keypair_bytes` is the 64-byte Solana keypair (32-byte seed || 32-byte pubkey). The seed is the Ed25519 private scalar.

---

## 5. FROST Threshold Signing (RFC 9591)

### 5.1 Key Generation — Trusted Dealer Mode

```
Dealer:
  secret     ← random_scalar()
  polynomial ← [secret, a_1, …, a_{t-1}]  where a_i ← random_scalar()
  For each participant i in 1..=n:
    share_i = polynomial.evaluate(i)        # Lagrange evaluation over Ed25519 scalar field
  group_pubkey = secret × G
```

Each `share_i` is transmitted to participant `i` via QR air-gap. The dealer's copy of `secret` is zeroed after distribution.

### 5.2 Key Generation — DKG Mode (RFC 9591 §5 / Pedersen)

Each participant `i` runs:

```
Round 1:
  f_i(x) = a_{i,0} + a_{i,1}x + … + a_{i,t-1}x^{t-1}   (random polynomial)
  Commitment_i = [a_{i,j} × G for j in 0..t]              (Feldman VSS commitments)
  Broadcast (Commitment_i, Proof_i)                        (Schnorr PoK of a_{i,0})

Round 2:
  For each j ≠ i:
    Send f_i(j) privately to participant j
  Verify received shares against commitments:
    f_k(i) × G == sum(Commitment_k[j] × i^j for j in 0..t)
    Abort if verification fails (identifies malicious dealer)

Finish:
  secret_share_i = sum(f_k(i) for all k)
  group_pubkey   = sum(Commitment_k[0] for all k)
```

AirSign's `afterimage-dkg` wraps `frost-ed25519`'s DKG implementation. The `dkg_finish` function returns a `KeyPackage` compatible with `WasmFrostParticipant`.

### 5.3 Signing Protocol (2 rounds)

```
Round 1 (Commit):
  (hiding_nonce_i, binding_nonce_i)             ← CSPRNG
  (hiding_commitment_i, binding_commitment_i)   = nonces × G
  Broadcast commitments to coordinator

Round 2 (Sign):
  Coordinator broadcasts SigningPackage = (message, all_commitments)
  binding_factor_i = H(group_pubkey, message, commitments, i)
  nonce_i          = hiding_nonce_i + binding_factor_i × binding_nonce_i
  lambda_i         = Lagrange_coefficient(participant_indices, i)
  partial_sig_i    = nonce_i + lambda_i × secret_share_i × challenge

Aggregation:
  R = sum(hiding_commitment_i + binding_factor_i × binding_commitment_i)
  s = sum(partial_sig_i)
  signature = (R, s)   ← standard Ed25519 Schnorr signature
```

The final `(R, s)` is a standard Ed25519 signature verifiable against `group_pubkey` with no FROST-specific knowledge required on-chain.

### 5.4 Security Properties

| Property | Mechanism |
|---|---|
| Unforgeability | t-1 colluding participants cannot produce a valid signature |
| Identifiable abort | Each partial_sig is verified before aggregation; invalid signers are identified |
| Non-interactive aggregation | Aggregator is untrusted; it only combines public values |
| Nonce binding | `binding_factor` ties each nonce to the specific message and all commitments |

---

## 6. Squads v4 Instruction Serialisation

### 6.1 Anchor Discriminator

```
discriminator[instruction_name] = SHA-256("global:<instruction_name>")[0..8]
```

Pre-computed values (verified by unit tests):

| Instruction | Discriminator (hex) |
|---|---|
| `multisig_create_v2` | `6e 55 aa 79 a5 fb 14 24` |
| `vault_transaction_create` | `71 d2 90 de 4d 48 51 3a` |
| `proposal_create` | `93 91 3c a0 c3 a3 d0 a0` |
| `proposal_approve` | `ea ec 78 6c 0f 59 a5 57` |
| `proposal_reject` | `7e a6 7b 19 39 5e 60 58` |
| `vault_transaction_execute` | `c8 96 89 8a 95 6d 4f 62` |

These are verified deterministic by `test discriminator_is_deterministic` and `test discriminators_are_distinct`.

### 6.2 PDA Derivation

```rust
fn multisig_pda(create_key: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"multisig", create_key.as_ref()],
        &SQUADS_V4_PROGRAM_ID,
    )
}

fn vault_pda(multisig_pda: &Pubkey, vault_index: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"vault", multisig_pda.as_ref(), &[vault_index]],
        &SQUADS_V4_PROGRAM_ID,
    )
}
```

Seeds are verified against known vectors in `test derive_multisig_pda_is_deterministic`.

---

## 7. Keystore Encryption

### Software Keystore

```
salt     ← CSPRNG(16 bytes)
nonce    ← CSPRNG(12 bytes)
key      ← Argon2id(password, salt, m=65536, t=3, p=1, len=32)
ciphertext = ChaCha20-Poly1305(key, nonce, keypair_64_bytes)
stored   = { "salt": hex(salt), "nonce": hex(nonce), "ciphertext": hex(ciphertext) }
```

### OS Keychain (via `keyring` v3)

Key material is stored in the platform keychain (macOS Keychain Services, Linux Secret Service, Windows Credential Store). AirSign does not perform additional encryption on top of the platform keychain — the platform is trusted for this layer.

---

## 8. References

- [RFC 9591](https://www.rfc-editor.org/rfc/rfc9591) — FROST
- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439) — ChaCha20-Poly1305
- [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106) — Argon2
- [RFC 8032](https://www.rfc-editor.org/rfc/rfc8032) — EdDSA / Ed25519
- [Anchor Discriminator](https://www.anchor-lang.com/docs/anchor-discriminator)
- [Squads v4 Source](https://github.com/Squads-Protocol/v4)