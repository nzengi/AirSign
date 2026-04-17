//! JSON-serializable wire types for the FROST protocol rounds.
//!
//! These types travel between participants and the aggregator.  They are
//! designed so that each field is a JSON-encoded sub-object (produced by
//! `serde_json::to_string`) — this makes them easy to copy-paste or transmit
//! as QR payloads without any binary encoding step.

use serde::{Deserialize, Serialize};

// ─── Setup ───────────────────────────────────────────────────────────────────

/// Output of the trusted-dealer key generation step.
///
/// `key_packages[i]` is the JSON `KeyPackage` for participant `i+1` (1-indexed).
/// `pubkey_package` is shared with every participant and the aggregator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrostSetup {
    /// Total number of signers (N).
    pub n: u16,
    /// Minimum signatures required (M / threshold).
    pub threshold: u16,
    /// Per-participant `KeyPackage` JSON strings (index 0 → participant 1).
    pub key_packages: Vec<String>,
    /// Shared `PublicKeyPackage` JSON string.
    pub pubkey_package: String,
}

// ─── Round 1 ─────────────────────────────────────────────────────────────────

/// Output of Round 1 for a single participant.
///
/// `nonces_json` MUST stay with the participant — it is NEVER sent to anyone.
/// `commitments_json` is sent to the aggregator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round1Output {
    /// 1-indexed participant identifier.
    pub identifier: u16,
    /// JSON-serialized `SigningNonces` — **private, never shared**.
    pub nonces_json: String,
    /// JSON-serialized `SigningCommitments` — sent to the aggregator.
    pub commitments_json: String,
}

// ─── Round 2 ─────────────────────────────────────────────────────────────────

/// Output of Round 2 for a single participant.
///
/// `share_json` is the `SignatureShare` sent to the aggregator for combination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Round2Output {
    /// 1-indexed participant identifier.
    pub identifier: u16,
    /// JSON-serialized `SignatureShare` — sent to the aggregator.
    pub share_json: String,
}

// ─── Final result ─────────────────────────────────────────────────────────────

/// Final output produced by the aggregator after combining all shares.
///
/// The `signature_hex` is a standard Ed25519 signature (64 bytes) and can be
/// verified against `verifying_key_hex` (32 bytes) without any FROST tooling.
/// It is indistinguishable from a single-signer Ed25519 signature on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrostResult {
    /// Hex-encoded aggregated Ed25519 signature (64 bytes).
    pub signature_hex: String,
    /// Hex-encoded group verifying key (32 bytes — the on-chain public key).
    pub verifying_key_hex: String,
    /// Hex-encoded message that was signed.
    pub message_hex: String,
    /// Threshold M used in this signing session.
    pub threshold: u16,
    /// Total participants N configured in the key setup.
    pub total_participants: u16,
}