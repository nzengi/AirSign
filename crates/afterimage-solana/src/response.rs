//! AirSign sign-response envelope.
//!
//! The air-gapped signer serialises this to JSON, encrypts it, and transmits
//! it back to the online machine as a QR stream.

use serde::{Deserialize, Serialize};

/// A signed transaction returned from the air-gapped signer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignResponse {
    /// AirSign envelope version — always `1` for this release.
    pub version: u8,

    /// Echo of the nonce from the corresponding [`SignRequest`].
    pub nonce: String,

    /// Base58-encoded public key that produced the signature.
    pub signer_pubkey: String,

    /// Base64-encoded 64-byte Ed25519 signature over the transaction message bytes.
    pub signature_b64: String,

    /// Bincode-serialised fully-signed `Transaction`, base64-encoded.
    /// The online machine can submit this directly.
    pub signed_transaction_b64: String,
}

impl SignResponse {
    /// Deserialise from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Serialise to compact JSON bytes.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Decode the 64-byte Ed25519 signature.
    pub fn decode_signature(&self) -> Result<[u8; 64], Box<dyn std::error::Error>> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let raw = STANDARD.decode(&self.signature_b64)?;
        let arr: [u8; 64] = raw.try_into().map_err(|_| "signature must be 64 bytes")?;
        Ok(arr)
    }

    /// Decode the fully-signed transaction.
    pub fn decode_transaction(
        &self,
    ) -> Result<solana_sdk::transaction::Transaction, Box<dyn std::error::Error>> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let raw = STANDARD.decode(&self.signed_transaction_b64)?;
        let tx: solana_sdk::transaction::Transaction = bincode::deserialize(&raw)?;
        Ok(tx)
    }
}