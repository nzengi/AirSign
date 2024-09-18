//! AirSign error types.

use thiserror::Error;

/// Errors produced by the AirSign Solana integration.
#[derive(Debug, Error)]
pub enum AirSignError {
    /// The sign request envelope could not be deserialised.
    #[error("invalid sign request: {0}")]
    InvalidRequest(String),

    /// The sign response envelope could not be deserialised.
    #[error("invalid sign response: {0}")]
    InvalidResponse(String),

    /// Ed25519 signature verification failed.
    #[error("signature verification failed")]
    VerificationFailed,

    /// The request nonce in the response did not match.
    #[error("nonce mismatch — possible replay attack")]
    NonceMismatch,

    /// AfterImage core session error.
    #[error("session error: {0}")]
    Session(#[from] afterimage_core::error::AfterImageError),

    /// JSON serialisation / deserialisation error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}