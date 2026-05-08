//! Latency benchmarks for the AirSign send→recv pipeline.
//!
//! Brutal Q23/Q24: *"What's the latency for a 1 KB transaction?"* and
//! *"How does this scale when the transaction is 4 KB?"*
//!
//! Runs as a `cargo test` so any dev / CI machine can reproduce numbers.
//! For more rigorous statistics use criterion, but for hackathon judging
//! the absolute milliseconds at typical Solana payload sizes are what
//! matter — and those are robust to single-run measurement.
//!
//! Usage:
//!   cargo test -p afterimage-core --test latency_benchmark --release -- --nocapture
//!
//! Output is a markdown-pasteable table the docs reference.

use afterimage_core::session::{recommended_frames, RecvSession, SendSession};
use std::time::Instant;

const PASSWORD: &str = "demo-password-123";

/// One pipeline run: build a session, drain frames into a receiver, decrypt.
/// Returns (encrypt_ms, transmit_drain_ms, decrypt_ms, total_ms, frames_used).
fn run_once(payload: &[u8]) -> (f64, f64, f64, f64, usize) {
    // ── encrypt + initialise sender ─────────────────────────────────────
    let t_total_start = Instant::now();
    let t_enc_start = Instant::now();
    let mut send = SendSession::new(payload, "tx.bin", PASSWORD).expect("send init");
    let enc_ms = t_enc_start.elapsed().as_secs_f64() * 1000.0;

    // ── drain frames into a receiver (simulated optical channel) ────────
    let t_drain_start = Instant::now();
    let mut recv = RecvSession::new(PASSWORD);
    let mut frames = 0;
    let max_frames = recommended_frames(payload.len()) * 4 + 16;
    while frames < max_frames {
        let Some(frame) = send.next_frame() else { break };
        frames += 1;
        if recv.ingest_frame(&frame).expect("ingest") {
            break;
        }
    }
    let drain_ms = t_drain_start.elapsed().as_secs_f64() * 1000.0;

    // ── decrypt ─────────────────────────────────────────────────────────
    let t_dec_start = Instant::now();
    let recovered = recv.get_data().expect("decrypt");
    let dec_ms = t_dec_start.elapsed().as_secs_f64() * 1000.0;

    let total_ms = t_total_start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(recovered.len(), payload.len(), "length mismatch");
    (enc_ms, drain_ms, dec_ms, total_ms, frames)
}

/// Median of N runs (more stable than min/avg under cold-cache jitter).
fn median(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs[xs.len() / 2]
}

#[test]
fn latency_benchmark_typical_solana_sizes() {
    // Sizes spanning the realistic Solana transaction range:
    //  - 200 B: simple SOL transfer (1 instruction, ~165 B serialized)
    //  - 500 B: token transfer with ATA creation (~3 instructions)
    //  - 1 KB:  Squads vote on a 2-instruction proposal
    //  - 2 KB:  Squads multi-instruction batch (3-5 ix)
    //  - 4 KB:  worst-case Squads or complex DEX route
    //  - 8 KB:  pathological — multi-transfer batch with many accounts
    let sizes: &[(&str, usize)] = &[
        ("200 B  (simple SOL transfer)", 200),
        ("500 B  (SPL token transfer)", 500),
        ("1 KB   (Squads vote)", 1_024),
        ("2 KB   (Squads multi-ix)", 2_048),
        ("4 KB   (complex batch)", 4_096),
        ("8 KB   (pathological)", 8_192),
    ];
    let runs_per_size = 5;

    println!();
    println!("=== AirSign Pipeline Latency (release build) ===");
    println!();
    println!("| Size                            | Frames |  Encrypt |  Channel | Decrypt |   Total |");
    println!("|---------------------------------|--------|----------|----------|---------|---------|");

    for (label, size) in sizes {
        let payload: Vec<u8> = (0..*size).map(|i| (i as u8).wrapping_mul(17)).collect();
        let mut enc_runs = Vec::with_capacity(runs_per_size);
        let mut drain_runs = Vec::with_capacity(runs_per_size);
        let mut dec_runs = Vec::with_capacity(runs_per_size);
        let mut total_runs = Vec::with_capacity(runs_per_size);
        let mut frames = 0usize;
        for _ in 0..runs_per_size {
            let (e, dr, dc, tot, f) = run_once(&payload);
            enc_runs.push(e);
            drain_runs.push(dr);
            dec_runs.push(dc);
            total_runs.push(tot);
            frames = f;
        }
        println!(
            "| {label:<31} | {frames:>5}  | {:>5.1} ms | {:>5.1} ms | {:>4.1} ms | {:>4.1} ms |",
            median(enc_runs),
            median(drain_runs),
            median(dec_runs),
            median(total_runs),
        );
    }
    println!();
    println!("Notes:");
    println!("• Encrypt = Argon2id KDF (default OWASP-2024 params: m=64MiB, t=3, p=1) + ChaCha20-Poly1305 + LT encoder init");
    println!("• Channel = drain enough fountain droplets for decoder to converge");
    println!("• Decrypt = AEAD verification + decompression");
    println!("• Most of the wall time is Argon2id KDF (memory-hard by design)");
    println!("• Numbers are medians of {runs_per_size} runs on the host CPU; mobile is ~2-3× slower");
    println!();
}
