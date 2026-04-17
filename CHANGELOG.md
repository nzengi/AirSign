# Changelog

All notable changes to AirSign are documented here.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
Versioning follows [SemVer](https://semver.org/).

---

## [Unreleased]

- Hardware wallet key import (Ledger via HID)
- `airsign-wasm`: React component library

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