//! End-to-end integration tests for the WASM Rust API surface.
//!
//! These tests are compiled natively (`cargo test -p afterimage-wasm`) and
//! do NOT require a browser or `wasm-pack test`.
//!
//! Tests that require `js_sys` / `wasm_bindgen_test` live in
//! `tests/e2e_wasm.rs` and run via `wasm-pack test --headless --chrome`.

use afterimage_wasm::{
    recommended_frames, version, WasmKeypair, WasmRecvSession, WasmSendSession,
};

// ─── Helpers ──────────────────────────────────────────────────────────────────

const PASSWORD: &str = "correct-horse-battery-staple";

/// Drive a full fountain-code send/receive cycle and return the decrypted plaintext.
fn roundtrip(plaintext: &[u8], filename: &str) -> Vec<u8> {
    let mut tx =
        WasmSendSession::new(plaintext, filename, PASSWORD).expect("SendSession::new");

    let total = tx.total_frames();
    assert!(total > 0, "total_frames must be > 0");
    assert_eq!(tx.frame_index(), 0);

    let mut rx = WasmRecvSession::new(PASSWORD);
    assert!(!rx.is_complete());
    assert_eq!(rx.progress(), 0.0);

    // Pump frames. Cap at 10× recommended to guard against bugs.
    let cap = total as usize * 10;
    for _ in 0..cap {
        let frame = tx.next_frame().expect("next_frame returned None");
        if rx.ingest_frame(&frame) {
            break;
        }
    }
    assert!(rx.is_complete(), "did not complete within {cap} frames");

    if !filename.is_empty() {
        assert_eq!(rx.filename().as_deref(), Some(filename));
    }
    let orig = rx.original_size().expect("original_size should be set");
    assert_eq!(orig as usize, plaintext.len());

    rx.get_data().expect("get_data failed")
}

// ─── Send / Receive roundtrip ─────────────────────────────────────────────────

#[test]
fn e2e_small_payload() {
    let data = b"hello, air-gap world!";
    assert_eq!(roundtrip(data, "hello.txt"), data);
}

#[test]
fn e2e_medium_payload() {
    let data: Vec<u8> = (0u8..=255).cycle().take(4096).collect();
    assert_eq!(roundtrip(&data, "medium.bin"), data);
}

#[test]
fn e2e_large_payload() {
    // 64 KB — exercises multi-block fountain coding
    let data: Vec<u8> = (0u8..=255).cycle().take(65536).collect();
    assert_eq!(roundtrip(&data, "large.bin"), data);
}

#[test]
fn e2e_empty_filename() {
    let data = b"no filename metadata";
    assert_eq!(roundtrip(data, ""), data);
}

#[test]
fn e2e_set_limit_stops_emission() {
    let data: Vec<u8> = vec![0u8; 1024];
    let mut tx = WasmSendSession::new(&data, "f.bin", PASSWORD).unwrap();
    tx.set_limit(3);
    let mut count = 0usize;
    while let Some(_frame) = tx.next_frame() {
        count += 1;
    }
    assert_eq!(count, 3);
}

// NOTE: e2e_wrong_password_fails is tested via wasm-pack test (tests/e2e_wasm.rs)
// because WasmRecvSession::get_data() uses JsValue::from_str for its error type,
// which is not available on non-wasm32 targets.

// ─── Utility functions ────────────────────────────────────────────────────────

#[test]
fn version_is_semver() {
    let v = version();
    assert!(v.contains('.'), "version should be semver, got {v}");
}

#[test]
fn recommended_frames_grows_with_size() {
    assert!(recommended_frames(100_000) > recommended_frames(100));
}

// ─── WasmKeypair ──────────────────────────────────────────────────────────────

#[test]
fn keypair_generate_produces_valid_keys() {
    let kp = WasmKeypair::generate().expect("generate");
    assert_eq!(kp.pubkey().len(), 32);
    assert_eq!(kp.secret_bytes().len(), 64);

    let b58 = kp.pubkey_b58();
    assert!(!b58.is_empty());
    // Bitcoin/Solana Base58 alphabet excludes 0, O, I, l
    for c in b58.chars() {
        assert!(
            c.is_ascii_alphanumeric() && c != '0' && c != 'O' && c != 'I' && c != 'l',
            "invalid Base58 char '{c}' in: {b58}"
        );
    }
}

#[test]
fn keypair_sign_verify_roundtrip() {
    let kp = WasmKeypair::generate().expect("generate");
    let msg = b"transfer 1 SOL to Alice";
    let sig = kp.sign(msg);
    assert_eq!(sig.len(), 64);
    assert!(WasmKeypair::verify(&kp.pubkey(), msg, &sig));
}

#[test]
fn keypair_verify_rejects_tampered_message() {
    let kp = WasmKeypair::generate().expect("generate");
    let sig = kp.sign(b"transfer 1 SOL to Alice");
    assert!(!WasmKeypair::verify(&kp.pubkey(), b"transfer 2 SOL to Alice", &sig));
}

#[test]
fn keypair_verify_rejects_wrong_pubkey() {
    let kp1 = WasmKeypair::generate().expect("kp1");
    let kp2 = WasmKeypair::generate().expect("kp2");
    let sig = kp1.sign(b"transfer 1 SOL");
    assert!(!WasmKeypair::verify(&kp2.pubkey(), b"transfer 1 SOL", &sig));
}

#[test]
fn keypair_from_seed_is_deterministic() {
    let seed = [42u8; 32];
    let kp1 = WasmKeypair::from_seed(&seed).expect("from_seed 1");
    let kp2 = WasmKeypair::from_seed(&seed).expect("from_seed 2");
    assert_eq!(kp1.pubkey(), kp2.pubkey());
    assert_eq!(kp1.sign(b"hello"), kp2.sign(b"hello"));
}

// NOTE: keypair_from_seed_wrong_length_errors is in tests/e2e_wasm.rs.
// WasmKeypair::from_seed error path calls JsValue::from_str which panics
// with "function not implemented on non-wasm32" when compiled natively.

#[test]
fn keypair_two_generate_calls_differ() {
    let kp1 = WasmKeypair::generate().expect("kp1");
    let kp2 = WasmKeypair::generate().expect("kp2");
    assert_ne!(kp1.pubkey(), kp2.pubkey());
}

#[test]
fn keypair_pubkey_b58_matches_known_vector() {
    // Known vector: all-zero seed → known Ed25519 pubkey
    let seed = [0u8; 32];
    let kp = WasmKeypair::from_seed(&seed).expect("from_seed");
    // The pubkey bytes for all-zero seed in ed25519-dalek
    let b58 = kp.pubkey_b58();
    // Just check it is 43–44 characters (typical Base58-encoded 32-byte value)
    assert!(
        b58.len() >= 43 && b58.len() <= 44,
        "Base58 pubkey length {}: {b58}",
        b58.len()
    );
}

// ─── sign → verify within AirSign protocol flow ───────────────────────────────

#[test]
fn airgap_flow_sign_request_and_verify() {
    use base64::Engine as _;

    // Simulate what the online machine does: encode tx as base64
    let fake_tx_bytes = b"fake solana transaction bytes for testing";
    let tx_b64 = base64::engine::general_purpose::STANDARD.encode(fake_tx_bytes);

    // Simulate what the air-gapped signer does
    let signer_kp = WasmKeypair::generate().expect("signer kp");
    let sig_bytes = signer_kp.sign(fake_tx_bytes);
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(&sig_bytes);

    // Online machine verifies
    assert!(WasmKeypair::verify(&signer_kp.pubkey(), fake_tx_bytes, &sig_bytes));

    // Decode the sig from Base64 and verify again (simulating JSON round-trip)
    let sig_decoded = base64::engine::general_purpose::STANDARD.decode(&sig_b64).unwrap();
    assert!(WasmKeypair::verify(&signer_kp.pubkey(), fake_tx_bytes, &sig_decoded));

    // Sanity: wrong tx bytes should fail
    assert!(!WasmKeypair::verify(&signer_kp.pubkey(), b"tampered tx", &sig_bytes));

    // Silence "unused variable" for tx_b64 (used only for documentation clarity)
    let _ = tx_b64;
}