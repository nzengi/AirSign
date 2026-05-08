//! afterimage_core::session
//! ========================
//! High-level `SendSession` and `RecvSession` that wire together
//! compress → encrypt → fountain-encode (send side) and
//! fountain-decode → decrypt → decompress (receive side).
//!
//! Both sessions are agnostic to the transport layer: they produce / consume
//! raw byte slices.  The optical layer (QR codes, camera) sits above this.

use crate::{
    crypto::{Argon2Params, CryptoLayer},
    error::AfterImageError,
    fountain::{LTDecoder, LTEncoder, BLOCK_SIZE, HEADER_SIZE},
    protocol::{MetadataFrame, METADATA_INTERVAL},
};

// ─── Free helpers ─────────────────────────────────────────────────────────────

/// Compute the recommended number of QR frames to transmit for a plaintext
/// payload of `size` bytes (includes compression/encryption overhead estimate
/// and a 1.5× redundancy factor).
pub fn recommended_frames(size: usize) -> usize {
    // Rough estimate: compressed+encrypted ≈ 110 % of original
    let ciphertext_len = (size as f64 * 1.1) as usize + 64;
    // Each source block holds BLOCK_SIZE bytes
    use crate::fountain::BLOCK_SIZE;
    let k = ciphertext_len.div_ceil(BLOCK_SIZE).max(1);
    // 1 METADATA every METADATA_INTERVAL frames + 1.5× redundancy droplets
    use crate::protocol::METADATA_INTERVAL;
    let droplets = (k as f64 * 1.5) as usize + 1;
    let metadata_count = droplets / METADATA_INTERVAL as usize + 1;
    droplets + metadata_count
}

// ─── SendSession ──────────────────────────────────────────────────────────────

/// Orchestrates compress → encrypt → fountain-encode for one file transfer.
///
/// # Example
/// ```rust,no_run
/// # use afterimage_core::session::SendSession;
/// let data = b"secret payload";
/// let mut session = SendSession::new(data, "payload.bin", "p4ssw0rd!").unwrap();
/// while let Some(frame) = session.next_frame() {
///     // transmit `frame` via QR code / display
/// }
/// ```
pub struct SendSession {
    encoder: LTEncoder,
    metadata: MetadataFrame,
    /// Frames emitted so far (used to interleave METADATA).
    frame_count: u32,
    /// Total frames to emit before stopping (None = infinite).
    limit: Option<u32>,
    done: bool,
}

impl SendSession {
    /// Create a new send session.
    ///
    /// `data`     — raw plaintext bytes to transfer  
    /// `filename` — cleartext filename embedded in the METADATA frame  
    /// `password` — Argon2id encryption password  
    ///
    /// The data is zlib-compressed then ChaCha20-Poly1305 encrypted before
    /// being split into LT source blocks.
    #[cfg(feature = "argon2")]
    pub fn new(
        data: &[u8],
        filename: &str,
        password: &str,
    ) -> Result<Self, AfterImageError> {
        let original_len = data.len() as u32;

        // 1. Compress
        #[cfg(feature = "std")]
        let compressed = crate::crypto::compress::compress(data);
        #[cfg(not(feature = "std"))]
        let compressed = data.to_vec();

        // 2. Encrypt
        let ciphertext = CryptoLayer::encrypt_v2(&compressed, password)?;

        // 3. Fountain-encode
        let encoder = LTEncoder::new(&ciphertext)?;
        let k = encoder.k as u32;

        let metadata = MetadataFrame::new_v2(k, original_len, filename);

        Ok(Self {
            encoder,
            metadata,
            frame_count: 0,
            limit: None,
            done: false,
        })
    }

    /// Create a new send session with custom Argon2id parameters.
    ///
    /// `data`     — raw plaintext bytes to transfer  
    /// `filename` — cleartext filename embedded in the METADATA frame  
    /// `password` — Argon2id encryption password  
    /// `params`   — Argon2id key-derivation parameters (embedded in a v3 METADATA frame)
    ///
    /// The Argon2 parameters are written into the 85-byte v3 METADATA frame so
    /// the receiver can reconstruct the encryption key without any out-of-band
    /// configuration.  Use `--argon2-mem` / `--argon2-iter` on the CLI.
    #[cfg(feature = "argon2")]
    pub fn new_with_argon2_params(
        data: &[u8],
        filename: &str,
        password: &str,
        params: Argon2Params,
    ) -> Result<Self, AfterImageError> {
        let original_len = data.len() as u32;

        #[cfg(feature = "std")]
        let compressed = crate::crypto::compress::compress(data);
        #[cfg(not(feature = "std"))]
        let compressed = data.to_vec();

        let ciphertext = CryptoLayer::encrypt_with_params(&compressed, password, &params)?;

        let encoder = LTEncoder::new(&ciphertext)?;
        let k = encoder.k as u32;

        let metadata =
            MetadataFrame::new_v3(k, original_len, filename, params.m_cost, params.t_cost);

        Ok(Self {
            encoder,
            metadata,
            frame_count: 0,
            limit: None,
            done: false,
        })
    }

    /// Set a hard upper limit on the number of frames to emit.
    /// After `limit` frames the iterator returns `None`.
    pub fn set_limit(&mut self, limit: u32) {
        self.limit = Some(limit);
    }

    /// Emit the next frame (METADATA or DATA).
    ///
    /// Returns `None` when the limit has been reached (or never, if no limit).
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }
        if let Some(limit) = self.limit {
            if self.frame_count >= limit {
                self.done = true;
                return None;
            }
        }

        let frame = if self.frame_count % METADATA_INTERVAL == 0 {
            self.metadata.to_bytes().to_vec()
        } else {
            self.encoder.generate_droplet()
        };

        self.frame_count += 1;
        Some(frame)
    }

    /// Recommended minimum number of DATA frames for reliable decoding.
    pub fn recommended_droplet_count(&self) -> usize {
        self.encoder.recommended_count()
    }

    /// Number of source blocks (before redundancy).
    pub fn droplet_count(&self) -> usize {
        self.encoder.k
    }

    /// `true` if the session has not yet reached its limit (or has no limit).
    pub fn has_next(&self) -> bool {
        !self.done
    }

    /// True if a fixed limit was set and has been reached.
    pub fn is_done(&self) -> bool {
        self.done
    }
}

// ─── RecvSession ──────────────────────────────────────────────────────────────

/// Orchestrates fountain-decode → decrypt → decompress for one file transfer.
///
/// # Example
/// ```rust,no_run
/// # use afterimage_core::session::RecvSession;
/// let mut session = RecvSession::new("p4ssw0rd!");
/// // ... feed QR-decoded frames in a loop:
/// // session.ingest_frame(&raw_bytes);
/// // if session.is_complete() { let data = session.get_data().unwrap(); }
/// ```
pub struct RecvSession {
    password: String,
    decoder: LTDecoder,
    metadata: Option<MetadataFrame>,
    frames_received: u64,
}

impl RecvSession {
    /// Create a new receive session with the given decryption password.
    pub fn new(password: impl Into<String>) -> Self {
        Self {
            password: password.into(),
            decoder: LTDecoder::new(),
            metadata: None,
            frames_received: 0,
        }
    }

    /// Feed a raw frame (METADATA or DATA) into the session.
    ///
    /// Returns `Ok(true)` when decoding is complete and data is ready.
    pub fn ingest_frame(&mut self, frame: &[u8]) -> Result<bool, AfterImageError> {
        self.frames_received += 1;

        if MetadataFrame::is_metadata(frame) {
            let meta = MetadataFrame::from_bytes(frame)?;
            if self.metadata.is_none() {
                self.decoder.set_block_count(meta.k as usize);
            }
            self.metadata = Some(meta);
            return Ok(self.decoder.is_complete());
        }

        // DATA frame — must be at least HEADER_SIZE + BLOCK_SIZE
        if frame.len() < HEADER_SIZE + BLOCK_SIZE {
            // Silently ignore malformed frames (best-effort optical channel)
            return Ok(false);
        }

        Ok(self.decoder.add_droplet(frame)?)
    }

    /// Returns `true` when all source blocks have been recovered.
    pub fn is_complete(&self) -> bool {
        self.decoder.is_complete()
    }

    /// Fraction of source blocks decoded [0.0, 1.0].
    pub fn progress(&self) -> f64 {
        self.decoder.progress()
    }

    /// Number of frames received so far.
    pub fn frames_received(&self) -> u64 {
        self.frames_received
    }

    /// The METADATA frame, if one has been received.
    pub fn metadata(&self) -> Option<&MetadataFrame> {
        self.metadata.as_ref()
    }

    /// Reconstruct, decrypt and decompress the original data.
    ///
    /// # Errors
    /// - [`AfterImageError::Fountain`] if decoding is not yet complete
    /// - [`AfterImageError::Crypto`]   if the password is wrong or data is tampered
    /// - [`AfterImageError::Io`]       if decompression fails
    pub fn get_data(&self) -> Result<Vec<u8>, AfterImageError> {
        // 1. Get fountain-decoded ciphertext
        let ciphertext = self.decoder.get_data()?;

        // 2. Determine protocol version
        let version = self
            .metadata
            .as_ref()
            .map(|m| m.version)
            .unwrap_or(2);

        // 3. Build Argon2 params — for v3 read from the embedded frame fields
        let argon2_params = if version == 3 {
            if let Some(ref meta) = self.metadata {
                Argon2Params {
                    m_cost: meta.argon2_m_cost,
                    t_cost: meta.argon2_t_cost,
                    p_cost: crate::crypto::ARGON2_P_COST,
                }
            } else {
                Argon2Params::default()
            }
        } else {
            Argon2Params::default()
        };

        // 3. Decrypt
        let compressed =
            CryptoLayer::decrypt_with_params(&ciphertext, &self.password, version, &argon2_params)?;

        // 4. Decompress
        #[cfg(feature = "std")]
        let data = crate::crypto::compress::decompress(&compressed)?;
        #[cfg(not(feature = "std"))]
        let data = compressed;

        Ok(data)
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(all(test, feature = "argon2", feature = "std"))]
mod tests {
    use super::*;

    const PASSWORD: &str = "session-test-password-42";

    fn make_data(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i % 127) as u8).collect()
    }

    #[test]
    fn send_recv_roundtrip_small() {
        roundtrip_check(make_data(512));
    }

    #[test]
    fn send_recv_roundtrip_medium() {
        roundtrip_check(make_data(8192));
    }

    #[test]
    fn metadata_interleaved_correctly() {
        let data = make_data(256);
        let mut session = SendSession::new(&data, "test.bin", PASSWORD).unwrap();

        // First frame must be METADATA (frame_count == 0)
        let first = session.next_frame().unwrap();
        assert!(MetadataFrame::is_metadata(&first));
    }

    fn roundtrip_check(data: Vec<u8>) {
        let mut send = SendSession::new(&data, "roundtrip.bin", PASSWORD).unwrap();
        let mut recv = RecvSession::new(PASSWORD);

        let limit = (send.recommended_droplet_count() * 4) as u32 + 200;
        send.set_limit(limit);

        while let Some(frame) = send.next_frame() {
            if recv.ingest_frame(&frame).unwrap() {
                break;
            }
        }

        assert!(recv.is_complete(), "session not complete; progress={:.1}%", recv.progress() * 100.0);
        let recovered = recv.get_data().unwrap();
        assert_eq!(recovered, data);
    }

    /// v3 session: custom Argon2 params embedded in frame, recovered automatically.
    ///
    /// Note: p_cost is NOT stored in the v3 wire frame — the receiver always
    /// reconstructs with ARGON2_P_COST (4). The test must therefore use the
    /// default parallelism so that sender and receiver derive the same key.
    #[test]
    fn send_recv_v3_custom_argon2_params() {
        let data = make_data(1024);
        // Use m_cost=8192 / t_cost=1 for test-speed, but p_cost MUST be the
        // default (4) since p_cost is not carried in the wire frame.
        let params = Argon2Params {
            m_cost: 8_192,
            t_cost: 1,
            p_cost: crate::crypto::ARGON2_P_COST,
        };

        let mut send =
            SendSession::new_with_argon2_params(&data, "v3test.bin", PASSWORD, params).unwrap();

        // Verify the METADATA frame is v3
        let first_frame = send.next_frame().unwrap();
        assert!(
            MetadataFrame::is_metadata(&first_frame),
            "first frame must be METADATA"
        );
        let meta = MetadataFrame::from_bytes(&first_frame).unwrap();
        assert_eq!(meta.version, 3, "session must emit v3 frames");
        assert_eq!(meta.argon2_m_cost, 8_192);
        assert_eq!(meta.argon2_t_cost, 1);

        // Full roundtrip — RecvSession reads params from the v3 frame
        let mut send2 =
            SendSession::new_with_argon2_params(&data, "v3test.bin", PASSWORD, params).unwrap();
        let mut recv = RecvSession::new(PASSWORD);
        let limit = (send2.recommended_droplet_count() * 4) as u32 + 200;
        send2.set_limit(limit);
        while let Some(frame) = send2.next_frame() {
            if recv.ingest_frame(&frame).unwrap() {
                break;
            }
        }
        assert!(recv.is_complete());
        let recovered = recv.get_data().unwrap();
        assert_eq!(recovered, data, "v3 roundtrip data mismatch");
    }

    /// v2 sessions (default params) must still decode correctly after the v3 changes.
    #[test]
    fn v2_backward_compat_roundtrip() {
        roundtrip_check(make_data(2048));
    }
}
