//! Watch-only wallet and fluent transaction builder for AirSign.
//!
//! [`WatchWallet`] holds only an Ed25519 *public key* — no private key material.
//! It queries the Solana RPC for balances, token accounts, and the recent
//! blockhash, then hands unsigned [`Transaction`]s to the AirSign signing pipeline.
//!
//! [`TransactionBuilder`] provides a fluent API for constructing common
//! transaction types (SOL transfer, SPL Token transfer, ATA creation, Memo,
//! Stake withdrawal) without touching a private key.
//!
//! # Example
//!
//! ```no_run
//! use afterimage_solana::wallet::WatchWallet;
//!
//! let wallet = WatchWallet::new(
//!     "4wTQa3bXHmhJkNMHJxAbCdEfGhIjKlMnOpQrStUvWxYZ".parse().unwrap(),
//!     "https://api.devnet.solana.com",
//! );
//! let balance = wallet.balance().unwrap();
//! println!("Balance: {:.9} SOL", balance as f64 / 1_000_000_000.0);
//! ```

use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    system_instruction,
    sysvar,
    transaction::Transaction,
};
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};

use crate::error::AirSignError;

// ─── Well-known program IDs ────────────────────────────────────────────────────

const MEMO_PROGRAM_ID: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

// ─── WatchWallet ───────────────────────────────────────────────────────────────

/// A watch-only Solana wallet: holds only a public key, never private key material.
///
/// All RPC calls are synchronous (blocking) and target the cluster URL supplied
/// at construction time.
pub struct WatchWallet {
    /// The Ed25519 public key of this wallet.
    pub pubkey: Pubkey,
    rpc_url: String,
}

impl WatchWallet {
    /// Create a new watch-only wallet pointing at `rpc_url`.
    pub fn new(pubkey: Pubkey, rpc_url: impl Into<String>) -> Self {
        Self {
            pubkey,
            rpc_url: rpc_url.into(),
        }
    }

    /// Parse a base-58 public key string and create a wallet.
    pub fn from_pubkey_str(pubkey_str: &str, rpc_url: impl Into<String>) -> Result<Self, AirSignError> {
        let pubkey = pubkey_str.parse::<Pubkey>().map_err(|e| {
            AirSignError::InvalidRequest(format!("invalid pubkey '{}': {}", pubkey_str, e))
        })?;
        Ok(Self::new(pubkey, rpc_url))
    }

    // ─── RPC queries ──────────────────────────────────────────────────────────

    fn rpc(&self) -> RpcClient {
        RpcClient::new(self.rpc_url.clone())
    }

    /// Return the SOL balance in lamports.
    pub fn balance(&self) -> Result<u64, AirSignError> {
        self.rpc()
            .get_balance(&self.pubkey)
            .map_err(|e| AirSignError::Rpc(e.to_string()))
    }

    /// Return the most recent blockhash from the cluster.
    pub fn recent_blockhash(&self) -> Result<Hash, AirSignError> {
        self.rpc()
            .get_latest_blockhash()
            .map_err(|e| AirSignError::Rpc(e.to_string()))
    }

    /// Return the Associated Token Account (ATA) address for the given mint
    /// under this wallet (standard SPL Token program).
    pub fn ata_address(&self, mint: &Pubkey) -> Pubkey {
        get_associated_token_address(&self.pubkey, mint)
    }

    /// Derive the ATA address for any `owner` / `mint` pair.
    pub fn ata_for(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
        get_associated_token_address(owner, mint)
    }

    // ─── Builder factory ──────────────────────────────────────────────────────

    /// Create a [`TransactionBuilder`] with this wallet as fee payer.
    pub fn builder(&self) -> TransactionBuilder {
        TransactionBuilder::new(self.pubkey, self.rpc_url.clone())
    }
}

// ─── TransactionBuilder ────────────────────────────────────────────────────────

/// Fluent builder for constructing unsigned Solana transactions.
///
/// The fee payer defaults to the wallet public key.  Call [`build`](Self::build)
/// when done to obtain an unsigned [`Transaction`] ready for AirSign QR
/// transmission.
///
/// # Example
///
/// ```no_run
/// use afterimage_solana::wallet::WatchWallet;
///
/// let wallet = WatchWallet::new(
///     "4wTQa3bXHmhJkNMHJxAbCdEfGhIjKlMnOpQrStUvWxYZ".parse().unwrap(),
///     "https://api.devnet.solana.com",
/// );
/// let tx = wallet
///     .builder()
///     .transfer("9xRzAbCdEfGhIjKlMnOpQrStUvWxYZ12345678901234".parse().unwrap(), 1_500_000_000)
///     .memo("DAO treasury payout Q2 2026")
///     .build()
///     .unwrap();
/// ```
pub struct TransactionBuilder {
    fee_payer: Pubkey,
    rpc_url: String,
    instructions: Vec<Instruction>,
    recent_blockhash: Option<Hash>,
}

impl TransactionBuilder {
    /// Create a standalone builder (normally called via [`WatchWallet::builder`]).
    pub fn new(fee_payer: Pubkey, rpc_url: impl Into<String>) -> Self {
        Self {
            fee_payer,
            rpc_url: rpc_url.into(),
            instructions: Vec::new(),
            recent_blockhash: None,
        }
    }

    /// Override the recent blockhash instead of fetching it from the RPC.
    ///
    /// Useful in tests or when the blockhash is known in advance.
    pub fn with_blockhash(mut self, blockhash: Hash) -> Self {
        self.recent_blockhash = Some(blockhash);
        self
    }

    // ─── SOL transfer ─────────────────────────────────────────────────────────

    /// Add a SOL transfer instruction from the fee payer to `to`.
    pub fn transfer(mut self, to: Pubkey, lamports: u64) -> Self {
        self.instructions
            .push(system_instruction::transfer(&self.fee_payer, &to, lamports));
        self
    }

    // ─── SPL Token transfer ───────────────────────────────────────────────────

    /// Add an SPL Token `TransferChecked` instruction.
    ///
    /// `source_ata` and `dest_ata` are Associated Token Accounts (not raw
    /// wallet addresses).  Use [`WatchWallet::ata_address`] /
    /// [`WatchWallet::ata_for`] to derive them.
    ///
    /// The fee payer is used as the authority (single-signature).
    pub fn token_transfer(
        mut self,
        source_ata: Pubkey,
        dest_ata: Pubkey,
        mint: Pubkey,
        amount: u64,
        decimals: u8,
    ) -> Result<Self, AirSignError> {
        let ix = spl_token::instruction::transfer_checked(
            &spl_token::id(),
            &source_ata,
            &mint,
            &dest_ata,
            &self.fee_payer,
            &[],
            amount,
            decimals,
        )
        .map_err(|e| AirSignError::InvalidRequest(format!("token_transfer ix: {}", e)))?;
        self.instructions.push(ix);
        Ok(self)
    }

    // ─── ATA creation ─────────────────────────────────────────────────────────

    /// Add an instruction to create the Associated Token Account for `wallet` /
    /// `mint` (standard SPL Token program, payer = fee payer).
    pub fn create_ata(mut self, wallet: Pubkey, mint: Pubkey) -> Self {
        let ix =
            create_associated_token_account(&self.fee_payer, &wallet, &mint, &spl_token::id());
        self.instructions.push(ix);
        self
    }

    // ─── Memo ─────────────────────────────────────────────────────────────────

    /// Attach a UTF-8 memo string to the transaction (SPL Memo Program v2).
    pub fn memo(mut self, text: impl Into<String>) -> Self {
        let text = text.into();
        let memo_program_id: Pubkey = MEMO_PROGRAM_ID.parse().expect("memo program id");
        self.instructions.push(Instruction {
            program_id: memo_program_id,
            accounts: vec![],
            data: text.into_bytes(),
        });
        self
    }

    // ─── Stake withdrawal ─────────────────────────────────────────────────────

    /// Add a `Withdraw` instruction for a stake account.
    ///
    /// Withdraws `lamports` from `stake_account` to `recipient`.
    /// The fee payer is used as the withdraw authority.
    ///
    /// Accounts: `[stake(w), recipient(w), clock(r), stake_history(r), authority(s)]`
    pub fn stake_withdraw(
        mut self,
        stake_account: Pubkey,
        recipient: Pubkey,
        lamports: u64,
    ) -> Self {
        let stake_program_id: Pubkey = solana_sdk::stake::program::id();
        // Stake::Withdraw discriminant = 4 (little-endian u32), followed by
        // the lamports amount (little-endian u64).
        let mut data = Vec::with_capacity(12);
        data.extend_from_slice(&4u32.to_le_bytes());
        data.extend_from_slice(&lamports.to_le_bytes());
        let accounts = vec![
            AccountMeta::new(stake_account, false),
            AccountMeta::new(recipient, false),
            AccountMeta::new_readonly(sysvar::clock::id(), false),
            AccountMeta::new_readonly(sysvar::stake_history::id(), false),
            AccountMeta::new_readonly(self.fee_payer, true), // withdraw authority = fee payer
        ];
        self.instructions.push(Instruction {
            program_id: stake_program_id,
            accounts,
            data,
        });
        self
    }

    // ─── Build ────────────────────────────────────────────────────────────────

    /// Finalise the builder and return an **unsigned** [`Transaction`].
    ///
    /// If no blockhash was set via [`with_blockhash`](Self::with_blockhash),
    /// one is fetched from the RPC endpoint.
    pub fn build(self) -> Result<Transaction, AirSignError> {
        if self.instructions.is_empty() {
            return Err(AirSignError::InvalidRequest(
                "TransactionBuilder: no instructions added".into(),
            ));
        }

        let blockhash = match self.recent_blockhash {
            Some(h) => h,
            None => RpcClient::new(self.rpc_url.clone())
                .get_latest_blockhash()
                .map_err(|e| AirSignError::Rpc(e.to_string()))?,
        };

        let message = Message::new_with_blockhash(
            &self.instructions,
            Some(&self.fee_payer),
            &blockhash,
        );
        Ok(Transaction::new_unsigned(message))
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::hash::Hash;

    fn devnet() -> &'static str {
        "https://api.devnet.solana.com"
    }

    fn b(payer: Pubkey) -> TransactionBuilder {
        TransactionBuilder::new(payer, devnet())
    }

    // ─── SOL transfer ─────────────────────────────────────────────────────────

    #[test]
    fn transfer_builds_one_instruction() {
        let payer = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .transfer(recipient, 1_000_000_000)
            .with_blockhash(bh)
            .build()
            .unwrap();

        assert_eq!(tx.message.instructions.len(), 1);
        assert!(tx.message.account_keys.contains(&payer));
        assert!(tx.message.account_keys.contains(&recipient));
    }

    #[test]
    fn transfer_correct_lamports_in_instruction_data() {
        let payer = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let lamports: u64 = 5_000_000_000;
        let bh = Hash::new_unique();

        let tx = b(payer)
            .transfer(recipient, lamports)
            .with_blockhash(bh)
            .build()
            .unwrap();

        // System::Transfer layout: [2u32 LE (ix index), lamports u64 LE]
        let ix_data = &tx.message.instructions[0].data;
        assert_eq!(ix_data.len(), 12);
        let encoded_lamports = u64::from_le_bytes(ix_data[4..12].try_into().unwrap());
        assert_eq!(encoded_lamports, lamports);
    }

    // ─── Memo ─────────────────────────────────────────────────────────────────

    #[test]
    fn transfer_with_memo_builds_two_instructions() {
        let payer = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .transfer(recipient, 500_000)
            .memo("test memo")
            .with_blockhash(bh)
            .build()
            .unwrap();

        assert_eq!(tx.message.instructions.len(), 2);
    }

    #[test]
    fn memo_data_is_utf8_text() {
        let payer = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .transfer(recipient, 1)
            .memo("hello airsign")
            .with_blockhash(bh)
            .build()
            .unwrap();

        let memo_ix = &tx.message.instructions[1];
        let prog_idx = memo_ix.program_id_index as usize;
        let prog = &tx.message.account_keys[prog_idx];
        let expected: Pubkey = MEMO_PROGRAM_ID.parse().unwrap();
        assert_eq!(prog, &expected);
        assert_eq!(std::str::from_utf8(&memo_ix.data).unwrap(), "hello airsign");
    }

    // ─── Empty builder error ──────────────────────────────────────────────────

    #[test]
    fn build_without_instructions_is_error() {
        let payer = Pubkey::new_unique();
        let bh = Hash::new_unique();
        let result = b(payer).with_blockhash(bh).build();
        assert!(result.is_err());
    }

    // ─── Token transfer ───────────────────────────────────────────────────────

    #[test]
    fn token_transfer_adds_one_instruction() {
        let payer = Pubkey::new_unique();
        let source = Pubkey::new_unique();
        let dest = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .token_transfer(source, dest, mint, 1_000_000, 6)
            .unwrap()
            .with_blockhash(bh)
            .build()
            .unwrap();

        assert_eq!(tx.message.instructions.len(), 1);
    }

    #[test]
    fn token_transfer_memo_creates_two_instructions() {
        let payer = Pubkey::new_unique();
        let source = Pubkey::new_unique();
        let dest = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .token_transfer(source, dest, mint, 500, 2)
            .unwrap()
            .memo("token payout")
            .with_blockhash(bh)
            .build()
            .unwrap();

        assert_eq!(tx.message.instructions.len(), 2);
    }

    // ─── ATA creation ─────────────────────────────────────────────────────────

    #[test]
    fn create_ata_adds_one_instruction() {
        let payer = Pubkey::new_unique();
        let wallet = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .create_ata(wallet, mint)
            .with_blockhash(bh)
            .build()
            .unwrap();

        assert_eq!(tx.message.instructions.len(), 1);
    }

    // ─── Stake withdrawal ─────────────────────────────────────────────────────

    #[test]
    fn stake_withdraw_adds_instruction() {
        let payer = Pubkey::new_unique();
        let stake_account = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let lamports: u64 = 5_000_000_000;
        let bh = Hash::new_unique();

        let tx = b(payer)
            .stake_withdraw(stake_account, recipient, lamports)
            .with_blockhash(bh)
            .build()
            .unwrap();

        assert_eq!(tx.message.instructions.len(), 1);
        assert!(tx.message.account_keys.contains(&stake_account));
        assert!(tx.message.account_keys.contains(&recipient));
    }

    #[test]
    fn stake_withdraw_lamports_in_instruction_data() {
        let payer = Pubkey::new_unique();
        let stake_account = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let lamports: u64 = 7_500_000_000;
        let bh = Hash::new_unique();

        let tx = b(payer)
            .stake_withdraw(stake_account, recipient, lamports)
            .with_blockhash(bh)
            .build()
            .unwrap();

        let ix_data = &tx.message.instructions[0].data;
        // data layout: [4u32 LE, lamports u64 LE]
        assert_eq!(u32::from_le_bytes(ix_data[0..4].try_into().unwrap()), 4);
        assert_eq!(
            u64::from_le_bytes(ix_data[4..12].try_into().unwrap()),
            lamports
        );
    }

    // ─── ATA address derivation ───────────────────────────────────────────────

    #[test]
    fn ata_address_is_deterministic() {
        let wallet = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let a = WatchWallet::ata_for(&wallet, &mint);
        let b = WatchWallet::ata_for(&wallet, &mint);
        assert_eq!(a, b);
    }

    #[test]
    fn ata_address_differs_per_wallet() {
        let wallet1 = Pubkey::new_unique();
        let wallet2 = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        assert_ne!(
            WatchWallet::ata_for(&wallet1, &mint),
            WatchWallet::ata_for(&wallet2, &mint)
        );
    }

    // ─── WatchWallet::from_pubkey_str ─────────────────────────────────────────

    #[test]
    fn watch_wallet_from_pubkey_str_valid() {
        use solana_sdk::signature::Signer as _;
        let kp = solana_sdk::signer::keypair::Keypair::new();
        let pk_str = kp.pubkey().to_string();
        let wallet = WatchWallet::from_pubkey_str(&pk_str, devnet()).unwrap();
        assert_eq!(wallet.pubkey.to_string(), pk_str);
    }

    #[test]
    fn watch_wallet_from_pubkey_str_invalid() {
        let result = WatchWallet::from_pubkey_str("not-a-valid-pubkey", devnet());
        assert!(result.is_err());
    }

    // ─── Multi-instruction chain ──────────────────────────────────────────────

    #[test]
    fn multi_instruction_chain() {
        let payer = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let source = Pubkey::new_unique();
        let dest = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .transfer(recipient, 1_000_000)
            .token_transfer(source, dest, mint, 500, 6)
            .unwrap()
            .memo("batch payout")
            .with_blockhash(bh)
            .build()
            .unwrap();

        // SOL transfer + SPL TokenTransferChecked + Memo = 3 instructions
        assert_eq!(tx.message.instructions.len(), 3);
    }

    // ─── Fee payer presence ───────────────────────────────────────────────────

    #[test]
    fn fee_payer_is_first_account_key() {
        let payer = Pubkey::new_unique();
        let recipient = Pubkey::new_unique();
        let bh = Hash::new_unique();

        let tx = b(payer)
            .transfer(recipient, 1)
            .with_blockhash(bh)
            .build()
            .unwrap();

        // The fee payer is always account_keys[0] in a Solana message.
        assert_eq!(tx.message.account_keys[0], payer);
    }
}