//! OS-native keychain integration for AirSign keypairs.
//!
//! [`KeyStore`] stores Ed25519 signing keypairs in the platform keychain:
//! - **macOS** — Keychain Services (Secure Enclave backed)
//! - **Linux** — Secret Service (GNOME Keyring / KWallet)
//! - **Windows** — Windows Credential Store
//!
//! The keypair bytes are stored as a JSON array `[u8; 64]` under the service
//! name `"airsign"` and the user-supplied label as the account name.
//!
//! # Example
//!
//! ```no_run
//! use afterimage_solana::keystore::KeyStore;
//!
//! // Generate and persist a new keypair
//! KeyStore::generate("my-mainnet-key").unwrap();
//!
//! // Load it back
//! let keypair = KeyStore::load("my-mainnet-key").unwrap();
//! println!("pubkey: {}", keypair.pubkey());
//!
//! // Remove it
//! KeyStore::delete("my-mainnet-key").unwrap();
//! ```

use keyring::Entry;
use solana_sdk::{signature::Signer as _, signer::keypair::Keypair};

use crate::error::KeyStoreError;

/// Service name used as the keyring namespace for all AirSign keys.
const KEYRING_SERVICE: &str = "airsign";

/// OS-native keychain backend for AirSign Ed25519 keypairs.
pub struct KeyStore;

impl KeyStore {
    // ─── Helpers ──────────────────────────────────────────────────────────────

    fn entry(label: &str) -> Result<Entry, KeyStoreError> {
        Entry::new(KEYRING_SERVICE, label).map_err(|e| KeyStoreError::Backend(e.to_string()))
    }

    fn keypair_to_bytes(kp: &Keypair) -> [u8; 64] {
        kp.to_bytes()
    }

    fn keypair_from_bytes(raw: &[u8]) -> Result<Keypair, KeyStoreError> {
        if raw.len() != 64 {
            return Err(KeyStoreError::InvalidKeyData(format!(
                "expected 64 bytes, got {}",
                raw.len()
            )));
        }
        Keypair::try_from(raw).map_err(|e| KeyStoreError::InvalidKeyData(e.to_string()))
    }

    // ─── Public API ───────────────────────────────────────────────────────────

    /// Generate a fresh Ed25519 keypair, persist it in the OS keychain, and
    /// return it.
    ///
    /// Returns [`KeyStoreError::AlreadyExists`] if `label` is already present.
    pub fn generate(label: &str) -> Result<Keypair, KeyStoreError> {
        // Check for collision before generating
        if Self::exists(label)? {
            return Err(KeyStoreError::AlreadyExists(label.to_owned()));
        }
        let kp = Keypair::new();
        Self::store(label, &kp)?;
        Ok(kp)
    }

    /// Persist an existing keypair in the OS keychain under `label`.
    ///
    /// Overwrites any existing entry with the same label.
    pub fn store(label: &str, keypair: &Keypair) -> Result<(), KeyStoreError> {
        let raw = Self::keypair_to_bytes(keypair);
        // Encode as hex — avoids any JSON quoting issues and is human-readable
        // in keychain inspection tools.
        let hex_str = hex::encode(raw);
        Self::entry(label)?
            .set_password(&hex_str)
            .map_err(|e| KeyStoreError::Backend(e.to_string()))
    }

    /// Load a keypair from the OS keychain.
    ///
    /// Returns [`KeyStoreError::NotFound`] if `label` does not exist.
    pub fn load(label: &str) -> Result<Keypair, KeyStoreError> {
        let hex_str = Self::entry(label)?
            .get_password()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeyStoreError::NotFound(label.to_owned()),
                other => KeyStoreError::Backend(other.to_string()),
            })?;
        let raw = hex::decode(hex_str.trim())
            .map_err(|e| KeyStoreError::InvalidKeyData(e.to_string()))?;
        Self::keypair_from_bytes(&raw)
    }

    /// Return `true` if `label` exists in the OS keychain.
    pub fn exists(label: &str) -> Result<bool, KeyStoreError> {
        match Self::entry(label)?.get_password() {
            Ok(_) => Ok(true),
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(KeyStoreError::Backend(e.to_string())),
        }
    }

    /// Delete a keypair entry from the OS keychain.
    ///
    /// Returns [`KeyStoreError::NotFound`] if `label` does not exist.
    pub fn delete(label: &str) -> Result<(), KeyStoreError> {
        Self::entry(label)?
            .delete_credential()
            .map_err(|e| match e {
                keyring::Error::NoEntry => KeyStoreError::NotFound(label.to_owned()),
                other => KeyStoreError::Backend(other.to_string()),
            })
    }

    /// Import a keypair from a Solana CLI JSON file (array of 64 u8) and store
    /// it in the OS keychain under `label`.
    ///
    /// Returns [`KeyStoreError::AlreadyExists`] if `label` is already present,
    /// unless `overwrite` is `true`.
    pub fn import_from_file(
        label: &str,
        path: &std::path::Path,
        overwrite: bool,
    ) -> Result<Keypair, KeyStoreError> {
        if !overwrite && Self::exists(label)? {
            return Err(KeyStoreError::AlreadyExists(label.to_owned()));
        }
        let data = std::fs::read(path)
            .map_err(|e| KeyStoreError::Io(format!("{}: {}", path.display(), e)))?;
        let bytes: Vec<u8> = serde_json::from_slice(&data).map_err(|e| {
            KeyStoreError::InvalidKeyData(format!("invalid keypair JSON: {}", e))
        })?;
        let kp = Self::keypair_from_bytes(&bytes)?;
        Self::store(label, &kp)?;
        Ok(kp)
    }

    /// Export a keypair from the OS keychain to a Solana CLI JSON file.
    pub fn export_to_file(
        label: &str,
        path: &std::path::Path,
    ) -> Result<(), KeyStoreError> {
        let kp = Self::load(label)?;
        let bytes = Self::keypair_to_bytes(&kp);
        let json = serde_json::to_vec(&bytes.to_vec())
            .map_err(|e| KeyStoreError::Io(e.to_string()))?;
        std::fs::write(path, json)
            .map_err(|e| KeyStoreError::Io(format!("{}: {}", path.display(), e)))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use solana_sdk::signature::Signer as _;
    use super::*;

    /// Construct a deterministic label for test isolation.
    fn label(suffix: &str) -> String {
        format!("airsign-test-{}-{}", std::process::id(), suffix)
    }

    #[test]
    fn generate_produces_valid_keypair() {
        let lbl = label("generate");
        let kp = KeyStore::generate(&lbl).expect("generate");
        // pubkey must be 32 bytes (always true for a valid Keypair)
        assert_eq!(kp.pubkey().to_bytes().len(), 32);
        KeyStore::delete(&lbl).ok();
    }

    #[test]
    fn store_load_roundtrip() {
        let lbl = label("roundtrip");
        let original = Keypair::new();
        KeyStore::store(&lbl, &original).expect("store");
        let loaded = KeyStore::load(&lbl).expect("load");
        assert_eq!(original.pubkey(), loaded.pubkey());
        KeyStore::delete(&lbl).ok();
    }

    #[test]
    fn load_not_found_returns_error() {
        let lbl = label("not-found-xyzzy");
        match KeyStore::load(&lbl) {
            Err(KeyStoreError::NotFound(l)) => assert_eq!(l, lbl),
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn exists_reflects_state() {
        let lbl = label("exists");
        assert!(!KeyStore::exists(&lbl).unwrap());
        let kp = Keypair::new();
        KeyStore::store(&lbl, &kp).unwrap();
        assert!(KeyStore::exists(&lbl).unwrap());
        KeyStore::delete(&lbl).unwrap();
        assert!(!KeyStore::exists(&lbl).unwrap());
    }

    #[test]
    fn delete_not_found_returns_error() {
        let lbl = label("delete-notfound");
        match KeyStore::delete(&lbl) {
            Err(KeyStoreError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }

    #[test]
    fn generate_rejects_duplicate_label() {
        let lbl = label("duplicate");
        KeyStore::generate(&lbl).unwrap();
        match KeyStore::generate(&lbl) {
            Err(KeyStoreError::AlreadyExists(_)) => {}
            other => panic!("expected AlreadyExists, got {:?}", other),
        }
        KeyStore::delete(&lbl).ok();
    }

    #[test]
    fn import_export_roundtrip() {
        let lbl = label("import-export");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id.json");

        // Write a Solana CLI keypair JSON
        let original = Keypair::new();
        let raw = original.to_bytes();
        let json = serde_json::to_vec(&raw.to_vec()).unwrap();
        std::fs::write(&path, &json).unwrap();

        // Import → export to a different file
        let imported = KeyStore::import_from_file(&lbl, &path, false).unwrap();
        assert_eq!(imported.pubkey(), original.pubkey());

        let out_path = dir.path().join("exported.json");
        KeyStore::export_to_file(&lbl, &out_path).unwrap();

        let exported_json = std::fs::read(&out_path).unwrap();
        let exported_bytes: Vec<u8> = serde_json::from_slice(&exported_json).unwrap();
        assert_eq!(exported_bytes, raw.to_vec());

        KeyStore::delete(&lbl).ok();
    }
}