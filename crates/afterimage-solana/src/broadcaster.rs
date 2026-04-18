//! Online-machine broadcaster — submits a signed transaction to a Solana cluster.
//!
//! This module wraps `RpcClient` with convenience methods for the full
//! broadcast pipeline: submit → retry → confirm → explore.
//!
//! # Example
//!
//! ```no_run
//! use afterimage_solana::broadcaster::Broadcaster;
//!
//! let response_json: &[u8] = b"{}"; // replace with real SignResponse JSON
//! let b = Broadcaster::devnet();
//! let result = b.broadcast_response_json_with_result(response_json).unwrap();
//! println!("{}", result.explorer_url);
//! ```

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    hash::Hash,
    pubkey::Pubkey,
    signature::Signature,
};
use std::str::FromStr;

use crate::{error::AirSignError, response::SignResponse};

/// Solana devnet RPC endpoint.
pub const DEVNET_URL: &str = "https://api.devnet.solana.com";
/// Solana mainnet-beta RPC endpoint.
pub const MAINNET_URL: &str = "https://api.mainnet-beta.solana.com";
/// Solana testnet RPC endpoint.
pub const TESTNET_URL: &str = "https://api.testnet.solana.com";

/// Rich result returned by [`Broadcaster::broadcast_with_result`].
#[derive(Debug, Clone)]
pub struct BroadcastResult {
    /// Base-58 transaction signature.
    pub signature: String,
    /// Cluster-relative slot at submission time (may lag finalization).
    pub slot: Option<u64>,
    /// Full Solana Explorer URL for this transaction.
    pub explorer_url: String,
    /// Human-readable cluster name (`devnet`, `testnet`, `mainnet-beta`, `custom`).
    pub cluster: String,
    /// Transaction fee in lamports (populated when available).
    pub fee_lamports: Option<u64>,
}

/// Submits signed Solana transactions to an RPC endpoint.
pub struct Broadcaster {
    pub(crate) client: RpcClient,
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

    /// Convenience constructor for Solana mainnet-beta.
    pub fn mainnet() -> Self {
        Self::new(MAINNET_URL)
    }

    /// Convenience constructor for Solana testnet.
    pub fn testnet() -> Self {
        Self::new(TESTNET_URL)
    }

    // ── Explorer URL helpers ───────────────────────────────────────────────

    /// Build the Solana Explorer URL for a given transaction signature.
    pub fn explorer_url(&self, sig: &str) -> String {
        match self.cluster.as_str() {
            "mainnet-beta" => format!("https://explorer.solana.com/tx/{sig}"),
            other => format!("https://explorer.solana.com/tx/{sig}?cluster={other}"),
        }
    }

    /// Build the Solscan URL for a given transaction signature.
    pub fn solscan_url(&self, sig: &str) -> String {
        match self.cluster.as_str() {
            "mainnet-beta" => format!("https://solscan.io/tx/{sig}"),
            other => format!("https://solscan.io/tx/{sig}?cluster={other}"),
        }
    }

    // ── Network helpers ────────────────────────────────────────────────────

    /// Return the current slot on the cluster (useful as a connectivity check).
    pub fn get_slot(&self) -> Result<u64, AirSignError> {
        self.client
            .get_slot()
            .map_err(|e| AirSignError::Rpc(e.to_string()))
    }

    /// Return the latest confirmed blockhash from the cluster.
    pub fn get_latest_blockhash(&self) -> Result<Hash, AirSignError> {
        self.client
            .get_latest_blockhash()
            .map_err(|e| AirSignError::Rpc(e.to_string()))
    }

    /// Return the SOL balance (in lamports) of the given base-58 public key.
    pub fn get_balance(&self, pubkey_str: &str) -> Result<u64, AirSignError> {
        let pk = Pubkey::from_str(pubkey_str)
            .map_err(|e| AirSignError::InvalidRequest(format!("invalid pubkey: {e}")))?;
        self.client
            .get_balance(&pk)
            .map_err(|e| AirSignError::Rpc(e.to_string()))
    }

    // ── Airdrop (devnet / testnet only) ────────────────────────────────────

    /// Request an airdrop of `lamports` to the given public key.
    ///
    /// **Only available on devnet and testnet.** Returns an error immediately
    /// if called against mainnet-beta to prevent accidental requests.
    pub fn airdrop(&self, pubkey_str: &str, lamports: u64) -> Result<String, AirSignError> {
        if self.cluster == "mainnet-beta" {
            return Err(AirSignError::InvalidRequest(
                "airdrop is not available on mainnet-beta".to_owned(),
            ));
        }
        let pk = Pubkey::from_str(pubkey_str)
            .map_err(|e| AirSignError::InvalidRequest(format!("invalid pubkey: {e}")))?;
        let sig = self
            .client
            .request_airdrop(&pk, lamports)
            .map_err(|e| AirSignError::Rpc(e.to_string()))?;
        Ok(sig.to_string())
    }

    // ── Broadcast variants ─────────────────────────────────────────────────

    /// Broadcast a raw `SignResponse` JSON payload.
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

    /// Broadcast a deserialised [`SignResponse`], returning a signature string.
    pub fn broadcast_response(&self, resp: &SignResponse) -> Result<String, AirSignError> {
        let tx = resp
            .decode_transaction()
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;
        self.broadcast_signed_transaction(&tx)
    }

    /// Broadcast an already-signed [`solana_sdk::transaction::Transaction`].
    ///
    /// Returns the transaction signature as a base-58 string on success.
    pub fn broadcast_signed_transaction(
        &self,
        tx: &solana_sdk::transaction::Transaction,
    ) -> Result<String, AirSignError> {
        let sig = self
            .client
            .send_and_confirm_transaction(tx)
            .map_err(|e| AirSignError::Rpc(e.to_string()))?;
        Ok(sig.to_string())
    }

    /// Broadcast a [`SignResponse`] with automatic retry on transient RPC errors.
    ///
    /// Retries up to `max_retries` times with exponential back-off starting at
    /// `base_delay_ms` milliseconds (capped at 32× the base delay).
    pub fn broadcast_with_retry(
        &self,
        resp: &SignResponse,
        max_retries: u32,
        base_delay_ms: u64,
    ) -> Result<String, AirSignError> {
        let mut last_err = AirSignError::Rpc("no attempts made".to_owned());
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let exponent = (attempt - 1).min(5) as u32;
                let delay_ms = base_delay_ms * (1u64 << exponent);
                eprintln!(
                    "[airsign] broadcast retry {}/{max_retries} — waiting {delay_ms} ms…",
                    attempt
                );
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }
            match self.broadcast_response(resp) {
                Ok(sig) => return Ok(sig),
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    /// Broadcast a [`SignResponse`] and return a [`BroadcastResult`] with
    /// explorer URL, cluster name, and current slot.
    pub fn broadcast_with_result(
        &self,
        resp: &SignResponse,
    ) -> Result<BroadcastResult, AirSignError> {
        let signature = self.broadcast_response(resp)?;
        let slot = self.get_slot().ok();
        let explorer_url = self.explorer_url(&signature);
        Ok(BroadcastResult {
            signature,
            slot,
            explorer_url,
            cluster: self.cluster.clone(),
            fee_lamports: None,
        })
    }

    /// Broadcast a raw JSON payload and return a rich [`BroadcastResult`].
    pub fn broadcast_response_json_with_result(
        &self,
        response_json: &[u8],
    ) -> Result<BroadcastResult, AirSignError> {
        let resp = SignResponse::from_json(response_json)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;
        self.broadcast_with_result(&resp)
    }

    // ── Confirmation polling ───────────────────────────────────────────────

    /// Poll for finalized confirmation of a signature, up to `max_polls`
    /// attempts with 1-second intervals.
    ///
    /// Returns the slot at which the transaction was finalized, or an error
    /// if the timeout is exceeded before finalization.
    pub fn wait_for_finalized(&self, sig_str: &str, max_polls: u32) -> Result<u64, AirSignError> {
        let signature = Signature::from_str(sig_str)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        for i in 0..max_polls {
            let statuses = self
                .client
                .get_signature_statuses(&[signature])
                .map_err(|e| AirSignError::Rpc(e.to_string()))?;

            if let Some(Some(status)) = statuses.value.first() {
                // confirmations == None means the transaction is finalized.
                if status.confirmations.is_none() && status.err.is_none() {
                    return Ok(status.slot);
                }
            }

            if i + 1 < max_polls {
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }

        Err(AirSignError::Rpc(format!(
            "timed out waiting for finalized confirmation after {max_polls} polls"
        )))
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Constructor / cluster detection ───────────────────────────────────

    #[test]
    fn cluster_name_devnet() {
        let b = Broadcaster::new("https://api.devnet.solana.com");
        assert_eq!(b.cluster, "devnet");
    }

    #[test]
    fn cluster_name_mainnet() {
        let b = Broadcaster::new("https://api.mainnet-beta.solana.com");
        assert_eq!(b.cluster, "mainnet-beta");
    }

    #[test]
    fn cluster_name_testnet() {
        let b = Broadcaster::new("https://api.testnet.solana.com");
        assert_eq!(b.cluster, "testnet");
    }

    #[test]
    fn cluster_name_custom() {
        let b = Broadcaster::new("http://localhost:8899");
        assert_eq!(b.cluster, "custom");
    }

    #[test]
    fn devnet_constructor_sets_cluster() {
        let b = Broadcaster::devnet();
        assert_eq!(b.cluster, "devnet");
    }

    #[test]
    fn mainnet_constructor_sets_cluster() {
        let b = Broadcaster::mainnet();
        assert_eq!(b.cluster, "mainnet-beta");
    }

    #[test]
    fn testnet_constructor_sets_cluster() {
        let b = Broadcaster::testnet();
        assert_eq!(b.cluster, "testnet");
    }

    // ── Explorer URL ──────────────────────────────────────────────────────

    #[test]
    fn explorer_url_devnet_has_cluster_param() {
        let b = Broadcaster::devnet();
        let url = b.explorer_url("TESTSIG123");
        assert_eq!(
            url,
            "https://explorer.solana.com/tx/TESTSIG123?cluster=devnet"
        );
    }

    #[test]
    fn explorer_url_mainnet_has_no_cluster_param() {
        let b = Broadcaster::mainnet();
        let url = b.explorer_url("TESTSIG123");
        assert_eq!(url, "https://explorer.solana.com/tx/TESTSIG123");
    }

    #[test]
    fn explorer_url_testnet_has_cluster_param() {
        let b = Broadcaster::testnet();
        let url = b.explorer_url("TESTSIG456");
        assert_eq!(
            url,
            "https://explorer.solana.com/tx/TESTSIG456?cluster=testnet"
        );
    }

    #[test]
    fn solscan_url_devnet() {
        let b = Broadcaster::devnet();
        let url = b.solscan_url("TESTSIG123");
        assert_eq!(url, "https://solscan.io/tx/TESTSIG123?cluster=devnet");
    }

    #[test]
    fn solscan_url_mainnet_has_no_param() {
        let b = Broadcaster::mainnet();
        let url = b.solscan_url("TESTSIG123");
        assert_eq!(url, "https://solscan.io/tx/TESTSIG123");
    }

    // ── BroadcastResult ───────────────────────────────────────────────────

    #[test]
    fn broadcast_result_fields() {
        let r = BroadcastResult {
            signature: "abc123".to_owned(),
            slot: Some(42),
            explorer_url: "https://explorer.solana.com/tx/abc123?cluster=devnet".to_owned(),
            cluster: "devnet".to_owned(),
            fee_lamports: Some(5_000),
        };
        assert_eq!(r.signature, "abc123");
        assert_eq!(r.slot, Some(42));
        assert_eq!(r.fee_lamports, Some(5_000));
        assert!(r.explorer_url.contains("abc123"));
        assert_eq!(r.cluster, "devnet");
    }

    #[test]
    fn broadcast_result_optional_fields_can_be_none() {
        let r = BroadcastResult {
            signature: "xyz".to_owned(),
            slot: None,
            explorer_url: "https://explorer.solana.com/tx/xyz?cluster=devnet".to_owned(),
            cluster: "devnet".to_owned(),
            fee_lamports: None,
        };
        assert!(r.slot.is_none());
        assert!(r.fee_lamports.is_none());
    }

    // ── Airdrop safety guard ──────────────────────────────────────────────

    #[test]
    fn airdrop_returns_error_on_mainnet() {
        let b = Broadcaster::mainnet();
        let result = b.airdrop("11111111111111111111111111111111", 1_000_000);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("mainnet"));
    }
}