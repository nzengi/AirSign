//! # afterimage-core
//!
//! Core primitives for the AfterImage air-gap data-transfer protocol:
//!
//! * [`crypto`]   — ChaCha20-Poly1305 + Argon2id encryption / decryption
//! * [`fountain`] — Rateless LT Fountain Code encoder and decoder
//! * [`protocol`] — METADATA / DATA frame wire format
//! * [`error`]    — Unified error types
//!
//! The `session` module provides a high-level `SendSession` / `RecvSession` pair
//! that wires all of the above together.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

extern crate alloc;

pub mod crypto;
pub mod error;
pub mod fountain;
pub mod protocol;
pub mod session;

// Re-export the most commonly used types at the crate root
pub use crypto::{blake3_digest, blake3_mac, Argon2Params, CryptoLayer};
pub use error::{AfterImageError, CryptoError, FountainError, ProtocolError};
pub use fountain::{LTDecoder, LTEncoder, RobustSoliton, BLOCK_SIZE, HEADER_SIZE};
pub use protocol::{
    MetadataFrame, CURRENT_VERSION, MAGIC, META_SIZE, META_SIZE_V2, META_SIZE_V3,
    METADATA_INTERVAL,
};
pub use session::{RecvSession, SendSession};
