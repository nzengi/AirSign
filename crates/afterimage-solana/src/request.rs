//! AirSign sign-request envelope.
//!
//! The online machine serialises this to JSON, encrypts it with the shared
//! AfterImage password, and transmits it as a QR stream.

use serde::{Deserialize, Serialize};

/// An unsigned Solana transaction waiting for an air-gapped signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignRequest {
    /// AirSign envelope version — always `1` for this release.
    pub version: u8,

    /// Random 32-byte nonce (hex-encoded) to prevent replay attacks.
    pub nonce: String,

    /// Base58-encoded public key that must sign this transaction.
    pub signer_pubkey: String,

    /// Bincode-serialised `solana_sdk::transaction::Transaction` (unsigned).
    /// Base64-encoded for JSON transport.
    pub transaction_b64: String,

    /// Human-readable description shown on the air-gapped machine's screen.
    pub description: String,

    /// Unix timestamp (seconds) when this request was created.
    pub created_at: i64,

    /// Optional: cluster URL hint (e.g. "mainnet-beta", "devnet").
    #[serde(default)]
    pub cluster: String,
}

impl SignRequest {
    /// Deserialise from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Serialise to compact JSON bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Decode the embedded transaction bytes (bincode).
    pub fn decode_transaction(
        &self,
    ) -> Result<solana_sdk::transaction::Transaction, Box<dyn std::error::Error>> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let raw = STANDARD.decode(&self.transaction_b64)?;
        let tx: solana_sdk::transaction::Transaction = bincode::deserialize(&raw)?;
        Ok(tx)
    }
}