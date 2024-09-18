//! # afterimage-solana — AirSign
//!
//! Air-gap Solana transaction signing via AfterImage QR streams.
//!
//! ## Protocol (AirSign v1)
//!
//! ```text
//! Online machine (watch-only wallet)         Air-gapped machine (signer)
//! ──────────────────────────────────         ──────────────────────────
//! 1. Build unsigned Transaction
//! 2. Serialise → CBOR/JSON envelope
//! 3. Encrypt + fountain-encode → QR stream ──► Receive QR stream
//!                                               Decrypt → SignRequest
//!                                               Sign with Ed25519 keypair
//!                                               Encrypt + fountain-encode ◄── Receive QR stream
//!                                          ◄── Return SignResponse
//! 4. Inject signature(s) into Transaction
//! 5. Submit to cluster
//! ```
//!
//! The encrypted channel prevents an observer from learning the transaction
//! contents from a screen recording or CCTV footage.

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod error;
pub mod request;
pub mod response;
pub mod signer;

pub use error::AirSignError;
pub use request::SignRequest;
pub use response::SignResponse;
pub use signer::AirSigner;