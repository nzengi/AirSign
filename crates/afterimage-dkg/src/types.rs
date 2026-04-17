use serde::{Deserialize, Serialize};

/// Configuration parameters for a DKG session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgSetupParams {
    /// Total number of participants (`n`).
    pub n: u16,
    /// Signing threshold (`t`). Requires `2 ≤ t ≤ n`.
    pub threshold: u16,
}

/// Output of Round 1 for a single participant.
///
/// `secret_package_json` is **private** and must never leave the participant's device.
/// `round1_package_json` is **public** and should be broadcast to all other participants.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound1Output {
    /// 1-based participant identifier (1..=n).
    pub identifier: u16,
    /// Serialised `frost_ed25519::keys::dkg::round1::SecretPackage` (PRIVATE).
    pub secret_package_json: String,
    /// Serialised `frost_ed25519::keys::dkg::round1::Package` (PUBLIC).
    pub round1_package_json: String,
}

/// Output of Round 2 for a single participant.
///
/// `secret_package_json` is **private**.
/// Each `DkgRound2PackageEntry` in `round2_packages` must be sent **only to its
/// intended `recipient_identifier`** — do not broadcast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2Output {
    /// 1-based participant identifier of the sender.
    pub identifier: u16,
    /// Serialised `frost_ed25519::keys::dkg::round2::SecretPackage` (PRIVATE).
    pub secret_package_json: String,
    /// One package per peer participant (PUBLIC, per-recipient).
    pub round2_packages: Vec<DkgRound2PackageEntry>,
}

/// A single directed Round-2 package from one participant to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgRound2PackageEntry {
    /// Identifier of the participant this package is intended for.
    pub recipient_identifier: u16,
    /// Serialised `frost_ed25519::keys::dkg::round2::Package` (PUBLIC, but directed).
    pub package_json: String,
}

/// Final DKG output for one participant.
///
/// `key_package_json` is **private** and must never leave the participant's device.
/// `pubkey_package_json` is **public** — it is identical for every participant and
/// can be published / stored openly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DkgOutput {
    /// 1-based participant identifier.
    pub identifier: u16,
    /// Serialised `frost_ed25519::keys::KeyPackage` (PRIVATE — this participant's key share).
    pub key_package_json: String,
    /// Serialised `frost_ed25519::keys::PublicKeyPackage` (PUBLIC — group public key).
    pub pubkey_package_json: String,
    /// Hex-encoded group public key (32 bytes), for quick display.
    pub group_pubkey_hex: String,
}