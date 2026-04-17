//! # afterimage-frost
//!
//! FROST (Flexible Round-Optimised Schnorr Threshold) signatures for the
//! AirSign air-gapped signing system.
//!
//! This crate implements the **RFC 9591 / ZF FROST** protocol over Ed25519,
//! using the [`frost-ed25519`](https://crates.io/crates/frost-ed25519) crate
//! from the Zcash Foundation as the cryptographic backend.
//!
//! ## Protocol overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │  SETUP (trusted dealer)                                 │
//! │  dealer::generate_setup(n, t) → FrostSetup              │
//! │    ├── key_packages[0..n-1]  (one per participant)      │
//! │    └── pubkey_package         (shared)                  │
//! └───────────────────────┬─────────────────────────────────┘
//!                         │  distribute out-of-band (QR stream)
//!                         ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │  ROUND 1  (parallel, t of n participants)               │
//! │  participant::round1_commit(key_pkg, id)                │
//! │    ├── nonces_json      ← PRIVATE, stays on device      │
//! │    └── commitments_json → send to aggregator            │
//! └───────────────────────┬─────────────────────────────────┘
//!                         │  collect t commitments
//!                         ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │  AGGREGATOR  builds SigningPackage                       │
//! │  aggregator::build_signing_package(commitments, msg)    │
//! │    └── signing_package_json → broadcast to t signers    │
//! └───────────────────────┬─────────────────────────────────┘
//!                         │
//!                         ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │  ROUND 2  (parallel, t of n participants)               │
//! │  participant::round2_sign(key_pkg, nonces, pkg, id)     │
//! │    └── share_json → send to aggregator                  │
//! └───────────────────────┬─────────────────────────────────┘
//!                         │  collect t shares
//!                         ▼
//! ┌─────────────────────────────────────────────────────────┐
//! │  AGGREGATOR  combines shares → Ed25519 signature        │
//! │  aggregator::aggregate(pkg, shares, pubkeys, t, n)      │
//! │    └── FrostResult { signature_hex, verifying_key_hex } │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! The final `signature_hex` (64 bytes) is a standard Ed25519 signature
//! indistinguishable from a single-signer one — no FROST tooling is needed
//! to verify or broadcast it on-chain.

pub mod aggregator;
pub mod dealer;
pub mod error;
pub mod participant;
pub mod types;

// ─── Re-exports ───────────────────────────────────────────────────────────────

pub use error::FrostError;
pub use types::{FrostResult, FrostSetup, Round1Output, Round2Output};