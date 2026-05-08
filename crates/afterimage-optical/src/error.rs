//! Error types for the optical layer.

use thiserror::Error;

/// Errors produced by QR encoding/decoding, camera, or display operations.
#[derive(Debug, Error)]
pub enum OpticalError {
    /// QR encoding failed (payload too large for the selected version/ECC).
    #[error("QR encode error: {0}")]
    QrEncode(String),

    /// QR decoding failed — no recognisable QR symbol in the image.
    #[error("QR decode error: no QR code found in image")]
    QrDecode,

    /// The decoded QR payload does not match the expected binary length.
    #[error("QR payload length mismatch: expected {expected} bytes, got {got}")]
    PayloadLength {
        /// Expected byte length.
        expected: usize,
        /// Actual byte length received.
        got: usize,
    },

    /// Camera device could not be opened or is not available.
    #[cfg(feature = "camera")]
    #[error("Camera error: {0}")]
    Camera(String),

    /// Display window error.
    #[cfg(feature = "display")]
    #[error("Display error: {0}")]
    Display(String),

    /// Image encode/decode error.
    #[error("Image error: {0}")]
    Image(String),
}