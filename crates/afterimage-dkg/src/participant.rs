//! Per-participant DKG logic: Round 1, Round 2, and Finish.
//!
//! Each participant runs these three functions sequentially.  Between rounds
//! the coordinator (see [`crate::coordinator`]) collects and routes packages.

use std::collections::BTreeMap;

use frost_ed25519::{
    keys::{
        dkg::{self, round1, round2},
        KeyPackage, PublicKeyPackage,
    },
    Identifier,
};
use rand_core::OsRng;

use crate::{
    error::DkgError,
    types::{DkgOutput, DkgRound1Output, DkgRound2Output, DkgRound2PackageEntry},
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn id_from_u16(v: u16) -> Result<Identifier, DkgError> {
    Identifier::try_from(v).map_err(|_| DkgError::InvalidIdentifier(v))
}

fn validate_config(n: u16, threshold: u16) -> Result<(), DkgError> {
    if n < 2 || threshold < 2 || threshold > n {
        return Err(DkgError::InvalidThreshold(n, threshold));
    }
    Ok(())
}

// ── public API ───────────────────────────────────────────────────────────────

/// **Round 1** — generate a commitment and secret package.
///
/// * `identifier`  — 1-based participant index (1..=n)
/// * `n`           — total number of participants
/// * `threshold`   — minimum signers required (must be 2 ≤ t ≤ n)
///
/// Returns a [`DkgRound1Output`] whose `round1_package_json` must be broadcast
/// to all other participants; `secret_package_json` must be kept private.
pub fn dkg_round1(identifier: u16, n: u16, threshold: u16) -> Result<DkgRound1Output, DkgError> {
    validate_config(n, threshold)?;
    let id = id_from_u16(identifier)?;

    let (secret_pkg, round1_pkg) = dkg::part1(id, n, threshold, OsRng)?;

    Ok(DkgRound1Output {
        identifier,
        secret_package_json: serde_json::to_string(&secret_pkg)?,
        round1_package_json: serde_json::to_string(&round1_pkg)?,
    })
}

/// **Round 2** — compute directed packages for every other participant.
///
/// * `my_round1`       — this participant's own Round-1 output
/// * `all_round1`      — Round-1 outputs from **all** participants (including self)
///
/// Returns a [`DkgRound2Output`] whose `round2_packages` entries must each be
/// sent **only** to the named `recipient_identifier`.
pub fn dkg_round2(
    my_round1: &DkgRound1Output,
    all_round1: &[DkgRound1Output],
) -> Result<DkgRound2Output, DkgError> {
    let secret_pkg: round1::SecretPackage =
        serde_json::from_str(&my_round1.secret_package_json)?;

    // Build the map of other participants' Round-1 packages (exclude self).
    let mut r1_map: BTreeMap<Identifier, round1::Package> = BTreeMap::new();
    for r1 in all_round1 {
        if r1.identifier == my_round1.identifier {
            continue;
        }
        let id = id_from_u16(r1.identifier)?;
        let pkg: round1::Package = serde_json::from_str(&r1.round1_package_json)?;
        r1_map.insert(id, pkg);
    }

    let (r2_secret, r2_packages) = dkg::part2(secret_pkg, &r1_map)?;

    let entries: Vec<DkgRound2PackageEntry> = r2_packages
        .into_iter()
        .map(|(id, pkg)| -> Result<DkgRound2PackageEntry, DkgError> {
            // Recover the numeric identifier from the Identifier scalar.
            // frost-ed25519 Identifier is a non-zero scalar; we stored it as u16
            // in the BTreeMap, so we round-trip via serialisation to recover the
            // canonical u16 value.
            // frost-ed25519 v2: Identifier::serialize() returns Vec<u8>
            let id_bytes: Vec<u8> = id.serialize();
            // The identifier is stored as a little-endian scalar: first two bytes
            // are the u16 value (for identifiers ≤ 65535).
            let recipient_id = u16::from_le_bytes([id_bytes[0], id_bytes[1]]);
            Ok(DkgRound2PackageEntry {
                recipient_identifier: recipient_id,
                package_json: serde_json::to_string(&pkg)?,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(DkgRound2Output {
        identifier: my_round1.identifier,
        secret_package_json: serde_json::to_string(&r2_secret)?,
        round2_packages: entries,
    })
}

/// **Finish** — combine all packages to produce the final key material.
///
/// * `my_round1`       — this participant's own Round-1 output
/// * `my_round2`       — this participant's own Round-2 output
/// * `all_round1`      — Round-1 outputs from **all** participants (including self)
/// * `all_round2`      — Round-2 outputs from **all** participants (including self)
///
/// Returns a [`DkgOutput`] whose `key_package_json` is private and
/// `pubkey_package_json` is public and identical across all participants.
pub fn dkg_finish(
    my_round1: &DkgRound1Output,
    my_round2: &DkgRound2Output,
    all_round1: &[DkgRound1Output],
    all_round2: &[DkgRound2Output],
) -> Result<DkgOutput, DkgError> {
    let r2_secret: round2::SecretPackage =
        serde_json::from_str(&my_round2.secret_package_json)?;

    // Rebuild Round-1 map (others only).
    let mut r1_map: BTreeMap<Identifier, round1::Package> = BTreeMap::new();
    for r1 in all_round1 {
        if r1.identifier == my_round1.identifier {
            continue;
        }
        let id = id_from_u16(r1.identifier)?;
        let pkg: round1::Package = serde_json::from_str(&r1.round1_package_json)?;
        r1_map.insert(id, pkg);
    }

    // Collect Round-2 packages directed **to this participant**.
    let my_id = my_round1.identifier;
    let mut r2_map: BTreeMap<Identifier, round2::Package> = BTreeMap::new();
    for r2 in all_round2 {
        if r2.identifier == my_id {
            continue; // skip self
        }
        // Find the entry in this sender's round2_packages that is for us.
        let entry = r2
            .round2_packages
            .iter()
            .find(|e| e.recipient_identifier == my_id)
            .ok_or(DkgError::MissingRound2Package(r2.identifier))?;
        let sender_id = id_from_u16(r2.identifier)?;
        let pkg: round2::Package = serde_json::from_str(&entry.package_json)?;
        r2_map.insert(sender_id, pkg);
    }

    let (key_pkg, pubkey_pkg): (KeyPackage, PublicKeyPackage) =
        dkg::part3(&r2_secret, &r1_map, &r2_map)?;

    // Extract the 32-byte compressed group public key.
    let group_pubkey_hex = {
        let bytes = pubkey_pkg
            .verifying_key()
            .serialize()
            .map_err(|e| DkgError::Frost(format!("{e:?}")))?;
        hex::encode(bytes)
    };

    Ok(DkgOutput {
        identifier: my_id,
        key_package_json: serde_json::to_string(&key_pkg)?,
        pubkey_package_json: serde_json::to_string(&pubkey_pkg)?,
        group_pubkey_hex,
    })
}

// ── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use frost_ed25519::keys::{KeyPackage, PublicKeyPackage};

    /// Run a complete DKG session for `n` participants with threshold `t`.
    /// Returns the outputs of all participants.
    fn run_dkg(n: u16, t: u16) -> Vec<DkgOutput> {
        // Round 1 — every participant commits.
        let r1_outputs: Vec<DkgRound1Output> = (1..=n)
            .map(|id| dkg_round1(id, n, t).expect("round1"))
            .collect();

        // Round 2 — every participant processes others' R1 packages.
        let r2_outputs: Vec<DkgRound2Output> = r1_outputs
            .iter()
            .map(|r1| dkg_round2(r1, &r1_outputs).expect("round2"))
            .collect();

        // Finish — every participant assembles the key.
        r1_outputs
            .iter()
            .zip(r2_outputs.iter())
            .map(|(r1, r2)| dkg_finish(r1, r2, &r1_outputs, &r2_outputs).expect("finish"))
            .collect()
    }

    // ── configuration validation ──────────────────────────────────────────

    #[test]
    fn test_invalid_threshold_t_equals_1() {
        assert!(matches!(
            dkg_round1(1, 3, 1),
            Err(DkgError::InvalidThreshold(3, 1))
        ));
    }

    #[test]
    fn test_invalid_threshold_t_equals_0() {
        assert!(matches!(
            dkg_round1(1, 3, 0),
            Err(DkgError::InvalidThreshold(3, 0))
        ));
    }

    #[test]
    fn test_invalid_threshold_t_greater_than_n() {
        assert!(matches!(
            dkg_round1(1, 2, 3),
            Err(DkgError::InvalidThreshold(2, 3))
        ));
    }

    #[test]
    fn test_invalid_n_equals_1() {
        assert!(matches!(
            dkg_round1(1, 1, 1),
            Err(DkgError::InvalidThreshold(1, 1))
        ));
    }

    #[test]
    fn test_invalid_identifier_zero() {
        // Identifier 0 is invalid in frost-ed25519.
        assert!(matches!(
            dkg_round1(0, 3, 2),
            Err(DkgError::InvalidIdentifier(0))
        ));
    }

    // ── roundtrip tests ───────────────────────────────────────────────────

    #[test]
    fn test_dkg_2_of_2_roundtrip() {
        let outputs = run_dkg(2, 2);
        assert_eq!(outputs.len(), 2);
        // All participants must share the same group public key.
        let first_hex = &outputs[0].group_pubkey_hex;
        for out in &outputs {
            assert_eq!(&out.group_pubkey_hex, first_hex);
        }
    }

    #[test]
    fn test_dkg_2_of_3_roundtrip() {
        let outputs = run_dkg(3, 2);
        assert_eq!(outputs.len(), 3);
        let first_hex = &outputs[0].group_pubkey_hex;
        for out in &outputs {
            assert_eq!(&out.group_pubkey_hex, first_hex);
        }
    }

    #[test]
    fn test_dkg_3_of_5_roundtrip() {
        let outputs = run_dkg(5, 3);
        assert_eq!(outputs.len(), 5);
        let first_hex = &outputs[0].group_pubkey_hex;
        for out in &outputs {
            assert_eq!(&out.group_pubkey_hex, first_hex);
        }
    }

    #[test]
    fn test_dkg_group_pubkey_is_32_bytes_hex() {
        let outputs = run_dkg(2, 2);
        // 32 bytes = 64 hex chars
        assert_eq!(outputs[0].group_pubkey_hex.len(), 64);
        assert!(outputs[0]
            .group_pubkey_hex
            .chars()
            .all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_dkg_key_packages_deserialise() {
        let outputs = run_dkg(2, 2);
        for out in &outputs {
            serde_json::from_str::<KeyPackage>(&out.key_package_json)
                .expect("key_package deserialises");
            serde_json::from_str::<PublicKeyPackage>(&out.pubkey_package_json)
                .expect("pubkey_package deserialises");
        }
    }

    #[test]
    fn test_dkg_key_packages_differ_per_participant() {
        let outputs = run_dkg(3, 2);
        // Each participant's private key share must be unique.
        assert_ne!(
            outputs[0].key_package_json,
            outputs[1].key_package_json,
            "key shares must differ"
        );
        assert_ne!(
            outputs[1].key_package_json,
            outputs[2].key_package_json,
            "key shares must differ"
        );
    }

    #[test]
    fn test_dkg_round1_packages_differ_per_participant() {
        // Each participant's Round-1 commitment must be distinct (fresh randomness).
        let r1_a = dkg_round1(1, 3, 2).unwrap();
        let r1_b = dkg_round1(2, 3, 2).unwrap();
        assert_ne!(
            r1_a.round1_package_json, r1_b.round1_package_json,
            "Round-1 packages must be distinct"
        );
    }

    #[test]
    fn test_dkg_round1_nonce_is_fresh_each_call() {
        // Calling round1 twice for the same participant must yield different nonces.
        let r1_first = dkg_round1(1, 2, 2).unwrap();
        let r1_second = dkg_round1(1, 2, 2).unwrap();
        assert_ne!(
            r1_first.round1_package_json, r1_second.round1_package_json,
            "Round-1 nonces must be ephemeral"
        );
    }

    #[test]
    fn test_dkg_identifiers_stored_correctly() {
        let outputs = run_dkg(3, 2);
        for (i, out) in outputs.iter().enumerate() {
            assert_eq!(out.identifier, (i + 1) as u16);
        }
    }

    #[test]
    fn test_dkg_output_compatible_with_frost_signing() {
        use frost_ed25519::{
            keys::{KeyPackage, PublicKeyPackage},
            round1 as frost_round1,
            round2 as frost_round2,
            SigningPackage,
        };
        use rand_core::OsRng;
        use std::collections::BTreeMap;

        // 1. Run DKG for 2-of-2.
        let outputs = run_dkg(2, 2);

        // 2. Deserialise key material.
        let kp1: KeyPackage =
            serde_json::from_str(&outputs[0].key_package_json).expect("kp1");
        let kp2: KeyPackage =
            serde_json::from_str(&outputs[1].key_package_json).expect("kp2");
        let pubkey_pkg: PublicKeyPackage =
            serde_json::from_str(&outputs[0].pubkey_package_json).expect("pubkey_pkg");

        // 3. FROST Round 1.
        let (nonces1, commitments1) = frost_round1::commit(kp1.signing_share(), &mut OsRng);
        let (nonces2, commitments2) = frost_round1::commit(kp2.signing_share(), &mut OsRng);

        let mut commitments_map = BTreeMap::new();
        commitments_map.insert(*kp1.identifier(), commitments1);
        commitments_map.insert(*kp2.identifier(), commitments2);

        let message = b"test message for dkg signing";
        let signing_pkg = SigningPackage::new(commitments_map, message);

        // 4. FROST Round 2.
        let sig_share1 =
            frost_round2::sign(&signing_pkg, &nonces1, &kp1).expect("sign1");
        let sig_share2 =
            frost_round2::sign(&signing_pkg, &nonces2, &kp2).expect("sign2");

        let mut shares = BTreeMap::new();
        shares.insert(*kp1.identifier(), sig_share1);
        shares.insert(*kp2.identifier(), sig_share2);

        // 5. Aggregate → verify.
        let signature =
            frost_ed25519::aggregate(&signing_pkg, &shares, &pubkey_pkg).expect("aggregate");

        pubkey_pkg
            .verifying_key()
            .verify(message, &signature)
            .expect("signature must verify");
    }
}