//! FROST participant logic — Round 1 and Round 2.
//!
//! Each participant runs on its own (air-gapped) machine:
//!
//! **Round 1** (`round1_commit`):
//!   - Generates fresh signing nonces from OS randomness.
//!   - Returns the *private* nonces (never leave this machine) and the
//!     *public* commitments (sent to the aggregator).
//!
//! **Round 2** (`round2_sign`):
//!   - Receives the aggregator's `SigningPackage` (which embeds the message and
//!     all participants' commitments).
//!   - Produces a `SignatureShare` using the private nonces from Round 1.

use frost_ed25519::{self as frost, keys::KeyPackage, round1, round2};

use crate::{
    error::FrostError,
    types::{Round1Output, Round2Output},
};

/// Round 1 — commit.
///
/// Generates ephemeral `SigningNonces` (private) and the corresponding
/// `SigningCommitments` (public).  The caller **must** keep `nonces_json`
/// locally and never transmit it; only `commitments_json` is sent to the
/// aggregator.
///
/// # Parameters
/// - `key_package_json` — JSON-serialized `KeyPackage` for this participant.
/// - `identifier` — 1-indexed participant number (must match the key package).
pub fn round1_commit(key_package_json: &str, identifier: u16) -> Result<Round1Output, FrostError> {
    let kp: KeyPackage = serde_json::from_str(key_package_json)?;
    let mut rng = rand_core::OsRng;
    let (nonces, commitments) = round1::commit(kp.signing_share(), &mut rng);
    Ok(Round1Output {
        identifier,
        nonces_json: serde_json::to_string(&nonces)?,
        commitments_json: serde_json::to_string(&commitments)?,
    })
}

/// Round 2 — sign.
///
/// Computes this participant's `SignatureShare` by combining the private nonces
/// from Round 1 with the aggregator's `SigningPackage`.
///
/// # Parameters
/// - `key_package_json` — JSON-serialized `KeyPackage` for this participant.
/// - `nonces_json` — JSON-serialized `SigningNonces` produced in Round 1.
/// - `signing_package_json` — JSON-serialized `SigningPackage` from the aggregator.
/// - `identifier` — 1-indexed participant number.
pub fn round2_sign(
    key_package_json: &str,
    nonces_json: &str,
    signing_package_json: &str,
    identifier: u16,
) -> Result<Round2Output, FrostError> {
    let kp: KeyPackage = serde_json::from_str(key_package_json)?;
    let nonces: round1::SigningNonces = serde_json::from_str(nonces_json)?;
    let pkg: frost::SigningPackage = serde_json::from_str(signing_package_json)?;
    let share = round2::sign(&pkg, &nonces, &kp)?;
    Ok(Round2Output {
        identifier,
        share_json: serde_json::to_string(&share)?,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dealer::generate_setup;

    #[test]
    fn round1_produces_non_empty_outputs() {
        let setup = generate_setup(3, 2).unwrap();
        let r1 = round1_commit(&setup.key_packages[0], 1).unwrap();
        assert_eq!(r1.identifier, 1);
        assert!(!r1.nonces_json.is_empty());
        assert!(!r1.commitments_json.is_empty());
    }

    #[test]
    fn round1_nonces_differ_per_call() {
        // FROST requires threshold >= 2, so use 2-of-2
        let setup = generate_setup(2, 2).unwrap();
        let r1a = round1_commit(&setup.key_packages[0], 1).unwrap();
        let r1b = round1_commit(&setup.key_packages[0], 1).unwrap();
        // Nonces must be freshly generated each time
        assert_ne!(r1a.nonces_json, r1b.nonces_json);
    }

    #[test]
    fn round1_commitments_differ_per_call() {
        let setup = generate_setup(2, 2).unwrap();
        let r1a = round1_commit(&setup.key_packages[0], 1).unwrap();
        let r1b = round1_commit(&setup.key_packages[0], 1).unwrap();
        assert_ne!(r1a.commitments_json, r1b.commitments_json);
    }

    #[test]
    fn invalid_key_package_json_errors() {
        let result = round1_commit("{bad json", 1);
        assert!(result.is_err());
    }
}