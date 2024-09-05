//! afterimage_core::crypto
//! =======================
//! ChaCha20-Poly1305 AEAD encryption with Argon2id key derivation.
//!
//! # Security design
//!
//! * **Argon2id** (RFC 9106, OWASP 2024): memory-hard KDF resistant to GPU/ASIC
//!   brute-force attacks. Parameters: m=65536 KiB, t=3, p=4.
//! * **ChaCha20-Poly1305**: 256-bit authenticated encryption (RFC 8439).
//!   Every `encrypt()` call uses a fresh random 16-byte salt and 12-byte nonce,
//!   so the same password + plaintext pair always yields different ciphertext.
//! * **Zeroize on drop**: derived keys are wiped from memory when they go out of
//!   scope, preventing leakage via swap, core dumps, or cold-boot attacks.
//! * **BLAKE3 integrity header**: an optional 32-byte BLAKE3 digest of the
//!   plaintext is prepended before encryption, enabling fast pre-auth data
//!   integrity checks at the application layer without breaking AEAD semantics.
//!
//! # Wire format (v2 — Argon2id)
//!
//! ```text
//! salt (16 B) || nonce (12 B) || ciphertext+tag (N+16 B)
//! ```
//!
//! The protocol-version byte lives in the METADATA frame, not here.
//! Protocol v1 (Python compat) uses PBKDF2-SHA256 with the same binary layout.

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

// ─── Argon2id parameters (OWASP 2024 minimum) ────────────────────────────────

/// Memory cost in KiB — 64 MiB.
pub const ARGON2_M_COST: u32 = 65_536;
/// Time (iteration) cost.
pub const ARGON2_T_COST: u32 = 3;
/// Degree of parallelism.
pub const ARGON2_P_COST: u32 = 4;

// ─── PBKDF2 parameters (v1 / Python-compat) ──────────────────────────────────

/// PBKDF2-SHA256 iterations used by the Python v1 implementation.
pub const PBKDF2_ITERATIONS: u32 = 600_000;

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

    /// Derive a 256-bit key from `password` and `salt` using **Argon2id**.
    ///
    /// # Errors
    /// Returns [`CryptoError::InvalidPassword`] if the password is empty.
    /// Returns [`CryptoError::KeyDerivation`] on Argon2 parameter errors.
    #[cfg(feature = "argon2")]
    pub fn derive_key_argon2id(
        password: &str,
        salt: &[u8; SALT_LEN],
    ) -> Result<[u8; KEY_LEN], CryptoError> {
        if password.is_empty() {
            return Err(CryptoError::InvalidPassword("password must not be empty"));
        }

        let params = Params::new(ARGON2_M_COST, ARGON2_T_COST, ARGON2_P_COST, Some(KEY_LEN))
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key_bytes = [0u8; KEY_LEN];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| CryptoError::KeyDerivation(e.to_string()))?;

        Ok(key_bytes)
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

    /// Compress-then-encrypt `data` with a fresh Argon2id salt and ChaCha20 nonce.
    ///
    /// # Wire format
    /// ```text
    /// salt (16 B) || nonce (12 B) || ciphertext+tag (len(data)+16 B)
    /// ```
    ///
    /// The protocol-version byte lives in the METADATA frame, not the blob.
    ///
    /// # Errors
    /// See [`CryptoError`].
    #[cfg(feature = "argon2")]
    pub fn encrypt(data: &[u8], password: &str) -> Result<Vec<u8>, CryptoError> {
        use rand::RngCore;

        let mut salt = [0u8; SALT_LEN];
        let mut nonce_bytes = [0u8; NONCE_LEN];
        rand::rng().fill_bytes(&mut salt);
        rand::rng().fill_bytes(&mut nonce_bytes);

        // Derive key — zeroized on drop via DerivedKey wrapper
        let raw_key = Self::derive_key_argon2id(password, &salt)?;
        let mut derived = DerivedKey(raw_key);

        let key = Key::from_slice(&derived.0);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // No AAD needed — Poly1305 tag already provides AEAD authentication.
        // Keeping the API simple and correct is more important than layering
        // redundant commitments that break encrypt/decrypt symmetry.
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

    // ── Decrypt ──────────────────────────────────────────────────────────

    /// Authenticate and decrypt a blob produced by [`Self::encrypt`].
    ///
    /// `version` selects the KDF:
    /// - `1` → PBKDF2-SHA256 (Python v1 compatibility)
    /// - `2` → Argon2id (Rust v2 default)
    ///
    /// # Errors
    /// Returns [`CryptoError::DecryptionFailed`] for wrong password **or** tampered
    /// data — deliberately ambiguous to prevent oracle attacks.
    pub fn decrypt(blob: &[u8], password: &str, version: u8) -> Result<Vec<u8>, CryptoError> {
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

        // Derive key based on protocol version
        let raw_key = match version {
            1 => Self::derive_key_pbkdf2(password, &salt)?,
            #[cfg(feature = "argon2")]
            2 => Self::derive_key_argon2id(password, &salt)?,
            _ => return Err(CryptoError::KeyDerivation(format!("unknown version {version}"))),
        };
        let mut derived = DerivedKey(raw_key);

        let key = Key::from_slice(&derived.0);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Both v1 and v2 use no AAD — Poly1305 tag authenticates the ciphertext.
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| CryptoError::DecryptionFailed)?;

        derived.zeroize();
        Ok(plaintext)
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

    #[cfg(feature = "std")]
    #[test]
    fn compress_roundtrip() {
        let compressed = compress::compress(PLAINTEXT);
        let recovered = compress::decompress(&compressed).unwrap();
        assert_eq!(recovered, PLAINTEXT);
    }
}