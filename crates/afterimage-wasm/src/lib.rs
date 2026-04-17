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