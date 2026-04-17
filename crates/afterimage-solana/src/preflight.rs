//! Pre-flight checker — RPC-based transaction simulation and fee estimation.
//!
//! [`PreflightChecker`] uses the Solana JSON-RPC `simulateTransaction` and
//! `getFeeForMessage` endpoints to validate a transaction **before** the user
//! approves it on the air-gapped machine.
//!
//! ## Usage
//!
//! ```no_run
//! use afterimage_solana::preflight::PreflightChecker;
//!
//! let checker = PreflightChecker::new("https://api.devnet.solana.com");
//! let tx_bytes = std::fs::read("unsigned_tx.bin").unwrap();
//! match checker.check(&tx_bytes) {
//!     Ok(result) => {
//!         println!("Fee: {} lamports", result.fee_lamports.unwrap_or(0));
//!         println!("Simulation: {}", if result.success { "OK" } else { "FAILED" });
//!     }
//!     Err(e) => eprintln!("Pre-flight check failed: {e}"),
//! }
//! ```

use solana_client::rpc_client::RpcClient;
use solana_sdk::transaction::Transaction;

// ─── PreflightResult ─────────────────────────────────────────────────────────

/// Result of a pre-flight check against a Solana cluster.
#[derive(Debug, Clone)]
pub struct PreflightResult {
    /// Estimated transaction fee in lamports, if the RPC call succeeded.
    pub fee_lamports: Option<u64>,

    /// Whether `simulateTransaction` returned a successful result.
    pub success: bool,

    /// RPC error message or simulation error, if any.
    pub error: Option<String>,

    /// Log messages returned by the simulation.
    pub logs: Vec<String>,

    /// The cluster URL used for the check.
    pub cluster_url: String,
}

impl PreflightResult {
    /// Render a human-readable summary suitable for the terminal.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("─────────────────────────────────────────────────────────────\n");
        out.push_str(" Pre-flight check (RPC simulation)\n");
        out.push_str(&format!(" Cluster  : {}\n", self.cluster_url));
        match self.fee_lamports {
            Some(fee) => {
                let sol = fee as f64 / 1e9;
                out.push_str(&format!(" Fee      : {} lamports ({:.9} SOL)\n", fee, sol));
            }
            None => {
                out.push_str(" Fee      : (unavailable)\n");
            }
        }
        if self.success {
            out.push_str(" Simulate : ✓ would succeed\n");
        } else {
            out.push_str(" Simulate : ✗ would FAIL\n");
            if let Some(ref err) = self.error {
                out.push_str(&format!("   Error  : {err}\n"));
            }
        }
        if !self.logs.is_empty() {
            out.push_str(" Logs:\n");
            for log in self.logs.iter().take(10) {
                out.push_str(&format!("   {log}\n"));
            }
            if self.logs.len() > 10 {
                out.push_str(&format!("   … ({} more)\n", self.logs.len() - 10));
            }
        }
        out
    }
}

// ─── PreflightChecker ────────────────────────────────────────────────────────

/// Runs RPC-based pre-flight checks against a Solana cluster.
pub struct PreflightChecker {
    /// RPC URL (e.g. `"https://api.devnet.solana.com"`).
    pub cluster_url: String,
}

impl PreflightChecker {
    /// Create a new checker for the given RPC URL.
    pub fn new(cluster_url: &str) -> Self {
        Self {
            cluster_url: cluster_url.to_owned(),
        }
    }

    /// Run simulation + fee estimation against `tx_bytes` (bincode Transaction).
    ///
    /// Returns `Ok(PreflightResult)` even when the simulation fails — the caller
    /// should inspect `result.success`.  Returns `Err` only if the transaction
    /// bytes cannot be deserialised.
    pub fn check(&self, tx_bytes: &[u8]) -> Result<PreflightResult, String> {
        let tx: Transaction = bincode::deserialize(tx_bytes)
            .map_err(|e| format!("failed to deserialise transaction: {e}"))?;
        Ok(self.check_tx(&tx))
    }

    /// Run simulation + fee estimation for a pre-deserialised [`Transaction`].
    pub fn check_tx(&self, tx: &Transaction) -> PreflightResult {
        let client = RpcClient::new(self.cluster_url.clone());

        // ── Fee estimation ──────────────────────────────────────────────────
        let fee_lamports = client
            .get_fee_for_message(&tx.message)
            .ok();

        // ── Simulation ──────────────────────────────────────────────────────
        use solana_client::rpc_config::RpcSimulateTransactionConfig;
        let sim_config = RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: None,
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: false,
        };

        match client.simulate_transaction_with_config(tx, sim_config) {
            Ok(response) => {
                let value = response.value;
                let success = value.err.is_none();
                let error = value.err.as_ref().map(|e| format!("{e:?}"));
                let logs = value.logs.unwrap_or_default();
                PreflightResult {
                    fee_lamports,
                    success,
                    error,
                    logs,
                    cluster_url: self.cluster_url.clone(),
                }
            }
            Err(e) => PreflightResult {
                fee_lamports,
                success: false,
                error: Some(format!("RPC error: {e}")),
                logs: Vec::new(),
                cluster_url: self.cluster_url.clone(),
            },
        }
    }
}

// ─── Cluster URL helpers ──────────────────────────────────────────────────────

/// Resolve a cluster shorthand to an RPC URL.
///
/// Accepts `"devnet"`, `"testnet"`, `"mainnet"` (or `"mainnet-beta"`), or a
/// full URL starting with `"http"`.
pub fn resolve_cluster_url(cluster: &str) -> String {
    match cluster {
        "devnet"                    => "https://api.devnet.solana.com".to_owned(),
        "testnet"                   => "https://api.testnet.solana.com".to_owned(),
        "mainnet" | "mainnet-beta"  => "https://api.mainnet-beta.solana.com".to_owned(),
        other if other.starts_with("http") => other.to_owned(),
        other => format!("https://api.{other}.solana.com"),
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_devnet() {
        assert_eq!(
            resolve_cluster_url("devnet"),
            "https://api.devnet.solana.com"
        );
    }

    #[test]
    fn resolve_mainnet() {
        assert_eq!(
            resolve_cluster_url("mainnet"),
            "https://api.mainnet-beta.solana.com"
        );
        assert_eq!(
            resolve_cluster_url("mainnet-beta"),
            "https://api.mainnet-beta.solana.com"
        );
    }

    #[test]
    fn resolve_custom_url() {
        let url = "https://my-custom-rpc.example.com";
        assert_eq!(resolve_cluster_url(url), url);
    }

    #[test]
    fn preflight_result_render_success() {
        let r = PreflightResult {
            fee_lamports: Some(5000),
            success: true,
            error: None,
            logs: vec!["Program log: hello".into()],
            cluster_url: "https://api.devnet.solana.com".into(),
        };
        let rendered = r.render();
        assert!(rendered.contains("5000 lamports"));
        assert!(rendered.contains("✓ would succeed"));
        assert!(rendered.contains("Program log: hello"));
    }

    #[test]
    fn preflight_result_render_failure() {
        let r = PreflightResult {
            fee_lamports: None,
            success: false,
            error: Some("InsufficientFunds".into()),
            logs: Vec::new(),
            cluster_url: "https://api.devnet.solana.com".into(),
        };
        let rendered = r.render();
        assert!(rendered.contains("would FAIL"));
        assert!(rendered.contains("InsufficientFunds"));
    }

    #[test]
    fn deserialise_error_on_invalid_bytes() {
        let checker = PreflightChecker::new("https://api.devnet.solana.com");
        let result = checker.check(b"garbage bytes");
        assert!(result.is_err());
    }
}