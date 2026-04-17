# Changelog

All notable changes to AirSign are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning follows [SemVer](https://semver.org/).

---

## [Unreleased]

### Added

- **Transaction Inspector & Pre-flight Checker** (`afterimage-solana`, `airsign` CLI)
  - `crates/afterimage-solana/src/inspector.rs` — `TransactionInspector` with
    static analysis of System Program transfers, SPL Token transfers/mints/burns,
    Associated Token Account creation, Memo instructions, and generic unknown
    programs.  Produces a `TransactionSummary` with per-instruction `InstructionInfo`
    records and a `Vec<RiskFlag>` (upgrade-authority change, large SOL transfer,
    large token transfer, unknown program, write-locked system accounts).
    `TransactionSummary::render()` produces a human-readable, emoji-annotated
    table; `has_high_risk()` returns `true` when any HIGH-severity flag is
    present.  20 unit tests covering all instruction variants and all risk-flag
    triggers, all passing.
  - `crates/afterimage-solana/src/preflight.rs` — `PreflightChecker` performs
    RPC simulation (`simulateTransaction`) and fee estimation
    (`getFeeForMessage`) against any Solana cluster.  `PreflightResult::render()`
    formats the simulation outcome, fee, and log lines.  `resolve_cluster_url()`
    maps `devnet` / `mainnet` / `testnet` shorthands to their canonical RPC
    URLs.  7 unit tests (all passing).
  - `signer.rs` — `summarize_request()` rewritten to delegate to
    `TransactionInspector`, replacing the hand-rolled decoder.  Test assertion
    updated from `"SOL Transfer"` to `"System :: Transfer"` to match the new
    renderer.
  - `lib.rs` — re-exports `inspector` and `preflight` modules.
  - `keystore.rs` — doctest fixed: added `use solana_sdk::signature::Signer as _`
    so the example compiles under strict doctest mode.
  - `airsign inspect <FILE> [--cluster CLUSTER] [--simulate]` — new CLI
    subcommand.  Accepts raw bincode Transactions (`.bin`) or SignRequest JSON
    files.  Prints the inspector summary to stdout; exits with code 2 on HIGH
    risk.  With `--cluster` and `--simulate`, also runs RPC pre-flight and
    prints the result.

- **Ledger hardware wallet support** (`afterimage-solana`, `airsign` CLI)
  - `crates/afterimage-solana/src/ledger_apdu.rs` — full Solana Ledger APDU
    codec: HID framing, BIP44 `DerivationPath` (parse / serialise / display),
    `build_apdu`, `apdu_to_hid_packets` / `hid_packets_to_apdu` roundtrip,
    status-word helpers, 8 unit tests (all passing).
  - `crates/afterimage-solana/src/ledger.rs` — `LedgerSigner` struct: USB HID
    device enumeration (`list_devices`), `connect` / `connect_by_path`,
    `app_version`, `pubkey` (with optional on-device confirmation), and
    `sign_transaction` with automatic chunking for large transactions.
  - `error.rs` — new `LedgerError` enum (`NotFound`, `AppNotOpen`,
    `UserDenied`, `Hid`, `InvalidResponse`, `InvalidData`).
  - `lib.rs` — re-exports `LedgerSigner`, `LedgerDeviceInfo`, `DerivationPath`,
    `LedgerError`.
  - `airsign ledger list` — lists all connected Ledger devices with name,
    serial number, and HID path.
  - `airsign ledger pubkey [--derivation PATH] [--confirm]` — prints the
    Ed25519 pubkey for a BIP44 path; `--confirm` shows it on the Ledger
    display.
  - `airsign ledger version` — prints the Solana app version installed on the
    device.
  - `airsign sign --keypair ledger:<PATH>` — `ledger:` prefix support in the
    keypair specifier; `ledger:default` uses `m/44'/501'/0'/0'`.
  - `hidapi = "2"` added to workspace dependencies and `afterimage-solana`
    crate dependencies.


- Hardware wallet key import (Ledger via HID)
- `airsign-wasm`: React component library

---

## [2.2.0] — 2026-04-17

### Added
- **OS-native keychain integration** (`afterimage-solana::keystore::KeyStore`) —
  Ed25519 keypairs can now be stored in and loaded from the platform keychain
  (macOS Keychain Services, Linux Secret Service / GNOME Keyring, Windows
  Credential Store) using the `keyring` v3 crate.
- **`KeyStoreError`** enum in `afterimage_solana::error` — typed errors for
  `NotFound`, `AlreadyExists`, `InvalidKeyData`, `Backend`, and `Io`.
- **`KeyStore` API** — `generate`, `store`, `load`, `exists`, `delete`,
  `import_from_file`, `export_to_file`.
- **`airsign key` subcommand** with six operations:
  - `airsign key generate <LABEL> [--overwrite] [--output PATH]`
  - `airsign key import  <LABEL> --file PATH [--overwrite]`
  - `airsign key show    <LABEL>`
  - `airsign key list`   (reads `~/.airsign/keys.json` index)
  - `airsign key export  <LABEL> --output PATH`
  - `airsign key delete  <LABEL> [--yes]`
- **`--keypair keychain:<LABEL>`** support on `airsign sign` — keypair can now
  be resolved from the OS keychain instead of a plaintext JSON file.
- `resolve_keypair_bytes()` helper in the CLI — transparently handles both file
  paths and `keychain:` prefixed specifiers.
- 7 unit tests covering the full `KeyStore` lifecycle
  (generate, store/load roundtrip, not-found, exists, delete, duplicate
  rejection, import/export file roundtrip).

---

## [2.1.0] — 2026-04-17

### Added
- **`SecurityProfile` enum** (`owasp-2024` / `mainnet` / `paranoid`) in
  `afterimage-core::crypto` — named Argon2id presets with hardened parameters
  for mainnet-beta and extreme-value signing sessions.
- **`--security-profile <PROFILE>`** CLI flag on `airsign send` — selects a
  preset and is mutually exclusive with `--argon2-mem` / `--argon2-iter`.
- `Argon2Params::meets_mainnet_minimum()` — returns `true` when params satisfy
  the mainnet recommendation (m ≥ 256 MiB, t ≥ 4).
- `Argon2Params::security_level()` — human-readable label (`"weak"`,
  `"owasp-2024"`, `"mainnet"`, `"paranoid"`).
- `SecurityProfile::from_str()` — case-insensitive parser accepting aliases
  (`"mainnet-beta"`, `"max"`, `"owasp2024"`, …).
- Named constants `ARGON2_M_COST_MAINNET`, `ARGON2_T_COST_MAINNET`,
  `ARGON2_M_COST_PARANOID`, `ARGON2_T_COST_PARANOID`.
- `airsign send` now prints the active security profile and emits a warning
  when params are below the mainnet minimum.
- 7 new unit tests covering all preset permutations and edge cases.

### Fixed
- `MetadataFrame::from_bytes` now checks frame length **before** magic bytes,
  ensuring callers always receive `MetadataTooShort` instead of `InvalidMagic`
  when a short slice is passed.
- `session::send_recv_v3_custom_argon2_params` test corrected to use
  `p_cost: ARGON2_P_COST` — `p_cost` is not stored in the v3 wire frame and
  the receiver always reconstructs it from the published constant.

---

## [0.2.0] — 2024-11-18

### Added
- `airsign-wasm`: WebAssembly bindings exposing `WasmSendSession` and
  `WasmRecvSession` for browser use
- CLI `bench` subcommand for offline encode/decode throughput measurement
- `SendSession::set_limit()` to cap maximum frames generated

### Changed
- Fountain code degree distribution retuned; 15 % fewer frames needed on
  average to achieve full recovery
- `RecvSession::progress()` now returns a `f32` in the range 0.0–1.0 instead
  of a frame count

### Fixed
- Rare panic when ingest received a zero-length frame
- Off-by-one in symbol indexing under high-redundancy settings

---

## [0.1.0] — 2024-09-03

### Added
- `airsign-core`: protocol framing, Argon2id key derivation,
  ChaCha20-Poly1305 encryption, BLAKE3 content hash, LT-code fountain encoder
  and decoder
- `airsign-optical`: QR encoding via `qrcode`, QR decoding via `rxing`,
  camera capture via `nokhwa`, live window display via `minifb`
- `airsign-solana`: `AirSigner` struct, `SignRequest` / `SignResponse`
  serialisation, `build_send_session` helper for the online machine
- `airsign` CLI: `send` and `recv` subcommands with optional camera/display
  feature flags
- MIT licence, CI workflow (GitHub Actions), initial documentation