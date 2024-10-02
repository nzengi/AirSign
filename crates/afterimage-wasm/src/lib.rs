//! # afterimage-wasm
//!
//! WebAssembly bindings that expose the AfterImage encode/decode pipeline
//! to browser JavaScript / TypeScript.
//!
//! ## Building
//! ```bash
//! wasm-pack build crates/afterimage-wasm --target web --release
//! ```
//!
//! ## JS API
//! ```js
//! import init, { WasmSendSession, WasmRecvSession } from './afterimage_wasm.js';
//! await init();
//!
//! // Sender side
//! const tx = new WasmSendSession(data_u8array, "filename.bin", "password");
//! while (tx.has_next()) {
//!   const frame = tx.next_frame();   // Uint8Array
//!   displayAsQr(frame);
//!   await sleep(150);
//! }
//!
//! // Receiver side
//! const rx = new WasmRecvSession("password");
//! rx.ingest_frame(qr_payload);      // call for each decoded QR frame
//! if (rx.is_complete()) {
//!   const data = rx.get_data();     // Uint8Array
//! }
//! ```

use wasm_bindgen::prelude::*;
use afterimage_core::session::{RecvSession, SendSession};

// Initialisation hook called by the JS glue code on WASM startup.
#[wasm_bindgen(start)]
pub fn start() {}

// ─── WasmSendSession ─────────────────────────────────────────────────────────

/// Encrypts and fountain-encodes data for transmission as QR frames.
#[wasm_bindgen]
pub struct WasmSendSession {
    inner: SendSession,
}

#[wasm_bindgen]
impl WasmSendSession {
    /// Create a new send session.
    ///
    /// # Errors
    /// Throws a JS exception if key derivation or encryption fails.
    #[wasm_bindgen(constructor)]
    pub fn new(data: &[u8], filename: &str, password: &str) -> Result<WasmSendSession, JsValue> {
        let inner = SendSession::new(data, filename, password)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self { inner })
    }

    /// Returns `true` if there are more frames to send.
    pub fn has_next(&self) -> bool {
        self.inner.has_next()
    }

    /// Returns the next encoded frame as a `Uint8Array`, or `null` if done.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        self.inner.next_frame()
    }

    /// Total number of source droplets (before redundancy).
    pub fn droplet_count(&self) -> usize {
        self.inner.droplet_count()
    }

    /// Recommended total frames to transmit (source + redundancy).
    pub fn recommended_droplet_count(&self) -> usize {
        self.inner.recommended_droplet_count()
    }

    /// Set a hard upper limit on frames generated (0 = unlimited).
    pub fn set_limit(&mut self, limit: u32) {
        self.inner.set_limit(limit);
    }
}

// ─── WasmRecvSession ─────────────────────────────────────────────────────────

/// Decodes and decrypts received QR frame payloads.
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

    /// Feed a raw QR-decoded payload into the session.
    ///
    /// Returns `true` if the transfer is complete.
    /// Silently ignores frames that cannot be parsed (corrupted / duplicate).
    pub fn ingest_frame(&mut self, frame: &[u8]) -> bool {
        self.inner.ingest_frame(frame).unwrap_or(false)
    }

    /// Returns `true` if all data has been received and decrypted successfully.
    pub fn is_complete(&self) -> bool {
        self.inner.is_complete()
    }

    /// Fraction of required droplets received (0.0 – 1.0).
    pub fn progress(&self) -> f64 {
        self.inner.progress() as f64
    }

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

    /// Number of frames received so far.
    pub fn received_count(&self) -> u64 {
        self.inner.frames_received()
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