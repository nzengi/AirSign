//! AirSign — air-gapped Ed25519 transaction signer.
//!
//! `AirSigner` runs on the **air-gapped machine**.  It:
//!
//! 1. Receives a QR stream (via camera or manual PNG import).
//! 2. Decrypts and deserialises the [`SignRequest`].
//! 3. Displays the transaction summary for human review.
//! 4. Signs the transaction message bytes with the loaded keypair.
//! 5. Encrypts and transmits the [`SignResponse`] back as a QR stream.

use base64::{engine::general_purpose::STANDARD, Engine};
use solana_sdk::{signature::Signer, signer::keypair::Keypair};

use afterimage_core::session::{RecvSession, SendSession};

use crate::{
    error::AirSignError,
    request::SignRequest,
    response::SignResponse,
};

/// Air-gapped signer holding a single Ed25519 keypair.
pub struct AirSigner {
    keypair: Keypair,
    /// AfterImage transfer password shared between both machines.
    password: String,
}

impl AirSigner {
    /// Load from a byte slice (bincode-serialised `Keypair`).
    pub fn from_bytes(keypair_bytes: &[u8], password: impl Into<String>) -> Self {
        let keypair = Keypair::try_from(keypair_bytes)
            .expect("invalid keypair bytes");
        Self {
            keypair,
            password: password.into(),
        }
    }

    /// Return the public key of the loaded keypair.
    pub fn pubkey(&self) -> solana_sdk::pubkey::Pubkey {
        self.keypair.pubkey()
    }

    /// Process a raw `SignRequest` payload (decrypted JSON bytes).
    ///
    /// Validates the nonce, signs the transaction, and returns a
    /// [`SignResponse`] ready for transmission.
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
            nonce: req.nonce,
            signer_pubkey: expected_pubkey,
            signature_b64: STANDARD.encode(signature.as_ref()),
            signed_transaction_b64: STANDARD.encode(&signed_tx_bytes),
        };

        Ok(response)
    }

    /// High-level helper: receive a sign request via AfterImage QR stream,
    /// sign it, and return the encrypted response payload ready for sending.
    ///
    /// `ingest_frames` is a closure that feeds raw QR frames into a
    /// [`RecvSession`] until it returns `true` (complete).
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
        let response = self.sign_request(&request_json)?;

        let response_json = response.to_json()?;

        let send = SendSession::new(&response_json, "airsign-response.json", &self.password)?;
        Ok(send)
    }
}

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
        system_instruction,
        signer::keypair::Keypair,
    };

    fn make_transfer_tx(from: &Keypair, to: &Pubkey, lamports: u64) -> solana_sdk::transaction::Transaction {
        let ix = system_instruction::transfer(&from.pubkey(), to, lamports);
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

        // Verify signature
        let sig_bytes = resp.decode_signature().unwrap();
        use solana_sdk::signature::Signature;
        let sig = Signature::from(sig_bytes);
        let signed_tx = resp.decode_transaction().unwrap();
        assert!(signed_tx
            .verify_with_results()
            .iter()
            .all(|&ok| ok));
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
            signer_pubkey: other.pubkey().to_string(), // different from signer
            transaction_b64: STANDARD.encode(&tx_bytes),
            description: "".to_owned(),
            created_at: 0,
            cluster: "".to_owned(),
        };

        let result = signer.sign_request(&req.to_json().unwrap());
        assert!(matches!(result, Err(AirSignError::InvalidRequest(_))));
    }
}