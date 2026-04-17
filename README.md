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
| `afterimage-solana` | `AirSigner` (Ed25519 signing), `KeyStore` (OS keychain), `LedgerSigner` (HID), `WatchWallet` + `TransactionBuilder` (offline tx construction), `TransactionInspector` (static analysis), `PreflightChecker` (RPC simulation), `Broadcaster` (RPC submit) |
| `afterimage-wasm` | WASM bindings for browser-based signers |
| `airsign` (CLI) | `send`, `recv`, `bench`, `sign`, `inspect`, `prepare`, `broadcast`, `key`, `ledger`, `multisign` subcommands |

---

## Security model

- **The private key never leaves the air-gapped machine.** The only data transmitted online → offline is the unsigned transaction (not sensitive). The only data transmitted offline → online is the Ed25519 signature and signed transaction bytes.
- **The optical channel is encrypted.** ChaCha20-Poly1305 with a key derived via Argon2id from a shared password. An observer with a camera recording the QR stream learns nothing without the password.
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

## Building unsigned transactions (`airsign prepare`)

Use `airsign prepare` to construct an unsigned transaction directly from the
command line — no custom dApp or wallet software required.  The output is a
raw bincode `Transaction` file that can be piped straight into `airsign inspect`
or `airsign send`.

### SOL transfer

```bash
airsign prepare transfer \
  --from 4wTQaBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890AB \
  --to   9xRzAbCdEfGhIjKlMnOpQrStUvWxYZ12345678CD \
  --amount 1.5 \
  --memo "treasury payout Q2-2026" \
  --cluster devnet \
  --out unsigned_tx.bin
```

### SPL Token transfer

ATAs are derived automatically if `--from-ata` / `--to-ata` are omitted.

```bash
airsign prepare token-transfer \
  --from     4wTQaBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890AB \
  --to       9xRzAbCdEfGhIjKlMnOpQrStUvWxYZ12345678CD \
  --mint     EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --amount   1000000 \
  --decimals 6 \
  --cluster  mainnet \
  --out      unsigned_tx.bin
```

### Stake account withdrawal

```bash
# Withdraw 5 SOL
airsign prepare stake-withdraw \
  --from          4wTQaBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890AB \
  --stake-account 3kZjAbCdEfGhIjKlMnOpQrStUvWxYZ1234567890AB \
  --to            9xRzAbCdEfGhIjKlMnOpQrStUvWxYZ12345678CD \
  --amount        5.0 \
  --cluster       mainnet \
  --out           unsigned_tx.bin

# Or withdraw the entire balance
airsign prepare stake-withdraw \
  --from          4wTQaBcDeFgHiJkLmNoPqRsTuVwXyZ1234567890AB \
  --stake-account 3kZjAbCdEfGhIjKlMnOpQrStUvWxYZ1234567890AB \
  --to            9xRzAbCdEfGhIjKlMnOpQrStUvWxYZ12345678CD \
  --amount-all \
  --cluster       mainnet \
  --out           unsigned_tx.bin
```

### Full workflow with `prepare`

```bash
# 1. Build the unsigned tx
airsign prepare transfer --from <ONLINE_PK> --to <DEST_PK> --amount 2.0 --cluster mainnet

# 2. Inspect it before sending
airsign inspect unsigned_tx.bin --cluster mainnet --simulate

# 3. Send over QR to the air-gapped machine
airsign send unsigned_tx.bin --fps 8

# (on air-gapped machine)
airsign recv sign_request.json
airsign sign sign_request.json --keypair ~/.config/solana/id.json
airsign send sign_response.json --fps 8

# 4. Receive signature and broadcast
airsign recv sign_response.json
airsign broadcast sign_response.json --cluster mainnet
```

---

## Transaction inspection and pre-flight

Before signing (or independently of the signing flow), you can inspect any
transaction file and run an optional RPC simulation:

```bash
# Static analysis only (no network required)
airsign inspect unsigned_tx.bin

# Parse a SignRequest JSON and inspect it
airsign inspect sign_request.json

# Static analysis + RPC simulation against devnet
airsign inspect sign_request.json --cluster devnet --simulate
```

### What the inspector checks

The inspector decodes every instruction in the transaction and displays a
structured summary:

| Instruction type | Fields shown |
|---|---|
| System :: Transfer | from, to, amount in SOL |
| SPL Token :: Transfer | source, dest, mint, amount |
| SPL Token :: MintTo | mint, dest, amount |
| SPL Token :: Burn | source, mint, amount |
| SPL Token :: SetAuthority | account, authority type, new authority |
| ATA :: Create | payer, wallet, mint |
| Memo | text content |
| Unknown | program ID, data hex, account count |

Risk flags are raised automatically:

| Flag | Severity | Trigger |
|---|---|---|
| `LARGE_SOL_TRANSFER` | HIGH | any single SOL transfer ≥ 100 SOL |
| `LARGE_TOKEN_TRANSFER` | HIGH | any token transfer ≥ 1 000 000 |
| `UPGRADE_AUTHORITY_CHANGE` | HIGH | `SetAuthority` on an upgrade-authority slot |
| `UNKNOWN_PROGRAM` | MEDIUM | any instruction targeting an unrecognised program |
| `SYSTEM_ACCOUNT_WRITE` | MEDIUM | system accounts (e.g. System Program) in writable position |

The command exits with code **0** if no HIGH risk flags were found, or **2**
if one or more HIGH risk flags are raised — making it easy to gate CI or
scripted signing flows:

```bash
airsign inspect sign_request.json || { echo "HIGH RISK — aborting"; exit 1; }
```

### Pre-flight (RPC simulation + fee estimation)

When `--cluster` is supplied (with or without `--simulate`), `airsign inspect`
also calls the cluster's `getFeeForMessage` RPC method and, if `--simulate` is
set, `simulateTransaction`.  The fee and simulation logs are printed below the
static summary.

```
Pre-flight against https://api.devnet.solana.com
  Fee              : 5000 lamports (0.000005000 SOL)
  Simulation       : ✓ success
  Compute units    : 450
```

---

## Ledger hardware wallet

AirSign supports Ledger devices (Nano S, Nano X, Nano S+, Stax, Flex) as an
alternative signing backend.  The device communicates over USB HID — no
proprietary Ledger Live software is required.

### Prerequisites

1. Connect the Ledger via USB.
2. Unlock the device with your PIN.
3. Open the **Solana** app on the device.
4. On Linux, add a udev rule so the device is accessible without `sudo`:
   ```text
   SUBSYSTEM=="usb", ATTRS{idVendor}=="2c97", MODE="0660", GROUP="plugdev"
   ```

### List connected devices

```bash
airsign ledger list
# [airsign] 1 Ledger device(s) found:
#   [0] Nano X  serial=abc123  path=/dev/hidraw2
```

### Show the Solana app version

```bash
airsign ledger version
# Solana app v1.4.0
```

### Get the public key for a derivation path

```bash
# Default BIP44 path: m/44'/501'/0'/0'
airsign ledger pubkey

# Custom path with on-device confirmation
airsign ledger pubkey --derivation "m/44'/501'/1'/0'" --confirm
# [airsign] please approve on the Ledger display…
# Hx3k…
```

### Sign a transaction with a Ledger

Use the `ledger:` prefix in `--keypair`.  The default path
(`m/44'/501'/0'/0'`) is used when you write `ledger:default`.

```bash
airsign sign request.json --keypair "ledger:m/44'/501'/0'/0'"
# [airsign] Ledger: Nano X (pid=0x0004, serial=abc123)
# [airsign] please approve on the Ledger display…
# [airsign] ✓ signed — response written to sign_response.json
```

---

## Key management (OS keychain)

AirSign v2.2.0 adds native keychain integration so that your Ed25519 signing
keypair never has to live as a plaintext JSON file.

### Generate a new keypair and store it in the OS keychain

```bash
airsign key generate my-mainnet-key
# [airsign] ✓ generated keypair 'my-mainnet-key'
# [airsign]   public key : 4wTQ…
# [airsign]   stored in  : OS keychain (service=airsign, account=my-mainnet-key)
```

### Import an existing Solana CLI keypair file

```bash
airsign key import my-mainnet-key --file ~/.config/solana/id.json
```

### Sign using a keychain key

```bash
# Instead of: --keypair ~/.config/solana/id.json
airsign sign request.json --keypair keychain:my-mainnet-key
```

### Export back to a file (e.g. for use with the Solana CLI)

```bash
airsign key export my-mainnet-key --output /tmp/id.json
```

### List all stored keys

```bash
airsign key list
  my-mainnet-key  →  4wTQ…
  devnet-hot-key  →  9xRz…
```

### Delete a key (irreversible)

```bash
airsign key delete my-mainnet-key
# [airsign] delete 'my-mainnet-key' from OS keychain? This cannot be undone. [y/N]:
```

The keychain service name is always `airsign`; the account name is the label
you supply. On macOS the entry is visible in **Keychain Access.app** under
*Login → Passwords*, filtered by service `airsign`.

---

## Changelog

### v2.2.0 — OS keychain integration

- `afterimage-solana::keystore::KeyStore` — full CRUD for Ed25519 keypairs in
  the platform keychain (macOS / Linux / Windows).
- `airsign key` subcommand: `generate`, `import`, `show`, `list`, `export`,
  `delete`.
- `airsign sign --keypair keychain:<LABEL>` — load signing key directly from
  the OS keychain.
- `KeyStoreError` enum with typed variants.
- 7 unit tests covering the full `KeyStore` lifecycle.

### v2.1.0 — Security profiles

- **`SecurityProfile` enum** (`owasp-2024` / `mainnet` / `paranoid`) with
  pre-tuned Argon2id parameters for every threat level.
- **`--security-profile <PROFILE>`** CLI flag on `airsign send` — mutually
  exclusive with `--argon2-mem` / `--argon2-iter`.
- `airsign send` now prints the active profile and warns when params are below
  the mainnet minimum (256 MiB / t=4).
- `Argon2Params::meets_mainnet_minimum()` and `security_level()` helpers.

```bash
# OWASP 2024 minimum (default — 64 MiB / t=3)
airsign send unsigned_tx.json --fps 8

# Recommended for mainnet-beta (256 MiB / t=4)
airsign send unsigned_tx.json --fps 8 --security-profile mainnet

# Maximum hardening (512 MiB / t=5)
airsign send unsigned_tx.json --fps 8 --security-profile paranoid
```

| Profile | Memory | Iterations | Use case |
|---|---|---|---|
| `owasp-2024` | 64 MiB | t=3 | Devnet / testnet (default) |
| `mainnet` | 256 MiB | t=4 | Mainnet-beta transactions |
| `paranoid` | 512 MiB | t=5 | Extreme-value signing |

### v2.0.0 — Protocol v3 + Configurable Argon2id

- **Protocol v3 frame** (85 bytes): embeds `argon2_m_cost` and `argon2_t_cost`
  directly in the METADATA frame so receivers never need out-of-band KDF config.
- **`SendSession::new_with_argon2_params`** — create a v3 session with custom
  Argon2id parameters.
- **`RecvSession`** auto-reads Argon2 params from the v3 METADATA frame.
- **CLI `airsign send`** gains `--argon2-mem <KiB>` and `--argon2-iter <N>` flags
  (defaults: 64 MiB / 3 iterations — OWASP 2024 minimums).
- **`Argon2Params`** and `META_SIZE_V2` / `META_SIZE_V3` are now public exports.
- Full backward compatibility: v1 and v2 frames decode unchanged.

## Roadmap

| Milestone | Status | Notes |
|---|---|---|
| AirSign v1 core (sign + broadcast) | ✅ Done | `afterimage-solana` crate |
| Persistent nonce store (replay protection) | ✅ Done | `~/.airsign/seen_nonces.json` |
| Terminal tx review + confirmation prompt | ✅ Done | `sign_request_confirmed()` |
| Versioned sign envelopes (v1) | ✅ Done | `SignRequest::version` field |
| `airsign sign` CLI subcommand | ✅ Done | `airsign sign <req.json> --keypair <path> [--yes]` |
| Password hardening (Argon2id tuning) | 🔜 Next | Increase memory cost for mainnet use; configurable via `--argon2-mem` |
| Multi-signature support | ✅ Done | M-of-N sequential QR rounds via `airsign multisign init/sign/next` |
| Hardware-backed key storage | 🔜 Planned | Optionally store keypair in OS keychain / TPM |
| External security audit | 🔜 Planned | Independent review of crypto and protocol before v1.0.0 release |
| `crates.io` publish | 🔜 Planned | After audit sign-off |

---

## M-of-N Multi-Signature Workflow

AirSign supports M-of-N sequential multi-signature sessions.  Each round
travels through the standard encrypted QR channel — no signers need to be
in the same room or online at the same time.

```
 ┌──────────────────────────────────────────────────────────────────────┐
 │                       ONLINE MACHINE                                 │
 │  airsign multisign init tx.bin                                       │
 │    --signers A,B,C --threshold 2 --out round1.json                   │
 │  airsign send round1.json                                            │
 └────────────────────────────┬─────────────────────────────────────────┘
          QR stream (encrypted)│
                               ▼
 ┌──────────────────────────────────────────────────────────────────────┐
 │               AIR-GAPPED MACHINE — Signer A                         │
 │  airsign recv round1.json                                            │
 │  airsign multisign sign round1.json --keypair id.json --out resp1.json│
 │  airsign send resp1.json                                             │
 └────────────────────────────┬─────────────────────────────────────────┘
          QR stream (encrypted)│
                               ▼
 ┌──────────────────────────────────────────────────────────────────────┐
 │                       ONLINE MACHINE                                 │
 │  airsign recv resp1.json                                             │
 │  airsign multisign next resp1.json --request round1.json             │
 │    --out round2.json                                                 │
 │  airsign send round2.json                                            │
 └────────────────────────────┬─────────────────────────────────────────┘
          QR stream (encrypted)│
                               ▼
 ┌──────────────────────────────────────────────────────────────────────┐
 │               AIR-GAPPED MACHINE — Signer B                         │
 │  airsign recv round2.json                                            │
 │  airsign multisign sign round2.json --keypair id.json --out resp2.json│
 │    → complete=true (threshold met)                                   │
 │  airsign send resp2.json                                             │
 └────────────────────────────┬─────────────────────────────────────────┘
          QR stream (encrypted)│
                               ▼
 ┌──────────────────────────────────────────────────────────────────────┐
 │                       ONLINE MACHINE                                 │
 │  airsign recv resp2.json                                             │
 │  airsign broadcast resp2.json --cluster mainnet-beta                 │
 └──────────────────────────────────────────────────────────────────────┘
```

**Security properties:**

- No private key material ever crosses the air gap — only public Ed25519 signature bytes travel in the response.
- Each signer verifies all prior partial signatures before adding its own (chain-of-custody check).
- A 32-byte random nonce embedded at session creation prevents cross-session replay attacks.
- Each round locks the expected signer pubkey; presenting the wrong keypair is rejected immediately.

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). In short: `cargo fmt`, `cargo clippy -- -D warnings`, tests required for new behaviour.

## License

AGPL-3.0-or-later — see [LICENSE](LICENSE).