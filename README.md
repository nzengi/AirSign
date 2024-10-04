# AirSign

Air-gapped transaction signing for Solana using animated QR codes.

The private key never touches an internet-connected machine. AirSign turns any
camera-equipped laptop into a hardware wallet without requiring USB, Bluetooth,
or any other data channel.

---

## How it works

```
Online machine                        Air-gapped machine
─────────────────                     ──────────────────
Build transaction
  │
  ▼
Encrypt + fountain-encode
  │
  ▼
Animate as QR stream  ──── camera ──▶  Decode QR stream
                                         │
                                         ▼
                                       Decrypt
                                         │
                                         ▼
                                       Show tx summary
                                         │
                                         ▼
                                       Sign with keypair
                                         │
                                         ▼
Broadcast to Solana  ◀── camera ────  Animate signed tx
```

The transfer layer uses [fountain codes](https://en.wikipedia.org/wiki/Fountain_code)
(a rateless erasure code) so dropped or blurry frames are recovered
automatically — no retransmission protocol needed.

---

## Crates

| Crate | Description |
|-------|-------------|
| `airsign-core` | Protocol, fountain coding, ChaCha20-Poly1305 encryption |
| `airsign-optical` | QR encode/decode, camera capture (nokhwa), display (minifb) |
| `airsign-solana` | Solana transaction signing logic |
| `airsign-wasm` | Browser WebAssembly bindings |
| `airsign` | CLI binary (`airsign send / recv / sign / bench`) |

---

## Quick start

### Prerequisites

- Rust 1.75+ (nightly recommended for WASM builds)
- Two machines: one online, one permanently offline
- A webcam on each machine

### Build

```bash
git clone https://github.com/nzengi/AirSign.git
cd AirSign
cargo build --release
```

The `airsign` binary ends up at `target/release/airsign`.

### Sign a Solana transaction (air-gapped)

**On the online machine** — build and broadcast the unsigned transaction, then
stream it to the air-gapped machine:

```bash
# Stream an unsigned transaction file as QR codes
airsign send unsigned_tx.bin --fps 8
```

**On the air-gapped machine** — receive, review, and sign:

```bash
# Read QR stream from camera, sign, stream response back
airsign sign --keypair ~/.config/solana/id.json
```

**On the online machine** — receive the signed transaction and broadcast:

```bash
airsign recv signed_tx.bin
# then: solana transaction broadcast signed_tx.bin
```

### Benchmark (no hardware needed)

```bash
cargo run --release -- bench /path/to/file
```

---

## Security model

- **ChaCha20-Poly1305** authenticated encryption (256-bit key)
- Key derived via **Argon2id** (memory-hard, 64 MB, 3 iterations)
- **BLAKE3** content hash verified on receipt before decryption
- The QR channel is one-way optical; no TCP/IP stack involved
- Air-gapped machine needs no network drivers loaded at all

The password is the only shared secret. Use a strong, random passphrase.

---

## Running tests

```bash
cargo test
```

All crates include unit tests and integration tests. The `airsign-solana`
tests run a full sign-request roundtrip against a simulated keypair without
needing a live cluster.

---

## WASM / browser

```bash
cargo install wasm-pack
wasm-pack build crates/airsign-wasm --target web --release
```

The resulting `pkg/` directory can be imported directly into any modern
JavaScript bundler.

---

## Contributing

Pull requests are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for
conventions. Please open an issue before starting large changes.

---

## License

MIT — see [LICENSE](LICENSE).