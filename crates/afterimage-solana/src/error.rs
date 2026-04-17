//! AirSign error types.

use thiserror::Error;

/// Errors produced by [`crate::keystore::KeyStore`].
#[derive(Debug, Error)]
pub enum KeyStoreError {
    /// No entry found for the given label.
    #[error("key not found: '{0}'")]
    NotFound(String),

    /// An entry with this label already exists.
    #[error("key already exists: '{0}' (use --overwrite to replace)")]
    AlreadyExists(String),

    /// The stored bytes are not a valid Ed25519 keypair.
    #[error("invalid key data: {0}")]
    InvalidKeyData(String),

    /// OS keychain backend error.
    #[error("keychain backend error: {0}")]
    Backend(String),

    /// File I/O error during import/export.
    #[error("I/O error: {0}")]
    Io(String),
}

/// Errors produced by [`crate::ledger::LedgerSigner`].
#[derive(Debug, Error)]
pub enum LedgerError {
    /// No Ledger device found via USB HID.
    #[error("no Ledger device found — connect a Ledger and open the Solana app")]
    NotFound,

    /// The Solana app is not open on the device (or the wrong app is open).
    #[error("Solana app not open on Ledger — unlock the device and open the Solana app")]
    AppNotOpen,

    /// The user rejected the action on the Ledger display.
    #[error("action rejected on Ledger device")]
    UserDenied,

    /// HID transport error (underlying `hidapi` error).
    #[error("HID transport error: {0}")]
    Hid(String),

    /// The device returned an unexpected or malformed response.
    #[error("invalid Ledger response: {0}")]
    InvalidResponse(String),

    /// The caller supplied invalid data (e.g. empty transaction bytes).
    #[error("invalid data: {0}")]
    InvalidData(String),
}

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
