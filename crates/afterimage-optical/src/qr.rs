//! afterimage_optical::qr
//! ======================
//! QR-code encode and decode helpers.
//!
//! # Encoding strategy
//! AfterImage frames (77–264 bytes) fit comfortably in a QR version 10–15
//! symbol at ECC level M.  We use **Binary mode** (ISO/IEC 18004 §7.4.5)
//! which is the only mode that preserves arbitrary byte values.
//!
//! # Decoding strategy
//! rxing is used to scan a raw luma8 image buffer.  The caller is responsible
//! for supplying the image (e.g., from the nokhwa camera crate or a PNG file).

use image::{GrayImage, RgbImage};
use qrcode::{EcLevel, QrCode};

use crate::error::OpticalError;

// ─── QR tuning parameters ─────────────────────────────────────────────────────

/// QR error-correction level.  M ≈ 15 % recovery.
const EC_LEVEL: EcLevel = EcLevel::M;

/// Quiet-zone width in modules on each side.
const QUIET_ZONE: u32 = 4;

/// Scale factor: pixels per QR module.
pub const MODULE_PX: u32 = 10;

// ─── QrFrame ─────────────────────────────────────────────────────────────────

/// A pre-rendered QR code image, ready to display or save.
pub struct QrFrame {
    /// Raw RGBA pixel data (width × height × 4 bytes).
    pub rgba: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels (= width for QR codes).
    pub height: u32,
}

impl QrFrame {
    /// Convert to a 0xRRGGBBAA packed `u32` slice suitable for minifb windows.
    pub fn to_u32_buf(&self) -> Vec<u32> {
        self.rgba
            .chunks_exact(4)
            .map(|c| {
                let r = c[0] as u32;
                let g = c[1] as u32;
                let b = c[2] as u32;
                (r << 16) | (g << 8) | b
            })
            .collect()
    }

    /// Save the QR image as a PNG file.
    pub fn save_png(&self, path: &str) -> Result<(), OpticalError> {
        use image::RgbaImage;
        let img = RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .ok_or_else(|| OpticalError::Image("invalid RGBA buffer".into()))?;
        img.save(path)
            .map_err(|e| OpticalError::Image(e.to_string()))
    }
}

// ─── encode_qr ────────────────────────────────────────────────────────────────

/// Encode `data` bytes into a [`QrFrame`] (scaled, quiet-zone included).
///
/// `data` should be at most ~550 bytes for a version-15 M symbol.
/// AfterImage droplets (264 bytes) and METADATA frames (77 bytes) fit easily.
///
/// # Errors
/// [`OpticalError::QrEncode`] if the payload is too large for any QR version.
pub fn encode_qr(data: &[u8]) -> Result<QrFrame, OpticalError> {
    let code = QrCode::with_error_correction_level(data, EC_LEVEL)
        .map_err(|e| OpticalError::QrEncode(e.to_string()))?;

    let modules = code.to_colors(); // flat QrColor slice, row-major
    let width_modules = code.width() as u32;
    let total_modules = width_modules + 2 * QUIET_ZONE;
    let px = total_modules * MODULE_PX;

    let mut img = RgbImage::new(px, px);

    // Fill with white (quiet zone + background)
    for pixel in img.pixels_mut() {
        *pixel = image::Rgb([255u8, 255, 255]);
    }

    // Render modules
    for row in 0..width_modules {
        for col in 0..width_modules {
            let idx = (row * width_modules + col) as usize;
            let dark = modules[idx] == qrcode::Color::Dark;
            let colour = if dark {
                image::Rgb([0u8, 0, 0])
            } else {
                image::Rgb([255u8, 255, 255])
            };
            let x0 = (col + QUIET_ZONE) * MODULE_PX;
            let y0 = (row + QUIET_ZONE) * MODULE_PX;
            for dy in 0..MODULE_PX {
                for dx in 0..MODULE_PX {
                    img.put_pixel(x0 + dx, y0 + dy, colour);
                }
            }
        }
    }

    // Convert to RGBA
    let rgba: Vec<u8> = img
        .pixels()
        .flat_map(|p| [p[0], p[1], p[2], 255u8])
        .collect();

    Ok(QrFrame {
        rgba,
        width: px,
        height: px,
    })
}

// ─── decode_qr ────────────────────────────────────────────────────────────────

/// Decode a QR code from a luma8 image buffer.
///
/// `luma` must be a row-major grayscale byte slice of dimensions `width × height`.
///
/// Returns the raw binary payload bytes.
///
/// # Errors
/// [`OpticalError::QrDecode`] if no QR code can be found.
pub fn decode_qr(luma: &[u8], width: u32, height: u32) -> Result<Vec<u8>, OpticalError> {
    use rxing::{
        BarcodeFormat, BinaryBitmap, DecodeHintValue, DecodeHints,
        Luma8LuminanceSource, MultiFormatReader, Reader,
        common::HybridBinarizer,
    };

    let source = Luma8LuminanceSource::new(luma.to_vec(), width, height);
    let binarizer = HybridBinarizer::new(source);
    let mut bmp = BinaryBitmap::new(binarizer);

    let hints = DecodeHints::default()
        .with(DecodeHintValue::PossibleFormats(
            std::collections::HashSet::from([BarcodeFormat::QR_CODE]),
        ))
        .with(DecodeHintValue::TryHarder(true));

    let mut reader = MultiFormatReader::default();
    let result = reader
        .decode_with_hints(&mut bmp, &hints)
        .map_err(|_| OpticalError::QrDecode)?;

    // For binary data use the raw bytes; fall back to Latin-1 text bytes
    let raw = result.getRawBytes().to_vec();
    if raw.is_empty() {
        Ok(result.getText().bytes().collect())
    } else {
        Ok(raw)
    }
}

/// Decode a QR code from an RGB image.
pub fn decode_qr_rgb(img: &RgbImage) -> Result<Vec<u8>, OpticalError> {
    let gray: GrayImage = image::imageops::colorops::grayscale(img);
    let (w, h) = gray.dimensions();
    decode_qr(gray.as_raw(), w, h)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_produces_correct_dimensions() {
        let data = vec![0x42u8; 77]; // METADATA-sized payload
        let frame = encode_qr(&data).unwrap();
        assert!(frame.width > 0);
        assert_eq!(frame.width, frame.height);
        assert_eq!(frame.rgba.len(), (frame.width * frame.height * 4) as usize);
    }

    #[test]
    fn u32_buf_length_matches() {
        let data = b"test payload";
        let frame = encode_qr(data).unwrap();
        let buf = frame.to_u32_buf();
        assert_eq!(buf.len(), (frame.width * frame.height) as usize);
    }

    #[test]
    fn empty_payload_encodes() {
        // QR codes can encode empty payloads; either Ok or Err is acceptable
        let _ = encode_qr(b"");
    }
}