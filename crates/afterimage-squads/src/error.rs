use thiserror::Error;

/// All errors that can arise within the `afterimage-squads` crate.
#[derive(Debug, Error)]
pub enum SquadsError {
    /// Threshold must be ≥ 1 and ≤ number of members.
    #[error("invalid threshold {threshold}: must be in range [1, {members}]")]
    InvalidThreshold { threshold: u16, members: usize },

    /// The member list must not be empty.
    #[error("member list is empty")]
    EmptyMembers,

    /// A pubkey appears more than once in the member list.
    #[error("duplicate member pubkey: {0}")]
    DuplicateMember(String),

    /// The supplied pubkey string is not valid base58.
    #[error("invalid pubkey '{0}': {1}")]
    InvalidPubkey(String, String),

    /// Borsh serialisation failed.
    #[error("borsh serialisation error: {0}")]
    Serialization(String),

    /// JSON serialisation / deserialisation failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// A base64 decode operation failed.
    #[error("base64 decode error: {0}")]
    Base64(String),

    /// The transaction message bytes are empty.
    #[error("transaction message is empty")]
    EmptyTransactionMessage,

    /// The transaction index must be ≥ 1.
    #[error("transaction index must be ≥ 1")]
    InvalidTransactionIndex,

    /// vault_index must fit in a u8.
    #[error("vault index {0} is out of range (max 255)")]
    VaultIndexOutOfRange(u16),

    /// Generic RPC / cluster error.
    #[error("RPC error: {0}")]
    Rpc(String),
}

pub type SquadsResult<T> = Result<T, SquadsError>;