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

    /// A previously seen nonce was replayed.
    #[error("replay attack detected: nonce '{0}' was already used")]
    ReplayDetected(String),

    /// The user declined to sign the transaction after reviewing it.
    #[error("signing aborted by user")]
    UserAborted,

    /// I/O error when reading/writing nonce store or keypair file.
    #[error("I/O error: {0}")]
    Io(String),

    /// AfterImage core session error.
    #[error("session error: {0}")]
    Session(#[from] afterimage_core::error::AfterImageError),

    /// JSON serialisation / deserialisation error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Solana RPC error (broadcast / query failure).
    #[error("RPC error: {0}")]
    Rpc(String),
}
