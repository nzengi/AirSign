//! M-of-N multi-signature response envelope.
//!
//! [`MultiSignResponse`] is produced by the **air-gapped machine** after
//! processing a [`MultiSignRequest`] and is transmitted back to the online
//! machine via the AfterImage QR channel.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

use crate::{error::AirSignError, multisig_request::PartialSig};

/// Response from one air-gapped signing round.
///
/// JSON schema version: `2`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSignResponse {
    /// Schema version — always `2`.
    pub version: u8,
    /// Echo of the request nonce (replay-protection token).
    pub nonce: String,
    /// Round that produced this response.
    pub round: u8,
    /// Base58 public key of the signer who produced this response.
    pub signer_pubkey: String,
    /// Base64-encoded Ed25519 signature contributed in this round (64 bytes).
    pub signature_b64: String,
    /// All partial signatures accumulated so far (including this round).
    pub partial_sigs: Vec<PartialSig>,
    /// Base64-encoded bincode-serialised `Transaction` with all signatures
    /// applied to date.
    pub signed_transaction_b64: String,
    /// `true` when `partial_sigs.len() >= threshold` — the session is done.
    pub complete: bool,
}

impl MultiSignResponse {
    /// Deserialise from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Serialise to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Decode the embedded `signed_transaction_b64` into a
    /// [`solana_sdk::transaction::Transaction`].
    pub fn decode_transaction(
        &self,
    ) -> Result<solana_sdk::transaction::Transaction, AirSignError> {
        let raw = STANDARD
            .decode(&self.signed_transaction_b64)
            .map_err(|e| AirSignError::InvalidRequest(format!("base64 decode: {e}")))?;
        bincode::deserialize(&raw)
            .map_err(|e| AirSignError::InvalidRequest(format!("bincode decode: {e}")))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_resp(complete: bool) -> MultiSignResponse {
        MultiSignResponse {
            version: 2,
            nonce: "aabbcc".to_owned(),
            round: 1,
            signer_pubkey: "A".to_owned(),
            signature_b64: "c2ln".to_owned(),
            partial_sigs: vec![PartialSig {
                signer_pubkey: "A".to_owned(),
                signature_b64: "c2ln".to_owned(),
            }],
            signed_transaction_b64: "dGVzdA==".to_owned(),
            complete,
        }
    }

    #[test]
    fn json_roundtrip_complete() {
        let resp = sample_resp(true);
        let json = resp.to_json().unwrap();
        let back = MultiSignResponse::from_json(json.as_bytes()).unwrap();
        assert!(back.complete);
        assert_eq!(back.nonce, "aabbcc");
    }

    #[test]
    fn json_roundtrip_incomplete() {
        let resp = sample_resp(false);
        let json = resp.to_json().unwrap();
        let back = MultiSignResponse::from_json(json.as_bytes()).unwrap();
        assert!(!back.complete);
    }
}