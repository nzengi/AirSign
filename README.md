# AirSign

**Air-gapped Solana transaction signing over an encrypted, fountain-coded QR stream.**

No USB. No Bluetooth. No network cable. Only a camera.

---

## Why AirSign?

### The problem with existing solutions

| Solution | Attack surface |
|---|---|
| **Ledger / Trezor** | USB or BLE connection; firmware must be trusted; supply-chain attacks on firmware updates are well documented |
| **Paper wallet** | Private key must be typed into an online machine to sign — the key is exposed at signing time |
| **Air-gapped laptop + USB drive** | USB ports are attack vectors (BadUSB, firmware implants); many high-security ops forbid USB entirely |
| **AirSign** | **Zero electrical connection.** Private key never leaves the air-gapped machine. Only photons cross the gap. |

### Who actually needs this?

- **Validator operators** — sign reward-withdrawal transactions without keeping the withdrawal keypair on an online machine
- **DAO treasuries** — offline multi-sig approval without flying signers to the same room
- **Exchanges and custodians** — cold-wallet signing on machines with no USB ports (common in hardened datacentres)
- **High-value token holders** — anyone who has ever typed a seed phrase into MetaMask and felt uneasy about it

### What makes AirSign different from "QR code hardware wallets" like Keystone?

Keystone and AirSign share the same physical channel (QR codes), but differ in the cryptographic layer:

- **Encrypted channel** — AirSign wraps every QR frame in ChaCha20-Poly1305 authenticated encryption. A camera pointed at the screen from across a room cannot read the transaction.
- **Fountain coding** — An LT-code fountain encoder means the receiver can reconstruct the data from *any* sufficient subset of frames. Packet loss or a blurry frame does not stall the transfer.
- **No proprietary firmware** — AirSign is a software library (Rust + WASM). Any machine that can run Rust or a modern browser can be the signer.

---

## Architecture

```text
┌──────────────────────────────────┐       ┌──────────────────────────────────┐
│        Online machine            │       │       Air-gapped machine         │
│   (watch-only wallet / dApp)     │       │   (private key, never networked) │
│                                  │       │                                  │
│  1. Build unsigned Transaction   │       │                                  │
│  2. SignRequest { tx, metadata } │       │                                  │
│  3. Argon2id KDF → session key   │       │  Argon2id KDF → same session key │
│  4. ChaCha20-Poly1305 encrypt    │       │                                  │
│  5. LT fountain encode           │       │                                  │
│  6. Animate QR frames ──────────►│──────►│  7. Camera captures QR stream    │
│                                  │       │  8. Fountain decode              │
│                                  │       │  9. Decrypt → SignRequest        │
│                                  │       │ 10. Ed25519 sign                 │
│                                  │       │ 11. Encrypt SignResponse          │
│                                  │       │ 12. Animate QR frames            │
│ 15. Inject signature(s)   ◄──────│◄──────│◄─── 13. Camera captures stream   │
│ 16. send_and_confirm_tx          │       │                                  │
│     → Solana cluster             │       │                                  │
└──────────────────────────────────┘       └──────────────────────────────────┘

Shared secret: a password known to both operators.
Private key:   stays inside the right box forever.
```

---

## Quick start

### Prerequisites

- Rust stable ≥ 1.78
- A webcam on the online machine (or any camera capable of reading QR codes)
- Optionally: a second machine with no network connectivity (the air-gapped signer)

### Build

```bash
git clone https://github.com/nzengi/AirSign.git
cd AirSign
cargo build --release -p airsign          # CLI binary
cargo build --release -p afterimage-solana  # Solana signing library
```

For headless / CI (no camera or display):

```bash
cargo build --release -p airsign --no-default-features
```

### Sign and broadcast a Solana transaction (full flow)

**Online machine** — prepare the unsigned transaction and start the QR stream:

```bash
# Example: sign a simple SOL transfer on devnet
# (your dApp or wallet generates the unsigned tx bytes)
airsign send unsigned_tx.json --fps 8
```

**Air-gapped machine** — receive, sign, and transmit back:

```bash
airsign recv sign_request.json --camera-index 0
# AirSigner loads your keypair from AIRSIGN_KEYPAIR_PATH or prompts
airsign send sign_response.json --fps 8
```

**Online machine** — receive the signature and broadcast:

```bash
airsign recv sign_response.json --camera-index 0
airsign broadcast sign_response.json --cluster devnet
# prints: https://explorer.solana.com/tx/<SIGNATURE>?cluster=devnet
```

### Offline benchmark (no hardware required)

```bash
echo "benchmark payload" > test.bin
airsign bench test.bin
# [bench] ✓ roundtrip OK in 12 ms (1.4 MB/s)
```

---

## Crate structure

| Crate | Purpose |
|---|---|
| `afterimage-core` | Protocol framing, fountain coding, Argon2id KDF, ChaCha20-Poly1305 encryption |
| `afterimage-optical` | QR encode/decode, camera capture, display window |
| `afterimage-solana` | `AirSigner` (Ed25519 signing), `Broadcaster` (RPC submit) |
| `afterimage-wasm` | WASM bindings for browser-based signers |
| `airsign` (CLI) | `send`, `recv`, `bench`, `broadcast` subcommands |

---

## Security model

- **The private key never leaves the air-gapped machine.** The only data transmitted online → offline is the unsigned transaction (not sensitive). The only data transmitted offline → online is the Ed25519 signature and signed transaction bytes.
- **The optical channel is encrypted.** ChaCha20-Poly1305 with a key derived via Argon2id (64 MB, 3 iterations) from a shared password. An observer with a camera recording the QR stream learns nothing without the password.
- **Replay protection.** Each `SignRequest` contains a random 32-byte nonce. The `AirSigner` rejects any `SignResponse` whose nonce does not match.
- **No unsafe code.** `#![forbid(unsafe_code)]` is set on all crates.

### Threat model (what AirSign does *not* protect against)

- A compromised display driver or OS on the online machine that shows a different transaction than the one the user approved.
- Physical compromise of the air-gapped machine itself.
- Password brute-force if the shared secret is weak.

---

## WASM build

```bash
cargo install wasm-pack
wasm-pack build crates/afterimage-wasm --target web
```

The generated `pkg/` directory can be imported directly into any JavaScript/TypeScript project.

---

## Roadmap

| Milestone | Status | Notes |
|---|---|---|
| AirSign v1 core (sign + broadcast) | ✅ Done | `afterimage-solana` crate |
| Persistent nonce store (replay protection) | ✅ Done | `~/.airsign/seen_nonces.json` |
| Terminal tx review + confirmation prompt | ✅ Done | `sign_request_confirmed()` |
| Versioned sign envelopes (v1) | ✅ Done | `SignRequest::version` field |
| `airsign sign` CLI subcommand | ✅ Done | `airsign sign <req.json> --keypair <path> [--yes]` |
| Password hardening (Argon2id tuning) | 🔜 Next | Increase memory cost for mainnet use; configurable via `--argon2-mem` |
| Multi-signature support | 🔜 Planned | Allow M-of-N signers via sequential QR rounds |
| Hardware-backed key storage | 🔜 Planned | Optionally store keypair in OS keychain / TPM |
| External security audit | 🔜 Planned | Independent review of crypto and protocol before v1.0.0 release |
| `crates.io` publish | 🔜 Planned | After audit sign-off |

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: `cargo fmt`, `cargo clippy -- -D warnings`, tests required for new behaviour.

## License

AGPL-3.0-or-later — see [LICENSE](LICENSE).