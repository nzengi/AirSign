//! Online-machine broadcaster — submits a signed transaction to a Solana cluster.
//!
//! This module is intentionally kept minimal: it wraps `RpcClient` with a
//! convenience method that accepts the serialised `SignResponse` payload and
//! returns the transaction signature as a base-58 string.
//!
//! # Example
//!
//! ```no_run
//! use afterimage_solana::broadcaster::Broadcaster;
//!
//! let b = Broadcaster::devnet();
//! let sig = b.broadcast_response_json(response_json_bytes).unwrap();
//! println!("https://explorer.solana.com/tx/{sig}?cluster=devnet");
//! ```

use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;

use crate::{error::AirSignError, response::SignResponse};

/// Cluster presets.
pub const DEVNET_URL: &str = "https://api.devnet.solana.com";
pub const MAINNET_URL: &str = "https://api.mainnet-beta.solana.com";
pub const TESTNET_URL: &str = "https://api.testnet.solana.com";

/// Submits signed Solana transactions to an RPC endpoint.
pub struct Broadcaster {
    client: RpcClient,
    /// Human-readable cluster name for log messages.
    pub cluster: String,
}

impl Broadcaster {
    /// Create a broadcaster for the given RPC URL.
    pub fn new(rpc_url: impl Into<String>) -> Self {
        let url: String = rpc_url.into();
        let cluster = if url.contains("devnet") {
            "devnet".to_owned()
        } else if url.contains("testnet") {
            "testnet".to_owned()
        } else if url.contains("mainnet") {
            "mainnet-beta".to_owned()
        } else {
            "custom".to_owned()
        };
        Self {
            client: RpcClient::new_with_commitment(url, CommitmentConfig::confirmed()),
            cluster,
        }
    }

    /// Convenience constructor for Solana devnet.
    pub fn devnet() -> Self {
        Self::new(DEVNET_URL)
    }

    /// Convenience constructor for Solana mainnet.
    pub fn mainnet() -> Self {
        Self::new(MAINNET_URL)
    }

    /// Broadcast a transaction from a raw `SignResponse` JSON payload
    /// (the decrypted bytes received back from the air-gapped machine).
    ///
    /// Returns the transaction signature as a base-58 string on success.
    pub fn broadcast_response_json(
        &self,
        response_json: &[u8],
    ) -> Result<String, AirSignError> {
        let resp = SignResponse::from_json(response_json)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        self.broadcast_response(&resp)
    }

    /// Broadcast a deserialised [`SignResponse`].
    pub fn broadcast_response(&self, resp: &SignResponse) -> Result<String, AirSignError> {
        let tx = resp
            .decode_transaction()
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        let sig = self
            .client
            .send_and_confirm_transaction(&tx)
            .map_err(|e| AirSignError::Rpc(e.to_string()))?;

        Ok(sig.to_string())
    }

    /// Return the current slot on the cluster (useful as a connectivity check).
    pub fn get_slot(&self) -> Result<u64, AirSignError> {
        self.client
            .get_slot()
            .map_err(|e| AirSignError::Rpc(e.to_string()))
    }
}