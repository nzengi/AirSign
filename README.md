# AirSign

**Air-gapped, fountain-coded, encrypted transaction signing for Solana.**

AirSign moves the private key onto a permanently offline device — an old laptop, a Raspberry Pi, a phone in airplane mode — and communicates with the online world over an optical channel (animated QR codes) that carries no writable interface. The same stack also implements M-of-N orchestration, FROST RFC 9591 threshold signatures, trustless Pedersen DKG, and offline Squads v4 instruction building.

> **Proprietary — All Rights Reserved.** Source is published for review only.
> Copying, forking, redistribution, modification, and any commercial use are
> prohibited without a separate signed agreement. See [LICENSE](LICENSE).

> [!WARNING]
> Experimental, unaudited software. Use **devnet/testnet only**. Do not sign
> mainnet transactions with real funds. The authors accept no liability.

## Why

Hot wallets that hold the private key on an internet-connected machine are the largest single attack surface in Solana key management. AirSign keeps the key on a device that has never had a writable network interface enabled. Versus a hardware wallet alone, AirSign also adds:

| Capability | Hardware wallet | AirSign |
|---|---|---|
| No USB / no Bluetooth attack surface | ✗ | ✓ |
| M-of-N multisig out of the box | vendor-dependent | ✓ |
| Open, auditable cryptographic stack | partial | ✓ |
| Channel survives frame loss (fountain code) | n/a | ✓ |
| Native browser stack (WASM) | via adapter only | ✓ |
| Rust + TypeScript SDKs | ✗ | ✓ |

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                  ONLINE MACHINE                      │
│  ┌────────────┐   unsigned tx    ┌────────────────┐  │
│  │ dApp / CLI │ ───────────────▶ │  AirSign Send  │  │
│  └────────────┘                  │ (fountain enc) │  │
│                                  └───────┬────────┘  │
│                                          │ QR stream │
└──────────────────────────────────────────┼──────────-┘
                                           │  ▲
                              webcam/screen │  │ webcam
                                           ▼  │
┌──────────────────────────────────────────┼──────────-┐
│                AIR-GAPPED MACHINE        │           │
│  ┌────────────────┐  ┌───────────────────┴────────┐  │
│  │  AirSign Sign  │  │  AirSign Receive           │  │
│  │  (decrypt +    │  │  (fountain decode +        │  │
│  │   Ed25519 sign)│  │   display signed QR stream)│  │
│  └───────┬────────┘  └────────────────────────────┘  │
│          │ signed tx                                 │
└──────────┼───────────────────────────────────────────┘
           │ QR stream
           ▼
┌──────────────────────────────────────────────────────┐
│                  ONLINE MACHINE                      │
│  ┌────────────────┐                                  │
│  │ AirSign Receive│ ──▶ broadcast to Solana RPC      │
│  └────────────────┘                                  │
└──────────────────────────────────────────────────────┘
```

### Threat model

- The online machine may be fully compromised; the private key never exists on it.
- The optical channel is treated as passive and observable; payloads are AEAD-encrypted with a per-session key derived from a shared password.
- The air-gapped machine is in the trust boundary; physical security of the offline device is the operator's responsibility.

### What AirSign does NOT do

- It is not a TEE or secure-enclave replacement; the private key exists in RAM on the air-gapped device while signing.
- It does not protect against vulnerabilities in third-party QR decoder libraries.
- It does not rate-limit password attempts; operators must use a strong, unique password per session.

---

## License

**AirSign Proprietary License — All Rights Reserved.**
Copyright © 2024–2026 nzengi.

This project is **not open source**. The source is published for review and prior-art purposes only. The following are **prohibited** without a separate signed written agreement with the copyright holder:

- copying, cloning, forking, mirroring, downloading
- modification, translation, derivative works
- redistribution, sublicensing, sale, lease
- any commercial use, in any jurisdiction
- ingestion into any AI / LLM / training corpus

See [LICENSE](LICENSE) for the full terms, governing law (Republic of Türkiye), and enforcement.

For commercial licensing inquiries, contact the copyright holder.

---
