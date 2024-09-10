//! # afterimage-optical
//!
//! QR-code generation, animated window display, and camera-based scanning
//! for the AfterImage air-gap data-transfer protocol.
//!
//! ## Modules
//!
//! * [`qr`]      — Encode raw bytes → QR image; decode QR image → bytes
//! * [`display`] — Animated minifb window that cycles through QR frames
//! * [`camera`]  — nokhwa camera capture loop feeding into a `RecvSession`

#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::all)]

pub mod error;
pub mod qr;

#[cfg(feature = "display")]
pub mod display;

#[cfg(feature = "camera")]
pub mod camera;

pub use error::OpticalError;
pub use qr::{decode_qr, encode_qr, QrFrame};