//! # afterimage-wasm
//!
//! WebAssembly bindings that expose the AfterImage encode/decode pipeline
//! to browser JavaScript / TypeScript.
//!
//! ## Building
//! ```bash
//! wasm-pack build crates/afterimage-wasm --target bundler --release
//! # output → crates/afterimage-wasm/pkg/
//! ```
//!
//! ## JS API
//! ```js
//! import init, { WasmSendSession, WasmRecvSession } from './afterimage_wasm.js';
//! await init();
//!
//! // Sender side
//! const tx = new WasmSendSession(data_u8array, "filename.bin", "password");
//! const total = tx.total_frames();
//! while (tx.has_next()) {
//!   const frame = tx.next_frame();   // Uint8Array
//!   const pct   = tx.progress();     // 0.0 – 1.0
//!   displayAsQr(frame);
//!   await sleep(150);
//! }
//!
//! // Receiver side
//! const rx = new WasmRecvSession("password");
//! rx.ingest_frame(qr_payload);       // call for each decoded QR frame
//! console.log(rx.progress());        // 0.0 – 1.0
//! if (rx.is_complete()) {
//!   const data     = rx.get_data();  // Uint8Array
//!   const filename = rx.filename();  // string | undefined
//! }
//! ```

use wasm_bindgen::prelude::*;
use afterimage_core::session::{RecvSession, SendSession};

// Initialisation hook called by the JS glue code on WASM startup.
#[wasm_bindgen(start)]
pub fn start() {
    // Forward Rust panics to the browser console for easier debugging.
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
}

// ─── WasmSendSession ─────────────────────────────────────────────────────────

/// Encrypts and fountain-encodes data for transmission as QR frames.
///
/// Call [`next_frame`](Self::next_frame) in a loop and display each returned
/// `Uint8Array` as a QR code.  Use [`progress`](Self::progress) to drive a
/// progress bar or to decide when enough frames have been sent.
#[wasm_bindgen]
pub struct WasmSendSession {
    inner: SendSession,
    /// Running count of frames emitted (for progress calculation).
    emitted: u32,
    /// Cached recommended total (computed once at construction).
    recommended: u32,
}

#[wasm_bindgen]
impl WasmSendSession {
    /// Create a new send session.
    ///
    /// - `data`     — raw plaintext bytes to transmit
    /// - `filename` — cleartext filename embedded in the metadata frame
    /// - `password` — shared Argon2id encryption password
    ///
    /// # Errors
    /// Throws a JS exception if key derivation or encryption fails.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8], filename: &str, password: &str) -> Result<WasmSendSession, JsValue> {
        let inner = SendSession::new(data, filename, password)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let recommended = afterimage_core::session::recommended_frames(data.len()) as u32;
        Ok(Self {
            inner,
            emitted: 0,
            recommended,
        })
    }

    // ─── Frame iteration ──────────────────────────────────────────────────────

    /// Returns `true` if there are more frames to send.
    ///
    /// When no [`set_limit`](Self::set_limit) has been set this always returns
    /// `true` — use [`progress`](Self::progress) or [`total_frames`](Self::total_frames)
    /// to decide when to stop.
    pub fn has_next(&self) -> bool {
        self.inner.has_next()
    }

    /// Returns the next encoded frame as a `Uint8Array`, or `null` if done.
    ///
    /// Each returned slice should be encoded as a QR code and displayed for
    /// the configured frame interval.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        let frame = self.inner.next_frame();
        if frame.is_some() {
            self.emitted += 1;
        }
        frame
    }

    // ─── Progress ─────────────────────────────────────────────────────────────

    /// Fraction of recommended frames emitted (0.0 – 1.0).
    ///
    /// Values above 1.0 are possible when sending beyond the recommended count
    /// for extra redundancy.
    pub fn progress(&self) -> f64 {
        if self.recommended == 0 {
            return 1.0;
        }
        self.emitted as f64 / self.recommended as f64
    }

    /// Number of frames emitted so far.
    pub fn frame_index(&self) -> u32 {
        self.emitted
    }

    /// Recommended total frames to transmit (source blocks + redundancy + metadata).
    ///
    /// This is a *suggestion* — transmitting more frames increases reliability
    /// on noisy optical channels.
    pub fn total_frames(&self) -> u32 {
        self.recommended
    }

    // ─── Source-block statistics ──────────────────────────────────────────────

    /// Total number of source droplets (before redundancy).
    pub fn droplet_count(&self) -> usize {
        self.inner.droplet_count()
    }

    /// Recommended total droplets to transmit (source + redundancy, no metadata).
    pub fn recommended_droplet_count(&self) -> usize {
        self.inner.recommended_droplet_count()
    }

    /// Set a hard upper limit on frames generated.
    ///
    /// After `limit` frames [`has_next`](Self::has_next) returns `false` and
    /// [`next_frame`](Self::next_frame) returns `null`.  Set to `0` to remove
    /// any previously set limit.
    pub fn set_limit(&mut self, limit: u32) {
        self.inner.set_limit(limit);
    }
}

// ─── WasmRecvSession ─────────────────────────────────────────────────────────

/// Decodes and decrypts received QR frame payloads.
///
/// Feed each QR-decoded `Uint8Array` into [`ingest_frame`](Self::ingest_frame).
/// Once [`is_complete`](Self::is_complete) returns `true`, call
/// [`get_data`](Self::get_data) to retrieve the plaintext.
#[wasm_bindgen]
pub struct WasmRecvSession {
    inner: RecvSession,
}

#[wasm_bindgen]
impl WasmRecvSession {
    /// Create a new receive session with the shared password.
    #[wasm_bindgen(constructor)]
    pub fn new(password: &str) -> WasmRecvSession {
        Self {
            inner: RecvSession::new(password),
        }
    }

    // ─── Ingestion ────────────────────────────────────────────────────────────

    /// Feed a raw QR-decoded payload into the session.
    ///
    /// Returns `true` if the transfer is complete after this frame.
    /// Silently ignores frames that cannot be parsed (corrupted / duplicate).
    pub fn ingest_frame(&mut self, frame: &[u8]) -> bool {
        self.inner.ingest_frame(frame).unwrap_or(false)
    }

    // ─── State queries ────────────────────────────────────────────────────────

    /// Returns `true` if all data has been received and decrypted successfully.
    pub fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    /// Fraction of required droplets received (0.0 – 1.0).
    pub fn progress(&self) -> f64 {
        self.inner.progress()
    }

    /// Number of frames received so far.
    pub fn received_count(&self) -> u64 {
        self.inner.frames_received()
    }

    // ─── Metadata accessors ───────────────────────────────────────────────────

    /// Returns the filename embedded in the metadata frame, or `undefined` if
    /// no metadata frame has been received yet.
    pub fn filename(&self) -> Option<String> {
        self.inner
            .metadata()
            .map(|m| m.filename.clone())
            .filter(|s| !s.is_empty())
    }

    /// Returns the original plaintext size in bytes, or `undefined` if no
    /// metadata frame has been received yet.
    pub fn original_size(&self) -> Option<u32> {
        self.inner.metadata().map(|m| m.original_len)
    }

    /// Returns the protocol version byte from the metadata frame (`1`, `2`, or
    /// `3`), or `undefined` if no metadata frame has been received yet.
    pub fn protocol_version(&self) -> Option<u8> {
        self.inner.metadata().map(|m| m.version)
    }

    // ─── Data retrieval ───────────────────────────────────────────────────────

    /// Retrieve the decrypted plaintext data.
    ///
    /// Returns an empty `Uint8Array` if the session is not yet complete.
    ///
    /// # Errors
    /// Throws a JS exception if decryption fails (wrong password / tampered data).
    pub fn get_data(&mut self) -> Result<Vec<u8>, JsValue> {
        if !self.inner.is_complete() {
            return Ok(Vec::new());
        }
        self.inner
            .get_data()
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ─── Utility exports ─────────────────────────────────────────────────────────

/// Returns the AfterImage library version string.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

/// Compute the recommended frame count for a payload of `size` bytes.
#[wasm_bindgen]
pub fn recommended_frames(size: usize) -> usize {
    afterimage_core::session::recommended_frames(size)
}

/// Encode a byte slice as a standard Base-64 string (RFC 4648, no padding stripped).
///
/// Useful for passing `Uint8Array` data through JSON / localStorage.
#[wasm_bindgen]
pub fn encode_base64(data: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(data)
}

/// Decode a Base-64 string back to bytes.
///
/// # Errors
/// Throws a JS exception if the input is not valid Base-64.
#[wasm_bindgen]
pub fn decode_base64(s: &str) -> Result<Vec<u8>, JsValue> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| JsValue::from_str(&format!("base64 decode error: {}", e)))
}

/// Hex-encode a byte slice (lowercase).
#[wasm_bindgen]
pub fn encode_hex(data: &[u8]) -> String {
    hex::encode(data)
}

/// Decode a hex string to bytes.
///
/// # Errors
/// Throws a JS exception if the input is not valid hex.
#[wasm_bindgen]
pub fn decode_hex(s: &str) -> Result<Vec<u8>, JsValue> {
    hex::decode(s).map_err(|e| JsValue::from_str(&format!("hex decode error: {}", e)))
}

// ─── WasmKeypair ─────────────────────────────────────────────────────────────

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

/// An Ed25519 keypair usable from JavaScript/TypeScript.
///
/// ```js
/// const kp = WasmKeypair.generate();
/// const sig = kp.sign(msgBytes);             // Uint8Array(64)
/// WasmKeypair.verify(kp.pubkey(), msgBytes, sig); // true
/// ```
#[wasm_bindgen]
pub struct WasmKeypair {
    inner: SigningKey,
}

#[wasm_bindgen]
impl WasmKeypair {
    /// Generate a fresh random Ed25519 keypair using the OS CSPRNG.
    ///
    /// # Errors
    /// Throws if the platform entropy source is unavailable.
    pub fn generate() -> Result<WasmKeypair, JsValue> {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed)
            .map_err(|e| JsValue::from_str(&format!("getrandom error: {}", e)))?;
        Ok(Self {
            inner: SigningKey::from_bytes(&seed),
        })
    }

    /// Derive a keypair deterministically from a 32-byte seed.
    ///
    /// # Errors
    /// Throws if `seed` is not exactly 32 bytes.
    pub fn from_seed(seed: &[u8]) -> Result<WasmKeypair, JsValue> {
        let arr: [u8; 32] = seed
            .try_into()
            .map_err(|_| JsValue::from_str("seed must be exactly 32 bytes"))?;
        Ok(Self {
            inner: SigningKey::from_bytes(&arr),
        })
    }

    /// Returns the 32-byte compressed public key.
    pub fn pubkey(&self) -> Vec<u8> {
        self.inner.verifying_key().to_bytes().to_vec()
    }

    /// Returns the public key as a Base58-encoded string (Solana-compatible).
    pub fn pubkey_b58(&self) -> String {
        bs58_encode(self.inner.verifying_key().as_bytes())
    }

    /// Returns the 64-byte raw secret seed || public key (Solana keypair format).
    ///
    /// Handle with care — this exports the private key material.
    pub fn secret_bytes(&self) -> Vec<u8> {
        self.inner.to_keypair_bytes().to_vec()
    }

    /// Sign `message` with Ed25519.  Returns a 64-byte signature.
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        self.inner.sign(message).to_bytes().to_vec()
    }

    /// Verify an Ed25519 `signature` over `message` using `pubkey` (32 bytes).
    ///
    /// Returns `true` if valid, `false` otherwise (never throws).
    pub fn verify(pubkey: &[u8], message: &[u8], signature: &[u8]) -> bool {
        let Ok(vk_arr): Result<[u8; 32], _> = pubkey.try_into() else {
            return false;
        };
        let Ok(sig_arr): Result<[u8; 64], _> = signature.try_into() else {
            return false;
        };
        let Ok(vk) = VerifyingKey::from_bytes(&vk_arr) else {
            return false;
        };
        let sig = Signature::from_bytes(&sig_arr);
        vk.verify(message, &sig).is_ok()
    }
}

// ─── WasmMultiSignOrchestrator ───────────────────────────────────────────────

use serde::{Deserialize, Serialize};

/// A single partial Ed25519 signature from one air-gapped signer.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PartialSig {
    signer_pubkey: String,
    signature_b64: String,
}

/// The JSON envelope sent to each air-gapped signer (schema v2).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct MultiSignRequest {
    version: u8,
    nonce: String,
    threshold: u8,
    signers: Vec<String>,
    round: u8,
    partial_sigs: Vec<PartialSig>,
    transaction_b64: String,
    description: String,
    created_at: i64,
    cluster: String,
}

/// The JSON envelope returned by an air-gapped signer (schema v2).
#[derive(Debug, Serialize, Deserialize)]
struct MultiSignResponse {
    version: u8,
    nonce: String,
    round: u8,
    signer_pubkey: String,
    signature_b64: String,
}

/// Orchestrates an M-of-N multi-signature session on the **online** machine.
///
/// ```js
/// const orch = WasmMultiSignOrchestrator.new(
///   txB64, ["pubkeyA","pubkeyB","pubkeyC"], 2, "devnet", "Treasury tx"
/// );
/// // Round 1 → send orch.get_request_json() as QR to signer A
/// // Receive response JSON from signer A
/// const done = orch.ingest_response_json(responseJson); // false
/// // Round 2 → send orch.get_request_json() as QR to signer B
/// const done2 = orch.ingest_response_json(responseJson2); // true (threshold=2)
/// const partialSigsJson = orch.get_partial_sigs_json();
/// ```
#[wasm_bindgen]
pub struct WasmMultiSignOrchestrator {
    nonce: String,
    threshold: u8,
    signers: Vec<String>,
    round: u8,
    partial_sigs: Vec<PartialSig>,
    transaction_b64: String,
    description: String,
    created_at: i64,
    cluster: String,
}

#[wasm_bindgen]
impl WasmMultiSignOrchestrator {
    /// Create a new orchestration session.
    ///
    /// - `transaction_b64` — Base64-encoded unsigned Solana transaction bytes
    /// - `signers`         — JS Array of N Base58 public key strings
    /// - `threshold`       — M (minimum signatures required)
    /// - `cluster`         — `"mainnet-beta"`, `"devnet"`, `"testnet"`, or `"localnet"`
    /// - `description`     — Human-readable description shown to each signer
    ///
    /// # Errors
    /// Throws if `threshold > signers.length` or entropy is unavailable.
    #[wasm_bindgen(constructor)]
    pub fn new(
        transaction_b64: &str,
        signers: js_sys::Array,
        threshold: u8,
        cluster: &str,
        description: &str,
    ) -> Result<WasmMultiSignOrchestrator, JsValue> {
        let signers: Vec<String> = signers
            .iter()
            .map(|v| {
                v.as_string()
                    .ok_or_else(|| JsValue::from_str("signers must be strings"))
            })
            .collect::<Result<_, _>>()?;

        if threshold as usize > signers.len() {
            return Err(JsValue::from_str("threshold cannot exceed number of signers"));
        }
        if signers.is_empty() {
            return Err(JsValue::from_str("at least one signer required"));
        }

        // 16-byte random nonce → hex string
        let mut nonce_bytes = [0u8; 16];
        getrandom::fill(&mut nonce_bytes)
            .map_err(|e| JsValue::from_str(&format!("getrandom error: {}", e)))?;
        let nonce = hex::encode(nonce_bytes);

        // Millisecond timestamp → seconds
        let created_at = (js_sys::Date::now() / 1000.0) as i64;

        Ok(Self {
            nonce,
            threshold,
            signers,
            round: 1,
            partial_sigs: Vec::new(),
            transaction_b64: transaction_b64.to_owned(),
            description: description.to_owned(),
            created_at,
            cluster: cluster.to_owned(),
        })
    }

    // ─── State queries ────────────────────────────────────────────────────────

    /// Returns the current round number (1-based).
    pub fn current_round(&self) -> u8 {
        self.round
    }

    /// Returns the total number of signers.
    pub fn signer_count(&self) -> usize {
        self.signers.len()
    }

    /// Returns the required threshold M.
    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    /// Returns `true` when enough partial signatures have been collected.
    pub fn threshold_met(&self) -> bool {
        self.partial_sigs.len() >= self.threshold as usize
    }

    /// Fraction of required signatures collected (0.0 – 1.0).
    pub fn progress(&self) -> f64 {
        if self.threshold == 0 {
            return 1.0;
        }
        self.partial_sigs.len() as f64 / self.threshold as f64
    }

    /// Base58 pubkey expected to sign in the current round, or `undefined`.
    pub fn current_signer_pubkey(&self) -> Option<String> {
        let idx = self.round.checked_sub(1)? as usize;
        self.signers.get(idx).cloned()
    }

    // ─── Round management ─────────────────────────────────────────────────────

    /// Serialise the current-round request envelope to JSON.
    ///
    /// Display this JSON as a QR stream to the next air-gapped signer.
    ///
    /// # Errors
    /// Throws if JSON serialisation fails (should never happen).
    pub fn get_request_json(&self) -> Result<String, JsValue> {
        let req = MultiSignRequest {
            version: 2,
            nonce: self.nonce.clone(),
            threshold: self.threshold,
            signers: self.signers.clone(),
            round: self.round,
            partial_sigs: self.partial_sigs.clone(),
            transaction_b64: self.transaction_b64.clone(),
            description: self.description.clone(),
            created_at: self.created_at,
            cluster: self.cluster.clone(),
        };
        serde_json::to_string_pretty(&req)
            .map_err(|e| JsValue::from_str(&format!("serialisation error: {}", e)))
    }

    /// Feed a signer's JSON response into the orchestrator.
    ///
    /// Returns `true` if the signature threshold has been reached after this
    /// response; `false` if more rounds are needed.
    ///
    /// # Errors
    /// Throws if the JSON is malformed, the nonce doesn't match, the round is
    /// wrong, or the pubkey is not the expected signer for this round.
    pub fn ingest_response_json(&mut self, json: &str) -> Result<bool, JsValue> {
        let resp: MultiSignResponse = serde_json::from_str(json)
            .map_err(|e| JsValue::from_str(&format!("invalid response JSON: {}", e)))?;

        // Replay-protection: nonce must match the session nonce.
        if resp.nonce != self.nonce {
            return Err(JsValue::from_str("nonce mismatch — possible replay attack"));
        }

        // Round must match.
        if resp.round != self.round {
            return Err(JsValue::from_str(&format!(
                "expected round {}, got {}",
                self.round, resp.round
            )));
        }

        // Pubkey must be the expected signer for this round.
        let expected = self
            .current_signer_pubkey()
            .ok_or_else(|| JsValue::from_str("no signer expected for this round"))?;
        if resp.signer_pubkey != expected {
            return Err(JsValue::from_str(&format!(
                "expected signer {expected}, got {}",
                resp.signer_pubkey
            )));
        }

        // Duplicate-sig guard.
        if self
            .partial_sigs
            .iter()
            .any(|ps| ps.signer_pubkey == resp.signer_pubkey)
        {
            return Err(JsValue::from_str("duplicate signature from same pubkey"));
        }

        self.partial_sigs.push(PartialSig {
            signer_pubkey: resp.signer_pubkey,
            signature_b64: resp.signature_b64,
        });
        self.round += 1;

        Ok(self.threshold_met())
    }

    // ─── Result extraction ────────────────────────────────────────────────────

    /// Returns accumulated partial signatures as a JSON array.
    ///
    /// Each element: `{ "signer_pubkey": "...", "signature_b64": "..." }`.
    /// Pass this to `@solana/web3.js` to embed signatures into the transaction.
    ///
    /// # Errors
    /// Throws if serialisation fails (should never happen).
    pub fn get_partial_sigs_json(&self) -> Result<String, JsValue> {
        serde_json::to_string_pretty(&self.partial_sigs)
            .map_err(|e| JsValue::from_str(&format!("serialisation error: {}", e)))
    }

    /// Returns the original Base64-encoded transaction bytes (unchanged —
    /// signatures are carried separately via [`get_partial_sigs_json`]).
    pub fn get_transaction_b64(&self) -> String {
        self.transaction_b64.clone()
    }

    /// Returns the session nonce (hex) for external logging / debugging.
    pub fn nonce(&self) -> String {
        self.nonce.clone()
    }
}

// ─── FROST WASM bindings ──────────────────────────────────────────────────────

use afterimage_frost::{aggregator, dealer, participant, Round1Output, Round2Output};

/// Trusted-dealer key-generation for FROST t-of-n signing.
///
/// ```js
/// const d = WasmFrostDealer.generate(3, 2);  // 3 signers, threshold 2
/// const setup = JSON.parse(d.setup_json());
/// // setup.key_packages[i] → JSON KeyPackage for participant i+1
/// // setup.pubkey_package  → JSON PublicKeyPackage (shared)
/// ```
#[wasm_bindgen]
pub struct WasmFrostDealer {
    setup_json: String,
}

#[wasm_bindgen]
impl WasmFrostDealer {
    /// Generate a fresh t-of-n FROST key setup.
    ///
    /// # Errors
    /// Throws if `threshold == 0`, `threshold > n`, or entropy is unavailable.
    pub fn generate(n: u16, threshold: u16) -> Result<WasmFrostDealer, JsValue> {
        let setup = dealer::generate_setup(n, threshold)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let json = serde_json::to_string(&setup)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { setup_json: json })
    }

    /// Returns the full `FrostSetup` as a JSON string.
    ///
    /// Deserialise with `JSON.parse` in JS.  Contains:
    /// - `n`, `threshold`
    /// - `key_packages[]` — one per participant (KEEP PRIVATE, distribute securely)
    /// - `pubkey_package` — share with all participants and the aggregator
    pub fn setup_json(&self) -> String {
        self.setup_json.clone()
    }

    /// Returns only the `key_packages` array as a JSON string.
    ///
    /// `key_packages[i]` is for participant `i+1` (1-indexed).
    ///
    /// # Errors
    /// Throws if the internal JSON is malformed (should never happen).
    pub fn key_packages_json(&self) -> Result<String, JsValue> {
        let v: serde_json::Value = serde_json::from_str(&self.setup_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&v["key_packages"])
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Returns the `pubkey_package` JSON string.
    ///
    /// # Errors
    /// Throws if the internal JSON is malformed (should never happen).
    pub fn pubkey_package_json(&self) -> Result<String, JsValue> {
        let v: serde_json::Value = serde_json::from_str(&self.setup_json)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&v["pubkey_package"])
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// FROST participant — holds a key share and executes Round 1 and Round 2.
///
/// ```js
/// const p = new WasmFrostParticipant(keyPackageJson, 1);
/// const r1 = JSON.parse(p.round1());
/// // r1.nonces_json      — keep private!
/// // r1.commitments_json — send to aggregator
/// const r2 = JSON.parse(p.round2(noncesJson, signingPackageJson));
/// // r2.share_json — send to aggregator
/// ```
#[wasm_bindgen]
pub struct WasmFrostParticipant {
    key_package_json: String,
    identifier: u16,
}

#[wasm_bindgen]
impl WasmFrostParticipant {
    /// Create a participant from its `KeyPackage` JSON and 1-indexed identifier.
    #[wasm_bindgen(constructor)]
    pub fn new(key_package_json: &str, identifier: u16) -> WasmFrostParticipant {
        Self {
            key_package_json: key_package_json.to_owned(),
            identifier,
        }
    }

    /// Round 1 — generate nonces and commitment.
    ///
    /// Returns a JSON string `{ identifier, nonces_json, commitments_json }`.
    ///
    /// - **Keep `nonces_json` private** — it must not leave this participant.
    /// - Send `commitments_json` to the aggregator.
    ///
    /// # Errors
    /// Throws if the key package JSON is invalid or entropy is unavailable.
    pub fn round1(&self) -> Result<String, JsValue> {
        let out = participant::round1_commit(&self.key_package_json, self.identifier)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&out).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Round 2 — sign the aggregator's signing package.
    ///
    /// - `nonces_json`          — the `nonces_json` field from your Round-1 output.
    /// - `signing_package_json` — the `SigningPackage` JSON from the aggregator.
    ///
    /// Returns a JSON string `{ identifier, share_json }`.
    /// Send `share_json` to the aggregator.
    ///
    /// # Errors
    /// Throws if any JSON is invalid or the cryptographic check fails.
    pub fn round2(
        &self,
        nonces_json: &str,
        signing_package_json: &str,
    ) -> Result<String, JsValue> {
        let out = participant::round2_sign(
            &self.key_package_json,
            nonces_json,
            signing_package_json,
            self.identifier,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&out).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Returns this participant's 1-indexed identifier.
    pub fn identifier(&self) -> u16 {
        self.identifier
    }
}

// ─────────────────────────────────────────────────────────────────────────────

/// FROST aggregator — coordinates the two rounds and produces the final signature.
///
/// The aggregator is **not trusted** with any private key material.
///
/// ```js
/// const agg = new WasmFrostAggregator(pubkeyPackageJson, 2, 3);
///
/// // After each participant submits their Round-1 commitment JSON:
/// agg.add_commitment(r1OutputJson_1);
/// agg.add_commitment(r1OutputJson_2);
///
/// // Build the signing package (message = Solana tx message bytes as hex)
/// const pkgJson = agg.build_signing_package(messageHex);
///
/// // After each participant submits their Round-2 share JSON:
/// agg.add_share(r2OutputJson_1);
/// agg.add_share(r2OutputJson_2);
///
/// // Aggregate → final Ed25519 signature
/// const result = JSON.parse(agg.aggregate());
/// // result.signature_hex     — 64-byte Ed25519 sig (128 hex chars)
/// // result.verifying_key_hex — 32-byte group public key (64 hex chars)
/// ```
#[wasm_bindgen]
pub struct WasmFrostAggregator {
    pubkey_package_json: String,
    threshold: u16,
    total_participants: u16,
    commitments: Vec<Round1Output>,
    shares: Vec<Round2Output>,
}

#[wasm_bindgen]
impl WasmFrostAggregator {
    /// Create an aggregator.
    ///
    /// - `pubkey_package_json` — from `WasmFrostDealer.pubkey_package_json()`
    /// - `threshold`           — M (minimum signers required)
    /// - `total_participants`  — N (total signers in the setup)
    #[wasm_bindgen(constructor)]
    pub fn new(
        pubkey_package_json: &str,
        threshold: u16,
        total_participants: u16,
    ) -> WasmFrostAggregator {
        Self {
            pubkey_package_json: pubkey_package_json.to_owned(),
            threshold,
            total_participants,
            commitments: Vec::new(),
            shares: Vec::new(),
        }
    }

    // ─── Round 1 ──────────────────────────────────────────────────────────────

    /// Ingest one participant's Round-1 output JSON.
    ///
    /// `r1_json` is the full JSON string returned by `WasmFrostParticipant.round1()`.
    ///
    /// # Errors
    /// Throws if the JSON is malformed.
    pub fn add_commitment(&mut self, r1_json: &str) -> Result<(), JsValue> {
        let out: Round1Output =
            serde_json::from_str(r1_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.commitments.push(out);
        Ok(())
    }

    /// Number of Round-1 commitments collected so far.
    pub fn commitment_count(&self) -> usize {
        self.commitments.len()
    }

    /// Build the `SigningPackage` once enough commitments have been collected.
    ///
    /// - `message_hex` — the message bytes as a lowercase hex string
    ///   (for Solana: the serialised `Message` bytes, NOT the full `Transaction`).
    ///
    /// Returns the JSON-serialised `SigningPackage` to broadcast to all signers.
    ///
    /// # Errors
    /// Throws if `message_hex` is not valid hex or commitment JSON is malformed.
    pub fn build_signing_package(&self, message_hex: &str) -> Result<String, JsValue> {
        let msg = hex::decode(message_hex)
            .map_err(|e| JsValue::from_str(&format!("hex decode: {e}")))?;
        aggregator::build_signing_package(&self.commitments, &msg)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    // ─── Round 2 ──────────────────────────────────────────────────────────────

    /// Ingest one participant's Round-2 output JSON.
    ///
    /// `r2_json` is the full JSON string returned by `WasmFrostParticipant.round2()`.
    ///
    /// # Errors
    /// Throws if the JSON is malformed.
    pub fn add_share(&mut self, r2_json: &str) -> Result<(), JsValue> {
        let out: Round2Output =
            serde_json::from_str(r2_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
        self.shares.push(out);
        Ok(())
    }

    /// Number of Round-2 shares collected so far.
    pub fn share_count(&self) -> usize {
        self.shares.len()
    }

    // ─── Aggregate ────────────────────────────────────────────────────────────

    /// Aggregate all Round-2 shares into a single Ed25519 signature.
    ///
    /// Returns a JSON string matching [`FrostResult`]:
    /// ```json
    /// {
    ///   "signature_hex":     "...",  // 128 hex chars = 64 bytes
    ///   "verifying_key_hex": "...",  // 64 hex chars  = 32 bytes
    ///   "message_hex":       "...",
    ///   "threshold":         2,
    ///   "total_participants":3
    /// }
    /// ```
    ///
    /// # Errors
    /// Throws if the cryptographic aggregation fails (e.g. invalid shares or
    /// wrong public-key package) or JSON serialisation fails.
    pub fn aggregate(&self, signing_package_json: &str) -> Result<String, JsValue> {
        let result = aggregator::aggregate(
            signing_package_json,
            &self.shares,
            &self.pubkey_package_json,
            self.threshold,
            self.total_participants,
        )
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
        serde_json::to_string(&result).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Reset state for a new signing session (keeps the key setup).
    pub fn reset(&mut self) {
        self.commitments.clear();
        self.shares.clear();
    }
}

// ─── Internal helpers ─────────────────────────────────────────────────────────

/// Minimal Base58 encoder (Bitcoin/Solana alphabet).
fn bs58_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut digits: Vec<u8> = Vec::with_capacity(input.len() * 140 / 100 + 1);
    for &byte in input {
        let mut carry = byte as usize;
        for d in digits.iter_mut() {
            carry += (*d as usize) << 8;
            *d = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    // leading zero bytes → leading '1's
    let leading_ones = input.iter().take_while(|&&b| b == 0).count();
    let mut out = String::with_capacity(leading_ones + digits.len());
    for _ in 0..leading_ones {
        out.push('1');
    }
    for &d in digits.iter().rev() {
        out.push(ALPHABET[d as usize] as char);
    }
    out
}
