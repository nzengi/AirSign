//! afterimage_core::crypto
//! =======================
//! ChaCha20-Poly1305 AEAD encryption with Argon2id key derivation.
//!
//! # Security design
//!
//! * **Argon2id** (RFC 9106, OWASP 2024): memory-hard KDF resistant to GPU/ASIC
//!   brute-force attacks. Default parameters: m=65536 KiB, t=3, p=4.
//! * **ChaCha20-Poly1305**: 256-bit authenticated encryption (RFC 8439).
//!   Every `encrypt()` call uses a fresh random 16-byte salt and 12-byte nonce,
//!   so the same password + plaintext pair always yields different ciphertext.
//! * **Zeroize on drop**: derived keys are wiped from memory when they go out of
//!   scope, preventing leakage via swap, core dumps, or cold-boot attacks.
//! * **BLAKE3 integrity header**: an optional 32-byte BLAKE3 digest of the
//!   plaintext is prepended before encryption, enabling fast pre-auth data
//!   integrity checks at the application layer without breaking AEAD semantics.
//!
//! # Wire format (v2/v3 — Argon2id)
//!
//! ```text
//! salt (16 B) || nonce (12 B) || ciphertext+tag (N+16 B)
//! ```
//!
//! The protocol-version byte and Argon2id parameters live in the METADATA frame,
//! not here.  Protocol v1 (Python compat) uses PBKDF2-SHA256 with the same binary
//! layout.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(feature = "argon2")]
use argon2::{Algorithm, Argon2, Params, Version};

use crate::error::CryptoError;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Random salt length in bytes (Argon2id & PBKDF2).
pub const SALT_LEN: usize = 16;
/// ChaCha20-Poly1305 nonce length.
pub const NONCE_LEN: usize = 12;
/// ChaCha20-Poly1305 key length (256-bit).
pub const KEY_LEN: usize = 32;
/// Poly1305 authentication tag length.
pub const TAG_LEN: usize = 16;
/// Minimum blob length: salt + nonce + tag (zero-length plaintext).
pub const MIN_BLOB_LEN: usize = SALT_LEN + NONCE_LEN + TAG_LEN;

// ─── Argon2id parameter constants ─────────────────────────────────────────────

/// Default (OWASP 2024 minimum) memory cost in KiB — 64 MiB.
pub const ARGON2_M_COST: u32 = 65_536;
/// Default (OWASP 2024 minimum) time (iteration) cost.
pub const ARGON2_T_COST: u32 = 3;
/// Default degree of parallelism.
pub const ARGON2_P_COST: u32 = 4;

/// Mainnet-recommended memory cost in KiB — 256 MiB.
///
/// Using this parameter makes offline dictionary attacks ~4× harder than the
/// OWASP 2024 minimum.  Recommended for any session that signs mainnet-beta
/// transactions or holds high-value assets.
pub const ARGON2_M_COST_MAINNET: u32 = 262_144;
/// Mainnet-recommended time (iteration) cost.
pub const ARGON2_T_COST_MAINNET: u32 = 4;

/// Paranoid-level memory cost in KiB — 512 MiB.
///
/// Suitable for extremely high-value signing sessions where the operator can
/// afford a ~4-second KDF latency.  Provides ~8× the brute-force resistance of
/// the OWASP 2024 minimum.
pub const ARGON2_M_COST_PARANOID: u32 = 524_288;
/// Paranoid-level time (iteration) cost.
pub const ARGON2_T_COST_PARANOID: u32 = 5;

// ─── PBKDF2 parameters (v1 / Python-compat) ──────────────────────────────────

/// PBKDF2-SHA256 iterations used by the Python v1 implementation.
pub const PBKDF2_ITERATIONS: u32 = 600_000;

// ─── Argon2id parameter set ───────────────────────────────────────────────────

/// Argon2id key-derivation parameters.
///
/// Both the sender and receiver must agree on these values; they are embedded
/// in the METADATA frame (protocol v3+) so the receiver can reconstruct the
/// exact same key without out-of-band configuration.
///
/// # Example — mainnet hardened settings
/// ```
/// use afterimage_core::crypto::Argon2Params;
///
/// let params = Argon2Params { m_cost: 131_072, t_cost: 4, p_cost: 4 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    /// Memory cost in KiB (must be ≥ 8 × `p_cost`). Default: 65 536 (64 MiB).
    pub m_cost: u32,
    /// Time (iteration) cost (must be ≥ 1). Default: 3.
    pub t_cost: u32,
    /// Degree of parallelism (must be ≥ 1). Default: 4.
    pub p_cost: u32,
}

impl Default for Argon2Params {
    /// OWASP 2024 minimum: m=64 MiB, t=3, p=4.
    fn default() -> Self {
        Self {
            m_cost: ARGON2_M_COST,
            t_cost: ARGON2_T_COST,
            p_cost: ARGON2_P_COST,
        }
    }
}

impl Argon2Params {
    /// Returns `true` if the parameters meet the mainnet minimum security level
    /// (m ≥ 256 MiB **and** t ≥ 4).
    ///
    /// Use this to warn operators who are about to sign mainnet transactions with
    /// sub-optimal KDF settings.
    pub fn meets_mainnet_minimum(&self) -> bool {
        self.m_cost >= ARGON2_M_COST_MAINNET && self.t_cost >= ARGON2_T_COST_MAINNET
    }

    /// Return a human-readable security-level label for these parameters.
    ///
    /// | Label        | Condition                                         |
    /// |---|---|
    /// | `"paranoid"` | m ≥ 512 MiB **and** t ≥ 5                        |
    /// | `"mainnet"`  | m ≥ 256 MiB **and** t ≥ 4                        |
    /// | `"owasp-2024"` | m ≥ 64 MiB **and** t ≥ 3 (OWASP 2024 minimum) |
    /// | `"weak"`     | below OWASP 2024 minimum                          |
    pub fn security_level(&self) -> &'static str {
        if self.m_cost >= ARGON2_M_COST_PARANOID && self.t_cost >= ARGON2_T_COST_PARANOID {
            "paranoid"
        } else if self.m_cost >= ARGON2_M_COST_MAINNET && self.t_cost >= ARGON2_T_COST_MAINNET {
            "mainnet"
        } else if self.m_cost >= ARGON2_M_COST && self.t_cost >= ARGON2_T_COST {
            "owasp-2024"
        } else {
            "weak"
        }
    }
}

// ─── Security presets ─────────────────────────────────────────────────────────

/// Named Argon2id security preset.
///
/// Each variant maps to a concrete [`Argon2Params`] value. Use
/// [`SecurityProfile::to_params`] to resolve the preset, and the `--security-profile`
/// CLI flag to select it from the command line.
///
/// | Profile    | Memory   | Iterations | Use case                            |
/// |---|---|---|---|
/// | `Owasp2024`| 64 MiB   | t=3        | Low-value devnet / testnet sessions |
/// | `Mainnet`  | 256 MiB  | t=4        | Mainnet transactions (recommended)  |
/// | `Paranoid` | 512 MiB  | t=5        | Extreme-value, latency-tolerant ops |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityProfile {
    /// OWASP 2024 minimum — 64 MiB, t=3, p=4. Default for all sessions.
    Owasp2024,
    /// Mainnet-recommended — 256 MiB, t=4, p=4.
    Mainnet,
    /// Maximum practical hardening — 512 MiB, t=5, p=4.
    Paranoid,
}

impl SecurityProfile {
    /// Resolve the preset to concrete [`Argon2Params`].
    pub fn to_params(self) -> Argon2Params {
        match self {
            SecurityProfile::Owasp2024 => Argon2Params {
                m_cost: ARGON2_M_COST,
                t_cost: ARGON2_T_COST,
                p_cost: ARGON2_P_COST,
            },
            SecurityProfile::Mainnet => Argon2Params {
                m_cost: ARGON2_M_COST_MAINNET,
                t_cost: ARGON2_T_COST_MAINNET,
                p_cost: ARGON2_P_COST,
            },
            SecurityProfile::Paranoid => Argon2Params {
                m_cost: ARGON2_M_COST_PARANOID,
                t_cost: ARGON2_T_COST_PARANOID,
                p_cost: ARGON2_P_COST,
            },
        }
    }

    /// Human-readable one-line description of the preset.
    pub fn description(self) -> &'static str {
        match self {
            SecurityProfile::Owasp2024 => "OWASP 2024 minimum (m=64 MiB, t=3, p=4)",
            SecurityProfile::Mainnet   => "mainnet-recommended (m=256 MiB, t=4, p=4)",
            SecurityProfile::Paranoid  => "paranoid (m=512 MiB, t=5, p=4)",
        }
    }

    /// Short lowercase name — mirrors the `--security-profile` CLI argument value.
    pub fn name(self) -> &'static str {
        match self {
            SecurityProfile::Owasp2024 => "owasp-2024",
            SecurityProfile::Mainnet   => "mainnet",
            SecurityProfile::Paranoid  => "paranoid",
        }
    }

    /// Parse from a lowercase string (`"owasp-2024"`, `"mainnet"`, `"paranoid"`).
    ///
    /// Returns `None` for unrecognised strings.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "owasp-2024" | "owasp2024" | "default" => Some(SecurityProfile::Owasp2024),
            "mainnet"    | "mainnet-beta"            => Some(SecurityProfile::Mainnet),
            "paranoid"   | "max"                     => Some(SecurityProfile::Paranoid),
            _ => None,
        }
    }
}

impl core::fmt::Display for SecurityProfile {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.description())
    }
}

// ─── Zeroizing key wrapper ───────────────────────────────────────────────────

/// A 32-byte encryption key that is zeroed on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
struct DerivedKey([u8; KEY_LEN]);

// ─── Public API ──────────────────────────────────────────────────────────────

/// Stateless helper for symmetric encryption / decryption.
///
/// All methods are `pub fn` (no `&self` needed) — callers do not need an instance.
pub struct CryptoLayer;

impl CryptoLayer {
    // ── Key derivation ────────────────────────────────────────────────────

    /// Derive a 256-bit key from `password` and `salt` using **Argon2id**
    /// with a custom [`Argon2Params`].
    ///
    /// # Errors
    /// Returns [`CryptoError::InvalidPassword`] if the password is empty.
    /// Returns [`CryptoError::KeyDerivation`] on Argon2 parameter errors.
    #[cfg(feature = "argon2")]
    pub fn derive_key_argon2id_with_params(
        password: &str,
        salt: &[u8; SALT_LEN],
        params: &Argon2Params,
    ) -> Result<[u8; KEY_LEN], CryptoError> {
        if password.is_empty() {
            return Err(CryptoError::InvalidPassword("password must not be empty"));
        }

        let ap = Params::new(
            params.m_cost,
            params.t_cost,
            params.p_cost,
            Some(KEY_LEN),
        )
        .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, ap);

        let mut key_bytes = [0u8; KEY_LEN];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

        Ok(key_bytes)
    }

    /// Derive a 256-bit key from `password` and `salt` using **Argon2id**
    /// with the default OWASP 2024 parameters.
    ///
    /// # Errors
    /// Returns [`CryptoError::InvalidPassword`] if the password is empty.
    /// Returns [`CryptoError::KeyDerivation`] on Argon2 parameter errors.
    #[cfg(feature = "argon2")]
    pub fn derive_key_argon2id(
        password: &str,
        salt: &[u8; SALT_LEN],
    ) -> Result<[u8; KEY_LEN], CryptoError> {
        Self::derive_key_argon2id_with_params(password, salt, &Argon2Params::default())
    }

    /// Derive a 256-bit key using **PBKDF2-SHA256** (v1 / Python compatibility).
    ///
    /// Use this only when decrypting files produced by the Python implementation.
    pub fn derive_key_pbkdf2(
        password: &str,
        salt: &[u8; SALT_LEN],
    ) -> Result<[u8; KEY_LEN], CryptoError> {
        if password.is_empty() {
            return Err(CryptoError::InvalidPassword("password must not be empty"));
        }

        let mut key = [0u8; KEY_LEN];
        pbkdf2::pbkdf2_hmac::<sha2::Sha256>(
            password.as_bytes(),
            salt,
            PBKDF2_ITERATIONS,
            &mut key,
        );
        Ok(key)
    }

    // ── Encrypt ──────────────────────────────────────────────────────────

    /// Encrypt `data` using a fresh Argon2id-derived key (custom [`Argon2Params`])
    /// and a random ChaCha20-Poly1305 nonce.
    ///
    /// # Wire format
    /// ```text
    /// salt (16 B) || nonce (12 B) || ciphertext+tag (len(data)+16 B)
    /// ```
    ///
    /// # Errors
    /// See [`CryptoError`].
    #[cfg(feature = "argon2")]
    pub fn encrypt_with_params(
        data: &[u8],
        password: &str,
        params: &Argon2Params,
    ) -> Result<Vec<u8>, CryptoError> {
        use rand::RngCore;

        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::rng().fill_bytes(&mut salt);
        rand::rng().fill_bytes(&mut nonce_bytes);

        let raw_key = Self::derive_key_argon2id_with_params(password, &salt, params)?;
        let mut derived = DerivedKey(raw_key);

        let key = Key::from_slice(&derived.0);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, data.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)?;

        derived.zeroize();

        let mut blob = Vec::with_capacity(SALT_LEN + NONCE_LEN + ciphertext.len());
        blob.extend_from_slice(&salt);
        blob.extend_from_slice(&nonce_bytes);
        blob.extend_from_slice(&ciphertext);

        Ok(blob)
    }

    /// Encrypt `data` using default Argon2id parameters.
    ///
    /// # Wire format
    /// ```text
    /// salt (16 B) || nonce (12 B) || ciphertext+tag (len(data)+16 B)
    /// ```
    #[cfg(feature = "argon2")]
    pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
        Self::encrypt_with_params(data, password, &Argon2Params::default())
    }

    // ── Decrypt ──────────────────────────────────────────────────────────

    /// Authenticate and decrypt a blob, using explicit [`Argon2Params`].
    ///
    /// `version` selects the KDF:
    /// - `1` → PBKDF2-SHA256 (Python v1 — `params` ignored)
    /// - `2` → Argon2id default params (params ignored; uses built-in defaults)
    /// - `3` → Argon2id with the supplied `params` (embedded in the v3 frame)
    ///
    /// # Errors
    /// Returns [`CryptoError::DecryptionFailed`] for wrong password **or** tampered
    /// data — deliberately ambiguous to prevent oracle attacks.
    pub fn decrypt_with_params(
        blob: &[u8],
        password: &str,
        version: u8,
        params: &Argon2Params,
    ) -> Result<Vec<u8>, CryptoError> {
        if blob.len() < MIN_BLOB_LEN {
            return Err(CryptoError::BlobTooShort {
                min: MIN_BLOB_LEN,
                got: blob.len(),
            });
        }

        let salt: [u8; SALT_LEN] = blob[..SALT_LEN].try_into().unwrap();
        let nonce_bytes: [u8; NONCE_LEN] =
            blob[SALT_LEN..SALT_LEN + NONCE_LEN].try_into().unwrap();
        let ciphertext = &blob[SALT_LEN + NONCE_LEN..];

        let raw_key = match version {
            1 => Self::derive_key_pbkdf2(password, &salt)?,
            #[cfg(feature = "argon2")]
            2 => Self::derive_key_argon2id(password, &salt)?,
            #[cfg(feature = "argon2")]
            3 => Self::derive_key_argon2id_with_params(password, &salt, params)?,
            _ => return Err(CryptoError::KeyDerivation(format!("unknown version {version}"))),
        };
        let mut derived = DerivedKey(raw_key);

        let key = Key::from_slice(&derived.0);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)?;

        derived.zeroize();
        Ok(plaintext)
    }

    /// Authenticate and decrypt a blob produced by [`Self::encrypt`] or
    /// [`Self::encrypt_with_params`] — uses default Argon2id params for v2.
    ///
    /// For protocol v3 (custom params), prefer [`Self::decrypt_with_params`].
    pub fn decrypt(blob: &[u8], password: &str, version: u8) -> Result<Vec<u8>, CryptoError> {
        Self::decrypt_with_params(blob, password, version, &Argon2Params::default())
    }

    /// Convenience wrapper: encrypt using the current default (Argon2id, version 2).
    #[cfg(feature = "argon2")]
    #[inline]
    pub fn encrypt_v2(data: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
        Self::encrypt(data, password)
    }

    /// Convenience wrapper: decrypt Python v1 (PBKDF2) blobs.
    #[inline]
    pub fn decrypt_v1(blob: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
        Self::decrypt(blob, password, 1)
    }

    /// Convenience wrapper: decrypt Rust v2 (Argon2id) blobs.
    #[inline]
    pub fn decrypt_v2(blob: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
        Self::decrypt(blob, password, 2)
    }
}

// ─── Compression helpers (std only) ──────────────────────────────────────────

#[cfg(feature = "std")]
/// Zlib compression / decompression helpers used by the session layer.
pub mod compress {
    use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
    use std::io::{Read, Write};

    /// zlib-compress `data` at level 9 (maximum).
    pub fn compress(data: &[u8]) -> Vec<u8> {
        let mut enc = ZlibEncoder::new(Vec::new(), Compression::best());
        enc.write_all(data).expect("compression failed");
        enc.finish().expect("compression finalization failed")
    }

    /// zlib-decompress `data`.
    pub fn decompress(data: &[u8]) -> std::io::Result<Vec<u8>> {
        let mut dec = ZlibDecoder::new(data);
        let mut out = Vec::new();
        dec.read_to_end(&mut out)?;
        Ok(out)
    }
}

// ─── BLAKE3 helpers ───────────────────────────────────────────────────────────

/// Compute a 32-byte BLAKE3 digest of `data`.
#[inline]
pub fn blake3_digest(data: &[u8]) -> [u8; 32] {
    *blake3::hash(data).as_bytes()
}

/// Compute a 32-byte BLAKE3 keyed MAC.
#[inline]
pub fn blake3_mac(key: &[u8; 32], data: &[u8]) -> [u8; 32] {
    *blake3::keyed_hash(key, data).as_bytes()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWORD: &str = "correct-horse-battery-staple";
    const PLAINTEXT: &[u8] = b"The quick brown fox jumps over the lazy dog";

    #[test]
    fn roundtrip_v2() {
        let blob = CryptoLayer::encrypt_v2(PLAINTEXT, PASSWORD).unwrap();
        assert!(blob.len() > MIN_BLOB_LEN);
        let recovered = CryptoLayer::decrypt_v2(&blob, PASSWORD).unwrap();
        assert_eq!(recovered, PLAINTEXT);
    }

    #[test]
    fn wrong_password_returns_error() {
        let blob = CryptoLayer::encrypt_v2(PLAINTEXT, PASSWORD).unwrap();
        let result = CryptoLayer::decrypt_v2(&blob, "wrong-password");
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn tampered_blob_returns_error() {
        let mut blob = CryptoLayer::encrypt_v2(PLAINTEXT, PASSWORD).unwrap();
        // Flip a bit in the ciphertext region
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;
        let result = CryptoLayer::decrypt_v2(&blob, PASSWORD);
        assert!(matches!(result, Err(CryptoError::DecryptionFailed)));
    }

    #[test]
    fn empty_password_rejected() {
        let result = CryptoLayer::encrypt_v2(PLAINTEXT, "");
        assert!(matches!(result, Err(CryptoError::InvalidPassword(_))));
    }

    #[test]
    fn blob_too_short_rejected() {
        let result = CryptoLayer::decrypt_v2(&[0u8; 10], PASSWORD);
        assert!(matches!(result, Err(CryptoError::BlobTooShort { .. })));
    }

    #[test]
    fn different_salts_produce_different_blobs() {
        let b1 = CryptoLayer::encrypt_v2(PLAINTEXT, PASSWORD).unwrap();
        let b2 = CryptoLayer::encrypt_v2(PLAINTEXT, PASSWORD).unwrap();
        // Same plaintext + password should never produce the same blob
        assert_ne!(b1, b2, "salt/nonce reuse detected!");
    }

    #[test]
    fn blake3_digest_stable() {
        let d1 = blake3_digest(b"hello");
        let d2 = blake3_digest(b"hello");
        assert_eq!(d1, d2);
        let d3 = blake3_digest(b"hellO");
        assert_ne!(d1, d3);
    }

    /// Custom Argon2 params roundtrip — using lower params for test speed.
    #[test]
    fn roundtrip_custom_argon2_params() {
        let params = Argon2Params {
            m_cost: 8_192, // 8 MiB — faster for tests
            t_cost: 1,
            p_cost: 1,
        };
        let blob = CryptoLayer::encrypt_with_params(PLAINTEXT, PASSWORD, &params).unwrap();
        let recovered =
            CryptoLayer::decrypt_with_params(&blob, PASSWORD, 3, &params).unwrap();
        assert_eq!(recovered, PLAINTEXT);
    }

    /// Wrong params must cause decryption failure.
    #[test]
    fn wrong_argon2_params_fail() {
        let params_a = Argon2Params { m_cost: 8_192, t_cost: 1, p_cost: 1 };
        let params_b = Argon2Params { m_cost: 8_192, t_cost: 2, p_cost: 1 };
        let blob = CryptoLayer::encrypt_with_params(PLAINTEXT, PASSWORD, &params_a).unwrap();
        let result = CryptoLayer::decrypt_with_params(&blob, PASSWORD, 3, &params_b);
        assert!(
            matches!(result, Err(CryptoError::DecryptionFailed)),
            "expected DecryptionFailed when params differ"
        );
    }

    /// `Argon2Params::default()` must equal the published constants.
    #[test]
    fn default_params_match_constants() {
        let d = Argon2Params::default();
        assert_eq!(d.m_cost, ARGON2_M_COST);
        assert_eq!(d.t_cost, ARGON2_T_COST);
        assert_eq!(d.p_cost, ARGON2_P_COST);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compress_roundtrip() {
        let compressed = compress::compress(PLAINTEXT);
        let recovered = compress::decompress(&compressed).unwrap();
        assert_eq!(recovered, PLAINTEXT);
    }

    // ── SecurityProfile tests ─────────────────────────────────────────────

    #[test]
    fn preset_owasp2024_matches_defaults() {
        let p = SecurityProfile::Owasp2024.to_params();
        assert_eq!(p, Argon2Params::default());
        assert_eq!(p.m_cost, ARGON2_M_COST);
        assert_eq!(p.t_cost, ARGON2_T_COST);
        assert_eq!(p.p_cost, ARGON2_P_COST);
    }

    #[test]
    fn preset_mainnet_params_correct() {
        let p = SecurityProfile::Mainnet.to_params();
        assert_eq!(p.m_cost, ARGON2_M_COST_MAINNET);
        assert_eq!(p.t_cost, ARGON2_T_COST_MAINNET);
        assert_eq!(p.p_cost, ARGON2_P_COST);
        assert_eq!(p.m_cost, 262_144, "mainnet must be 256 MiB");
        assert_eq!(p.t_cost, 4);
    }

    #[test]
    fn preset_paranoid_params_correct() {
        let p = SecurityProfile::Paranoid.to_params();
        assert_eq!(p.m_cost, ARGON2_M_COST_PARANOID);
        assert_eq!(p.t_cost, ARGON2_T_COST_PARANOID);
        assert_eq!(p.m_cost, 524_288, "paranoid must be 512 MiB");
        assert_eq!(p.t_cost, 5);
    }

    #[test]
    fn meets_mainnet_minimum_correct() {
        assert!(!Argon2Params::default().meets_mainnet_minimum(),
            "owasp-2024 default must NOT meet mainnet minimum");
        assert!(SecurityProfile::Mainnet.to_params().meets_mainnet_minimum(),
            "mainnet preset must meet mainnet minimum");
        assert!(SecurityProfile::Paranoid.to_params().meets_mainnet_minimum(),
            "paranoid preset must also meet mainnet minimum");
        // Edge case: exactly at boundary
        let edge = Argon2Params { m_cost: ARGON2_M_COST_MAINNET, t_cost: ARGON2_T_COST_MAINNET, p_cost: 4 };
        assert!(edge.meets_mainnet_minimum());
        // Just below boundary
        let below = Argon2Params { m_cost: ARGON2_M_COST_MAINNET - 1, t_cost: ARGON2_T_COST_MAINNET, p_cost: 4 };
        assert!(!below.meets_mainnet_minimum());
    }

    #[test]
    fn security_level_labels_correct() {
        assert_eq!(Argon2Params::default().security_level(), "owasp-2024");
        assert_eq!(SecurityProfile::Mainnet.to_params().security_level(), "mainnet");
        assert_eq!(SecurityProfile::Paranoid.to_params().security_level(), "paranoid");
        let weak = Argon2Params { m_cost: 8_192, t_cost: 1, p_cost: 1 };
        assert_eq!(weak.security_level(), "weak");
    }

    #[test]
    fn from_str_parses_all_variants() {
        assert_eq!(SecurityProfile::from_str("owasp-2024"), Some(SecurityProfile::Owasp2024));
        assert_eq!(SecurityProfile::from_str("owasp2024"),  Some(SecurityProfile::Owasp2024));
        assert_eq!(SecurityProfile::from_str("default"),    Some(SecurityProfile::Owasp2024));
        assert_eq!(SecurityProfile::from_str("mainnet"),    Some(SecurityProfile::Mainnet));
        assert_eq!(SecurityProfile::from_str("mainnet-beta"), Some(SecurityProfile::Mainnet));
        assert_eq!(SecurityProfile::from_str("paranoid"),   Some(SecurityProfile::Paranoid));
        assert_eq!(SecurityProfile::from_str("max"),        Some(SecurityProfile::Paranoid));
        assert_eq!(SecurityProfile::from_str("MAINNET"),    Some(SecurityProfile::Mainnet),
            "from_str must be case-insensitive");
        assert_eq!(SecurityProfile::from_str("unknown"),    None);
        assert_eq!(SecurityProfile::from_str(""),           None);
    }

    #[test]
    fn profile_names_and_display() {
        assert_eq!(SecurityProfile::Owasp2024.name(), "owasp-2024");
        assert_eq!(SecurityProfile::Mainnet.name(),   "mainnet");
        assert_eq!(SecurityProfile::Paranoid.name(),  "paranoid");
        // Display must not panic and must contain the memory size
        let s = format!("{}", SecurityProfile::Mainnet);
        assert!(s.contains("256"), "Display must mention 256 MiB: {s}");
    }
}
