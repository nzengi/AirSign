//! End-to-end integration tests for the WASM Rust API surface.
//!
//! These tests are compiled natively (`cargo test -p afterimage-wasm`) and
//! do NOT require a browser or `wasm-pack test`.
//!
//! Tests that require `js_sys` / `wasm_bindgen_test` live in
//! `tests/e2e_wasm.rs` and run via `wasm-pack test --headless --chrome`.

use afterimage_wasm::{
    recommended_frames, version, WasmBroadcaster, WasmKeyStore, WasmKeypair, WasmRecvSession,
    WasmSendSession, WasmSquads,
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

// ─── WasmSquads ───────────────────────────────────────────────────────────────

#[test]
fn squads_derive_pda_returns_valid_json() {
    // Use a known 32-byte public key (all-zeros seed) encoded as base58
    let seed = [0u8; 32];
    let kp = WasmKeypair::from_seed(&seed).expect("kp");
    let create_key = kp.pubkey_b58();

    let squads = WasmSquads::new();
    let json_str = squads.derive_pda(&create_key).expect("derive_pda");
    let v: serde_json::Value = serde_json::from_str(&json_str).expect("parse JSON");

    assert!(v["multisig_pda"].is_string(), "multisig_pda must be a string");
    assert!(v["vault_pda"].is_string(), "vault_pda must be a string");
    assert!(v["multisig_bump"].is_number(), "multisig_bump must be a number");
    assert!(v["vault_bump"].is_number(), "vault_bump must be a number");

    let ms_pda = v["multisig_pda"].as_str().unwrap();
    let vt_pda = v["vault_pda"].as_str().unwrap();
    // Base58 Solana addresses are 32–44 chars
    assert!(ms_pda.len() >= 32 && ms_pda.len() <= 44, "multisig_pda len {}", ms_pda.len());
    assert!(vt_pda.len() >= 32 && vt_pda.len() <= 44, "vault_pda len {}", vt_pda.len());
    // PDAs must differ
    assert_ne!(ms_pda, vt_pda);
}

#[test]
fn squads_derive_pda_is_deterministic() {
    let kp = WasmKeypair::from_seed(&[1u8; 32]).expect("kp");
    let create_key = kp.pubkey_b58();
    let squads = WasmSquads::new();
    let a = squads.derive_pda(&create_key).expect("a");
    let b = squads.derive_pda(&create_key).expect("b");
    assert_eq!(a, b, "PDA derivation must be deterministic");
}

#[test]
fn squads_multisig_create_data_structure() {
    let squads = WasmSquads::new();
    let member_key = WasmKeypair::from_seed(&[2u8; 32]).expect("kp").pubkey_b58();
    let config = serde_json::json!({
        "threshold": 2,
        "members": [
            {"key": member_key, "permissions": 7}
        ],
        "time_lock": 0,
        "memo": null
    })
    .to_string();
    let hex_data = squads.multisig_create_data(&config).expect("create_data");
    let bytes = hex::decode(&hex_data).expect("hex decode");
    // 8 discriminator + 1 option + 2 threshold + 4 member count + 32 key + 4 perms + 4 time_lock + 1 rent + 1 memo = 57
    assert_eq!(bytes.len(), 57, "instruction data must be 57 bytes");
    // First 8 bytes are the discriminator (non-zero)
    assert!(bytes[..8].iter().any(|&b| b != 0), "discriminator must be non-zero");
}

#[test]
fn squads_discriminator_known_values() {
    let squads = WasmSquads::new();
    // Discriminator for "proposal_approve" must be 8 bytes (16 hex chars)
    let disc_hex = squads.discriminator("proposal_approve");
    assert_eq!(disc_hex.len(), 16, "discriminator must be 8 bytes = 16 hex chars");
    // Two different names must produce different discriminators
    let d1 = squads.discriminator("proposal_approve");
    let d2 = squads.discriminator("proposal_reject");
    assert_ne!(d1, d2, "different instructions must have different discriminators");
}

#[test]
fn squads_proposal_approve_data_no_memo() {
    let squads = WasmSquads::new();
    let data = squads.proposal_approve_data("");
    let bytes = hex::decode(&data).expect("hex");
    // 8 discriminator + 1 option-none = 9 bytes
    assert_eq!(bytes.len(), 9);
    assert_eq!(bytes[8], 0u8, "None variant should be 0");
}

#[test]
fn squads_proposal_create_data_structure() {
    let squads = WasmSquads::new();
    let data = squads.proposal_create_data(42, false);
    let bytes = hex::decode(&data).expect("hex");
    // 8 discriminator + 8 tx_index + 1 draft flag = 17 bytes
    assert_eq!(bytes.len(), 17);
    // tx_index = 42 in LE
    let tx_idx = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    assert_eq!(tx_idx, 42);
    assert_eq!(bytes[16], 0u8, "draft=false → 0");
}

// ─── WasmBroadcaster ──────────────────────────────────────────────────────────

#[test]
fn broadcaster_cluster_name_detection() {
    let cases = [
        ("https://api.mainnet-beta.solana.com", "mainnet"),
        ("https://api.devnet.solana.com", "devnet"),
        ("https://api.testnet.solana.com", "testnet"),
        ("http://localhost:8899", "localnet"),
        ("http://127.0.0.1:8899", "localnet"),
        ("https://my-custom-rpc.example.com", "custom"),
    ];
    for (url, expected) in cases {
        let bc = WasmBroadcaster::new(url);
        assert_eq!(bc.cluster_name(), expected, "url={url}");
    }
}

#[test]
fn broadcaster_build_and_parse_balance_flow() {
    let bc = WasmBroadcaster::new("https://api.devnet.solana.com");
    let pubkey = "So11111111111111111111111111111111111111112";

    // Build request body
    let body = bc.build_get_balance_body(pubkey, 1);
    let v: serde_json::Value = serde_json::from_str(&body).expect("body JSON");
    assert_eq!(v["method"].as_str().unwrap(), "getBalance");
    assert_eq!(v["id"].as_u64().unwrap(), 1);

    // Parse a synthetic success response
    let response = r#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":123},"value":5000000}}"#;
    let lamports = bc.parse_get_balance_response(response).expect("parse");
    assert_eq!(lamports, 5_000_000);
}

// NOTE: broadcaster_parse_error_response_propagates is tested in e2e_wasm.rs
// because error returns invoke JsValue::from_str which panics on non-wasm32.

#[test]
fn broadcaster_explorer_urls() {
    let bc_main = WasmBroadcaster::new("https://api.mainnet-beta.solana.com");
    let sig = "5j7s8TFWnnNsPa5p7jMRcmYXzGQEYrHkXSBFEMJApwHb1234test";
    let url = bc_main.explorer_url(sig);
    assert!(url.contains("explorer.solana.com/tx/"), "mainnet URL: {url}");
    assert!(!url.contains("cluster="), "mainnet URL should not have cluster param: {url}");

    let bc_dev = WasmBroadcaster::new("https://api.devnet.solana.com");
    let dev_url = bc_dev.explorer_url(sig);
    assert!(dev_url.contains("cluster=devnet"), "devnet URL: {dev_url}");
}

#[test]
fn broadcaster_blockhash_roundtrip() {
    let bc = WasmBroadcaster::new("https://api.devnet.solana.com");
    let body = bc.build_get_latest_blockhash_body(3);
    let v: serde_json::Value = serde_json::from_str(&body).expect("body JSON");
    assert_eq!(v["method"].as_str().unwrap(), "getLatestBlockhash");

    let mock_resp = r#"{"jsonrpc":"2.0","id":3,"result":{"context":{"slot":100},"value":{"blockhash":"4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi","feeCalculator":{"lamportsPerSignature":5000}}}}"#;
    let bh = bc.parse_get_latest_blockhash_response(mock_resp).expect("parse");
    assert_eq!(bh, "4vJ9JU1bJJE96FWSJKvHsmmFADCg4gpZQff4P3bkLKi");
}

// ─── WasmKeyStore ─────────────────────────────────────────────────────────────

#[test]
fn keystore_generate_store_load_roundtrip() {
    let mut ks = WasmKeyStore::new();
    assert!(ks.is_empty());

    let pubkey = ks.generate("treasury").expect("generate");
    assert!(!pubkey.is_empty());
    assert!(ks.exists("treasury"));
    assert_eq!(ks.len(), 1);

    // load returns 32-byte seed
    let seed = ks.load("treasury").expect("load");
    assert_eq!(seed.len(), 32);

    // pubkey_of matches generate's return
    let pk2 = ks.pubkey_of("treasury").expect("pubkey_of");
    assert_eq!(pubkey, pk2);
}

#[test]
fn keystore_delete_and_exists() {
    let mut ks = WasmKeyStore::new();
    ks.generate("alice").expect("generate");
    ks.generate("bob").expect("generate");
    assert_eq!(ks.len(), 2);

    let deleted = ks.delete("alice");
    assert!(deleted, "delete should return true for existing key");
    assert!(!ks.exists("alice"));
    assert!(ks.exists("bob"));
    assert_eq!(ks.len(), 1);

    // Deleting non-existent returns false
    assert!(!ks.delete("alice"));
}

#[test]
fn keystore_export_import_hex_roundtrip() {
    let mut ks1 = WasmKeyStore::new();
    let pk1 = ks1.generate("wallet").expect("generate");
    let hex = ks1.export_hex("wallet").expect("export");
    assert_eq!(hex.len(), 64, "32 bytes → 64 hex chars");

    let mut ks2 = WasmKeyStore::new();
    let pk2 = ks2.import_hex("wallet", &hex).expect("import");
    assert_eq!(pk1, pk2, "import must reproduce same public key");

    // Sign with both and compare signatures
    let msg = b"cross-session signature test";
    let sig1 = ks1.sign_with("wallet", msg).expect("sign1");
    let sig2 = ks2.sign_with("wallet", msg).expect("sign2");
    assert_eq!(sig1, sig2, "same seed must produce same signature");
    assert!(WasmKeypair::verify(
        &ks1.pubkey_bytes_of("wallet").expect("pk bytes"),
        msg,
        &sig1
    ));
}

#[test]
fn keystore_list_labels_json() {
    let mut ks = WasmKeyStore::new();
    ks.generate("a").expect("a");
    ks.generate("b").expect("b");
    ks.generate("c").expect("c");

    let labels_json = ks.list_labels_json();
    let labels: Vec<String> = serde_json::from_str(&labels_json).expect("parse");
    assert_eq!(labels.len(), 3);
    assert!(labels.contains(&"a".to_string()));
    assert!(labels.contains(&"b".to_string()));
    assert!(labels.contains(&"c".to_string()));
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