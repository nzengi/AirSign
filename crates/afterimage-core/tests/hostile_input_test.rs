//! Hostile-input regression tests for AirSign frame parsing.
//!
//! Brutal Q7: *"What if the QR decoder library has a vulnerability and a
//! malicious frame triggers RCE on the air-gapped device?"*
//!
//! AirSign trusts `jsQR` (browser) and our own `MetadataFrame::from_bytes`
//! (Rust) to handle adversarial input. These tests pin down the parser's
//! behaviour on:
//!
//! 1. Truncated frames (must error, not panic)
//! 2. Wrong magic bytes
//! 3. Unknown protocol versions
//! 4. Hostile `k` (block count) — would otherwise cause OOM
//! 5. Hostile `original_len` — would otherwise cause OOM
//! 6. Garbage filename bytes (non-UTF-8)
//! 7. Random fuzz over the metadata frame surface
//!
//! Brutal Q19 ("`total_frames = u32::MAX`") collapses to (4) here; the bound
//! is enforced before any allocation runs.

use afterimage_core::{
    AfterImageError, MetadataFrame, ProtocolError, MAGIC, META_SIZE_V2, META_SIZE_V3,
};
use afterimage_core::protocol::{MAX_BLOCK_COUNT, MAX_ORIGINAL_LEN};

/// Helper — build a syntactically-valid v3 metadata frame as raw bytes,
/// caller can then mutate fields to produce hostile inputs.
fn valid_v3_frame() -> Vec<u8> {
    let meta = MetadataFrame::new_v3(10, 1024, "tx.bin", 65_536, 3);
    meta.to_bytes().to_vec()
}

#[test]
fn truncated_frame_rejects_cleanly() {
    let bytes = vec![0u8; 16];
    let result = MetadataFrame::from_bytes(&bytes);
    assert!(matches!(
        result,
        Err(ProtocolError::MetadataTooShort { .. })
    ));
}

#[test]
fn empty_frame_rejects_cleanly() {
    let result = MetadataFrame::from_bytes(&[]);
    assert!(matches!(
        result,
        Err(ProtocolError::MetadataTooShort { .. })
    ));
}

#[test]
fn wrong_magic_is_rejected_with_typed_error() {
    let mut bytes = vec![0u8; META_SIZE_V2];
    bytes[..4].copy_from_slice(b"XXXX");
    let result = MetadataFrame::from_bytes(&bytes);
    assert!(matches!(result, Err(ProtocolError::InvalidMagic { .. })));
}

#[test]
fn unknown_version_is_rejected() {
    let mut bytes = vec![0u8; META_SIZE_V2];
    bytes[..4].copy_from_slice(MAGIC);
    bytes[4] = 99; // version
    let result = MetadataFrame::from_bytes(&bytes);
    assert!(matches!(result, Err(ProtocolError::UnknownVersion(99))));
}

#[test]
fn hostile_block_count_is_rejected() {
    // Q19: total_frames = u32::MAX would cause a ~1 TB allocation in the
    // decoder. The metadata parser must reject it before the decoder is
    // even instantiated.
    let mut bytes = valid_v3_frame();
    let hostile_k: u32 = u32::MAX;
    bytes[5..9].copy_from_slice(&hostile_k.to_be_bytes());
    let result = MetadataFrame::from_bytes(&bytes);
    match result {
        Err(ProtocolError::BlockCountTooLarge { got, max }) => {
            assert_eq!(got, u32::MAX);
            assert_eq!(max, MAX_BLOCK_COUNT);
        }
        other => panic!("expected BlockCountTooLarge, got {other:?}"),
    }
}

#[test]
fn block_count_at_limit_is_accepted() {
    let mut bytes = valid_v3_frame();
    bytes[5..9].copy_from_slice(&MAX_BLOCK_COUNT.to_be_bytes());
    let parsed = MetadataFrame::from_bytes(&bytes).expect("at-limit k must parse");
    assert_eq!(parsed.k, MAX_BLOCK_COUNT);
}

#[test]
fn block_count_one_over_limit_is_rejected() {
    let mut bytes = valid_v3_frame();
    let hostile = MAX_BLOCK_COUNT + 1;
    bytes[5..9].copy_from_slice(&hostile.to_be_bytes());
    assert!(matches!(
        MetadataFrame::from_bytes(&bytes),
        Err(ProtocolError::BlockCountTooLarge { .. })
    ));
}

#[test]
fn hostile_original_len_is_rejected() {
    let mut bytes = valid_v3_frame();
    let hostile_len: u32 = u32::MAX;
    bytes[9..13].copy_from_slice(&hostile_len.to_be_bytes());
    let result = MetadataFrame::from_bytes(&bytes);
    match result {
        Err(ProtocolError::OriginalLenTooLarge { got, max }) => {
            assert_eq!(got, u32::MAX);
            assert_eq!(max, MAX_ORIGINAL_LEN);
        }
        other => panic!("expected OriginalLenTooLarge, got {other:?}"),
    }
}

#[test]
fn original_len_at_limit_is_accepted() {
    let mut bytes = valid_v3_frame();
    bytes[9..13].copy_from_slice(&MAX_ORIGINAL_LEN.to_be_bytes());
    let parsed =
        MetadataFrame::from_bytes(&bytes).expect("at-limit original_len must parse");
    assert_eq!(parsed.original_len, MAX_ORIGINAL_LEN);
}

#[test]
fn non_utf8_filename_is_rejected() {
    let mut bytes = valid_v3_frame();
    // Filename region for v3 is at offsets 21..85.
    // Write four invalid-UTF-8 lead bytes followed by the rest of the
    // filename region zeroed out so the NUL-terminated decode actually
    // tries to UTF-8 the hostile bytes.
    let filename_start = 21;
    let filename_end = 85;
    for b in &mut bytes[filename_start..filename_end] {
        *b = 0;
    }
    bytes[filename_start..filename_start + 4].copy_from_slice(&[0xFF, 0xFE, 0xFD, 0xFC]);
    let result = MetadataFrame::from_bytes(&bytes);
    assert!(
        matches!(result, Err(ProtocolError::InvalidFilename)),
        "expected InvalidFilename, got {result:?}"
    );
}

#[test]
fn garbage_after_required_size_is_ignored() {
    // A v3 frame is META_SIZE_V3 (85) bytes. Tail bytes shouldn't affect parsing.
    let mut bytes = valid_v3_frame();
    bytes.extend(vec![0xFF; 1024]);
    assert_eq!(bytes.len(), META_SIZE_V3 + 1024);
    let parsed = MetadataFrame::from_bytes(&bytes).expect("trailing bytes must not break parsing");
    assert_eq!(parsed.version, 3);
}

#[test]
fn fuzz_random_bytes_never_panics() {
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    let mut rng = ChaCha8Rng::seed_from_u64(0x412346f9);
    for _ in 0..2_000 {
        let len = rng.random_range(0..256);
        let mut bytes = vec![0u8; len];
        rng.fill(&mut bytes[..]);
        // Result is allowed to be Ok or Err — we just require no panic.
        let _ = MetadataFrame::from_bytes(&bytes);
    }
}

#[test]
fn fuzz_with_valid_magic_never_panics() {
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha8Rng;
    let mut rng = ChaCha8Rng::seed_from_u64(0xc00ffee0);
    // Inputs that look more "valid" — same magic, random body. This is the
    // fuzz path that exercises the deepest parser code paths.
    for _ in 0..2_000 {
        let len = rng.random_range(META_SIZE_V2..META_SIZE_V3 + 64);
        let mut bytes = vec![0u8; len];
        rng.fill(&mut bytes[..]);
        bytes[..4].copy_from_slice(MAGIC);
        let _ = MetadataFrame::from_bytes(&bytes);
    }
}

#[test]
fn ingest_droplet_too_short_does_not_panic() {
    use afterimage_core::session::RecvSession;
    let mut recv = RecvSession::new("password");
    // Random short slices — must not panic, must return Ok(false) or Err.
    for len in 0..(afterimage_core::HEADER_SIZE + afterimage_core::BLOCK_SIZE) {
        let frame = vec![0xAB; len];
        let _: Result<bool, AfterImageError> = recv.ingest_frame(&frame);
    }
}
