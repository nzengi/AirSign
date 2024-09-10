//! afterimage_optical::camera
//! ==========================
//! nokhwa-based camera capture loop that feeds decoded QR payloads
//! into an `afterimage_core::session::RecvSession`.
//!
//! # Usage
//! ```rust,no_run
//! use afterimage_optical::camera::CameraReceiver;
//!
//! let mut rx = CameraReceiver::open(0, "passw0rd").unwrap();
//! let data = rx.receive().unwrap();
//! std::fs::write("recovered.bin", &data).unwrap();
//! ```

use nokhwa::{
    pixel_format::RgbFormat,
    utils::{CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};

use afterimage_core::session::RecvSession;

use crate::{error::OpticalError, qr::decode_qr};

/// A camera-backed AfterImage receiver.
pub struct CameraReceiver {
    camera: Camera,
    session: RecvSession,
    /// Print progress to stderr every N frames.
    pub progress_interval: u32,
}

impl CameraReceiver {
    /// Open camera device at `index` (0 = first/default camera).
    ///
    /// # Errors
    /// [`OpticalError::Camera`] if the device could not be opened.
    pub fn open(index: u32, password: &str) -> Result<Self, OpticalError> {
        let idx = CameraIndex::Index(index);
        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        let mut camera = Camera::new(idx, requested)
            .map_err(|e| OpticalError::Camera(e.to_string()))?;

        camera
            .open_stream()
            .map_err(|e| OpticalError::Camera(e.to_string()))?;

        Ok(Self {
            camera,
            session: RecvSession::new(password),
            progress_interval: 10,
        })
    }

    /// Receive until the session is complete, then return the plaintext data.
    ///
    /// Blocks the calling thread.  Call from a dedicated thread if needed.
    ///
    /// # Errors
    /// - [`OpticalError::Camera`] on capture errors
    /// - Propagates `AfterImageError` from the session on decryption failure
    pub fn receive(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut frame_no = 0u32;

        loop {
            // nokhwa 0.10: `frame()` replaces the old `capture_frame()`
            let buf = self
                .camera
                .frame()
                .map_err(|e| OpticalError::Camera(e.to_string()))?;

            let res = buf.resolution();
            let width = res.width_x;
            let height = res.height_y;
            let rgb = buf.buffer(); // &[u8] — raw RGB24 bytes

            // Convert RGB24 to luma8
            let luma: Vec<u8> = rgb
                .chunks_exact(3)
                .map(|p| {
                    let r = p[0] as u32;
                    let g = p[1] as u32;
                    let b = p[2] as u32;
                    ((r * 299 + g * 587 + b * 114) / 1000) as u8
                })
                .collect();

            // Try to decode a QR code from the current frame
            if let Ok(payload) = decode_qr(&luma, width, height) {
                if let Ok(complete) = self.session.ingest_frame(&payload) {
                    if complete {
                        break;
                    }
                }
            }

            frame_no += 1;
            if frame_no % self.progress_interval == 0 {
                eprintln!(
                    "[afterimage] camera rx: frame={frame_no} progress={:.1}%",
                    self.session.progress() * 100.0
                );
            }
        }

        Ok(self.session.get_data()?)
    }

    /// Provide direct access to the underlying `RecvSession` for custom loops.
    pub fn session(&mut self) -> &mut RecvSession {
        &mut self.session
    }
}

impl Drop for CameraReceiver {
    fn drop(&mut self) {
        let _ = self.camera.stop_stream();
    }
}