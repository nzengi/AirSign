//! # afterimage-dkg
//!
//! Distributed Key Generation (DKG) for FROST RFC 9591 threshold signatures
//! over Ed25519 — the curve used by Solana.
//!
//! Unlike a *trusted dealer* (see `afterimage-frost`), DKG ensures that **no
//! single party ever holds the full secret key**.  Every participant contributes
//! randomness; the group key is only derivable collectively.
//!
//! ## Protocol overview
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────────┐
//! │  ROUND 1 (broadcast)                                             │
//! │  Each participant i calls dkg_round1(i, n, t)                   │
//! │  → publishes round1_package_json to all peers                   │
//! │  → keeps secret_package_json private                            │
//! └──────────────────────────────────────────────────────────────────┘
//!              ↓  coordinator collects all Round-1 packages
//! ┌──────────────────────────────────────────────────────────────────┐
//! │  ROUND 2 (directed)                                              │
//! │  Each participant i calls dkg_round2(my_r1, all_r1)             │
//! │  → sends each round2_packages[j].package_json only to j        │
//! │  → keeps secret_package_json private                            │
//! └──────────────────────────────────────────────────────────────────┘
//!              ↓  coordinator routes per-recipient Round-2 packages
//! ┌──────────────────────────────────────────────────────────────────┐
//! │  FINISH                                                          │
//! │  Each participant i calls dkg_finish(my_r1, my_r2, all_r1, all_r2)│
//! │  → produces key_package_json (PRIVATE)                          │
//! │  → produces pubkey_package_json (PUBLIC, same for all)          │
//! └──────────────────────────────────────────────────────────────────┘
//! ```
//!
//! The resulting `key_package_json` / `pubkey_package_json` are compatible with
//! the FROST signing pipeline in `afterimage-frost`.
//!
//! ## Example
//!
//! ```rust,no_run
//! use afterimage_dkg::participant::{dkg_round1, dkg_round2, dkg_finish};
//!
//! // 2-of-3 DKG
//! let r1_1 = dkg_round1(1, 3, 2).unwrap();
//! let r1_2 = dkg_round1(2, 3, 2).unwrap();
//! let r1_3 = dkg_round1(3, 3, 2).unwrap();
//! let all_r1 = [r1_1.clone(), r1_2.clone(), r1_3.clone()];
//!
//! let r2_1 = dkg_round2(&r1_1, &all_r1).unwrap();
//! let r2_2 = dkg_round2(&r1_2, &all_r1).unwrap();
//! let r2_3 = dkg_round2(&r1_3, &all_r1).unwrap();
//! let all_r2 = [r2_1.clone(), r2_2.clone(), r2_3.clone()];
//!
//! let out_1 = dkg_finish(&r1_1, &r2_1, &all_r1, &all_r2).unwrap();
//! // out_1.key_package_json  → participant 1's private key share
//! // out_1.pubkey_package_json → public group key (identical for all)
//! // out_1.group_pubkey_hex  → 64-char hex of the Solana-compatible pubkey
//! ```

pub mod coordinator;
pub mod error;
pub mod participant;
pub mod types;

pub use error::DkgError;
pub use types::{DkgOutput, DkgRound1Output, DkgRound2Output, DkgRound2PackageEntry, DkgSetupParams};