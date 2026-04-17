//! # afterimage-squads
//!
//! Squads v4 on-chain multisig integration for AirSign.
//!
//! Connects AirSign's air-gap signing infrastructure to the
//! [Squads v4](https://github.com/Squads-Protocol/v4) multisig program
//! (`SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`) on Solana mainnet and
//! devnet.
//!
//! ## What this crate provides
//!
//! - **PDA derivation** — `derive_multisig_pda`, `derive_vault_pda`,
//!   `derive_transaction_pda`, `derive_proposal_pda`
//! - **Instruction builders** (no RPC, no private keys required):
//!   - `multisig_create_v2` — create a new multisig
//!   - `vault_transaction_create` — propose an inner transaction
//!   - `proposal_create` / `proposal_approve` / `proposal_reject`
//!   - `vault_transaction_execute`
//!   - `config_transaction_create` / `config_transaction_execute`
//! - **Convenience helpers** — `add_member_ix`, `remove_member_ix`,
//!   `change_threshold_ix`
//! - **AirSign adapter** — converts an [`ApprovalRequest`] into a complete
//!   unsigned `Transaction` and a QR-tunnel-ready [`AirSignSquadsPayload`]
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use afterimage_squads::{
//!     multisig::{create_multisig_ix, derive_pda_info},
//!     types::{Member, MultisigConfig},
//!     adapter::{build_airsign_payload, build_approval_transaction},
//!     types::ApprovalRequest,
//! };
//! use solana_sdk::hash::Hash;
//!
//! // 1. Derive PDAs for a new multisig
//! let create_key = solana_sdk::pubkey::Pubkey::new_unique().to_string();
//! let info = derive_pda_info(&create_key).unwrap();
//! println!("multisig PDA : {}", info.multisig_pda);
//! println!("vault PDA    : {}", info.vault_pda);
//!
//! // 2. Build a create instruction (2-of-3)
//! let config = MultisigConfig {
//!     create_key,
//!     members: vec![
//!         Member::full("Alice111111111111111111111111111111111111111"),
//!         Member::full("Bob1111111111111111111111111111111111111111"),
//!         Member::voter("Carol11111111111111111111111111111111111111"),
//!     ],
//!     threshold: 2,
//!     time_lock: 0,
//!     memo: Some("treasury".into()),
//! };
//! let (ix, ms_pda) = create_multisig_ix(&config).unwrap();
//!
//! // 3. Build an air-gap approval payload for tx index 7
//! let approval = ApprovalRequest {
//!     multisig_pda: ms_pda.to_string(),
//!     transaction_index: 7,
//!     approver: "Alice111111111111111111111111111111111111111".into(),
//!     memo: None,
//! };
//! let payload = build_airsign_payload(&approval, Hash::default()).unwrap();
//! // → send `payload` through the AirSign QR tunnel to the offline signer
//! ```

pub mod adapter;
pub mod config_tx;
pub mod error;
pub mod multisig;
pub mod types;
pub mod vault_tx;

// ── Re-exports for ergonomic usage ────────────────────────────────────────────

pub use adapter::{
    AirSignSquadsPayload,
    approval_instruction_json,
    build_airsign_payload,
    build_approval_transaction,
    payload_from_json,
    payload_to_json,
    transaction_from_b64,
    transaction_to_b64,
};

pub use config_tx::{
    add_member_ix,
    change_threshold_ix,
    config_transaction_create_ix,
    config_transaction_create_json,
    config_transaction_execute_ix,
    remove_member_ix,
};

pub use error::{SquadsError, SquadsResult};

pub use multisig::{
    create_multisig_ix,
    create_multisig_ix_with_creator,
    create_multisig_json,
    derive_multisig_pda,
    derive_pda_info,
    derive_vault_pda,
    discriminator,
    instruction_to_json,
    parse_pubkey,
    validate_config,
};

pub use types::{
    AccountMetaJson,
    ApprovalRequest,
    ConfigAction,
    DEFAULT_VAULT_INDEX,
    InstructionResult,
    Member,
    MultisigConfig,
    MultisigPdaInfo,
    ProposalInfo,
    ProposalStatus,
    SQUADS_V4_PROGRAM_ID,
    VaultTransactionRequest,
    permissions,
};

pub use vault_tx::{
    derive_proposal_pda,
    derive_transaction_pda,
    proposal_approve_ix,
    proposal_approve_json,
    proposal_create_ix,
    proposal_create_json,
    proposal_reject_ix,
    vault_transaction_create_ix,
    vault_transaction_create_json,
    vault_transaction_execute_ix,
};