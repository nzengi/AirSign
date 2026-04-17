//! Solana Ledger APDU encoding and decoding.
//!
//! The Ledger Solana app communicates over a simple APDU (Application Protocol
//! Data Unit) framing layer on top of raw HID packets.
//!
//! ## Packet layout (HID level)
//!
//! Each HID report is 64 bytes:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────────────┐
//! │  [0x01][0x01]  channel (2 B)                                           │
//! │  [0x05]        tag: init frame (0x05) or continuation frame (0x00)     │
//! │  [seq_hi][seq_lo]  sequence index (2 B), init frame = 0x0000           │
//! │  [data_len_hi][data_len_lo]  total APDU length (2 B, init frame only)  │
//! │  [payload ...]  up to 57 B (init) or 59 B (continuation)               │
//! └────────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## APDU command layout
//!
//! ```text
//! CLA  INS  P1  P2  Lc  [data …]
//! ```
//!
//! Solana-specific commands (app version ≥ 1.4):
//!
//! | Command      | CLA   | INS  | P1               | P2  |
//! |---|---|---|---|---|
//! | Get pubkey   | 0xE0  | 0x05 | 0x00 (no confirm) / 0x01 (confirm) | 0x00 |
//! | Sign tx      | 0xE0  | 0x06 | 0x00 (first)     | 0x00 |
//! | Sign tx cont.| 0xE0  | 0x06 | 0x80 (more data) | 0x00 |
//! | Get version  | 0xE0  | 0x01 | 0x00             | 0x00 |
//!
//! BIP44 derivation path is serialised as: `[count][index_0]…[index_N]` where
//! each index is a 4-byte big-endian u32 with the hardened bit (0x80000000) set
//! when required.

/// HID channel identifier used by the Ledger transport.
pub const HID_CHANNEL: [u8; 2] = [0x01, 0x01];
/// HID tag for the initial frame.
pub const HID_TAG_INIT: u8 = 0x05;
/// HID tag for continuation frames.
pub const HID_TAG_CONT: u8 = 0x00;
/// Size of a HID report (packet) in bytes.
pub const HID_PACKET_SIZE: usize = 64;

/// Solana Ledger app CLA byte.
pub const CLA: u8 = 0xE0;

/// APDU INS codes for the Solana Ledger app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Ins {
    /// Get app version.
    GetVersion = 0x01,
    /// Get the Ed25519 public key for a derivation path.
    GetPubkey = 0x05,
    /// Sign a transaction (or transaction chunk).
    SignTransaction = 0x06,
}

/// P1 flag: do not prompt the user for confirmation.
pub const P1_NO_CONFIRM: u8 = 0x00;
/// P1 flag: prompt the user for confirmation on the Ledger display.
pub const P1_CONFIRM: u8 = 0x01;
/// P1 flag: first (or only) data chunk.
pub const P1_FIRST: u8 = 0x00;
/// P1 flag: continuation data chunk.
pub const P1_MORE: u8 = 0x80;

/// Hardened derivation bit for BIP44 path components.
pub const HARDENED: u32 = 0x8000_0000;

/// Maximum payload size in a single APDU.
///
/// The Ledger Solana app accepts at most 255 bytes of data per command.
pub const MAX_APDU_PAYLOAD: usize = 255;

/// Maximum transaction chunk size (bytes).
///
/// Solana transactions can be larger than 255 bytes. The Ledger app expects
/// them split into chunks; the first chunk carries P1=0x00, subsequent chunks
/// carry P1=0x80.
pub const MAX_TX_CHUNK: usize = 250;

// ─── BIP44 path serialisation ─────────────────────────────────────────────────

/// A BIP44 derivation path component sequence.
///
/// The standard Solana path is `m/44'/501'/account'/0'`.
#[derive(Debug, Clone)]
pub struct DerivationPath(pub Vec<u32>);

impl DerivationPath {
    /// Parse a BIP44 path string like `m/44'/501'/0'/0'`.
    ///
    /// `'` denotes hardened components.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim_start_matches("m/").trim_start_matches('/');
        if s.is_empty() {
            return Ok(DerivationPath(vec![]));
        }
        let mut components = Vec::new();
        for part in s.split('/') {
            let (num_str, hardened) = if let Some(stripped) = part.strip_suffix('\'') {
                (stripped, true)
            } else {
                (part, false)
            };
            let index: u32 = num_str
                .parse()
                .map_err(|_| format!("invalid path component: {:?}", part))?;
            components.push(if hardened { index | HARDENED } else { index });
        }
        Ok(DerivationPath(components))
    }

    /// Serialise the path into the Ledger wire format:
    /// `[count as u8][index_0 BE u32]…[index_N BE u32]`
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + self.0.len() * 4);
        buf.push(self.0.len() as u8);
        for &idx in &self.0 {
            buf.extend_from_slice(&idx.to_be_bytes());
        }
        buf
    }

    /// The standard Solana derivation path: `m/44'/501'/0'/0'`.
    pub fn default_solana() -> Self {
        DerivationPath(vec![
            44 | HARDENED,
            501 | HARDENED,
            0 | HARDENED,
            0 | HARDENED,
        ])
    }
}

impl std::fmt::Display for DerivationPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "m")?;
        for &idx in &self.0 {
            if idx & HARDENED != 0 {
                write!(f, "/{}'", idx & !HARDENED)?;
            } else {
                write!(f, "/{}", idx)?;
            }
        }
        Ok(())
    }
}

// ─── APDU builder ─────────────────────────────────────────────────────────────

/// Build a raw APDU byte vector.
pub fn build_apdu(ins: Ins, p1: u8, p2: u8, data: &[u8]) -> Vec<u8> {
    let mut apdu = Vec::with_capacity(5 + data.len());
    apdu.push(CLA);
    apdu.push(ins as u8);
    apdu.push(p1);
    apdu.push(p2);
    apdu.push(data.len() as u8);
    apdu.extend_from_slice(data);
    apdu
}

// ─── HID frame codec ──────────────────────────────────────────────────────────

/// Wrap an APDU byte vector into one or more 64-byte HID packets.
///
/// The returned packets are ready to be sent via `HidDevice::write()`.
/// Each packet is exactly [`HID_PACKET_SIZE`] bytes (zero-padded).
pub fn apdu_to_hid_packets(apdu: &[u8]) -> Vec<[u8; HID_PACKET_SIZE]> {
    let mut packets = Vec::new();
    let total_len = apdu.len();

    // ── Initial frame ─────────────────────────────────────────────────────────
    // Header: channel(2) + tag(1) + seq(2) + data_len(2) = 7 bytes
    // Payload capacity: 64 - 7 = 57 bytes
    let mut pkt = [0u8; HID_PACKET_SIZE];
    pkt[0..2].copy_from_slice(&HID_CHANNEL);
    pkt[2] = HID_TAG_INIT;
    pkt[3] = 0x00; // seq_hi
    pkt[4] = 0x00; // seq_lo
    pkt[5] = (total_len >> 8) as u8;
    pkt[6] = (total_len & 0xFF) as u8;

    let first_chunk = apdu.len().min(57);
    pkt[7..7 + first_chunk].copy_from_slice(&apdu[..first_chunk]);
    packets.push(pkt);

    // ── Continuation frames ───────────────────────────────────────────────────
    // Header: channel(2) + tag(1) + seq(2) = 5 bytes
    // Payload capacity: 64 - 5 = 59 bytes
    let mut offset = first_chunk;
    let mut seq: u16 = 1;
    while offset < apdu.len() {
        let mut cpkt = [0u8; HID_PACKET_SIZE];
        cpkt[0..2].copy_from_slice(&HID_CHANNEL);
        cpkt[2] = HID_TAG_CONT;
        cpkt[3] = (seq >> 8) as u8;
        cpkt[4] = (seq & 0xFF) as u8;

        let chunk = (apdu.len() - offset).min(59);
        cpkt[5..5 + chunk].copy_from_slice(&apdu[offset..offset + chunk]);
        packets.push(cpkt);
        offset += chunk;
        seq += 1;
    }

    packets
}

/// Reassemble HID packets back into an APDU response byte vector.
///
/// `packets` must be in order starting with the initial frame.
/// Returns the raw APDU response (without SW1/SW2 — those are included as the
/// last 2 bytes).
pub fn hid_packets_to_apdu(packets: &[[u8; HID_PACKET_SIZE]]) -> Result<Vec<u8>, String> {
    if packets.is_empty() {
        return Err("no packets".into());
    }
    let init = &packets[0];
    if init[0..2] != HID_CHANNEL || init[2] != HID_TAG_INIT {
        return Err("invalid initial frame header".into());
    }
    let total_len = ((init[5] as usize) << 8) | (init[6] as usize);
    let mut data = Vec::with_capacity(total_len);

    // Initial frame payload
    let first_payload = init[7..].to_vec();
    data.extend_from_slice(&first_payload[..first_payload.len().min(total_len)]);

    // Continuation frames
    for (i, pkt) in packets[1..].iter().enumerate() {
        if pkt[0..2] != HID_CHANNEL || pkt[2] != HID_TAG_CONT {
            return Err(format!("invalid continuation frame header at index {}", i + 1));
        }
        let remaining = total_len.saturating_sub(data.len());
        if remaining == 0 {
            break;
        }
        let payload = &pkt[5..];
        data.extend_from_slice(&payload[..payload.len().min(remaining)]);
    }

    data.truncate(total_len);
    Ok(data)
}

/// APDU status words (SW1SW2).
pub mod sw {
    /// Success.
    pub const OK: u16 = 0x9000;
    /// Solana app is not open / wrong app.
    pub const APP_NOT_OPEN: u16 = 0x6700;
    /// User rejected the action on the device.
    pub const USER_DENIED: u16 = 0x6985;
    /// Invalid data.
    pub const INVALID_DATA: u16 = 0x6A80;
    /// Instruction not supported.
    pub const INS_NOT_SUPPORTED: u16 = 0x6D00;
    /// CLA not supported.
    pub const CLA_NOT_SUPPORTED: u16 = 0x6E00;
}

/// Extract the status word from the last 2 bytes of an APDU response.
pub fn status_word(resp: &[u8]) -> Option<u16> {
    if resp.len() < 2 {
        return None;
    }
    let n = resp.len();
    Some(((resp[n - 2] as u16) << 8) | (resp[n - 1] as u16))
}

/// Extract the payload (everything except the trailing SW1SW2).
pub fn apdu_payload(resp: &[u8]) -> &[u8] {
    if resp.len() < 2 {
        &[]
    } else {
        &resp[..resp.len() - 2]
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derivation_path_parse_roundtrip() {
        let path = DerivationPath::parse("m/44'/501'/0'/0'").unwrap();
        assert_eq!(path.0.len(), 4);
        assert_eq!(path.0[0], 44 | HARDENED);
        assert_eq!(path.0[1], 501 | HARDENED);
        assert_eq!(path.to_string(), "m/44'/501'/0'/0'");
    }

    #[test]
    fn derivation_path_default_solana() {
        let p = DerivationPath::default_solana();
        assert_eq!(p.to_string(), "m/44'/501'/0'/0'");
    }

    #[test]
    fn derivation_path_to_bytes_length() {
        let p = DerivationPath::default_solana();
        let b = p.to_bytes();
        // 1 count byte + 4 components × 4 bytes = 17
        assert_eq!(b.len(), 17);
        assert_eq!(b[0], 4); // count
    }

    #[test]
    fn build_apdu_get_version() {
        let apdu = build_apdu(Ins::GetVersion, P1_NO_CONFIRM, 0x00, &[]);
        assert_eq!(apdu, vec![0xE0, 0x01, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn hid_packet_roundtrip_short() {
        // A short APDU that fits in a single HID packet
        let apdu = build_apdu(Ins::GetVersion, 0x00, 0x00, &[]);
        let pkts = apdu_to_hid_packets(&apdu);
        assert_eq!(pkts.len(), 1);
        let recovered = hid_packets_to_apdu(&pkts).unwrap();
        assert_eq!(recovered, apdu);
    }

    #[test]
    fn hid_packet_roundtrip_multi() {
        // An APDU that spans multiple HID packets (> 57 bytes of payload)
        let data = vec![0xABu8; 120];
        let apdu = build_apdu(Ins::SignTransaction, P1_FIRST, 0x00, &data);
        let pkts = apdu_to_hid_packets(&apdu);
        assert!(pkts.len() >= 2, "expected multiple packets");
        let recovered = hid_packets_to_apdu(&pkts).unwrap();
        assert_eq!(recovered, apdu);
    }

    #[test]
    fn status_word_ok() {
        let resp = vec![0x01, 0x02, 0x90, 0x00];
        assert_eq!(status_word(&resp), Some(sw::OK));
        assert_eq!(apdu_payload(&resp), &[0x01, 0x02]);
    }

    #[test]
    fn derivation_path_non_hardened() {
        let p = DerivationPath::parse("m/44'/501'/3").unwrap();
        assert_eq!(p.0[2], 3); // not hardened
        assert_eq!(p.to_string(), "m/44'/501'/3");
    }
}