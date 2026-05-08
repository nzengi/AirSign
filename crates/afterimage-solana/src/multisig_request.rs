//! M-of-N multi-signature request envelope.
//!
//! [`MultiSignRequest`] is produced by the **online machine** and transmitted
//! to each air-gapped signer in turn.  It carries:
//!
//! - The unsigned (or partially-signed) Solana transaction in base64/bincode.
//! - The ordered list of N signer public keys.
//! - The M-of-N threshold.
//! - The current round number (1-based).
//! - Accumulated [`PartialSig`] entries from previous rounds.
//! - A random nonce for replay protection.

use serde::{Deserialize, Serialize};

// ─── Types ────────────────────────────────────────────────────────────────────

/// A single Ed25519 partial signature contributed by one signer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialSig {
    /// Base58 public key of the signer who produced this signature.
    pub signer_pubkey: String,
    /// Base64-encoded raw Ed25519 signature (64 bytes).
    pub signature_b64: String,
}

/// Round-N multi-signature request sent to the next air-gapped signer.
///
/// JSON schema version: `2`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiSignRequest {
    /// Schema version — always `2` for this implementation.
    pub version: u8,
    /// 32-byte random hex nonce (set once at session creation, carried unchanged
    /// through all rounds to prevent cross-session replay).
    pub nonce: String,
    /// Minimum number of signatures required to consider the session complete.
    pub threshold: u8,
    /// Ordered list of N signer public keys (base58).
    /// Round `r` must be signed by `signers[r - 1]`.
    pub signers: Vec<String>,
    /// Current round number (1-based).  Round 1 has no prior partial sigs.
    pub round: u8,
    /// Partial signatures accumulated from rounds 1 … (round-1).
    pub partial_sigs: Vec<PartialSig>,
    /// Base64-encoded bincode-serialised `Transaction` (unsigned or partially
    /// signed — the air-gapped machine re-signs with its key).
    pub transaction_b64: String,
    /// Human-readable description shown on the air-gapped screen.
    pub description: String,
    /// Unix timestamp (seconds) when the round-1 request was created.
    pub created_at: i64,
    /// Solana cluster hint (`mainnet-beta`, `devnet`, `testnet`, `localnet`).
    pub cluster: String,
}

impl MultiSignRequest {
    /// Deserialise from JSON bytes.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Serialise to a pretty-printed JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Return the base58 public key expected to sign the current round,
    /// or `None` if `round` exceeds the signer list length.
    pub fn current_signer(&self) -> Option<&str> {
        let idx = self.round.checked_sub(1)? as usize;
        self.signers.get(idx).map(String::as_str)
    }

    /// Return `true` if `pubkey` has already contributed a partial signature.
    pub fn has_signed(&self, pubkey: &str) -> bool {
        self.partial_sigs.iter().any(|ps| ps.signer_pubkey == pubkey)
    }

    /// Return `true` if enough partial signatures have been collected.
    pub fn threshold_met(&self) -> bool {
        self.partial_sigs.len() >= self.threshold as usize
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_req(round: u8, partial_sigs: Vec<PartialSig>) -> MultiSignRequest {
        MultiSignRequest {
            version: 2,
            nonce: "aabbcc".to_owned(),
            threshold: 2,
            signers: vec!["A".to_owned(), "B".to_owned(), "C".to_owned()],
            round,
            partial_sigs,
            transaction_b64: "dGVzdA==".to_owned(),
            description: "test".to_owned(),
            created_at: 0,
            cluster: "devnet".to_owned(),
        }
    }

    #[test]
    fn current_signer_round1() {
        assert_eq!(sample_req(1, vec![]).current_signer(), Some("A"));
    }

    #[test]
    fn current_signer_round3() {
        assert_eq!(sample_req(3, vec![]).current_signer(), Some("C"));
    }

    #[test]
    fn current_signer_out_of_range() {
        assert_eq!(sample_req(4, vec![]).current_signer(), None);
    }

    #[test]
    fn has_signed_detects_duplicate() {
        let ps = PartialSig {
            signer_pubkey: "A".to_owned(),
            signature_b64: "sig".to_owned(),
        };
        let req = sample_req(2, vec![ps]);
        assert!(req.has_signed("A"));
        assert!(!req.has_signed("B"));
    }

    #[test]
    fn threshold_met_false_initially() {
        assert!(!sample_req(1, vec![]).threshold_met());
    }

    #[test]
    fn threshold_met_after_enough_sigs() {
        let sigs = vec![
            PartialSig { signer_pubkey: "A".to_owned(), signature_b64: "s1".to_owned() },
            PartialSig { signer_pubkey: "B".to_owned(), signature_b64: "s2".to_owned() },
        ];
        assert!(sample_req(3, sigs).threshold_met());
    }

    #[test]
    fn json_roundtrip() {
        let req = sample_req(1, vec![]);
        let json = req.to_json().unwrap();
        let back: MultiSignRequest = MultiSignRequest::from_json(json.as_bytes()).unwrap();
        assert_eq!(back.round, 1);
        assert_eq!(back.signers, req.signers);
    }
}