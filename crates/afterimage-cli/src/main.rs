//! afterimage — CLI binary
//! =======================
//! Air-gap file transfer via animated QR codes.
//!
//! ## Sub-commands
//!
//! ```text
//! afterimage send  <FILE> [--fps N] [--window-size PX]
//! afterimage recv  <OUTPUT> [--camera-index N]
//! afterimage bench <FILE>            # offline encode/decode benchmark
//! ```

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use indicatif::{ProgressBar, ProgressStyle};

use afterimage_core::session::{RecvSession, SendSession};

// ─── CLI definition ───────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name  = "afterimage",
    about = "Air-gap file transfer via animated QR codes (Rust v2)",
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
    }
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
        ProgressStyle::with_template("[bench] decoding {bar:40} {pos}/{len} frames")
            .unwrap(),
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