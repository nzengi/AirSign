//! afterimage_core::protocol
//! =========================
//! Wire-format serialisation / deserialisation for METADATA and DATA frames.
//!
//! # Frame types
//!
//! ## METADATA frame — v1 / v2 (77 bytes)
//! ```text
//! magic    (4 B)  – b'AFTI'
//! version  (1 B)  – u8;  1 = Python/PBKDF2, 2 = Rust/Argon2id (default params)
//! k        (4 B)  – u32 BE — number of source blocks
//! orig_len (4 B)  – u32 BE — original file size in bytes (before compress+encrypt)
//! filename (64 B) – UTF-8, NUL-padded
//! ```
//!
//! ## METADATA frame — v3 (85 bytes)
//! ```text
//! magic    (4 B)  – b'AFTI'
//! version  (1 B)  – 3
//! k        (4 B)  – u32 BE — number of source blocks
//! orig_len (4 B)  – u32 BE — original file size in bytes (before compress+encrypt)
//! m_cost   (4 B)  – u32 BE — Argon2id memory cost in KiB
//! t_cost   (4 B)  – u32 BE — Argon2id time (iteration) cost
//! filename (64 B) – UTF-8, NUL-padded
//! ```
//! Sent every `METADATA_INTERVAL` droplets so a late receiver can synchronise.
//!
//! ## DATA frame
//! An LT droplet produced by `LTEncoder::generate_droplet()` — everything that
//! does NOT start with the magic bytes `b'AFTI'`.
//!
//! # Security note
//! The filename is transmitted in **cleartext**.  Operators who require filename
//! anonymity should pass an empty string or a decoy name.

use crate::crypto::{ARGON2_M_COST, ARGON2_T_COST};
use crate::error::ProtocolError;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Magic bytes identifying an AfterImage METADATA frame.
pub const MAGIC: &[u8; 4] = b"AFTI";

/// Current protocol version emitted by this implementation.
pub const CURRENT_VERSION: u8 = 3;

/// METADATA frame length for protocol v1 and v2 (no embedded Argon2 params).
pub const META_SIZE_V2: usize = 77;

/// METADATA frame length for protocol v3 (includes embedded Argon2 params).
pub const META_SIZE_V3: usize = 85;

/// Legacy alias for `META_SIZE_V2` — kept for backward compatibility.
pub const META_SIZE: usize = META_SIZE_V2;

/// Filename field length inside the METADATA frame.
pub const FILENAME_LEN: usize = 64;

/// Send one METADATA frame every N droplets.
pub const METADATA_INTERVAL: u32 = 50;

// ─── Layout offsets — v1/v2 (77-byte frame) ──────────────────────────────────

const OFF_MAGIC: core::ops::Range<usize>      = 0..4;
const OFF_VER: usize                           = 4;
const OFF_K: core::ops::Range<usize>          = 5..9;
const OFF_LEN: core::ops::Range<usize>        = 9..13;
const OFF_FNAME_V2: core::ops::Range<usize>   = 13..77; // v1/v2

// ─── Layout offsets — v3 (85-byte frame) ─────────────────────────────────────

const OFF_MCOST_V3: core::ops::Range<usize>   = 13..17;
const OFF_TCOST_V3: core::ops::Range<usize>   = 17..21;
const OFF_FNAME_V3: core::ops::Range<usize>   = 21..85; // v3

// ─── MetadataFrame ────────────────────────────────────────────────────────────

/// Parsed representation of an AfterImage METADATA frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataFrame {
    /// Protocol version byte (1, 2, or 3).
    pub version: u8,
    /// Number of LT source blocks.
    pub k: u32,
    /// Original plaintext file size (before compress + encrypt).
    pub original_len: u32,
    /// Argon2id memory cost in KiB embedded in v3 frames.
    /// For v1/v2 frames, this defaults to [`ARGON2_M_COST`].
    pub argon2_m_cost: u32,
    /// Argon2id time (iteration) cost embedded in v3 frames.
    /// For v1/v2 frames, this defaults to [`ARGON2_T_COST`].
    pub argon2_t_cost: u32,
    /// Filename (cleartext; may be a decoy).
    pub filename: String,
}

impl MetadataFrame {
    /// Serialise into a wire frame.
    ///
    /// - Protocol v1 / v2 → 77-byte frame (no Argon2 params).
    /// - Protocol v3      → 85-byte frame (includes m_cost and t_cost).
    pub fn to_bytes(&self) -> Vec<u8> {
        match self.version {
            3 => {
                let mut frame = vec![0u8; META_SIZE_V3];
                frame[OFF_MAGIC].copy_from_slice(MAGIC);
                frame[OFF_VER] = self.version;
                frame[OFF_K].copy_from_slice(&self.k.to_be_bytes());
                frame[OFF_LEN].copy_from_slice(&self.original_len.to_be_bytes());
                frame[OFF_MCOST_V3].copy_from_slice(&self.argon2_m_cost.to_be_bytes());
                frame[OFF_TCOST_V3].copy_from_slice(&self.argon2_t_cost.to_be_bytes());
                let fname_bytes = self.filename.as_bytes();
                let copy_len = fname_bytes.len().min(FILENAME_LEN);
                frame[OFF_FNAME_V3.start..OFF_FNAME_V3.start + copy_len]
                    .copy_from_slice(&fname_bytes[..copy_len]);
                frame
            }
            _ => {
                // v1 / v2 legacy 77-byte format
                let mut frame = vec![0u8; META_SIZE_V2];
                frame[OFF_MAGIC].copy_from_slice(MAGIC);
                frame[OFF_VER] = self.version;
                frame[OFF_K].copy_from_slice(&self.k.to_be_bytes());
                frame[OFF_LEN].copy_from_slice(&self.original_len.to_be_bytes());
                let fname_bytes = self.filename.as_bytes();
                let copy_len = fname_bytes.len().min(FILENAME_LEN);
                frame[OFF_FNAME_V2.start..OFF_FNAME_V2.start + copy_len]
                    .copy_from_slice(&fname_bytes[..copy_len]);
                frame
            }
        }
    }

    /// Parse a METADATA frame from raw bytes.
    ///
    /// Supports both the 77-byte v1/v2 format and the 85-byte v3 format.
    ///
    /// # Errors
    /// - [`ProtocolError::MetadataTooShort`] — frame shorter than required for its version
    /// - [`ProtocolError::InvalidMagic`]     — magic bytes mismatch
    /// - [`ProtocolError::UnknownVersion`]   — version not 1, 2, or 3
    /// - [`ProtocolError::InvalidFilename`]  — filename bytes not valid UTF-8
    pub fn from_bytes(frame: &[u8]) -> Result<Self, ProtocolError> {
        // Need at least 5 bytes to identify magic + version
        if frame.len() < 5 {
            return Err(ProtocolError::MetadataTooShort {
                min: META_SIZE_V2,
                got: frame.len(),
            });
        }

        let magic: [u8; 4] = frame[OFF_MAGIC].try_into().unwrap();
        if &magic != MAGIC {
            return Err(ProtocolError::InvalidMagic { got: magic });
        }

        let version = frame[OFF_VER];

        // Determine the minimum frame size and field offsets based on version
        let (required_size, fname_range) = match version {
            1 | 2 => (META_SIZE_V2, OFF_FNAME_V2),
            3 => (META_SIZE_V3, OFF_FNAME_V3),
            _ => return Err(ProtocolError::UnknownVersion(version)),
        };

        if frame.len() < required_size {
            return Err(ProtocolError::MetadataTooShort {
                min: required_size,
                got: frame.len(),
            });
        }

        let k = u32::from_be_bytes(frame[OFF_K].try_into().unwrap());
        let original_len = u32::from_be_bytes(frame[OFF_LEN].try_into().unwrap());

        let (argon2_m_cost, argon2_t_cost) = if version == 3 {
            let m = u32::from_be_bytes(frame[OFF_MCOST_V3].try_into().unwrap());
            let t = u32::from_be_bytes(frame[OFF_TCOST_V3].try_into().unwrap());
            (m, t)
        } else {
            // v1/v2 — use the published defaults; receiver will use them automatically
            (ARGON2_M_COST, ARGON2_T_COST)
        };

        let fname_raw = &frame[fname_range];
        let nul_pos = fname_raw.iter().position(|&b| b == 0).unwrap_or(FILENAME_LEN);
        let filename = core::str::from_utf8(&fname_raw[..nul_pos])
            .map_err(|_| ProtocolError::InvalidFilename)?
            .to_owned();

        Ok(Self {
            version,
            k,
            original_len,
            argon2_m_cost,
            argon2_t_cost,
            filename,
        })
    }

    /// Return `true` if `bytes` starts with the AfterImage magic prefix.
    #[inline]
    pub fn is_metadata(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && &bytes[..4] == MAGIC
    }
}

// ─── Convenience builders ─────────────────────────────────────────────────────

impl MetadataFrame {
    /// Construct a **v3** METADATA frame (Argon2id with explicit params).
    ///
    /// This is the recommended constructor for new sessions. The Argon2id
    /// parameters are embedded in the 85-byte frame so the receiver can
    /// reconstruct the key without out-of-band configuration.
    pub fn new_v3(
        k: u32,
        original_len: u32,
        filename: impl Into<String>,
        m_cost: u32,
        t_cost: u32,
    ) -> Self {
        Self {
            version: 3,
            k,
            original_len,
            argon2_m_cost: m_cost,
            argon2_t_cost: t_cost,
            filename: filename.into(),
        }
    }

    /// Construct a **v2** METADATA frame (Argon2id, default params — 77 bytes).
    ///
    /// Kept for backward compatibility. Prefer [`MetadataFrame::new_v3`] for
    /// new sessions that need configurable Argon2id parameters.
    pub fn new_v2(k: u32, original_len: u32, filename: impl Into<String>) -> Self {
        Self {
            version: 2,
            k,
            original_len,
            argon2_m_cost: ARGON2_M_COST,
            argon2_t_cost: ARGON2_T_COST,
            filename: filename.into(),
        }
    }

    /// Construct a **v1** METADATA frame (Python compat / PBKDF2 — 77 bytes).
    pub fn new_v1(k: u32, original_len: u32, filename: impl Into<String>) -> Self {
        Self {
            version: 1,
            k,
            original_len,
            argon2_m_cost: ARGON2_M_COST,
            argon2_t_cost: ARGON2_T_COST,
            filename: filename.into(),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── v1/v2 backward compat ─────────────────────────────────────────────

    #[test]
    fn roundtrip_v2() {
        let meta = MetadataFrame::new_v2(42, 1024 * 1024, "secret.zip");
        let bytes = meta.to_bytes();
        assert_eq!(bytes.len(), META_SIZE_V2, "v2 frame must be 77 bytes");
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.k, 42);
        assert_eq!(parsed.original_len, 1024 * 1024);
        assert_eq!(parsed.filename, "secret.zip");
        // v2 argon2 fields default to published constants
        assert_eq!(parsed.argon2_m_cost, ARGON2_M_COST);
        assert_eq!(parsed.argon2_t_cost, ARGON2_T_COST);
    }

    #[test]
    fn roundtrip_v1() {
        let meta = MetadataFrame::new_v1(10, 256, "data.bin");
        let bytes = meta.to_bytes();
        assert_eq!(bytes.len(), META_SIZE_V2, "v1 frame must be 77 bytes");
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.filename, "data.bin");
    }

    // ── v3 (new) ──────────────────────────────────────────────────────────

    #[test]
    fn roundtrip_v3_custom_params() {
        let meta = MetadataFrame::new_v3(99, 2048, "vault.bin", 131_072, 4);
        let bytes = meta.to_bytes();
        assert_eq!(bytes.len(), META_SIZE_V3, "v3 frame must be 85 bytes");
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version, 3);
        assert_eq!(parsed.k, 99);
        assert_eq!(parsed.original_len, 2048);
        assert_eq!(parsed.argon2_m_cost, 131_072);
        assert_eq!(parsed.argon2_t_cost, 4);
        assert_eq!(parsed.filename, "vault.bin");
    }

    #[test]
    fn roundtrip_v3_default_params() {
        // v3 with the published default params must also round-trip
        let meta = MetadataFrame::new_v3(1, 1, "f", ARGON2_M_COST, ARGON2_T_COST);
        let bytes = meta.to_bytes();
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.argon2_m_cost, ARGON2_M_COST);
        assert_eq!(parsed.argon2_t_cost, ARGON2_T_COST);
    }

    // ── edge cases ────────────────────────────────────────────────────────

    #[test]
    fn roundtrip_empty_filename_v3() {
        let meta = MetadataFrame::new_v3(5, 100, "", 65_536, 3);
        let bytes = meta.to_bytes();
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.filename, "");
    }

    #[test]
    fn roundtrip_long_filename_truncated_v3() {
        let long_name = "a".repeat(200);
        let meta = MetadataFrame::new_v3(1, 1, long_name, 65_536, 3);
        let bytes = meta.to_bytes();
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert!(parsed.filename.len() <= FILENAME_LEN);
    }

    #[test]
    fn roundtrip_empty_filename() {
        let meta = MetadataFrame::new_v2(5, 100, "");
        let bytes = meta.to_bytes();
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.filename, "");
    }

    #[test]
    fn roundtrip_long_filename_truncated() {
        let long_name = "a".repeat(200);
        let meta = MetadataFrame::new_v2(1, 1, long_name);
        let bytes = meta.to_bytes();
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert!(parsed.filename.len() <= FILENAME_LEN);
    }

    #[test]
    fn is_metadata_detection() {
        let bytes_v2 = MetadataFrame::new_v2(1, 1, "f").to_bytes();
        assert!(MetadataFrame::is_metadata(&bytes_v2));
        let bytes_v3 = MetadataFrame::new_v3(1, 1, "f", 65_536, 3).to_bytes();
        assert!(MetadataFrame::is_metadata(&bytes_v3));
        assert!(!MetadataFrame::is_metadata(b"not-a-meta-frame"));
        assert!(!MetadataFrame::is_metadata(b"AFT")); // too short
    }

    #[test]
    fn bad_magic_returns_error() {
        let mut bytes = MetadataFrame::new_v2(1, 1, "f").to_bytes();
        bytes[0] = b'X';
        assert!(matches!(
            MetadataFrame::from_bytes(&bytes),
            Err(ProtocolError::InvalidMagic { .. })
        ));
    }

    #[test]
    fn unknown_version_returns_error() {
        let mut bytes = MetadataFrame::new_v2(1, 1, "f").to_bytes();
        bytes[4] = 99; // unknown version
        assert!(matches!(
            MetadataFrame::from_bytes(&bytes),
            Err(ProtocolError::UnknownVersion(99))
        ));
    }

    #[test]
    fn too_short_returns_error() {
        assert!(matches!(
            MetadataFrame::from_bytes(&[0u8; 10]),
            Err(ProtocolError::MetadataTooShort { .. })
        ));
    }

    /// A v3 frame that is cut short (< 85 bytes) must be rejected.
    #[test]
    fn v3_too_short_returns_error() {
        // Craft a v3 magic+version prefix but only 80 bytes total
        let mut partial = MetadataFrame::new_v3(1, 1, "f", 65_536, 3).to_bytes();
        partial.truncate(80);
        assert!(matches!(
            MetadataFrame::from_bytes(&partial),
            Err(ProtocolError::MetadataTooShort { min: 85, .. })
        ));
    }
}