# Changelog

All notable changes to AirSign are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning follows [SemVer](https://semver.org/).

---

## [Unreleased]

### Added

- **`@airsign/react` — React SDK package** (`packages/react`)
  - `src/types.ts` — full TypeScript interface definitions: `AirSignWasm`,
    `WasmSendSession`, `WasmRecvSession`, `SendSessionState`, `RecvSessionState`,
    `TransactionSummary`, `RiskFlag`, `InstructionInfo`, and all component prop types.
  - `src/initAirSign.ts` — `initAirSign(wasmUrl?)` initialises the WASM module once
    and caches it on `globalThis.__airsign_wasm__`.  Also exports `isAirSignReady()`
    and `getAirSignWasm()` helpers.  Uses a string-expression dynamic import so the
    package builds cleanly before `wasm-pack` has been run.
  - `src/hooks/useSendSession.ts` — `useSendSession` hook drives the QR animation
    loop: creates a `WasmSendSession`, ticks frames at the requested FPS via
    `setInterval`, exposes `start / stop / reset`, and fires `onProgress` /
    `onComplete` callbacks.
  - `src/hooks/useRecvSession.ts` — `useRecvSession` hook manages the receive
    pipeline: `ingest(frame)` feeds raw bytes into `WasmRecvSession.ingest_frame()`,
    tracks progress, and calls `onComplete(data, filename)` exactly once on success.
  - `src/components/QrAnimator.tsx` — animated QR canvas component; wraps
    `useSendSession`, draws each frame with `qrcode`, exposes an imperative
    `QrAnimatorHandle` (start / stop / reset) via `forwardRef`, shows a progress
    bar and status text.
  - `src/components/QrScanner.tsx` — camera capture component; opens
    `getUserMedia`, decodes QR frames with `jsqr` (lazy-loaded), feeds bytes into
    `useRecvSession`, renders a progress bar and "Scan again" reset button.
  - `src/components/TransactionReview.tsx` — read-only transaction summary UI;
    renders risk flags (colour-coded HIGH / MEDIUM / LOW badges) and a per-instruction
    list with optional field expansion.
  - `src/index.ts` — public API barrel re-exporting all hooks, components, and types.
  - Build: `tsup` produces ESM + CJS + `.d.ts` bundles (25 KB JS, 10 KB types).
  - Tests: 19 Vitest unit tests covering both hooks with a stub WASM session;
    all 19 pass under `jsdom`.

- **Watch-only Wallet & Transaction Builder** (`afterimage-solana`, `airsign prepare`)
  - `crates/afterimage-solana/src/wallet.rs` — `WatchWallet` (public-key-only,
    never touches private key material) with `balance()`, `recent_blockhash()`,
    `ata_address()`, `ata_for()` helpers, and a `builder()` factory.
    `TransactionBuilder` fluent API supports: `transfer()` (SOL),
    `token_transfer()` (SPL Token `TransferChecked`), `create_ata()`,
    `memo()` (SPL Memo v2), `stake_withdraw()` (Stake program Withdraw
    instruction, using `solana_sdk::stake::program::id()`), and
    `with_blockhash()` for test / offline use.  `build()` fetches the recent
    blockhash from the cluster when none is pre-set.  16 unit tests, all passing.
  - `lib.rs` — re-exports `wallet` module (`WatchWallet`, `TransactionBuilder`).
  - `airsign prepare <SUBCOMMAND>` — new CLI subcommand with three operations:
    - `transfer --from PUBKEY --to PUBKEY --amount SOL [--memo TEXT] [--cluster] [--out FILE]`
    - `token-transfer --from PUBKEY --mint PUBKEY --to PUBKEY --amount N [--decimals N]
      [--from-ata PUBKEY] [--to-ata PUBKEY] [--memo TEXT] [--cluster] [--out FILE]`
    - `stake-withdraw --from PUBKEY --stake-account PUBKEY --to PUBKEY
      (--amount SOL | --amount-all | --lamports N) [--memo TEXT] [--cluster] [--out FILE]`
    All three write an unsigned bincode `Transaction` to disk, ready for
    `airsign inspect` or `airsign send`.

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