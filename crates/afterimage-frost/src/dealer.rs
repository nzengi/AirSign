//! Trusted-dealer key generation for FROST.
//!
//! In the trusted-dealer model a single coordinator generates all N key shares
//! from OS randomness and distributes them (securely, out-of-band) to the
//! corresponding participants.  This is simpler than DKG and appropriate for
//! small, high-trust groups (e.g. a DAO multisig whose members all trust the
//! coordinator).
//!
//! For higher-assurance deployments the coordinator can be run on an
//! air-gapped machine and the key shares transmitted via QR stream.

use std::collections::BTreeMap;

use frost_ed25519::{
    self as frost,
    keys::{IdentifierList, KeyPackage, PublicKeyPackage, SecretShare},
    Identifier,
};

use crate::{error::FrostError, types::FrostSetup};

/// Generate a complete `FrostSetup` for a t-of-n signing group.
///
/// Returns [`FrostSetup`] containing:
/// - One JSON-serialized `KeyPackage` per participant (index 0 = participant 1).
/// - A JSON-serialized `PublicKeyPackage` to be shared with every participant
///   and the aggregator.
///
/// # Errors
/// Returns [`FrostError::InvalidThreshold`] if `threshold == 0`, `threshold > n`,
/// or `n == 0`.
pub fn generate_setup(n: u16, threshold: u16) -> Result<FrostSetup, FrostError> {
    // frost-ed25519 requires at least 2 participants and threshold >= 2.
    if n < 2 || threshold < 2 || threshold > n {
        return Err(FrostError::InvalidThreshold(threshold, n));
    }

    let mut rng = rand_core::OsRng;
    let (shares, pubkey_package): (BTreeMap<Identifier, SecretShare>, PublicKeyPackage) =
        frost::keys::generate_with_dealer(n, threshold, IdentifierList::Default, &mut rng)?;

    let pubkey_json = serde_json::to_string(&pubkey_package)?;

    // Convert SecretShare → KeyPackage for each participant (1-indexed)
    let mut key_packages: Vec<String> = Vec::with_capacity(n as usize);
    for i in 1..=n {
        let id = Identifier::try_from(i).map_err(|_| FrostError::InvalidIdentifier(i))?;
        let share = shares.get(&id).ok_or(FrostError::MissingCommitment(i))?;
        let kp = KeyPackage::try_from(share.clone())?;
        key_packages.push(serde_json::to_string(&kp)?);
    }

    Ok(FrostSetup {
        n,
        threshold,
        key_packages,
        pubkey_package: pubkey_json,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_1_of_1_rejected() {
        // frost-ed25519 requires min_signers >= 2
        assert!(generate_setup(1, 1).is_err());
    }

    #[test]
    fn generate_2_of_2() {
        let s = generate_setup(2, 2).unwrap();
        assert_eq!(s.n, 2);
        assert_eq!(s.threshold, 2);
        assert_eq!(s.key_packages.len(), 2);
        serde_json::from_str::<serde_json::Value>(&s.key_packages[0]).unwrap();
        serde_json::from_str::<serde_json::Value>(&s.pubkey_package).unwrap();
    }

    #[test]
    fn generate_2_of_3() {
        let s = generate_setup(3, 2).unwrap();
        assert_eq!(s.key_packages.len(), 3);
    }

    #[test]
    fn generate_3_of_5() {
        let s = generate_setup(5, 3).unwrap();
        assert_eq!(s.key_packages.len(), 5);
    }

    #[test]
    fn zero_n_rejected() {
        assert!(generate_setup(0, 0).is_err());
    }

    #[test]
    fn zero_threshold_rejected() {
        assert!(generate_setup(3, 0).is_err());
    }

    #[test]
    fn threshold_one_rejected() {
        // FROST requires threshold >= 2
        assert!(generate_setup(3, 1).is_err());
    }

    #[test]
    fn threshold_exceeds_n_rejected() {
        assert!(generate_setup(2, 3).is_err());
    }

    #[test]
    fn key_packages_are_unique() {
        let s = generate_setup(3, 2).unwrap();
        assert_ne!(s.key_packages[0], s.key_packages[1]);
        assert_ne!(s.key_packages[1], s.key_packages[2]);
    }
}