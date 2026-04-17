//! FROST aggregator logic — builds the signing package and combines shares.
//!
//! The aggregator is an **untrusted** coordinator: it never sees any private
//! key material.  Its only jobs are:
//!
//! 1. Collect Round-1 commitments from the threshold participants.
//! 2. Build a `SigningPackage` (message + all commitments) and broadcast it.
//! 3. Collect Round-2 signature shares from the participants.
//! 4. Combine them into a single standard Ed25519 signature.

use std::collections::BTreeMap;

use frost_ed25519::{self as frost, keys::PublicKeyPackage, round1, round2, Identifier};

use crate::{
    error::FrostError,
    types::{FrostResult, Round1Output, Round2Output},
};

/// Build a JSON-serialized `SigningPackage` from Round-1 commitments.
///
/// # Parameters
/// - `commitments` — slice of [`Round1Output`] from the threshold participants.
/// - `message` — raw bytes to be signed (e.g. a Solana transaction message).
pub fn build_signing_package(
    commitments: &[Round1Output],
    message: &[u8],
) -> Result<String, FrostError> {
    let map: BTreeMap<Identifier, round1::SigningCommitments> = commitments
        .iter()
        .map(|c| {
            let id = Identifier::try_from(c.identifier)
                .map_err(|_| FrostError::InvalidIdentifier(c.identifier))?;
            let sc: round1::SigningCommitments =
                serde_json::from_str(&c.commitments_json).map_err(FrostError::Serde)?;
            Ok((id, sc))
        })
        .collect::<Result<_, FrostError>>()?;

    let pkg = frost::SigningPackage::new(map, message);
    Ok(serde_json::to_string(&pkg)?)
}

/// Aggregate Round-2 shares into a single Ed25519 signature.
///
/// # Parameters
/// - `signing_package_json` — JSON `SigningPackage` produced by [`build_signing_package`].
/// - `shares` — slice of [`Round2Output`] from the threshold participants.
/// - `pubkey_package_json` — JSON `PublicKeyPackage` from the dealer setup.
/// - `threshold` — M value (informational, stored in result).
/// - `total_participants` — N value (informational, stored in result).
pub fn aggregate(
    signing_package_json: &str,
    shares: &[Round2Output],
    pubkey_package_json: &str,
    threshold: u16,
    total_participants: u16,
) -> Result<FrostResult, FrostError> {
    let pkg: frost::SigningPackage = serde_json::from_str(signing_package_json)?;
    let pubkeys: PublicKeyPackage = serde_json::from_str(pubkey_package_json)?;

    let share_map: BTreeMap<Identifier, round2::SignatureShare> = shares
        .iter()
        .map(|s| {
            let id = Identifier::try_from(s.identifier)
                .map_err(|_| FrostError::InvalidIdentifier(s.identifier))?;
            let share: round2::SignatureShare =
                serde_json::from_str(&s.share_json).map_err(FrostError::Serde)?;
            Ok((id, share))
        })
        .collect::<Result<_, FrostError>>()?;

    let signature = frost::aggregate(&pkg, &share_map, &pubkeys)?;

    let sig_bytes = signature
        .serialize()
        .map_err(|e| FrostError::Frost(format!("{e:?}")))?;
    let vk_bytes = pubkeys
        .verifying_key()
        .serialize()
        .map_err(|e| FrostError::Frost(format!("{e:?}")))?;
    let msg_hex = hex::encode(pkg.message());

    Ok(FrostResult {
        signature_hex: hex::encode(&sig_bytes),
        verifying_key_hex: hex::encode(&vk_bytes),
        message_hex: msg_hex,
        threshold,
        total_participants,
    })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{dealer::generate_setup, participant};

    fn run_full_sign(n: u16, t: u16, message: &[u8]) -> FrostResult {
        let setup = generate_setup(n, t).unwrap();

        // Round 1 — first `t` participants commit
        let r1_outputs: Vec<_> = (1..=t)
            .map(|i| {
                participant::round1_commit(&setup.key_packages[(i - 1) as usize], i).unwrap()
            })
            .collect();

        // Aggregator builds signing package
        let pkg_json = build_signing_package(&r1_outputs, message).unwrap();

        // Round 2 — each participant signs
        let r2_outputs: Vec<_> = (1..=t)
            .map(|i| {
                participant::round2_sign(
                    &setup.key_packages[(i - 1) as usize],
                    &r1_outputs[(i - 1) as usize].nonces_json,
                    &pkg_json,
                    i,
                )
                .unwrap()
            })
            .collect();

        // Aggregator combines
        aggregate(&pkg_json, &r2_outputs, &setup.pubkey_package, t, n).unwrap()
    }

    #[test]
    fn full_roundtrip_2_of_2() {
        // frost-ed25519 requires threshold >= 2; smallest valid config is 2-of-2
        let result = run_full_sign(2, 2, b"hello solana");
        assert_eq!(result.signature_hex.len(), 128); // 64 bytes = 128 hex chars
        assert_eq!(result.verifying_key_hex.len(), 64); // 32 bytes
        assert_eq!(result.message_hex, hex::encode(b"hello solana"));
        assert_eq!(result.threshold, 2);
        assert_eq!(result.total_participants, 2);
    }

    #[test]
    fn full_roundtrip_2_of_3() {
        let result = run_full_sign(3, 2, b"transfer 1 SOL");
        assert_eq!(result.signature_hex.len(), 128);
        assert_eq!(result.verifying_key_hex.len(), 64);
        assert_eq!(result.threshold, 2);
        assert_eq!(result.total_participants, 3);
    }

    #[test]
    fn full_roundtrip_3_of_5() {
        let result = run_full_sign(5, 3, b"DAO proposal vote");
        assert_eq!(result.signature_hex.len(), 128);
        assert_eq!(result.threshold, 3);
        assert_eq!(result.total_participants, 5);
    }

    #[test]
    fn signing_package_is_valid_json() {
        let setup = generate_setup(2, 2).unwrap();
        let r1_outputs: Vec<_> = (1..=2u16)
            .map(|i| participant::round1_commit(&setup.key_packages[(i - 1) as usize], i).unwrap())
            .collect();
        let pkg_json = build_signing_package(&r1_outputs, b"test").unwrap();
        serde_json::from_str::<serde_json::Value>(&pkg_json).unwrap();
    }

    #[test]
    fn different_messages_produce_different_signatures() {
        let setup = generate_setup(2, 2).unwrap();

        let sign = |msg: &[u8]| -> String {
            let r1: Vec<_> = (1..=2u16)
                .map(|i| {
                    participant::round1_commit(&setup.key_packages[(i - 1) as usize], i).unwrap()
                })
                .collect();
            let pkg = build_signing_package(&r1, msg).unwrap();
            let r2: Vec<_> = (1..=2u16)
                .map(|i| {
                    participant::round2_sign(
                        &setup.key_packages[(i - 1) as usize],
                        &r1[(i - 1) as usize].nonces_json,
                        &pkg,
                        i,
                    )
                    .unwrap()
                })
                .collect();
            aggregate(&pkg, &r2, &setup.pubkey_package, 2, 2)
                .unwrap()
                .signature_hex
        };

        let sig_a = sign(b"message A");
        let sig_b = sign(b"message B");
        assert_ne!(sig_a, sig_b);
    }

    #[test]
    fn wrong_share_causes_aggregation_failure() {
        let setup_a = generate_setup(2, 2).unwrap();
        let setup_b = generate_setup(2, 2).unwrap();

        let r1: Vec<_> = (1..=2u16)
            .map(|i| {
                participant::round1_commit(&setup_a.key_packages[(i - 1) as usize], i).unwrap()
            })
            .collect();
        let pkg = build_signing_package(&r1, b"attack").unwrap();

        // Participant 1 from setup_a, participant 2 from setup_b → mismatch
        let share1 = participant::round2_sign(
            &setup_a.key_packages[0],
            &r1[0].nonces_json,
            &pkg,
            1,
        )
        .unwrap();
        let r1b_2 =
            participant::round1_commit(&setup_b.key_packages[1], 2).unwrap();
        let pkg_b = build_signing_package(&[r1[0].clone(), r1b_2.clone()], b"attack").unwrap();
        let share2 = participant::round2_sign(
            &setup_b.key_packages[1],
            &r1b_2.nonces_json,
            &pkg_b,
            2,
        )
        .unwrap();

        // Aggregating mismatched shares must fail
        let result = aggregate(&pkg, &[share1, share2], &setup_a.pubkey_package, 2, 2);
        assert!(result.is_err());
    }
}