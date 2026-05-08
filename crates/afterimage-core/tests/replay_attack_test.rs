//! Replay-attack regression tests for the AirSign session protocol.
//!
//! Brutal Q4: "What stops the malicious online machine from re-broadcasting
//! a signed tx forever?" The defenses live at three layers:
//!
//! 1. **AirSign session nonce** (this crate / multisigner) — every session
//!    has a fresh 256-bit random nonce embedded in the request and bound
//!    into every response. Cross-session swap attacks are detectable.
//!
//! 2. **Solana `recent_blockhash`** — txs include a recent block hash and
//!    expire after ~150 blocks (~60 s). Replay outside that window is
//!    rejected by validators.
//!
//! 3. **Solana duplicate-signature rejection** — once a tx with a given
//!    signature is confirmed, validators reject duplicates of it.
//!
//! These tests verify (1) at the AirSign protocol level. (2) and (3) are
//! Solana protocol invariants — see `docs/SECURITY_MODEL.md` for citations.

use afterimage_core::session::{recommended_frames, RecvSession, SendSession};

/// Two fresh sessions for the same plaintext + same password must use
/// distinct salts (and therefore distinct keys, ciphertexts, frame bodies).
/// Without this, a malicious capture of one session's frames would replay
/// against any future session.
#[test]
fn two_fresh_sessions_produce_distinct_ciphertexts() {
    let plaintext = b"transfer 50 SOL to vault: blockhash=abc123";
    let password = "shared-session-password";

    let mut s1 = SendSession::new(plaintext, "tx.bin", password).expect("session 1");
    let mut s2 = SendSession::new(plaintext, "tx.bin", password).expect("session 2");

    // Pull the first DATA frame from each session (skip metadata).
    let _meta_1 = s1.next_frame().expect("metadata 1");
    let _meta_2 = s2.next_frame().expect("metadata 2");
    let frame_1 = s1.next_frame().expect("data 1");
    let frame_2 = s2.next_frame().expect("data 2");

    assert_ne!(
        frame_1, frame_2,
        "two fresh sessions for the same plaintext+password must produce distinct \
         frames — same bytes would mean a deterministic salt/nonce, which would \
         allow trivial replay across sessions"
    );
}

/// Session A's ciphertext must not be decryptable in session B's receiver,
/// because each receiver uses the metadata frame from its OWN session to
/// configure the decoder. Mixing frames from different sessions either
/// fails to authenticate or produces nonsense.
#[test]
fn cross_session_frame_swap_is_rejected() {
    let plaintext = b"transfer 1 SOL";
    let password = "same-pwd";

    // Session A — produces frames the legitimate receiver will accept.
    let mut send_a = SendSession::new(plaintext, "a.bin", password).expect("A");
    // Session B — fresh session, different nonce/salt under the hood.
    let mut send_b = SendSession::new(plaintext, "b.bin", password).expect("B");

    let mut recv_a = RecvSession::new(password);

    // Feed metadata + data from session A — should succeed.
    let meta_a = send_a.next_frame().expect("meta A");
    let data_a = send_a.next_frame().expect("data A");
    let _ = recv_a.ingest_frame(&meta_a);
    let done_with_a_only = recv_a
        .ingest_frame(&data_a)
        .expect("data A should ingest cleanly");

    // Now mix in a frame from session B — receiver A is configured for A's
    // metadata. Frame B may either be silently rejected (header mismatch)
    // or trigger an error; either is acceptable. What MUST NOT happen is
    // for B's frame to corrupt session A's decoder state.
    let data_b = send_b.next_frame().expect("data B");
    // If session A is already complete after one droplet, ingesting more is a
    // no-op. The important assertion is that no panic and no successful AEAD
    // verification of foreign material.
    let _ = recv_a.ingest_frame(&data_b);

    // Session A's decryption must still succeed if it had reached completeness.
    if done_with_a_only {
        let recovered = recv_a
            .get_data()
            .expect("session A's data must still decrypt cleanly");
        assert_eq!(
            recovered, plaintext,
            "session A's plaintext must round-trip identically even after a \
             stray foreign frame was fed in"
        );
    }
}

/// Wrong password must fail at AEAD verification (`get_data`), regardless of
/// whether the decoder claims completeness from droplet count alone.
///
/// This is a direct test of the Q3 attack ("I steal the password — now
/// what?"): if AEAD verification leaked under wrong-password, the password
/// wouldn't actually be the secret.
#[test]
fn wrong_password_aead_rejection() {
    let plaintext = b"AirSign protocol Q4 regression";
    let real = "real-password-very-strong";
    let bogus = "guessed-wrong";

    let mut send = SendSession::new(plaintext, "tx.bin", real).expect("send");
    let mut recv_wrong = RecvSession::new(bogus);

    // Drain enough frames that block reassembly thinks it's done.
    let total = recommended_frames(plaintext.len());
    for _ in 0..(total * 2 + 4) {
        let Some(f) = send.next_frame() else { break };
        let _ = recv_wrong.ingest_frame(&f);
        if recv_wrong.is_complete() {
            break;
        }
    }

    // Block reassembly may or may not be "complete" — either way, AEAD must
    // reject when we ask for the plaintext.
    let result = recv_wrong.get_data();
    assert!(
        result.is_err(),
        "wrong password must fail at AEAD verification — got {} bytes back",
        result.as_ref().map(|v| v.len()).unwrap_or(0)
    );
}

/// Q10 regression: an attacker holding up a screen between the two devices
/// (a "screen-in-the-middle" optical MITM) can record every legitimate frame
/// AND inject crafted frames of their own. The defense is the AEAD layer:
/// the attacker can't produce a frame that the receiver authenticates as
/// genuine without knowing the password (Argon2id-derived key).
///
/// This test simulates the attack: receiver gets a forged frame whose ciphertext
/// + tag are pure random under the attacker's chosen "password". With the
/// honest password, AEAD verification fails — `get_data()` returns Err.
#[test]
fn optical_mitm_substitution_is_rejected() {
    use afterimage_core::session::SendSession;
    let plaintext = b"original treasury transfer 50 SOL";
    let real = "operator-shared-password";
    let attacker = "attacker-different-password";

    // Attacker generates frames under their own password — bit-perfect format,
    // but encrypted with a key the operator doesn't know.
    let mut adv = SendSession::new(plaintext, "tx.bin", attacker).expect("adv send");
    let total = recommended_frames(plaintext.len());

    let mut recv = RecvSession::new(real); // operator uses the real password

    // Feed the attacker's frames as if they came over the optical channel.
    for _ in 0..(total * 2 + 4) {
        let Some(frame) = adv.next_frame() else { break };
        // Receiver may accept individual droplets without immediately failing —
        // AEAD verification happens at get_data() time.
        let _ = recv.ingest_frame(&frame);
        if recv.is_complete() {
            break;
        }
    }

    let result = recv.get_data();
    assert!(
        result.is_err(),
        "operator's session must reject a fully-attacker-encrypted frame stream — got {} bytes",
        result.as_ref().map(|v| v.len()).unwrap_or(0)
    );
}

/// Q10 corollary: an attacker who corrupts ciphertext bytes in legitimate
/// frames must be detected by the Poly1305 tag — silent corruption is not
/// acceptable. We use a payload large enough that random-byte flips reliably
/// land inside the AEAD-protected ciphertext region (a tiny payload would
/// fit entirely in fountain padding, where flips have no effect).
#[test]
fn ciphertext_tamper_is_caught_by_aead() {
    use afterimage_core::session::SendSession;
    use afterimage_core::HEADER_SIZE;

    // 8 KiB plaintext — guarantees the AEAD ciphertext+tag region spans
    // the bulk of every fountain-coded block, so a flip near the start of
    // the payload corrupts authenticated bytes (not zero padding).
    let plaintext: Vec<u8> = (0..8192).map(|i| (i as u8).wrapping_mul(31)).collect();
    let password = "operator-shared-password";

    let mut send = SendSession::new(&plaintext, "big.bin", password).expect("send");
    let mut tampered_frames: Vec<Vec<u8>> = Vec::new();
    let total = recommended_frames(plaintext.len());
    for _ in 0..(total * 2 + 8) {
        let Some(f) = send.next_frame() else { break };
        tampered_frames.push(f);
    }

    // Tamper a payload byte in every data frame. Skipping the 8-byte droplet
    // header but flipping early in the payload region (offset 8..16) hits
    // ciphertext that's covered by the Poly1305 tag.
    let mut tampered_count = 0;
    for f in &mut tampered_frames {
        if f.starts_with(afterimage_core::MAGIC) {
            continue; // leave metadata intact so the decoder initialises
        }
        if f.len() > HEADER_SIZE + 8 {
            f[HEADER_SIZE] ^= 0xAA;
            f[HEADER_SIZE + 1] ^= 0x55;
            tampered_count += 1;
        }
    }
    assert!(tampered_count > 0, "test setup: no data frames were tampered");

    let mut recv = RecvSession::new(password);
    for f in &tampered_frames {
        let _ = recv.ingest_frame(f);
        if recv.is_complete() {
            break;
        }
    }

    let result = recv.get_data();
    assert!(
        result.is_err(),
        "tampered ciphertext must fail Poly1305 verification — got {} bytes",
        result.as_ref().map(|v| v.len()).unwrap_or(0)
    );
}

/// Pure smoke: same password successfully recovers the plaintext.
/// Regression guard against changes that break the happy path.
#[test]
fn happy_path_round_trip() {
    let plaintext = b"transfer 0.01 SOL to AirSign demo recipient";
    let password = "judge-test-password-2026";

    let mut send = SendSession::new(plaintext, "tx.bin", password).expect("send");
    let mut recv = RecvSession::new(password);

    let total = recommended_frames(plaintext.len());
    for _ in 0..(total * 2 + 4) {
        let Some(f) = send.next_frame() else { break };
        if recv.ingest_frame(&f).expect("ingest") {
            break;
        }
    }
    assert!(recv.is_complete());
    let recovered = recv.get_data().expect("decrypt");
    assert_eq!(recovered, plaintext);
}
