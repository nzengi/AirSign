use thiserror::Error;

/// All errors that can be produced by the `afterimage-dkg` crate.
#[derive(Debug, Error)]
pub enum DkgError {
    /// Participant identifier must be in 1..=n.
    #[error("invalid identifier {0}: must be in 1..=n")]
    InvalidIdentifier(u16),

    /// Configuration is logically invalid.
    #[error("invalid threshold config: n={0}, t={1} — require 2 ≤ t ≤ n")]
    InvalidThreshold(u16, u16),

    /// A required Round-1 package from participant `{0}` is missing.
    #[error("missing Round-1 package from participant {0}")]
    MissingRound1Package(u16),

    /// A required Round-2 package intended for this participant is missing.
    #[error("missing Round-2 package for participant {0}")]
    MissingRound2Package(u16),

    /// An unexpected number of packages was provided.
    #[error("expected {expected} packages, got {got}")]
    PackageCountMismatch { expected: usize, got: usize },

    /// An error produced by the `frost-ed25519` library.
    #[error("frost error: {0}")]
    Frost(String),

    /// JSON serialisation / deserialisation error.
    #[error("serialisation error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl From<frost_ed25519::Error> for DkgError {
    fn from(e: frost_ed25519::Error) -> Self {
        DkgError::Frost(format!("{e:?}"))
    }
}