//! Coordinator helpers — collect and route DKG packages between participants.
//!
//! The coordinator is **untrusted**: it only sees public Round-1 packages and
//! routes directed Round-2 packages without ever seeing private material.

use crate::types::{DkgRound1Output, DkgRound2Output, DkgRound2PackageEntry};

/// Returns `true` when Round-1 outputs have been received from every participant.
///
/// `slots` has one entry per participant (indexed 0..n-1).  An entry is `None`
/// until the participant delivers their Round-1 output.
pub fn all_round1_received(slots: &[Option<DkgRound1Output>]) -> bool {
    slots.iter().all(|s| s.is_some())
}

/// Returns `true` when Round-2 outputs have been received from every participant.
pub fn all_round2_received(slots: &[Option<DkgRound2Output>]) -> bool {
    slots.iter().all(|s| s.is_some())
}

/// Extract the Round-2 packages that are addressed to `recipient_id` from the
/// full set of all participants' Round-2 outputs.
///
/// Returns a `Vec` of `(sender_identifier, DkgRound2PackageEntry)` tuples.
pub fn packages_for_participant(
    recipient_id: u16,
    all_r2: &[DkgRound2Output],
) -> Vec<(u16, DkgRound2PackageEntry)> {
    all_r2
        .iter()
        .filter(|r2| r2.identifier != recipient_id)
        .filter_map(|r2| {
            r2.round2_packages
                .iter()
                .find(|e| e.recipient_identifier == recipient_id)
                .map(|e| (r2.identifier, e.clone()))
        })
        .collect()
}

/// Collect all **public** Round-1 packages from the slot buffer (panics if any
/// slot is empty — call [`all_round1_received`] first).
pub fn collect_round1_packages(slots: &[Option<DkgRound1Output>]) -> Vec<DkgRound1Output> {
    slots
        .iter()
        .map(|s| s.clone().expect("slot must be filled before collecting"))
        .collect()
}

/// Collect all Round-2 outputs from the slot buffer (panics if any slot is
/// empty — call [`all_round2_received`] first).
pub fn collect_round2_outputs(slots: &[Option<DkgRound2Output>]) -> Vec<DkgRound2Output> {
    slots
        .iter()
        .map(|s| s.clone().expect("slot must be filled before collecting"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DkgRound2PackageEntry;

    fn make_r1(id: u16) -> DkgRound1Output {
        DkgRound1Output {
            identifier: id,
            secret_package_json: "{}".to_string(),
            round1_package_json: "{}".to_string(),
        }
    }

    fn make_r2(sender: u16, recipients: &[u16]) -> DkgRound2Output {
        DkgRound2Output {
            identifier: sender,
            secret_package_json: "{}".to_string(),
            round2_packages: recipients
                .iter()
                .map(|&r| DkgRound2PackageEntry {
                    recipient_identifier: r,
                    package_json: format!("{{\"from\":{},\"to\":{}}}", sender, r),
                })
                .collect(),
        }
    }

    #[test]
    fn test_all_round1_received_false_when_empty() {
        let slots: Vec<Option<DkgRound1Output>> = vec![None, None, None];
        assert!(!all_round1_received(&slots));
    }

    #[test]
    fn test_all_round1_received_false_when_partial() {
        let slots = vec![Some(make_r1(1)), None, Some(make_r1(3))];
        assert!(!all_round1_received(&slots));
    }

    #[test]
    fn test_all_round1_received_true_when_full() {
        let slots = vec![Some(make_r1(1)), Some(make_r1(2)), Some(make_r1(3))];
        assert!(all_round1_received(&slots));
    }

    #[test]
    fn test_all_round2_received_false_when_partial() {
        let slots: Vec<Option<DkgRound2Output>> = vec![Some(make_r2(1, &[2, 3])), None, None];
        assert!(!all_round2_received(&slots));
    }

    #[test]
    fn test_all_round2_received_true_when_full() {
        let slots = vec![
            Some(make_r2(1, &[2, 3])),
            Some(make_r2(2, &[1, 3])),
            Some(make_r2(3, &[1, 2])),
        ];
        assert!(all_round2_received(&slots));
    }

    #[test]
    fn test_packages_for_participant_correct_routing() {
        // Participant 1 is the recipient; should receive packages from 2 and 3.
        let r2_outputs = vec![make_r2(2, &[1, 3]), make_r2(3, &[1, 2])];
        let pkgs = packages_for_participant(1, &r2_outputs);
        assert_eq!(pkgs.len(), 2);
        let senders: Vec<u16> = pkgs.iter().map(|(s, _)| *s).collect();
        assert!(senders.contains(&2));
        assert!(senders.contains(&3));
    }

    #[test]
    fn test_packages_for_participant_excludes_self() {
        let r2_outputs = vec![
            make_r2(1, &[2, 3]), // self — should be excluded
            make_r2(2, &[1, 3]),
            make_r2(3, &[1, 2]),
        ];
        let pkgs = packages_for_participant(1, &r2_outputs);
        // Only senders 2 and 3 should appear (not 1).
        let senders: Vec<u16> = pkgs.iter().map(|(s, _)| *s).collect();
        assert!(!senders.contains(&1));
        assert_eq!(pkgs.len(), 2);
    }

    #[test]
    fn test_collect_round1_packages() {
        let slots = vec![Some(make_r1(1)), Some(make_r1(2))];
        let pkgs = collect_round1_packages(&slots);
        assert_eq!(pkgs.len(), 2);
        assert_eq!(pkgs[0].identifier, 1);
        assert_eq!(pkgs[1].identifier, 2);
    }
}