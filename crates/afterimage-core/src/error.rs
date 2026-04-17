//! Unified error types for AfterImage core.

use thiserror::Error;

/// Top-level AfterImage error.
#[derive(Debug, Error)]
pub enum AfterImageError {
    #[error("Cryptographic error: {0}")]
    /// Wraps a cryptography-layer error.
    Crypto(#[from] CryptoError),

    #[error("Fountain-code error: {0}")]
    /// Wraps a fountain-code error.
    Fountain(#[from] FountainError),

    #[error("Protocol error: {0}")]
    /// Wraps a protocol-framing error.
    Protocol(#[from] ProtocolError),

    #[cfg(feature = "std")]
    #[error("I/O error: {0}")]
    /// Wraps a standard I/O error (compression / decompression).
    Io(#[from] std::io::Error),
}

/// Errors produced by the cryptographic layer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CryptoError {
    /// Wrong password or tampered ciphertext (deliberately vague — no oracle).
    #[error("Decryption failed: wrong password or corrupted data")]
    DecryptionFailed,

    /// The encrypted blob is shorter than the minimum header.
    #[error("Blob too short: need at least {min} bytes, got {got}")]
    BlobTooShort {
        /// Minimum required byte length.
        min: usize,
        /// Actual byte length received.
        got: usize,
    },

    /// Password was empty or otherwise invalid.
    #[error("Invalid password: {0}")]
    InvalidPassword(&'static str),

    /// Argon2 internal error (parameter misconfiguration).
    #[error("Key derivation error: {0}")]
    KeyDerivation(String),

    /// BLAKE3 keyed hash length mismatch (should never happen).
    #[error("Hash output length error")]
    HashLength,
}

/// Errors produced by the LT Fountain Code layer.
#[derive(Debug, Error, PartialEq)]
pub enum FountainError {
    /// Decoder is not yet complete; contains the current progress (0.0–1.0).
    #[error("Decoding incomplete: {:.1}% recovered", progress * 100.0)]
    Incomplete {
        /// Fraction of source blocks decoded so far (0.0 – 1.0).
        progress: f64,
    },

    /// `add_droplet` was called before `set_block_count`.
    #[error("Block count not initialised — call set_block_count() first")]
    BlockCountNotSet,

    /// A droplet packet is too short to contain the header.
    #[error("Droplet too short: need at least {min} bytes, got {got}")]
    DropletTooShort {
        /// Minimum required byte length.
        min: usize,
        /// Actual byte length received.
        got: usize,
    },

    /// Source data is empty.
    #[error("Input data must not be empty")]
    EmptyInput,
}

/// Errors produced by the protocol / wire-format layer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolError {
    /// METADATA magic bytes did not match `AFTI`.
    #[error("Invalid magic: expected b\"AFTI\", got {got:?}")]
    InvalidMagic {
        /// The four magic bytes that were found instead of the expected value.
        got: [u8; 4],
    },

    /// Protocol version not supported by this implementation.
    #[error("Unknown protocol version {0}; this build supports versions 1, 2, and 3")]
    UnknownVersion(u8),

    /// METADATA frame is truncated.
    #[error("Metadata frame too short: need {min} bytes, got {got}")]
    MetadataTooShort {
        /// Minimum required byte length.
        min: usize,
        /// Actual byte length received.
        got: usize,
    },

    /// Filename in the METADATA frame is not valid UTF-8.
    #[error("Filename is not valid UTF-8")]
    InvalidFilename,
}