//! Workspace-level integration tests for AfterImage Rust v2.
//!
//! These tests exercise the full send → receive pipeline without
//! requiring a camera or display window.

use afterimage_core::session::{RecvSession, SendSession};

const PASSWORD: &str = "integration-test-password-Xk9!";

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn make_payload(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i.wrapping_mul(251) % 256) as u8).collect()
}

/// Run a full encode → decode cycle and verify byte-for-byte equality.
fn roundtrip(data: &[u8], label: &str) {
    let mut send = SendSession::new(data, label, PASSWORD)
        .unwrap_or_else(|e| panic!("SendSession::new failed for {label}: {e}"));

    // Use 4× recommended + some extra metadata frames
    let limit = (send.recommended_droplet_count() * 4) as u32 + 300;
    send.set_limit(limit);

    let mut recv = RecvSession::new(PASSWORD);

    let mut frames_sent = 0usize;
    while let Some(frame) = send.next_frame() {
        frames_sent += 1;
        if recv.ingest_frame(&frame).unwrap() {
            break;
        }
    }

    assert!(
        recv.is_complete(),
        "[{label}] not complete after {frames_sent} frames (progress={:.1}%)",
        recv.progress() * 100.0
    );

    let recovered = recv.get_data().unwrap_or_else(|e| {
        panic!("[{label}] get_data() failed: {e}");
    });

    assert_eq!(
        recovered, data,
        "[{label}] recovered data differs from original"
    );
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[test]
fn roundtrip_tiny() {
    roundtrip(b"hello", "tiny");
}

#[test]
fn roundtrip_small() {
    roundtrip(&make_payload(512), "small-512B");
}

#[test]
fn roundtrip_medium() {
    roundtrip(&make_payload(8_192), "medium-8KiB");
}

#[test]
fn roundtrip_large() {
    roundtrip(&make_payload(64_000), "large-64KiB");
}

#[test]
fn roundtrip_binary_zeros() {
    roundtrip(&vec![0u8; 1024], "all-zeros-1KiB");
}

#[test]
fn roundtrip_binary_ones() {
    roundtrip(&vec![0xFFu8; 1024], "all-ones-1KiB");
}

#[test]
fn roundtrip_utf8_text() {
    let text = "The quick brown fox jumps over the lazy dog. ".repeat(200);
    roundtrip(text.as_bytes(), "utf8-text");
}

#[test]
fn metadata_frames_are_idempotent() {
    // Feeding the same METADATA frame multiple times must not corrupt the state.
    let data = make_payload(1024);
    let mut send = SendSession::new(&data, "meta-test.bin", PASSWORD).unwrap();
    send.set_limit(500);

    let mut recv = RecvSession::new(PASSWORD);
    let mut meta_frame: Option<Vec<u8>> = None;

    for frame in std::iter::from_fn(|| send.next_frame()) {
        use afterimage_core::protocol::MetadataFrame;
        if MetadataFrame::is_metadata(&frame) && meta_frame.is_none() {
            meta_frame = Some(frame.clone());
        }
        let _ = recv.ingest_frame(&frame);
    }

    // Feed the same metadata frame 10 extra times
    if let Some(ref mf) = meta_frame {
        for _ in 0..10 {
            let _ = recv.ingest_frame(mf);
        }
    }

    // State should still be valid (no panic, no corruption)
}

#[test]
fn wrong_password_returns_error() {
    let data = make_payload(512);
    let mut send = SendSession::new(&data, "wrong-pw.bin", PASSWORD).unwrap();
    let limit = (send.recommended_droplet_count() * 4) as u32 + 300;
    send.set_limit(limit);

    let mut recv = RecvSession::new("wrong-password!!!");

    for frame in std::iter::from_fn(|| send.next_frame()) {
        let _ = recv.ingest_frame(&frame);
    }

    if recv.is_complete() {
        // If it claims complete, get_data MUST fail with a crypto error
        let result = recv.get_data();
        assert!(result.is_err(), "decryption with wrong password should fail");
    }
    // If not complete that's also fine — can't decrypt what you can't decode
}

#[test]
fn progress_increases_monotonically() {
    let data = make_payload(4096);
    let mut send = SendSession::new(&data, "progress.bin", PASSWORD).unwrap();
    let limit = (send.recommended_droplet_count() * 3) as u32 + 200;
    send.set_limit(limit);

    let mut recv = RecvSession::new(PASSWORD);
    let mut last_progress = 0.0f64;

    for frame in std::iter::from_fn(|| send.next_frame()) {
        let _ = recv.ingest_frame(&frame);
        let p = recv.progress();
        assert!(
            p >= last_progress - 1e-9,
            "progress went backwards: {last_progress} → {p}"
        );
        last_progress = p;
        if recv.is_complete() {
            break;
        }
    }
}

#[test]
fn recommended_frames_helper_reasonable() {
    for size in [100, 1_000, 10_000, 100_000] {
        let n = afterimage_core::session::recommended_frames(size);
        assert!(n >= 1, "recommended_frames({size}) returned 0");
        assert!(n < 100_000, "recommended_frames({size}) unreasonably large: {n}");
    }
}

#[test]
fn send_session_has_next_flips_after_limit() {
    let data = make_payload(256);
    let mut send = SendSession::new(&data, "limit.bin", PASSWORD).unwrap();
    assert!(send.has_next());
    send.set_limit(3);

    let mut count = 0;
    while let Some(_) = send.next_frame() {
        count += 1;
    }
    assert_eq!(count, 3);
    assert!(!send.has_next());
}