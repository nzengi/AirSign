//! afterimage / airsign — CLI binary
//! ===================================
//! Air-gap file transfer and Solana transaction signing via animated QR codes.
//!
//! ## Sub-commands
//!
//! ```text
//! airsign send      <FILE>  [--fps N] [--window-size PX]
//! airsign recv      <OUTPUT> [--camera-index N]
//! airsign bench     <FILE>
//! airsign sign      <REQUEST_FILE> --keypair <PATH> [--output PATH] [--yes]
//! airsign broadcast <RESPONSE_FILE> [--cluster devnet|mainnet|testnet]
//! ```

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

use afterimage_core::session::{RecvSession, SendSession};
use afterimage_solana::{
    broadcaster::Broadcaster,
    signer::{AirSigner, summarize_request, default_nonce_store_path},
    SignRequest,
};

// ─── CLI definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name  = "airsign",
    about = "Air-gapped Solana transaction signing and file transfer via QR codes",
    version,
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Encrypt a file and transmit it as an animated QR stream.
    Send {
        /// File to transmit.
        file: PathBuf,

        /// Frames per second (default: 6).
        #[arg(long, default_value_t = 6)]
        fps: u32,

        /// Display window edge size in pixels (default: 600).
        #[arg(long, default_value_t = 600)]
        window_size: usize,

        /// Password (prompted securely if omitted).
        #[arg(long, env = "AFTERIMAGE_PASSWORD")]
        password: Option<String>,
    },

    /// Receive and decrypt a file from the camera QR stream.
    Recv {
        /// Output file path.
        output: PathBuf,

        /// Camera device index (default: 0).
        #[arg(long, default_value_t = 0)]
        camera_index: u32,

        /// Password (prompted securely if omitted).
        #[arg(long, env = "AFTERIMAGE_PASSWORD")]
        password: Option<String>,
    },

    /// Offline encode + decode benchmark (no camera/display required).
    Bench {
        /// File to benchmark.
        file: PathBuf,

        /// Password (default: "benchmark").
        #[arg(long, default_value = "benchmark")]
        password: String,
    },

    /// Sign a SignRequest JSON file with an air-gapped keypair.
    ///
    /// Reads a decrypted SignRequest JSON file (produced by `airsign recv`),
    /// displays a full transaction summary, prompts for confirmation, and
    /// writes a SignResponse JSON file ready for `airsign send`.
    ///
    /// Keypair format: the standard Solana JSON keypair file
    /// (`~/.config/solana/id.json`) — a JSON array of 64 bytes.
    Sign {
        /// Path to the decrypted SignRequest JSON file.
        #[arg(value_name = "REQUEST_FILE")]
        request_file: PathBuf,

        /// Path to the Solana keypair JSON file (64-byte array).
        #[arg(long, value_name = "PATH", env = "AIRSIGN_KEYPAIR")]
        keypair: PathBuf,

        /// Output file path for the SignResponse JSON.
        #[arg(long, value_name = "PATH", default_value = "sign_response.json")]
        output: PathBuf,

        /// Path to the nonce store (default: ~/.airsign/seen_nonces.json).
        /// Pass an empty string to disable nonce tracking.
        #[arg(long, value_name = "PATH")]
        nonce_store: Option<PathBuf>,

        /// Disable persistent nonce tracking entirely (not recommended).
        #[arg(long)]
        no_nonce_store: bool,

        /// Skip the interactive confirmation prompt (for scripted use).
        #[arg(long)]
        yes: bool,
    },

    /// Broadcast a signed-transaction file (SignResponse JSON) to a Solana cluster.
    Broadcast {
        /// Path to the SignResponse JSON file produced by the air-gapped machine.
        #[arg(value_name = "RESPONSE_FILE")]
        response_file: PathBuf,

        /// Solana cluster URL or shorthand: devnet | mainnet | testnet.
        #[arg(long, default_value = "devnet")]
        cluster: String,
    },
}

// ─── Entry point ─────────────────────────────────────────────────────────────

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Send {
            file,
            fps,
            window_size,
            password,
        } => cmd_send(file, fps, window_size, password),

        Commands::Recv {
            output,
            camera_index,
            password,
        } => cmd_recv(output, camera_index, password),

        Commands::Bench { file, password } => cmd_bench(file, password),

        Commands::Sign {
            request_file,
            keypair,
            output,
            nonce_store,
            no_nonce_store,
            yes,
        } => cmd_sign(request_file, keypair, output, nonce_store, no_nonce_store, yes),

        Commands::Broadcast {
            response_file,
            cluster,
        } => cmd_broadcast(response_file, cluster),
    }
}

// ─── sign ─────────────────────────────────────────────────────────────────────

fn cmd_sign(
    request_file: PathBuf,
    keypair_path: PathBuf,
    output: PathBuf,
    nonce_store_override: Option<PathBuf>,
    no_nonce_store: bool,
    yes: bool,
) {
    // 1. Read the SignRequest JSON
    let request_json = std::fs::read(&request_file).unwrap_or_else(|e| {
        eprintln!("error: cannot read {:?}: {e}", request_file);
        std::process::exit(1);
    });

    // Validate it parses before loading the keypair
    let req = SignRequest::from_json(&request_json).unwrap_or_else(|e| {
        eprintln!("error: {:?} is not a valid SignRequest: {e}", request_file);
        std::process::exit(1);
    });

    // 2. Load the Solana keypair JSON (64-byte array)
    let kp_text = std::fs::read_to_string(&keypair_path).unwrap_or_else(|e| {
        eprintln!("error: cannot read keypair {:?}: {e}", keypair_path);
        std::process::exit(1);
    });
    let kp_bytes: Vec<u8> = serde_json::from_str(&kp_text).unwrap_or_else(|e| {
        eprintln!(
            "error: {:?} is not a valid Solana keypair JSON (expected [u8; 64]): {e}",
            keypair_path
        );
        std::process::exit(1);
    });
    if kp_bytes.len() != 64 {
        eprintln!(
            "error: keypair must be 64 bytes, got {} bytes in {:?}",
            kp_bytes.len(),
            keypair_path
        );
        std::process::exit(1);
    }

    // 3. Build AirSigner with appropriate nonce store
    // Password is not used for local sign_request() calls (only for QR sessions)
    let signer = {
        let base = AirSigner::from_bytes(&kp_bytes, "");
        if no_nonce_store {
            eprintln!("[airsign] ⚠  nonce store disabled — replay protection off");
            base
        } else if let Some(path) = nonce_store_override {
            base.with_nonce_store(path)
        } else {
            match default_nonce_store_path() {
                Some(p) => {
                    eprintln!("[airsign] nonce store: {}", p.display());
                    base.with_nonce_store(p)
                }
                None => {
                    eprintln!("[airsign] ⚠  could not determine HOME, nonce store disabled");
                    base
                }
            }
        }
    };

    // 4. Verify the keypair matches the request's signer_pubkey
    let our_pubkey = signer.pubkey().to_string();
    if our_pubkey != req.signer_pubkey {
        eprintln!(
            "error: keypair pubkey {our_pubkey}\n       does not match request signer {}\n       Wrong keypair file?",
            req.signer_pubkey
        );
        std::process::exit(1);
    }

    // 5. Sign — with or without interactive confirmation
    let response = if yes {
        // Print summary but skip the stdin prompt
        eprintln!("{}", summarize_request(&req));
        eprintln!("\n[airsign] --yes flag set, signing without interactive prompt");
        signer.sign_request(&request_json)
    } else {
        // sign_request_confirmed() prints the summary and asks yes/no
        signer.sign_request_confirmed(&request_json)
    };

    let response = response.unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    // 6. Write the SignResponse JSON
    let response_json = response.to_json().unwrap_or_else(|e| {
        eprintln!("error: failed to serialise response: {e}");
        std::process::exit(1);
    });
    std::fs::write(&output, &response_json).unwrap_or_else(|e| {
        eprintln!("error: cannot write {:?}: {e}", output);
        std::process::exit(1);
    });

    eprintln!(
        "[airsign] ✓ signed — response written to {:?}",
        output
    );
    eprintln!(
        "[airsign]   signature: {}",
        response.signature_b64
    );
    eprintln!("[airsign]   next step: airsign send {:?}", output);
}

// ─── send ─────────────────────────────────────────────────────────────────────

fn cmd_send(file: PathBuf, fps: u32, window_size: usize, password: Option<String>) {
    let data = std::fs::read(&file).unwrap_or_else(|e| {
        eprintln!("error: cannot read {:?}: {e}", file);
        std::process::exit(1);
    });

    let password = resolve_password(password, "Encryption password: ");

    let filename = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("data.bin");

    let mut session = SendSession::new(&data, filename, &password).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let frame_ms = 1000 / fps.max(1);
    let recommended = session.recommended_droplet_count();

    eprintln!(
        "[afterimage] send: {} bytes | ~{} droplets recommended | {fps} fps",
        data.len(),
        recommended
    );

    #[cfg(feature = "display")]
    {
        use afterimage_optical::display::QrDisplay;

        let mut disp = QrDisplay::new("AfterImage — Transmitting", window_size)
            .unwrap_or_else(|e| {
                eprintln!("error opening window: {e}");
                std::process::exit(1);
            });
        disp.frame_ms = frame_ms as u64;

        let count = disp.run_session(&mut session);
        eprintln!("[afterimage] sent {count} frames");
    }

    #[cfg(not(feature = "display"))]
    {
        use afterimage_optical::qr::encode_qr;
        let _ = (frame_ms, window_size);
        eprintln!("[afterimage] display feature not enabled — saving QR PNGs instead");
        let mut i = 0usize;
        while let Some(frame) = session.next_frame() {
            let qr = encode_qr(&frame).unwrap();
            qr.save_png(&format!("frame_{i:05}.png")).unwrap();
            i += 1;
        }
        eprintln!("[afterimage] saved {i} QR PNG files");
    }
}

// ─── recv ─────────────────────────────────────────────────────────────────────

fn cmd_recv(output: PathBuf, camera_index: u32, password: Option<String>) {
    let password = resolve_password(password, "Decryption password: ");

    eprintln!("[afterimage] recv: waiting for QR stream on camera {camera_index}…");

    #[cfg(feature = "camera")]
    {
        use afterimage_optical::camera::CameraReceiver;

        let mut rx = CameraReceiver::open(camera_index, &password).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

        let data = rx.receive().unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });

        std::fs::write(&output, &data).unwrap_or_else(|e| {
            eprintln!("error writing {:?}: {e}", output);
            std::process::exit(1);
        });

        eprintln!(
            "[afterimage] recv: wrote {} bytes to {:?}",
            data.len(),
            output
        );
    }

    #[cfg(not(feature = "camera"))]
    {
        let _ = (output, camera_index, password);
        eprintln!("error: camera feature not enabled; rebuild with --features camera");
        std::process::exit(1);
    }
}

// ─── bench ────────────────────────────────────────────────────────────────────

fn cmd_bench(file: PathBuf, password: String) {
    let data = std::fs::read(&file).unwrap_or_else(|e| {
        eprintln!("error: cannot read {:?}: {e}", file);
        std::process::exit(1);
    });

    let size = data.len();
    eprintln!("[bench] file size: {size} bytes");

    // ── Encode phase ──────────────────────────────────────────────────────
    let t0 = std::time::Instant::now();
    let mut send = SendSession::new(&data, "bench.bin", &password).unwrap_or_else(|e| {
        eprintln!("error: {e}");
        std::process::exit(1);
    });

    let recommended = send.recommended_droplet_count();
    let limit = (recommended * 3) as u32 + 200;
    send.set_limit(limit);

    let frames: Vec<Vec<u8>> = std::iter::from_fn(|| send.next_frame()).collect();
    let encode_ms = t0.elapsed().as_millis();
    eprintln!(
        "[bench] encoded {} frames in {encode_ms} ms ({:.1} MB/s)",
        frames.len(),
        size as f64 / 1e6 / (encode_ms as f64 / 1000.0).max(0.001)
    );

    // ── Decode phase ──────────────────────────────────────────────────────
    let pb = ProgressBar::new(frames.len() as u64);
    pb.set_style(
        ProgressStyle::with_template("[bench] decoding {bar:40} {pos}/{len} frames").unwrap(),
    );

    let t1 = std::time::Instant::now();
    let mut recv = RecvSession::new(&password);
    for frame in &frames {
        pb.inc(1);
        if recv.ingest_frame(frame).unwrap() {
            break;
        }
    }
    pb.finish_and_clear();

    let decode_ms = t1.elapsed().as_millis();

    if recv.is_complete() {
        let recovered = recv.get_data().unwrap();
        if recovered == data {
            eprintln!(
                "[bench] ✓ roundtrip OK in {decode_ms} ms ({:.1} MB/s)",
                size as f64 / 1e6 / (decode_ms as f64 / 1000.0).max(0.001)
            );
        } else {
            eprintln!("[bench] ✗ data mismatch after roundtrip!");
            std::process::exit(2);
        }
    } else {
        eprintln!(
            "[bench] ✗ decoding incomplete after {} frames (progress={:.1}%)",
            frames.len(),
            recv.progress() * 100.0
        );
        std::process::exit(2);
    }
}

// ─── broadcast ────────────────────────────────────────────────────────────────

fn cmd_broadcast(response_file: PathBuf, cluster: String) {
    let json_bytes = std::fs::read(&response_file).unwrap_or_else(|e| {
        eprintln!("error: cannot read {:?}: {e}", response_file);
        std::process::exit(1);
    });

    let rpc_url = match cluster.as_str() {
        "devnet"  => afterimage_solana::broadcaster::DEVNET_URL.to_owned(),
        "testnet" => afterimage_solana::broadcaster::TESTNET_URL.to_owned(),
        "mainnet" => afterimage_solana::broadcaster::MAINNET_URL.to_owned(),
        custom    => custom.to_owned(),
    };

    eprintln!("[airsign] broadcasting to {}…", rpc_url);

    let b = Broadcaster::new(&rpc_url);

    match b.broadcast_response_json(&json_bytes) {
        Ok(sig) => {
            println!("{sig}");
            let cluster_param = match cluster.as_str() {
                "mainnet" => String::new(),
                other => format!("?cluster={other}"),
            };
            eprintln!(
                "[airsign] ✓ confirmed on {}\nhttps://explorer.solana.com/tx/{sig}{cluster_param}",
                b.cluster
            );
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn resolve_password(opt: Option<String>, prompt: &str) -> String {
    if let Some(p) = opt {
        return p;
    }
    rpassword::prompt_password(prompt).unwrap_or_else(|e| {
        eprintln!("error reading password: {e}");
        std::process::exit(1);
    })
}