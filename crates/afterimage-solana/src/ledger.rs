//! Ledger hardware wallet integration for AirSign.
//!
//! [`LedgerSigner`] communicates with a Ledger device running the official
//! **Solana** app over USB HID.  It provides:
//!
//! - [`LedgerSigner::connect`] — open the first available Ledger device
//! - [`LedgerSigner::connect_by_path`] — open a specific HID device path
//! - [`LedgerSigner::list_devices`] — enumerate all connected Ledgers
//! - [`LedgerSigner::app_version`] — query the Solana app version string
//! - [`LedgerSigner::pubkey`] — derive the Ed25519 public key for a BIP44 path
//! - [`LedgerSigner::sign_transaction`] — sign raw transaction bytes and return
//!   the 64-byte Ed25519 signature
//!
//! ## Prerequisites
//!
//! - The device must be unlocked and the **Solana** app must be open.
//! - On Linux the user must have read/write access to the HID device node.
//!   Add the udev rule:
//!   ```text
//!   SUBSYSTEM=="usb", ATTRS{idVendor}=="2c97", MODE="0660", GROUP="plugdev"
//!   ```
//!
//! ## Example
//!
//! ```no_run
//! use afterimage_solana::ledger::LedgerSigner;
//! use afterimage_solana::ledger_apdu::DerivationPath;
//!
//! let signer = LedgerSigner::connect().unwrap();
//! let path = DerivationPath::default_solana();
//! let pubkey = signer.pubkey(&path, false).unwrap();
//! println!("Ledger pubkey: {pubkey}");
//! ```

use hidapi::{HidApi, HidDevice};
use solana_sdk::pubkey::Pubkey;

use crate::error::LedgerError;
use crate::ledger_apdu::{
    DerivationPath, Ins, MAX_TX_CHUNK,
    P1_CONFIRM, P1_FIRST, P1_MORE, P1_NO_CONFIRM,
    apdu_payload, apdu_to_hid_packets, build_apdu, hid_packets_to_apdu, status_word, sw,
    HID_PACKET_SIZE,
};

/// Ledger USB vendor ID.
pub const LEDGER_VID: u16 = 0x2C97;

/// Known Ledger product IDs.
///
/// Nano S = 0x0001, Nano X = 0x0004, Nano S+ = 0x0005, Stax = 0x0006,
/// Flex = 0x0007.  We match on vendor ID only so future devices work.
const KNOWN_PIDS: &[u16] = &[0x0001, 0x0004, 0x0005, 0x0006, 0x0007,
                               // Nano S HID usage-page variants
                               0x1011, 0x1015];

/// HID usage page for the Ledger FIDO / generic HID interface.
/// The Ledger transport uses usage 0xFFA0 (vendor-defined).
const LEDGER_USAGE_PAGE: u16 = 0xFFA0;

// ─── Device info ──────────────────────────────────────────────────────────────

/// Information about a connected Ledger device.
#[derive(Debug, Clone)]
pub struct LedgerDeviceInfo {
    /// HID device path (OS-specific string, e.g. `/dev/hidraw0`).
    pub path: String,
    /// USB product ID.
    pub product_id: u16,
    /// Human-readable product name (e.g. "Nano X").
    pub product_string: Option<String>,
    /// USB serial number string.
    pub serial_number: Option<String>,
}

impl std::fmt::Display for LedgerDeviceInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self
            .product_string
            .as_deref()
            .unwrap_or("Ledger device");
        let serial = self.serial_number.as_deref().unwrap_or("unknown");
        write!(f, "{name} (pid={:#06x}, serial={serial})", self.product_id)
    }
}

// ─── LedgerSigner ─────────────────────────────────────────────────────────────

/// HID-connected Ledger hardware wallet signer.
pub struct LedgerSigner {
    device: HidDevice,
    /// Human-readable description (for log messages).
    pub info: LedgerDeviceInfo,
}

impl LedgerSigner {
    // ─── Constructors ─────────────────────────────────────────────────────────

    /// Enumerate all Ledger HID devices currently connected.
    pub fn list_devices() -> Result<Vec<LedgerDeviceInfo>, LedgerError> {
        let api = HidApi::new().map_err(|e| LedgerError::Hid(e.to_string()))?;
        let mut devices = Vec::new();
        for dev in api.device_list() {
            if dev.vendor_id() != LEDGER_VID {
                continue;
            }
            // Filter to the correct HID interface (usage page 0xFFA0 on
            // devices that expose it; fall back to matching on known PIDs).
            let pid = dev.product_id();
            let usage = dev.usage_page();
            if usage != LEDGER_USAGE_PAGE && !KNOWN_PIDS.contains(&pid) {
                continue;
            }
            devices.push(LedgerDeviceInfo {
                path: dev.path().to_string_lossy().into_owned(),
                product_id: pid,
                product_string: dev.product_string().map(|s| s.to_owned()),
                serial_number: dev.serial_number().map(|s| s.to_owned()),
            });
        }
        Ok(devices)
    }

    /// Open the first available Ledger device.
    ///
    /// Returns [`LedgerError::NotFound`] if no Ledger is connected.
    pub fn connect() -> Result<Self, LedgerError> {
        let devices = Self::list_devices()?;
        let info = devices.into_iter().next().ok_or(LedgerError::NotFound)?;
        Self::connect_by_path(&info.path)
    }

    /// Open a Ledger device at the given HID path.
    pub fn connect_by_path(path: &str) -> Result<Self, LedgerError> {
        let api = HidApi::new().map_err(|e| LedgerError::Hid(e.to_string()))?;
        let device = api
            .open_path(std::ffi::CStr::from_bytes_with_nul(
                &[path.as_bytes(), b"\0"].concat(),
            )
            .map_err(|_| LedgerError::Hid(format!("invalid path: {path}")))?)
            .map_err(|e| LedgerError::Hid(e.to_string()))?;

        // Re-read device info for the opened device
        let devices = Self::list_devices()?;
        let info = devices
            .into_iter()
            .find(|d| d.path == path)
            .unwrap_or(LedgerDeviceInfo {
                path: path.to_owned(),
                product_id: 0,
                product_string: None,
                serial_number: None,
            });

        Ok(LedgerSigner { device, info })
    }

    // ─── Low-level transport ──────────────────────────────────────────────────

    /// Send an APDU and read back the response.
    ///
    /// Handles HID framing, sends all packets, then reads packets until the
    /// total expected response length is assembled.
    fn exchange(&self, apdu: &[u8]) -> Result<Vec<u8>, LedgerError> {
        // ── Send ──────────────────────────────────────────────────────────────
        let packets = apdu_to_hid_packets(apdu);
        for pkt in &packets {
            // hidapi write expects a leading 0x00 report-ID byte on most platforms
            let mut report = vec![0x00u8];
            report.extend_from_slice(pkt);
            self.device
                .write(&report)
                .map_err(|e| LedgerError::Hid(e.to_string()))?;
        }

        // ── Receive ───────────────────────────────────────────────────────────
        // Read the first (init) packet to learn the total response length.
        let mut first = [0u8; HID_PACKET_SIZE + 1]; // +1 for report-ID
        self.device
            .read_timeout(&mut first, 5_000)
            .map_err(|e| LedgerError::Hid(e.to_string()))?;

        // Strip the leading report-ID byte (0x00) returned by hidapi
        let init_pkt: [u8; HID_PACKET_SIZE] = first[1..HID_PACKET_SIZE + 1]
            .try_into()
            .map_err(|_| LedgerError::Hid("short read".into()))?;

        let total_len = ((init_pkt[5] as usize) << 8) | (init_pkt[6] as usize);
        let first_payload_len = (HID_PACKET_SIZE - 7).min(total_len);
        let mut collected = first_payload_len;
        let mut response_pkts = vec![init_pkt];

        while collected < total_len {
            let mut cont = [0u8; HID_PACKET_SIZE + 1];
            self.device
                .read_timeout(&mut cont, 5_000)
                .map_err(|e| LedgerError::Hid(e.to_string()))?;
            let cont_pkt: [u8; HID_PACKET_SIZE] = cont[1..HID_PACKET_SIZE + 1]
                .try_into()
                .map_err(|_| LedgerError::Hid("short continuation read".into()))?;
            collected += (HID_PACKET_SIZE - 5).min(total_len - collected);
            response_pkts.push(cont_pkt);
        }

        let resp = hid_packets_to_apdu(&response_pkts)
            .map_err(|e| LedgerError::Hid(format!("frame reassembly: {e}")))?;

        // ── Status check ──────────────────────────────────────────────────────
        let sw_val = status_word(&resp).ok_or(LedgerError::InvalidResponse(
            "response too short for status word".into(),
        ))?;

        match sw_val {
            sw::OK => Ok(apdu_payload(&resp).to_vec()),
            sw::USER_DENIED => Err(LedgerError::UserDenied),
            sw::APP_NOT_OPEN | sw::INS_NOT_SUPPORTED | sw::CLA_NOT_SUPPORTED => {
                Err(LedgerError::AppNotOpen)
            }
            other => Err(LedgerError::InvalidResponse(format!(
                "unexpected status word: {other:#06x}"
            ))),
        }
    }

    // ─── Public API ───────────────────────────────────────────────────────────

    /// Query the Solana app version string (e.g. `"1.4.0"`).
    pub fn app_version(&self) -> Result<String, LedgerError> {
        let apdu = build_apdu(Ins::GetVersion, 0x00, 0x00, &[]);
        let resp = self.exchange(&apdu)?;
        // Response: [format(1)] [major(1)] [minor(1)] [patch(1)]
        if resp.len() < 4 {
            return Err(LedgerError::InvalidResponse(format!(
                "version response too short: {} bytes",
                resp.len()
            )));
        }
        Ok(format!("{}.{}.{}", resp[1], resp[2], resp[3]))
    }

    /// Get the Ed25519 public key for `path`.
    ///
    /// If `confirm` is `true`, the user must approve the export on the device
    /// display. Set to `false` for scripted / silent use.
    pub fn pubkey(
        &self,
        path: &DerivationPath,
        confirm: bool,
    ) -> Result<Pubkey, LedgerError> {
        let p1 = if confirm { P1_CONFIRM } else { P1_NO_CONFIRM };
        let data = path.to_bytes();
        let apdu = build_apdu(Ins::GetPubkey, p1, 0x00, &data);
        let resp = self.exchange(&apdu)?;

        // Response: [pubkey(32)] (no length prefix in newer app versions)
        if resp.len() < 32 {
            return Err(LedgerError::InvalidResponse(format!(
                "pubkey response too short: {} bytes",
                resp.len()
            )));
        }
        let mut pk_bytes = [0u8; 32];
        pk_bytes.copy_from_slice(&resp[..32]);
        Ok(Pubkey::from(pk_bytes))
    }

    /// Sign `tx_bytes` (raw serialised transaction message) and return the
    /// 64-byte Ed25519 signature.
    ///
    /// Large transactions are automatically split into chunks as required by
    /// the Ledger Solana app protocol.  The user must approve the transaction
    /// on the device display.
    ///
    /// `path` is the BIP44 derivation path of the signing key.
    pub fn sign_transaction(
        &self,
        tx_bytes: &[u8],
        path: &DerivationPath,
    ) -> Result<[u8; 64], LedgerError> {
        if tx_bytes.is_empty() {
            return Err(LedgerError::InvalidData(
                "transaction bytes must not be empty".into(),
            ));
        }

        // The derivation path bytes are prepended to the first chunk.
        let path_bytes = path.to_bytes();

        // Build all chunks.
        // First chunk: path_bytes + first MAX_TX_CHUNK bytes of tx
        // Subsequent chunks: continuation tx bytes only
        let first_data_capacity = MAX_TX_CHUNK.saturating_sub(path_bytes.len());
        let first_tx_chunk = tx_bytes.len().min(first_data_capacity);

        let mut first_payload = path_bytes.clone();
        first_payload.extend_from_slice(&tx_bytes[..first_tx_chunk]);

        let more_after_first = first_tx_chunk < tx_bytes.len();
        let p1_first = if more_after_first { P1_MORE } else { P1_FIRST };

        let resp = {
            let apdu = build_apdu(Ins::SignTransaction, p1_first, 0x00, &first_payload);
            if more_after_first {
                // Send first chunk without waiting for a response
                let packets = apdu_to_hid_packets(&apdu);
                for pkt in &packets {
                    let mut report = vec![0x00u8];
                    report.extend_from_slice(pkt);
                    self.device
                        .write(&report)
                        .map_err(|e| LedgerError::Hid(e.to_string()))?;
                }
                // Send continuation chunks
                let mut offset = first_tx_chunk;
                while offset < tx_bytes.len() {
                    let end = (offset + MAX_TX_CHUNK).min(tx_bytes.len());
                    let chunk = &tx_bytes[offset..end];
                    let is_last = end == tx_bytes.len();
                    let p1 = if is_last { P1_FIRST } else { P1_MORE };
                    let cont_apdu = build_apdu(Ins::SignTransaction, p1, 0x00, chunk);
                    if is_last {
                        // Last chunk — exchange and get signature
                        let sig_payload = self.exchange(&cont_apdu)?;
                        return Self::extract_signature(&sig_payload);
                    } else {
                        let pkts = apdu_to_hid_packets(&cont_apdu);
                        for pkt in &pkts {
                            let mut report = vec![0x00u8];
                            report.extend_from_slice(pkt);
                            self.device
                                .write(&report)
                                .map_err(|e| LedgerError::Hid(e.to_string()))?;
                        }
                    }
                    offset = end;
                }
                unreachable!()
            } else {
                self.exchange(&apdu)?
            }
        };

        Self::extract_signature(&resp)
    }

    fn extract_signature(resp: &[u8]) -> Result<[u8; 64], LedgerError> {
        if resp.len() < 64 {
            return Err(LedgerError::InvalidResponse(format!(
                "signature response too short: {} bytes (expected ≥ 64)",
                resp.len()
            )));
        }
        let mut sig = [0u8; 64];
        sig.copy_from_slice(&resp[..64]);
        Ok(sig)
    }
}

impl std::fmt::Debug for LedgerSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LedgerSigner")
            .field("info", &self.info)
            .finish()
    }
}