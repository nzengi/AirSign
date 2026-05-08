//! afterimage_optical::display
//! ===========================
//! Animated minifb window that cycles through pre-rendered QR frames.
//!
//! # Usage
//! ```rust,no_run
//! use afterimage_optical::display::QrDisplay;
//! use afterimage_core::session::SendSession;
//!
//! let data = std::fs::read("secret.zip").unwrap();
//! let mut session = SendSession::new(&data, "secret.zip", "passw0rd").unwrap();
//! let mut disp = QrDisplay::new("AfterImage – send", 600).unwrap();
//! disp.run_session(&mut session);
//! ```

use minifb::{Key, Window, WindowOptions};

use crate::error::OpticalError;
use crate::qr::encode_qr;

/// Default inter-frame delay in milliseconds.
pub const DEFAULT_FRAME_MS: u64 = 150;

/// A minifb display window for transmitting AfterImage QR streams.
pub struct QrDisplay {
    window: Window,
    /// Target window size (width = height, square).
    size: usize,
    /// Inter-frame delay in milliseconds.
    pub frame_ms: u64,
}

impl QrDisplay {
    /// Open a new square window with the given title and edge size in pixels.
    ///
    /// # Errors
    /// [`OpticalError::Display`] if the window could not be created.
    pub fn new(title: &str, size: usize) -> Result<Self, OpticalError> {
        let window = Window::new(
            title,
            size,
            size,
            WindowOptions {
                resize: false,
                ..WindowOptions::default()
            },
        )
        .map_err(|e| OpticalError::Display(e.to_string()))?;

        Ok(Self {
            window,
            size,
            frame_ms: DEFAULT_FRAME_MS,
        })
    }

    /// Display a single pre-encoded frame (raw bytes → QR → window).
    ///
    /// Returns `true` if the window is still open, `false` if the user closed it.
    pub fn show_frame(&mut self, data: &[u8]) -> Result<bool, OpticalError> {
        if !self.window.is_open() || self.window.is_key_down(Key::Escape) {
            return Ok(false);
        }

        let qr = encode_qr(data)?;

        // Scale to window size using nearest-neighbour
        let buf = self.scale_to_window(&qr.to_u32_buf(), qr.width as usize);

        self.window
            .update_with_buffer(&buf, self.size, self.size)
            .map_err(|e| OpticalError::Display(e.to_string()))?;

        std::thread::sleep(std::time::Duration::from_millis(self.frame_ms));
        Ok(true)
    }

    /// Run a complete `SendSession` until either the session ends or the user
    /// closes the window / presses Escape.
    ///
    /// Returns the number of frames displayed.
    pub fn run_session(
        &mut self,
        session: &mut afterimage_core::session::SendSession,
    ) -> usize {
        let mut count = 0usize;
        while let Some(frame) = session.next_frame() {
            match self.show_frame(&frame) {
                Ok(true) => count += 1,
                _ => break,
            }
        }
        count
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    /// Nearest-neighbour scale of a square `src` buffer to `self.size × self.size`.
    fn scale_to_window(&self, src: &[u32], src_size: usize) -> Vec<u32> {
        let dst_size = self.size;
        let mut dst = vec![0u32; dst_size * dst_size];
        for dy in 0..dst_size {
            let sy = dy * src_size / dst_size;
            for dx in 0..dst_size {
                let sx = dx * src_size / dst_size;
                dst[dy * dst_size + dx] = src[sy * src_size + sx];
            }
        }
        dst
    }
}