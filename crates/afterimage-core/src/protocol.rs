//! afterimage_core::protocol
//! =========================
//! Wire-format serialisation / deserialisation for METADATA and DATA frames.
//!
//! # Frame types
//!
//! ## METADATA frame (77 bytes)
//! ```text
//! magic    (4 B)  – b'AFTI'
//! version  (1 B)  – u8;  1 = Python/PBKDF2, 2 = Rust/Argon2id
//! k        (4 B)  – u32 BE — number of source blocks
//! orig_len (4 B)  – u32 BE — original file size in bytes (before compress+encrypt)
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

use crate::error::ProtocolError;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Magic bytes identifying an AfterImage METADATA frame.
pub const MAGIC: &[u8; 4] = b"AFTI";

/// Current protocol version emitted by this implementation.
pub const CURRENT_VERSION: u8 = 2;

/// METADATA frame total length in bytes.
pub const META_SIZE: usize = 77;

/// Filename field length inside the METADATA frame.
pub const FILENAME_LEN: usize = 64;

/// Send one METADATA frame every N droplets.
pub const METADATA_INTERVAL: u32 = 50;

// Layout offsets inside a METADATA frame
const OFF_MAGIC: std::ops::Range<usize>   = 0..4;
const OFF_VER: usize                       = 4;
const OFF_K: std::ops::Range<usize>       = 5..9;
const OFF_LEN: std::ops::Range<usize>     = 9..13;
const OFF_FNAME: std::ops::Range<usize>   = 13..77;

// ─── MetadataFrame ────────────────────────────────────────────────────────────

/// Parsed representation of an AfterImage METADATA frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataFrame {
    /// Protocol version byte.
    pub version: u8,
    /// Number of LT source blocks.
    pub k: u32,
    /// Original plaintext file size (before compress + encrypt).
    pub original_len: u32,
    /// Filename (cleartext; may be a decoy).
    pub filename: String,
}

impl MetadataFrame {
    /// Serialise into a 77-byte wire frame.
    pub fn to_bytes(&self) -> [u8; META_SIZE] {
        let mut frame = [0u8; META_SIZE];
        frame[OFF_MAGIC].copy_from_slice(MAGIC);
        frame[OFF_VER] = self.version;
        frame[OFF_K].copy_from_slice(&self.k.to_be_bytes());
        frame[OFF_LEN].copy_from_slice(&self.original_len.to_be_bytes());

        let fname_bytes = self.filename.as_bytes();
        let copy_len = fname_bytes.len().min(FILENAME_LEN);
        frame[OFF_FNAME][..copy_len].copy_from_slice(&fname_bytes[..copy_len]);
        // remaining bytes are already 0 (NUL padding)

        frame
    }

    /// Parse a METADATA frame from raw bytes.
    ///
    /// # Errors
    /// - [`ProtocolError::MetadataTooShort`] — frame shorter than `META_SIZE`
    /// - [`ProtocolError::InvalidMagic`]     — magic bytes mismatch
    /// - [`ProtocolError::UnknownVersion`]   — version not 1 or 2
    /// - [`ProtocolError::InvalidFilename`]  — filename bytes not valid UTF-8
    pub fn from_bytes(frame: &[u8]) -> Result<Self, ProtocolError> {
        if frame.len() < META_SIZE {
            return Err(ProtocolError::MetadataTooShort {
                min: META_SIZE,
                got: frame.len(),
            });
        }

        let magic: [u8; 4] = frame[OFF_MAGIC].try_into().unwrap();
        if &magic != MAGIC {
            return Err(ProtocolError::InvalidMagic { got: magic });
        }

        let version = frame[OFF_VER];
        if version != 1 && version != 2 {
            return Err(ProtocolError::UnknownVersion(version));
        }

        let k = u32::from_be_bytes(frame[OFF_K].try_into().unwrap());
        let original_len = u32::from_be_bytes(frame[OFF_LEN].try_into().unwrap());

        let fname_raw = &frame[OFF_FNAME];
        // Strip trailing NUL bytes
        let nul_pos = fname_raw.iter().position(|&b| b == 0).unwrap_or(FILENAME_LEN);
        let filename = std::str::from_utf8(&fname_raw[..nul_pos])
            .map_err(|_| ProtocolError::InvalidFilename)?
            .to_owned();

        Ok(Self {
            version,
            k,
            original_len,
            filename,
        })
    }

    /// Return `true` if `bytes` starts with the AfterImage magic prefix.
    #[inline]
    pub fn is_metadata(bytes: &[u8]) -> bool {
        bytes.len() >= 4 && &bytes[..4] == MAGIC
    }
}

// ─── Convenience builder ──────────────────────────────────────────────────────

impl MetadataFrame {
    /// Construct a v2 METADATA frame (Argon2id protocol).
    pub fn new_v2(k: u32, original_len: u32, filename: impl Into<String>) -> Self {
        Self {
            version: CURRENT_VERSION,
            k,
            original_len,
            filename: filename.into(),
        }
    }

    /// Construct a v1 METADATA frame (Python compat).
    pub fn new_v1(k: u32, original_len: u32, filename: impl Into<String>) -> Self {
        Self {
            version: 1,
            k,
            original_len,
            filename: filename.into(),
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_v2() {
        let meta = MetadataFrame::new_v2(42, 1024 * 1024, "secret.zip");
        let bytes = meta.to_bytes();
        assert_eq!(bytes.len(), META_SIZE);
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.k, 42);
        assert_eq!(parsed.original_len, 1024 * 1024);
        assert_eq!(parsed.filename, "secret.zip");
    }

    #[test]
    fn roundtrip_v1() {
        let meta = MetadataFrame::new_v1(10, 256, "data.bin");
        let bytes = meta.to_bytes();
        let parsed = MetadataFrame::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.filename, "data.bin");
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
        // Filename should be truncated to FILENAME_LEN bytes
        assert!(parsed.filename.len() <= FILENAME_LEN);
    }

    #[test]
    fn is_metadata_detection() {
        let meta = MetadataFrame::new_v2(1, 1, "f");
        let bytes = meta.to_bytes();
        assert!(MetadataFrame::is_metadata(&bytes));
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
}