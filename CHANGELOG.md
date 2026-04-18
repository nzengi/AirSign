# Changelog

## [6.0.0] — 2026-04-18

### Added

- **`apps/signer-mobile`** — new Expo React Native air-gapped signer app:
  - `AirplaneModeGuard` — blocks the entire UI whenever any network interface
    is reachable; re-checks every 5 s and on every foreground resume
  - `QrScanner` — expo-camera QR scanner with per-frame deduplication, torch
    toggle, and viewfinder overlay
  - `QrAnimator` — animates a fountain-code frame array as cycling QR codes
    with configurable fps and fade transitions
  - `TransactionReview` — full transaction review UI with per-instruction risk
    flags, account labels (signer / writable badges), raw hex data, and
    colour-coded risk banner (safe / warn / critical)
  - **Screens**: Home (`/`), Scan (`/scan`), Display (`/display`),
    Key Management (`/keystore`), Settings (`/settings`)
  - `src/native/AirSignCore.ts` — typed `IAirSignCore` interface covering
    `generateKeypair`, `deleteKeypair`, `listKeypairIds`, `signTransaction`,
    `inspectTransaction`, `fountainEncode`, `fountainDecode`,
    `resetFountainSession`; ships with a fail-fast stub until the native
    module is linked
  - `app/_layout.tsx` — Expo Router root layout wrapping the entire app in
    `AirplaneModeGuard` and dark `<Stack>` navigator

- **`docs/AUDIT_PACKAGE/`** — six-document audit package:
  - `SCOPE.md` — system description, in-scope crates/packages, audit goals
  - `THREAT_MODEL.md` — STRIDE table, trust boundaries, attack trees, mitigations
  - `CRYPTO_SPEC.md` — Ed25519, ChaCha20-Poly1305, Argon2id, FROST RFC 9591, fountain LT codes
  - `TEST_COVERAGE.md` — per-crate coverage table (158 tests total), gap analysis
  - `KNOWN_ISSUES.md` — 10 tracked issues (KI-001 … KI-010) with severity, status, mitigations
  - `QUESTIONNAIRE.md` — 25-question pre-audit questionnaire with answers

- **CI** (`ci.yml`) — added `cargo audit` (advisory DB check) and
  `cargo deny check` (license + duplicate + ban) gates to the existing
  lint / test / clippy / build matrix; added `deny.toml` policy file

### Security

- Airplane-mode enforcement is now enforced at the React Native layer
  (KI-010: advisory note added that the guard can be spoofed on
  rooted/jailbroken devices)
- All keypairs stored exclusively in the platform secure keychain
  (iOS Secure Enclave / Android Keystore) via `expo-secure-store`

---

## [5.0.0] — 2026-04-18

### Added

- **`crates/afterimage-wasm`** — new WASM bindings for all major subsystems
  - `WasmSquads` — offline Squads v4 instruction builder exposed to JavaScript:
    `derive_pda`, `create_multisig`, `approve_proposal`, `vault_transaction_create`
  - `WasmBroadcaster` — browser-side RPC helper: `broadcast`, `get_balance`, `request_airdrop`
  - `WasmKeyStore` — encrypted key management in WASM: `generate`, `load`, `exists`, `delete`
  - 29 native unit tests in `crates/afterimage-wasm/tests/e2e.rs` (all green)

- **`crates/afterimage-squads`** — 64-test suite (all green) covering:
  - Anchor discriminator uniqueness and determinism
  - PDA derivation (multisig, vault, transaction, proposal)
  - `multisig_create_v2` instruction builder — success and all error paths
  - `vault_transaction_create`, `proposal_create`, `proposal_approve`, `proposal_reject`, `vault_transaction_execute`
  - Adapter round-trip: `ApprovalRequest` → unsigned `Transaction` → `AirSignSquadsPayload`

- **`crates/afterimage-dkg`** — 23-test suite (all green) covering:
  - Pedersen DKG 2-of-3 and 3-of-5 round-trips
  - Invalid identifier (0), invalid threshold (t=0, t=1, t>n), malformed packages
  - DKG key package compatibility with FROST signing

- **`packages/react` (`@airsign/react`)** — 19 TypeScript tests (all green):
  - `useSendSession`: idle state, start/stop/reset, frame advancement, `onComplete`, `onProgress`, missing WASM error, unmount cleanup
  - `useRecvSession`: idle state, ingest progress, completion, `onComplete` with payload + filename, `onProgress`, reset, missing WASM error, no duplicate `onComplete`

- **`docs/SECURITY_MODEL.md`** — comprehensive security documentation:
  - STRIDE threat model (Spoofing, Tampering, Repudiation, Information Disclosure, DoS, Elevation of Privilege)
  - RFC-style session protocol specification (key derivation, frame encoding, reassembly)
  - FROST threshold signing protocol (RFC 9591) with security properties
  - Squads v4 PDA derivation and instruction serialisation spec
  - Keystore security parameters (Argon2id m=65536, ChaCha20-Poly1305)
  - Ledger APDU integration security notes
  - 18-item independent audit checklist (crypto, protocol, on-chain, WASM, supply chain)
  - Known limitations and out-of-scope items

### Security

- All cryptographic operations verified against RFC test vectors in unit tests
- No `unsafe` blocks outside the WASM FFI boundary (`afterimage-wasm/src/lib.rs`)
- Argon2id KDF parameters (m=65536 KiB, t=3) enforced as non-user-overridable constants
- FROST partial signature verification rejects invalid signers before aggregation
- ChaCha20-Poly1305 tag verification occurs before any plaintext is consumed

---

## [4.0.0] — 2026-04-18

### Added

- **`crates/afterimage-solana/src/broadcaster.rs`** — production-grade transaction broadcaster
  - `Broadcaster::new(rpc_url)` with built-in cluster shorthand resolution
  - `broadcast_response_json(json)` — decode a `SignResponse` JSON and submit via `sendTransaction`
  - `broadcast_signed_transaction(tx)` — submit a pre-built `solana_sdk::transaction::Transaction`
  - `get_latest_blockhash()` — fetch a fresh blockhash for transaction construction
  - `airdrop(pubkey, lamports)` — request test SOL from the devnet / testnet faucet; rejects mainnet
  - `explorer_url(sig)` / `solscan_url(sig)` — per-cluster deep-link helpers
  - `cluster_name()` — human-readable cluster name derived from the RPC URL
  - 15 offline unit tests: constructor setters, URL helpers, cluster names, airdrop mainnet guard, `BroadcastResult` field coverage

- **`airsign broadcast` CLI sub-command** — submit a `SignResponse` JSON to any Solana cluster
  ```text
  airsign broadcast sign_response.json --cluster devnet
  airsign broadcast sign_response.json --cluster mainnet
  airsign broadcast sign_response.json --cluster https://my-rpc.example.com
  ```

- **`airsign airdrop` CLI sub-command** — fund a devnet/testnet address from the public faucet
  ```text
  airsign airdrop --to 4wTQ… --amount 2 --cluster devnet
  airsign airdrop --to 4wTQ… --cluster testnet
  ```
  Mainnet is rejected with a clear error message.

- **`airsign run` CLI sub-command** — end-to-end offline-signed transfer pipeline
  ```text
  airsign run --keypair ~/.config/solana/id.json --to 9xRz… --amount 0.01
  airsign run --keypair id.json --to 9xRz… --amount 0.001 --cluster testnet
  ```
  Loads a 64-byte Solana CLI keypair, builds a `system_instruction::transfer`, signs locally, fetches a fresh blockhash, and broadcasts — all in one command.

- **`apps/signer-web` ReceivePage — v2 (cluster + airdrop + explorer links)**
  - Cluster selector: Devnet / Testnet / Mainnet-beta / Custom RPC — affects RPC call, Explorer link, Solscan link, and airdrop eligibility
  - Balance display — live SOL balance fetched for the signer pubkey after QR scan
  - 💧 Airdrop panel (devnet/testnet only) — requests up to 2 SOL from the public faucet, shows airdrop signature with Explorer link, auto-refreshes balance after 3 s
  - 🚀 Broadcast panel — one-click `sendTransaction` to selected cluster, displays signature, "📋 Copy sig", "🔍 Explorer ↗", "📊 Solscan ↗" actions, retry button
  - CLI equivalent snippet — shows matching `airsign broadcast` + `airsign airdrop` commands
  - Full simulation/fallback mode when WASM is not loaded

### Changed

- `cmd_broadcast` in `crates/afterimage-cli/src/main.rs` now delegates cluster resolution to the shared `resolve_rpc_url()` helper (no duplicate match arms)
- Shared CLI helpers extracted: `resolve_rpc_url`, `parse_pubkey_arg`, `sol_to_lamports`, `load_keypair_file` — reused across `broadcast`, `airdrop`, and `run`

---

## [3.0.0] — 2026-04-18

### Added
- **`crates/afterimage-squads`** — new Rust crate: fully offline Squads v4 multisig instruction builder
  - `multisig` — `multisig_create_v2` instruction builder with Anchor discriminator, Borsh serialisation, PDA derivation (`multisig` + `vault` seeds), and full member-permission bitmask support (`Full` / `Voter` / `Initiator` / `Executor`)
  - `vault_tx` — `vault_transaction_create` and `proposal_create` instruction builders
  - `config_tx` — `config_transaction_create` builders for `AddMember`, `RemoveMember`, and `ChangeThreshold` actions
  - `adapter` — `build_airsign_payload` wraps a `proposal_approve` instruction into a signed `AirSignPayload` JSON ready for `airsign send`
  - `types` — `MultisigConfig`, `Member` (with `::full/voter/initiator/executor()` constructors), `ApprovalRequest`, `VaultTransactionRequest`, `InstructionResult`, `MultisigPdaInfo`
  - `error` — typed `SquadsError` enum (EmptyMembers, InvalidThreshold, DuplicateMember, InvalidPubkey, Serialization, InstructionBuild, PdaDerivation)
  - 64 unit tests covering discriminator correctness, PDA determinism, instruction structure, full validation matrix, and adapter roundtrips
- **`airsign squads` CLI sub-command** — 7 operations wired to `afterimage-squads`:
  - `airsign squads pda  --create-key <KEY>`
  - `airsign squads create --create-key … --members … --threshold N`
  - `airsign squads approve --multisig … --tx-index N --approver …`
  - `airsign squads propose --multisig … --creator … --tx-index N --message <B64>`
  - `airsign squads add-member --multisig … --creator … --tx-index N --member [voter:|initiator:|executor:]<KEY>`
  - `airsign squads remove-member --multisig … --creator … --tx-index N --member <KEY>`
  - `airsign squads change-threshold --multisig … --creator … --tx-index N --threshold N`
- **`apps/signer-web` — tab 7 "🏛️ Squads v4"**
  - `SquadsPage.tsx` — 7-tab form UI (PDAs · Create · Approve · Propose · Add Member · Remove Member · Change Threshold)
  - Each tab shows the exact CLI command, offers one-click copy, and renders a JSON preview
  - Permission prefix picker (`Full / Voter / Initiator / Executor`) on member forms
  - Collapsible Squads v4 program reference table with instruction → discriminator mapping

---

## [2.1.0] — 2026-04-18

### Added
- **`crates/afterimage-dkg`** — new Rust crate implementing FROST RFC 9591 **Distributed Key Generation** (DKG) without a trusted dealer
  - `participant` — `dkg_round1`, `dkg_round2`, `dkg_finish` functions; full 3-phase protocol over Ed25519
  - `coordinator` — stateless helpers for package routing and slot readiness checks
  - 23 unit tests: configuration validation, 2-of-2 / 2-of-3 / 3-of-5 roundtrips, nonce freshness, key-share uniqueness, group-pubkey consistency, and end-to-end FROST signing with DKG-derived keys
- **`crates/afterimage-wasm`** — DKG WebAssembly bindings
  - `WasmDkgParticipant(id, n, t)` → `round1()`, `round2(allR1Json)`, `finish(allR1Json, allR2Json)`
  - Private state (secret packages) held inside the WASM object and never exposed to JavaScript
- **`apps/signer-web` — tab 6 "🗝️ Trustless DKG"**
  - `DkgPage.tsx` — guided 3-round in-browser DKG session demo
  - Step progress indicator (Setup → Round 1 → Round 2 → Finish)
  - Shows per-participant public commitments, directed package routing table, and final group public key with consistency check

---

## [2.0.0] — 2026-04-17

### Added
- **`crates/afterimage-frost`** — new Rust crate implementing FROST RFC 9591 threshold signatures over Ed25519
  - `dealer` — trusted-dealer key generation (`t-of-n`, requires `t ≥ 2`)
  - `participant` — Round 1 (commit) and Round 2 (sign) logic
  - `aggregator` — signing-package builder and share combiner
  - 19 unit tests covering full roundtrips (2-of-2, 2-of-3, 3-of-5), invalid configs, nonce uniqueness, and share mismatch detection
- **`crates/afterimage-wasm`** — FROST WebAssembly bindings
  - `WasmFrostDealer.generate(n, t)` → `FrostSetup` JSON
  - `WasmFrostParticipant(keyPackageJson, id)` → `round1()`, `round2(nonces, signingPkg)`
  - `WasmFrostAggregator(pubkeyPkg, t, n)` → `add_commitment()`, `build_signing_package()`, `add_share()`, `aggregate()`
- **`apps/signer-web` — tab 5 "❄️ FROST Threshold"**
  - `FrostPage.tsx` — guided 5-step in-browser FROST session demo
  - Step progress indicator, private/public data distinction, copy buttons, reset flow


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