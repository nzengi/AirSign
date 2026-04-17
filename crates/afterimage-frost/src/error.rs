//! Error type for the AirSign FROST crate.

use thiserror::Error;

/// All errors produced by the `afterimage-frost` crate.
#[derive(Debug, Error)]
pub enum FrostError {
    /// A FROST protocol-level error (invalid share, bad signature, etc.).
    #[error("FROST protocol error: {0}")]
    Frost(String),

    /// JSON serialization / deserialization failure.
    #[error("JSON error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Participant identifier out of range (must be 1 ≤ id ≤ N).
    #[error("invalid participant identifier {0} (must be 1..=N)")]
    InvalidIdentifier(u16),

    /// A required Round-1 commitment is absent.
    #[error("Round-1 commitment missing for participant {0}")]
    MissingCommitment(u16),

    /// A required Round-2 signature share is absent.
    #[error("Round-2 signature share missing for participant {0}")]
    MissingShare(u16),

    /// Threshold or participant count is logically inconsistent.
    #[error("invalid threshold: t={0} n={1} (need 1 ≤ t ≤ n)")]
    InvalidThreshold(u16, u16),

    /// Hex decode failure (e.g. when parsing the message bytes).
    #[error("hex decode error: {0}")]
    Hex(#[from] hex::FromHexError),
}

impl From<frost_ed25519::Error> for FrostError {
    fn from(e: frost_ed25519::Error) -> Self {
        FrostError::Frost(format!("{e:?}"))
    }
}