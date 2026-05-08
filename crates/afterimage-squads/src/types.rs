//! Core types for the Squads v4 on-chain multisig program.
//!
//! Squads Multisig v4 program ID (mainnet + devnet):
//! `SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf`

use serde::{Deserialize, Serialize};

// ─── Program constants ────────────────────────────────────────────────────────

/// Squads Multisig v4 program ID (both mainnet and devnet).
pub const SQUADS_V4_PROGRAM_ID: &str = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";

/// Default vault index used when the caller does not specify one.
pub const DEFAULT_VAULT_INDEX: u8 = 0;

// ─── Member permissions ───────────────────────────────────────────────────────

/// Bit-flags for a member's permissions within a Squads multisig.
///
/// Squads v4 uses a bitmask where:
/// - bit 0 → **Initiate** (create vault/config transactions)
/// - bit 1 → **Vote**     (approve or reject proposals)
/// - bit 2 → **Execute**  (execute approved transactions)
pub mod permissions {
    pub const INITIATE: u8 = 1;
    pub const VOTE: u8 = 2;
    pub const EXECUTE: u8 = 4;
    /// Full access: initiate + vote + execute.
    pub const ALL: u8 = 7;
}

// ─── Member ───────────────────────────────────────────────────────────────────

/// A member of a Squads multisig, identified by their base58 public key and
/// a permissions bitmask.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Base58-encoded Ed25519 public key.
    pub key: String,
    /// Bitmask of [`permissions`] flags.
    pub permissions: u8,
}

impl Member {
    /// Create a member with full permissions (initiate + vote + execute).
    pub fn full(key: impl Into<String>) -> Self {
        Self { key: key.into(), permissions: permissions::ALL }
    }

    /// Create a voting-only member (no initiate, no execute).
    pub fn voter(key: impl Into<String>) -> Self {
        Self { key: key.into(), permissions: permissions::VOTE }
    }

    /// Create an initiator-only member.
    pub fn initiator(key: impl Into<String>) -> Self {
        Self { key: key.into(), permissions: permissions::INITIATE }
    }

    /// Create an executor-only member.
    pub fn executor(key: impl Into<String>) -> Self {
        Self { key: key.into(), permissions: permissions::EXECUTE }
    }

    /// `true` if the member can initiate new transactions.
    pub fn can_initiate(&self) -> bool { self.permissions & permissions::INITIATE != 0 }

    /// `true` if the member can vote on proposals.
    pub fn can_vote(&self) -> bool { self.permissions & permissions::VOTE != 0 }

    /// `true` if the member can execute approved transactions.
    pub fn can_execute(&self) -> bool { self.permissions & permissions::EXECUTE != 0 }
}

// ─── MultisigConfig ───────────────────────────────────────────────────────────

/// Parameters required to create a new Squads v4 multisig on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigConfig {
    /// A freshly-generated random keypair address.  The multisig PDA is
    /// derived from this key; it never needs to sign after creation.
    pub create_key: String,
    /// Ordered list of members.
    pub members: Vec<Member>,
    /// Approval threshold (1 ≤ threshold ≤ members.len()).
    pub threshold: u16,
    /// Seconds a proposal must wait before it can be executed (0 = no lock).
    pub time_lock: u32,
    /// Optional human-readable note stored in the on-chain account.
    pub memo: Option<String>,
}

// ─── MultisigPdaInfo ──────────────────────────────────────────────────────────

/// Derived PDA addresses for a Squads multisig instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultisigPdaInfo {
    /// The multisig account PDA (program-owned).
    pub multisig_pda: String,
    /// The default vault PDA (index 0), which holds the multisig treasury SOL.
    pub vault_pda: String,
    /// The bump seed used to derive `multisig_pda`.
    pub bump: u8,
}

// ─── ProposalStatus ───────────────────────────────────────────────────────────

/// On-chain status of a Squads proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    /// Proposal has been created but not yet activated for voting.
    Draft,
    /// Proposal is open for member votes.
    Active,
    /// Enough members have voted to reject the proposal.
    Rejected,
    /// Enough members have approved; ready for execution.
    Approved,
    /// Execution is in progress.
    Executing,
    /// The underlying transaction has been executed on-chain.
    Executed,
    /// The proposal was cancelled before execution.
    Cancelled,
}

impl std::fmt::Display for ProposalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Draft     => "draft",
            Self::Active    => "active",
            Self::Rejected  => "rejected",
            Self::Approved  => "approved",
            Self::Executing => "executing",
            Self::Executed  => "executed",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{}", s)
    }
}

// ─── ProposalInfo ─────────────────────────────────────────────────────────────

/// Summary of a Squads proposal returned by read-only helpers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposalInfo {
    /// Base58 address of the multisig account.
    pub multisig_pda: String,
    /// Sequential 1-based index of the vault transaction.
    pub transaction_index: u64,
    /// Base58 address of the proposal account PDA.
    pub proposal_pda: String,
    /// Current proposal status.
    pub status: ProposalStatus,
    /// Members who have approved.
    pub approved: Vec<String>,
    /// Members who have rejected.
    pub rejected: Vec<String>,
}

// ─── ApprovalRequest ─────────────────────────────────────────────────────────

/// Input to the adapter: everything needed to build a `proposal_approve`
/// transaction that can be signed via the AirSign air-gap flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Base58 address of the multisig PDA.
    pub multisig_pda: String,
    /// Transaction index to approve (1-based).
    pub transaction_index: u64,
    /// Base58 pubkey of the member who is approving.
    pub approver: String,
    /// Optional memo to include with the approval.
    pub memo: Option<String>,
}

// ─── VaultTransactionRequest ──────────────────────────────────────────────────

/// Input for creating a new vault transaction proposal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultTransactionRequest {
    /// Base58 address of the multisig PDA.
    pub multisig_pda: String,
    /// Member who is creating the transaction (must have INITIATE permission).
    pub creator: String,
    /// Vault index (usually 0).
    pub vault_index: u8,
    /// Number of ephemeral signing PDAs required by the inner transaction.
    pub ephemeral_signers: u8,
    /// Borsh/bincode-serialised `TransactionMessage` bytes (base64-encoded).
    pub transaction_message_b64: String,
    /// Optional memo.
    pub memo: Option<String>,
}

// ─── ConfigAction ─────────────────────────────────────────────────────────────

/// The set of changes that a config transaction can apply to a multisig.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfigAction {
    /// Add a new member.
    AddMember { new_member: Member },
    /// Remove an existing member by pubkey.
    RemoveMember { old_member: String },
    /// Change the approval threshold.
    ChangeThreshold { new_threshold: u16 },
    /// Update the time-lock (in seconds).
    SetTimeLock { new_time_lock: u32 },
}

// ─── InstructionResult ───────────────────────────────────────────────────────

/// A built Solana instruction serialised to JSON, ready to be embedded in an
/// AirSign `SignRequest`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionResult {
    /// Program ID (base58).
    pub program_id: String,
    /// Ordered list of account metas.
    pub accounts: Vec<AccountMetaJson>,
    /// Instruction data (base64-encoded).
    pub data_b64: String,
}

/// JSON-serialisable version of `solana_sdk::instruction::AccountMeta`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountMetaJson {
    /// Base58 pubkey.
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_full_permissions() {
        let m = Member::full("11111111111111111111111111111111");
        assert!(m.can_initiate());
        assert!(m.can_vote());
        assert!(m.can_execute());
        assert_eq!(m.permissions, permissions::ALL);
    }

    #[test]
    fn member_voter_permissions() {
        let m = Member::voter("11111111111111111111111111111111");
        assert!(!m.can_initiate());
        assert!(m.can_vote());
        assert!(!m.can_execute());
    }

    #[test]
    fn member_initiator_permissions() {
        let m = Member::initiator("11111111111111111111111111111111");
        assert!(m.can_initiate());
        assert!(!m.can_vote());
        assert!(!m.can_execute());
    }

    #[test]
    fn member_executor_permissions() {
        let m = Member::executor("11111111111111111111111111111111");
        assert!(!m.can_initiate());
        assert!(!m.can_vote());
        assert!(m.can_execute());
    }

    #[test]
    fn proposal_status_display() {
        assert_eq!(ProposalStatus::Active.to_string(), "active");
        assert_eq!(ProposalStatus::Approved.to_string(), "approved");
        assert_eq!(ProposalStatus::Executed.to_string(), "executed");
    }

    #[test]
    fn multisig_config_roundtrip_json() {
        let cfg = MultisigConfig {
            create_key: "11111111111111111111111111111111".into(),
            members: vec![Member::full("22222222222222222222222222222222")],
            threshold: 1,
            time_lock: 0,
            memo: Some("treasury".into()),
        };
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: MultisigConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.threshold, 1);
        assert_eq!(decoded.members.len(), 1);
        assert_eq!(decoded.memo.as_deref(), Some("treasury"));
    }

    #[test]
    fn approval_request_roundtrip_json() {
        let req = ApprovalRequest {
            multisig_pda: "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf".into(),
            transaction_index: 3,
            approver: "11111111111111111111111111111111".into(),
            memo: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: ApprovalRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.transaction_index, 3);
        assert_eq!(decoded.approver, "11111111111111111111111111111111");
    }

    #[test]
    fn config_action_roundtrip_json() {
        let actions = vec![
            ConfigAction::AddMember { new_member: Member::full("33333333333333333333333333333333") },
            ConfigAction::ChangeThreshold { new_threshold: 3 },
            ConfigAction::SetTimeLock { new_time_lock: 86_400 },
        ];
        let json = serde_json::to_string(&actions).unwrap();
        let decoded: Vec<ConfigAction> = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.len(), 3);
        if let ConfigAction::ChangeThreshold { new_threshold } = &decoded[1] {
            assert_eq!(*new_threshold, 3);
        } else {
            panic!("unexpected action variant");
        }
    }
}