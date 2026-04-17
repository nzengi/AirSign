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

pub mod broadcaster;
pub mod error;
pub mod keystore;
pub mod ledger;
pub mod ledger_apdu;
pub mod multisig_request;
pub mod multisig_response;
pub mod multisigner;
pub mod request;
pub mod response;
pub mod signer;

pub use broadcaster::Broadcaster;
pub use error::{AirSignError, KeyStoreError, LedgerError};
pub use keystore::KeyStore;
pub use ledger::{LedgerSigner, LedgerDeviceInfo};
pub use ledger_apdu::DerivationPath;
pub use multisig_request::{MultiSignRequest, PartialSig};
pub use multisig_response::MultiSignResponse;
pub use multisigner::{MultiSigner, build_multisig_session, advance_round, advance_round_from};
pub use request::SignRequest;
pub use response::SignResponse;
pub use signer::{AirSigner, summarize_request, default_nonce_store_path};
