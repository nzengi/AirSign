//! AirSign — air-gapped Ed25519 transaction signer.
//!
//! `AirSigner` runs on the **air-gapped machine**. It:
//!
//! 1. Receives a QR stream (via camera or manual PNG import).
//! 2. Decrypts and deserialises the [`SignRequest`].
//! 3. Checks the nonce against the persistent nonce store to prevent replay.
//! 4. Displays the transaction summary for human review.
//! 5. Signs the transaction message bytes with the loaded keypair.
//! 6. Persists the nonce so it can never be replayed.
//! 7. Encrypts and transmits the [`SignResponse`] back as a QR stream.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use solana_sdk::{
    signature::Signer,
    signer::keypair::Keypair,
};
use afterimage_core::session::{RecvSession, SendSession};

use crate::{
    error::AirSignError,
    inspector::TransactionInspector,
    request::SignRequest,
    response::SignResponse,
};

// ─── Nonce store helpers ──────────────────────────────────────────────────────

/// Returns the default nonce-store path: `~/.airsign/seen_nonces.json`.
pub fn default_nonce_store_path() -> Option<PathBuf> {
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".airsign").join("seen_nonces.json"))
}

fn load_seen_nonces(path: &Path) -> HashSet<String> {
    let Ok(data) = std::fs::read(path) else {
        return HashSet::new();
    };
    serde_json::from_slice::<Vec<String>>(&data)
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn persist_nonce(path: &Path, nonce: &str) -> Result<(), AirSignError> {
    // Ensure directory exists
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| AirSignError::Io(e.to_string()))?;
    }

    let mut seen = load_seen_nonces(path);
    seen.insert(nonce.to_owned());

    let list: Vec<&String> = seen.iter().collect();
    let json = serde_json::to_vec(&list).map_err(AirSignError::Json)?;
    std::fs::write(path, &json).map_err(|e| AirSignError::Io(e.to_string()))?;
    Ok(())
}

// ─── Transaction summary ──────────────────────────────────────────────────────

/// Parse a `SignRequest` and return a human-readable summary string.
///
/// Uses [`TransactionInspector`] to decode all known instruction types and
/// generate risk flags.  Shown on the air-gapped machine before the user
/// approves signing.
pub fn summarize_request(req: &SignRequest) -> String {
    let mut out = String::new();

    // Header
    out.push_str("┌─ AirSign — Transaction Review ──────────────────────────────┐\n");
    out.push_str(&format!(
        "│  Description : {}\n",
        truncate(&req.description, 50)
    ));
    out.push_str(&format!(
        "│  Cluster     : {}\n",
        if req.cluster.is_empty() { "unknown" } else { &req.cluster }
    ));
    out.push_str(&format!("│  Nonce       : {}\n", truncate(&req.nonce, 16)));
    out.push_str(&format!(
        "│  Signer      : {}\n",
        truncate(&req.signer_pubkey, 44)
    ));
    out.push_str("└──────────────────────────────────────────────────────────────┘\n");

    // Inspector analysis
    match req.decode_transaction() {
        Ok(tx) => {
            let summary = TransactionInspector::inspect_tx(&tx);
            out.push_str(&summary.render());

            // Extra warning when HIGH risk detected
            if summary.has_high_risk() {
                out.push_str(
                    "\n⛔ HIGH RISK — review the flags above very carefully.\n"
                );
            }
        }
        Err(e) => {
            out.push_str(&format!("⚠  Could not decode transaction: {e}\n"));
        }
    }

    out
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_owned()
    } else {
        format!("{}…", &s[..max.saturating_sub(1)])
    }
}

// ─── AirSigner ────────────────────────────────────────────────────────────────

/// Air-gapped signer holding a single Ed25519 keypair.
pub struct AirSigner {
    keypair: Keypair,
    /// AfterImage transfer password shared between both machines.
    password: String,
    /// Optional path to the persistent nonce store.
    /// When set, seen nonces are saved to disk and replays are rejected.
    nonce_store: Option<PathBuf>,
}

impl AirSigner {
    /// Load from a byte slice (bincode-serialised `Keypair`).
    pub fn from_bytes(keypair_bytes: &[u8], password: impl Into<String>) -> Self {
        let keypair = Keypair::try_from(keypair_bytes).expect("invalid keypair bytes");
        Self {
            keypair,
            password: password.into(),
            nonce_store: None,
        }
    }

    /// Enable persistent nonce tracking at the given path.
    ///
    /// Once enabled, every successfully signed nonce is written to the file.
    /// Any attempt to re-use a nonce returns [`AirSignError::ReplayDetected`].
    #[must_use]
    pub fn with_nonce_store(mut self, path: impl Into<PathBuf>) -> Self {
        self.nonce_store = Some(path.into());
        self
    }

    /// Enable nonce tracking at the default path (`~/.airsign/seen_nonces.json`).
    ///
    /// Returns `self` unchanged if the home directory cannot be determined.
    #[must_use]
    pub fn with_default_nonce_store(mut self) -> Self {
        self.nonce_store = default_nonce_store_path();
        self
    }

    /// Return the public key of the loaded keypair.
    pub fn pubkey(&self) -> solana_sdk::pubkey::Pubkey {
        self.keypair.pubkey()
    }

    /// Process a raw `SignRequest` payload (decrypted JSON bytes).
    ///
    /// - Validates that the request targets this signer's public key.
    /// - Checks the nonce against the persistent store (if enabled).
    /// - Signs the transaction and persists the nonce.
    ///
    /// Returns a [`SignResponse`] ready for transmission.
    /// Does **not** prompt the user — call [`AirSigner::sign_request_confirmed`]
    /// for interactive use.
    pub fn sign_request(&self, request_json: &[u8]) -> Result<SignResponse, AirSignError> {
        let req = SignRequest::from_json(request_json)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        // Verify this request targets our public key
        let expected_pubkey = self.keypair.pubkey().to_string();
        if req.signer_pubkey != expected_pubkey {
            return Err(AirSignError::InvalidRequest(format!(
                "request targets {}, but signer is {}",
                req.signer_pubkey, expected_pubkey
            )));
        }

        // Replay-attack check
        if let Some(ref store) = self.nonce_store {
            let seen = load_seen_nonces(store);
            if seen.contains(&req.nonce) {
                return Err(AirSignError::ReplayDetected(req.nonce.clone()));
            }
        }

        // Decode the unsigned transaction
        let tx_raw = STANDARD
            .decode(&req.transaction_b64)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;
        let tx: solana_sdk::transaction::Transaction = bincode::deserialize(&tx_raw)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        // Sign the transaction message
        let message_bytes = tx.message_data();
        let signature = self.keypair.sign_message(&message_bytes);

        // Build a fully-signed transaction
        let mut signed_tx = tx;
        if let Some(pos) = signed_tx
            .message
            .account_keys
            .iter()
            .position(|k| k == &self.keypair.pubkey())
        {
            signed_tx.signatures[pos] = signature;
        } else {
            return Err(AirSignError::InvalidRequest(
                "signer pubkey not found in transaction account keys".into(),
            ));
        }

        // Serialise signed transaction
        let signed_tx_bytes = bincode::serialize(&signed_tx)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        let response = SignResponse {
            version: 1,
            nonce: req.nonce.clone(),
            signer_pubkey: expected_pubkey,
            signature_b64: STANDARD.encode(signature.as_ref()),
            signed_transaction_b64: STANDARD.encode(&signed_tx_bytes),
        };

        // Persist nonce AFTER successful signing
        if let Some(ref store) = self.nonce_store {
            persist_nonce(store, &req.nonce)?;
        }

        Ok(response)
    }

    /// Interactive variant: prints the transaction summary to stderr,
    /// prompts the user to confirm, then calls [`sign_request`].
    ///
    /// Returns [`AirSignError::UserAborted`] if the user types anything
    /// other than `yes` / `y`.
    pub fn sign_request_confirmed(&self, request_json: &[u8]) -> Result<SignResponse, AirSignError> {
        let req = SignRequest::from_json(request_json)
            .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

        // Display the summary
        let summary = summarize_request(&req);
        eprintln!("{summary}");
        eprint!("\nType 'yes' to sign, anything else to abort: ");

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| AirSignError::Io(e.to_string()))?;

        let answer = input.trim().to_lowercase();
        if answer != "yes" && answer != "y" {
            return Err(AirSignError::UserAborted);
        }

        // Re-serialise and call sign_request (to reuse all validation)
        let json = req.to_json()?;
        self.sign_request(&json)
    }

    /// High-level helper: receive a sign request via AfterImage QR stream,
    /// show the summary, prompt for confirmation, sign, and return the
    /// encrypted response payload ready for sending.
    pub fn receive_sign_and_encode<F>(
        &self,
        ingest_frames: F,
    ) -> Result<SendSession, AirSignError>
    where
        F: FnOnce(&mut RecvSession) -> Result<(), AirSignError>,
    {
        let mut recv = RecvSession::new(&self.password);
        ingest_frames(&mut recv)?;

        let request_json = recv.get_data()?;
        let response = self.sign_request_confirmed(&request_json)?;

        let response_json = response.to_json()?;
        let send = SendSession::new(&response_json, "airsign-response.json", &self.password)?;
        Ok(send)
    }
}

// ─── Online-machine helper ────────────────────────────────────────────────────

/// Online-machine helper: build a `SignRequest` and wrap it in a `SendSession`.
pub fn build_send_session(
    tx: &solana_sdk::transaction::Transaction,
    signer_pubkey: &solana_sdk::pubkey::Pubkey,
    description: &str,
    cluster: &str,
    password: &str,
) -> Result<SendSession, AirSignError> {
    use rand::RngCore;

    // Generate random nonce
    let mut nonce_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);

    let tx_bytes = bincode::serialize(tx)
        .map_err(|e| AirSignError::InvalidRequest(e.to_string()))?;

    let request = SignRequest {
        version: 1,
        nonce,
        signer_pubkey: signer_pubkey.to_string(),
        transaction_b64: STANDARD.encode(&tx_bytes),
        description: description.to_owned(),
        created_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        cluster: cluster.to_owned(),
    };

    let json = request.to_json()?;
    let send = SendSession::new(&json, "airsign-request.json", password)?;
    Ok(send)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        message::Message,
        pubkey::Pubkey,
        signer::keypair::Keypair,
    };
    use solana_system_interface::instruction as system_ix;

    fn make_transfer_tx(
        from: &Keypair,
        to: &Pubkey,
        lamports: u64,
    ) -> solana_sdk::transaction::Transaction {
        let ix = system_ix::transfer(&from.pubkey(), to, lamports);
        let msg = Message::new(&[ix], Some(&from.pubkey()));
        solana_sdk::transaction::Transaction::new_unsigned(msg)
    }

    #[test]
    fn sign_request_roundtrip() {
        let keypair = Keypair::new();
        let signer = AirSigner::from_bytes(&keypair.to_bytes(), "test-password");

        let recipient = Pubkey::new_unique();
        let tx = make_transfer_tx(&keypair, &recipient, 1_000_000);

        let tx_bytes = bincode::serialize(&tx).unwrap();
        let req = SignRequest {
            version: 1,
            nonce: "deadbeef".to_owned(),
            signer_pubkey: keypair.pubkey().to_string(),
            transaction_b64: STANDARD.encode(&tx_bytes),
            description: "test transfer".to_owned(),
            created_at: 0,
            cluster: "devnet".to_owned(),
        };

        let req_json = req.to_json().unwrap();
        let resp = signer.sign_request(&req_json).unwrap();

        assert_eq!(resp.nonce, "deadbeef");
        assert_eq!(resp.signer_pubkey, keypair.pubkey().to_string());

        let signed_tx = resp.decode_transaction().unwrap();
        assert!(signed_tx.verify_with_results().iter().all(|&ok| ok));
    }

    #[test]
    fn wrong_pubkey_rejected() {
        let keypair = Keypair::new();
        let other = Keypair::new();
        let signer = AirSigner::from_bytes(&keypair.to_bytes(), "test-password");

        let recipient = Pubkey::new_unique();
        let tx = make_transfer_tx(&other, &recipient, 1_000);
        let tx_bytes = bincode::serialize(&tx).unwrap();

        let req = SignRequest {
            version: 1,
            nonce: "aabb".to_owned(),
            signer_pubkey: other.pubkey().to_string(),
            transaction_b64: STANDARD.encode(&tx_bytes),
            description: "".to_owned(),
            created_at: 0,
            cluster: "".to_owned(),
        };

        let result = signer.sign_request(&req.to_json().unwrap());
        assert!(matches!(result, Err(AirSignError::InvalidRequest(_))));
    }

    #[test]
    fn replay_attack_detected() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let store_path = tmp.path().to_owned();
        // Remove file so the nonce store starts empty
        let _ = std::fs::remove_file(&store_path);

        let keypair = Keypair::new();
        let signer = AirSigner::from_bytes(&keypair.to_bytes(), "pw")
            .with_nonce_store(&store_path);

        let recipient = Pubkey::new_unique();
        let tx = make_transfer_tx(&keypair, &recipient, 500_000);
        let tx_bytes = bincode::serialize(&tx).unwrap();

        let req = SignRequest {
            version: 1,
            nonce: "unique-nonce-xyz".to_owned(),
            signer_pubkey: keypair.pubkey().to_string(),
            transaction_b64: STANDARD.encode(&tx_bytes),
            description: "replay test".to_owned(),
            created_at: 0,
            cluster: "devnet".to_owned(),
        };
        let json = req.to_json().unwrap();

        // First sign — should succeed
        signer.sign_request(&json).expect("first sign should succeed");

        // Second sign with same nonce — must fail
        let result = signer.sign_request(&json);
        assert!(
            matches!(result, Err(AirSignError::ReplayDetected(_))),
            "expected ReplayDetected, got: {result:?}"
        );
    }

    #[test]
    fn tx_summary_contains_transfer_info() {
        let keypair = Keypair::new();
        let recipient = Pubkey::new_unique();
        let tx = make_transfer_tx(&keypair, &recipient, 1_500_000_000); // 1.5 SOL
        let tx_bytes = bincode::serialize(&tx).unwrap();

        let req = SignRequest {
            version: 1,
            nonce: "abc".to_owned(),
            signer_pubkey: keypair.pubkey().to_string(),
            transaction_b64: STANDARD.encode(&tx_bytes),
            description: "pay rent".to_owned(),
            created_at: 0,
            cluster: "mainnet".to_owned(),
        };

        let summary = summarize_request(&req);
        assert!(summary.contains("System :: Transfer"), "summary: {summary}");
        assert!(summary.contains("1.500000000"), "summary: {summary}");
        assert!(summary.contains("pay rent"), "summary: {summary}");
    }
}