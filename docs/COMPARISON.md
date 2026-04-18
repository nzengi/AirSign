# AirSign vs. Alternatives — Competitive Comparison

> **Why existing solutions leave a gap — and how AirSign fills it.**

---

## The Problem

Every Solana wallet, multisig tool, and hardware device that handles private keys faces the same fundamental tension: **the signing device must be accessible enough to use, but isolated enough to be safe.**

Current solutions force a choice between convenience and security. AirSign eliminates that trade-off.

---

## Comparison Matrix

| Capability | **AirSign** | Ledger / Trezor | Phantom Cold Signing | Squads v4 | Paper Wallet |
|---|:---:|:---:|:---:|:---:|:---:|
| **Hardware cost** | $0 (any device) | $79–$219 | $0 | $0 | $0 |
| **True air-gap** (zero network during signing) | ✅ Enforced | ✅ Yes | ❌ No | ❌ No | ✅ Yes |
| **FROST threshold signing** | ✅ Yes | ❌ No | ❌ No | ⚠️ Partial | ❌ No |
| **DKG (distributed key gen)** | ✅ Yes | ❌ No | ❌ No | ❌ No | ❌ No |
| **Squads multisig integration** | ✅ Yes | ⚠️ Manual | ❌ No | ✅ Native | ❌ No |
| **Fountain-code QR transport** | ✅ Yes | ❌ No | ❌ No | ❌ No | ❌ No |
| **Open source (all layers)** | ✅ MIT | ⚠️ Partial | ❌ No | ✅ Yes | N/A |
| **Solana-native (transaction inspection)** | ✅ Yes | ⚠️ Generic | ⚠️ Generic | ✅ Yes | ❌ No |
| **Works on any existing device** | ✅ Yes | ❌ Special HW | ❌ Requires wallet | ❌ Web-only | ✅ Yes |
| **Programmable / SDK** | ✅ React + Rust | ❌ No | ❌ No | ✅ Yes | ❌ No |
| **Mobile signer app** | ✅ iOS + Android | ✅ Companion app | ✅ Phantom app | ❌ Web-only | ❌ No |
| **Formal threat model** | ✅ Published | ✅ Yes | ❌ No | ⚠️ Partial | ❌ No |

---

## Why Existing Solutions Fall Short

### Ledger / Trezor
Hardware wallets are the gold standard for cold storage, but they carry three structural limitations:

1. **Cost barrier** — A Ledger Nano X costs $149. For a DAO with 7 signers, that's $1,043 in hardware before any software costs. Many Solana ecosystem teams, especially in emerging markets, cannot absorb this.
2. **Supply chain risk** — Physical devices can be tampered with in transit (the "evil maid" and "interdiction" attacks). AirSign has no physical supply chain.
3. **No threshold signing** — Ledger devices sign individually. Coordinating a 3-of-5 multisig requires each signer to have a device, be online in sequence, and manually approve. FROST threshold signing in AirSign allows M-of-N signing where the private key is never reconstructed in full on any single device.

### Phantom / Backpack Cold Signing
Wallet apps offer a "view-only" mode with offline signing flows, but these are **not truly air-gapped**:

- The signing device still has network interfaces active during the signing ceremony.
- There is no enforcement mechanism — the operating system can exfiltrate key material through any network interface at any time.
- AirSign's `AirplaneModeGuard` actively blocks the UI and refuses to proceed if any network interface is reachable, making the air-gap a hard protocol requirement, not a user convention.

### Squads v4
Squads is excellent on-chain multisig infrastructure. AirSign is complementary, not competitive:

- Squads manages the on-chain multisig state machine (proposals, approvals, execution).
- AirSign handles the off-chain signing ceremony in a secure, air-gapped environment.
- The `afterimage-squads` crate provides a direct integration adapter — AirSign-signed transactions can be submitted directly to Squads proposals.

Where Squads alone falls short: all signers must be online with hot wallets to approve transactions. AirSign brings cold-signing capability to Squads-managed treasuries.

### Paper Wallets
Paper wallets are air-gapped by definition but have no programmability, no transaction inspection, no threshold signing, and are destroyed (or compromised) by a single use or fire/water incident.

---

## Use Cases

### 1. DAO Treasury Management
A DAO with $10M+ in on-chain assets needs 5-of-9 multisig approval for treasury transactions. With AirSign + Squads:
- Each signer keeps their key on a dedicated air-gapped device (old phone or laptop)
- Transaction proposals originate online → QR-encoded → air-gapped signer reviews and signs → QR back → broadcasted
- No key ever touches a networked device

### 2. Validator Key Management
Solana validators use vote accounts and identity keys worth millions in expected rewards. A compromise means immediate financial loss. AirSign lets validator operators:
- Keep the validator identity key on an air-gapped machine
- Sign key rotation and withdrawal transactions offline
- Use FROST to distribute the signing key across multiple operators (no single point of failure)

### 3. Protocol / Program Upgrade Authority
Solana programs with upgrade authorities need occasional re-deployments. With AirSign:
- The upgrade authority keypair lives air-gapped
- Upgrade transactions are assembled online, QR-transported to the signer, reviewed, and signed
- The `afterimage-solana` inspector verifies the transaction is a program upgrade (not a fund transfer) before presenting it for signing

### 4. Institutional Custody (Without a Custodian)
Crypto funds and family offices that want self-custody without paying custodian fees can use AirSign as a DIY MPC/HSM solution. FROST DKG distributes key shares across geographically separated signing parties, with no share ever combined except during signing.

### 5. Developer / CI Signing
Programs that need to sign transactions in automated pipelines can use the `afterimage-cli` with hardware-isolated CI runners. The CLI reads from `stdin`, signs, and writes to `stdout` — composable with any deployment script.

---

## Technical Differentiation

### Fountain Code QR Transport
Competitors that use QR codes (e.g., AirGap Wallet) encode a single static QR per transaction, limiting payload to ~2KB. AirSign uses **fountain codes** (rateless erasure codes) to split arbitrarily large payloads into a continuous stream of QR frames. The receiver reconstructs the original data from any sufficient subset of frames — no ordering required, no retransmission protocol needed. This enables:
- Transactions with many instructions (e.g., complex DeFi interactions)
- Batch signing of multiple transactions in a single QR session
- Key material export/import across air-gap

### FROST Threshold Signing
AirSign implements FROST (Flexible Round-Optimized Schnorr Threshold) signatures. Unlike naive multisig (N separate signatures combined on-chain), FROST produces a **single Ed25519 signature** indistinguishable from a standard signature:
- Lower on-chain footprint — no multisig account overhead
- Compatible with any program that accepts a standard Ed25519 signer
- The signing threshold (M-of-N) is enforced cryptographically, not by on-chain logic

### Verifiable Air-Gap
The `AirplaneModeGuard` component does not rely on the user disabling Wi-Fi manually. It polls the device's network interface state every 5 seconds and on every app foreground event. If any interface reports reachability, the UI is blocked with a hard error. This is a **protocol-level guarantee**, not a UX suggestion.

---

## Grant Relevance

Projects applying to Solana Foundation Grants or Superteam Grants are evaluated on:

- **Ecosystem gap filled** — AirSign addresses cold signing and institutional custody, which are identified gaps in the Solana tooling ecosystem.
- **Technical depth** — FROST, DKG, fountain codes, and Squads integration demonstrate non-trivial cryptographic engineering.
- **Open source** — MIT-licensed across all layers (Rust crates, React package, mobile app).
- **Composability** — The `@airsign/react` SDK and Rust crates are designed to be integrated into other projects, multiplying ecosystem impact.

---

*Last updated: April 2026*