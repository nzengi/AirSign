//! Instruction builders for Squads v4 config transactions.
//!
//! Config transactions modify the multisig's own parameters (members,
//! threshold, time-lock) rather than executing vault transfers.

use borsh::BorshSerialize;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use std::str::FromStr;

use crate::{
    error::{SquadsError, SquadsResult},
    multisig::{discriminator, instruction_to_json, parse_pubkey},
    types::{ConfigAction, InstructionResult, Member, SQUADS_V4_PROGRAM_ID},
    vault_tx::derive_proposal_pda,
};

// ─── Borsh representation of ConfigAction ─────────────────────────────────────

/// Borsh-serialisable member for on-chain config actions.
#[derive(BorshSerialize)]
struct BorshMember {
    key: [u8; 32],
    permissions: u8,
}

/// Borsh-serialisable variant tag + payload for a single config action.
///
/// Squads v4 encodes enum variants as a `u8` discriminant followed by fields.
/// Variant indices match the on-chain IDL order:
/// 0 = AddMember, 1 = RemoveMember, 2 = ChangeThreshold, 3 = SetTimeLock
#[derive(BorshSerialize)]
enum BorshConfigAction {
    AddMember { new_member: BorshMember },
    RemoveMember { old_member: [u8; 32] },
    ChangeThreshold { new_threshold: u16 },
    SetTimeLock { new_time_lock: u32 },
}

/// Top-level args for `config_transaction_create`.
#[derive(BorshSerialize)]
struct ConfigTransactionCreateArgs {
    actions: Vec<BorshConfigAction>,
    memo: Option<String>,
}

// ─── config_transaction_create ───────────────────────────────────────────────

/// Build a `config_transaction_create` instruction.
///
/// The resulting config transaction must be paired with a `proposal_create`
/// call (see [`crate::vault_tx::proposal_create_ix`]) to become voteable.
pub fn config_transaction_create_ix(
    multisig_pda_b58: &str,
    creator_b58: &str,
    transaction_index: u64,
    actions: &[ConfigAction],
    memo: Option<String>,
) -> SquadsResult<Instruction> {
    if transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }
    if actions.is_empty() {
        return Err(SquadsError::EmptyMembers); // reuse for empty actions
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(multisig_pda_b58)?;
    let creator = parse_pubkey(creator_b58)?;

    // Derive the transaction PDA for config transactions.
    // Squads uses the same PDA seeds as vault transactions for the account,
    // but a different seed prefix: "config_transaction".
    let program_id_ref = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)
        .map_err(|e| SquadsError::InvalidPubkey(SQUADS_V4_PROGRAM_ID.into(), e.to_string()))?;
    let (tx_pda, _) = Pubkey::find_program_address(
        &[b"config_transaction", multisig_pda.as_ref(), &transaction_index.to_le_bytes()],
        &program_id_ref,
    );

    // Convert actions to Borsh representation.
    let borsh_actions: Vec<BorshConfigAction> = actions
        .iter()
        .map(|a| config_action_to_borsh(a))
        .collect::<SquadsResult<_>>()?;

    let args = ConfigTransactionCreateArgs { actions: borsh_actions, memo };

    let mut data = discriminator("config_transaction_create").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    // Accounts: [multisig(r), transaction(w,init), creator(w,s), rent_payer(w,s), system_program]
    let accounts = vec![
        AccountMeta::new(multisig_pda, false),
        AccountMeta::new(tx_pda, false),
        AccountMeta::new_readonly(creator, true),
        AccountMeta::new(creator, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Ok(Instruction { program_id, accounts, data })
}

/// JSON wrapper for [`config_transaction_create_ix`].
pub fn config_transaction_create_json(
    multisig_pda: &str,
    creator: &str,
    tx_index: u64,
    actions: &[ConfigAction],
    memo: Option<String>,
) -> SquadsResult<InstructionResult> {
    let ix = config_transaction_create_ix(multisig_pda, creator, tx_index, actions, memo)?;
    Ok(instruction_to_json(&ix))
}

// ─── config_transaction_execute ──────────────────────────────────────────────

/// Build a `config_transaction_execute` instruction.
///
/// Call this after the associated proposal has been approved.
pub fn config_transaction_execute_ix(
    multisig_pda_b58: &str,
    member_b58: &str,
    transaction_index: u64,
) -> SquadsResult<Instruction> {
    if transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(multisig_pda_b58)?;
    let member = parse_pubkey(member_b58)?;

    let program_id_ref = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)
        .map_err(|e| SquadsError::InvalidPubkey(SQUADS_V4_PROGRAM_ID.into(), e.to_string()))?;
    let (tx_pda, _) = Pubkey::find_program_address(
        &[b"config_transaction", multisig_pda.as_ref(), &transaction_index.to_le_bytes()],
        &program_id_ref,
    );
    let (proposal_pda, _) = derive_proposal_pda(&multisig_pda, transaction_index)?;

    let data = discriminator("config_transaction_execute").to_vec();

    // Accounts: [multisig(w), member(s), proposal(w), transaction(w), system_program]
    let accounts = vec![
        AccountMeta::new(multisig_pda, false),
        AccountMeta::new_readonly(member, true),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new(tx_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Ok(Instruction { program_id, accounts, data })
}

// ─── Convenience builders ─────────────────────────────────────────────────────

/// Build a config transaction that adds a single new member.
pub fn add_member_ix(
    multisig_pda: &str,
    creator: &str,
    tx_index: u64,
    new_member: Member,
    memo: Option<String>,
) -> SquadsResult<Instruction> {
    config_transaction_create_ix(
        multisig_pda,
        creator,
        tx_index,
        &[ConfigAction::AddMember { new_member }],
        memo,
    )
}

/// Build a config transaction that removes a single member.
pub fn remove_member_ix(
    multisig_pda: &str,
    creator: &str,
    tx_index: u64,
    old_member_key: &str,
    memo: Option<String>,
) -> SquadsResult<Instruction> {
    config_transaction_create_ix(
        multisig_pda,
        creator,
        tx_index,
        &[ConfigAction::RemoveMember { old_member: old_member_key.to_string() }],
        memo,
    )
}

/// Build a config transaction that changes the threshold.
pub fn change_threshold_ix(
    multisig_pda: &str,
    creator: &str,
    tx_index: u64,
    new_threshold: u16,
    memo: Option<String>,
) -> SquadsResult<Instruction> {
    config_transaction_create_ix(
        multisig_pda,
        creator,
        tx_index,
        &[ConfigAction::ChangeThreshold { new_threshold }],
        memo,
    )
}

// ─── Internal conversion helper ───────────────────────────────────────────────

fn config_action_to_borsh(action: &ConfigAction) -> SquadsResult<BorshConfigAction> {
    match action {
        ConfigAction::AddMember { new_member } => {
            let pk = parse_pubkey(&new_member.key)?;
            Ok(BorshConfigAction::AddMember {
                new_member: BorshMember {
                    key: pk.to_bytes(),
                    permissions: new_member.permissions,
                },
            })
        }
        ConfigAction::RemoveMember { old_member } => {
            let pk = parse_pubkey(old_member)?;
            Ok(BorshConfigAction::RemoveMember { old_member: pk.to_bytes() })
        }
        ConfigAction::ChangeThreshold { new_threshold } => {
            Ok(BorshConfigAction::ChangeThreshold { new_threshold: *new_threshold })
        }
        ConfigAction::SetTimeLock { new_time_lock } => {
            Ok(BorshConfigAction::SetTimeLock { new_time_lock: *new_time_lock })
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ConfigAction, Member};
    use solana_sdk::pubkey::Pubkey;

    fn ms()  -> String { Pubkey::new_unique().to_string() }
    fn key() -> String { Pubkey::new_unique().to_string() }

    // ── config_transaction_create ────────────────────────────────────────────

    #[test]
    fn config_tx_add_member_ix_builds() {
        let ix = add_member_ix(&ms(), &key(), 1, Member::full(key()), None).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
        assert_eq!(ix.accounts.len(), 5);
    }

    #[test]
    fn config_tx_add_member_ix_discriminator() {
        let ix = add_member_ix(&ms(), &key(), 1, Member::full(key()), None).unwrap();
        let expected = discriminator("config_transaction_create");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn config_tx_remove_member_ix_builds() {
        let ix = remove_member_ix(&ms(), &key(), 2, &key(), None).unwrap();
        assert_eq!(ix.accounts.len(), 5);
        let expected = discriminator("config_transaction_create");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn config_tx_change_threshold_ix_builds() {
        let ix = change_threshold_ix(&ms(), &key(), 3, 2, None).unwrap();
        assert_eq!(ix.accounts.len(), 5);
        let expected = discriminator("config_transaction_create");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn config_tx_rejects_zero_tx_index() {
        let result = config_transaction_create_ix(
            &ms(),
            &key(),
            0,
            &[ConfigAction::ChangeThreshold { new_threshold: 2 }],
            None,
        );
        assert!(matches!(result, Err(SquadsError::InvalidTransactionIndex)));
    }

    #[test]
    fn config_tx_with_memo() {
        let ix = change_threshold_ix(&ms(), &key(), 1, 2, Some("raise threshold".into())).unwrap();
        // Data must contain more bytes than discriminator + empty memo (9 bytes minimum).
        assert!(ix.data.len() > 9);
    }

    #[test]
    fn config_tx_multiple_actions_in_one_tx() {
        let actions = vec![
            ConfigAction::AddMember { new_member: Member::full(key()) },
            ConfigAction::ChangeThreshold { new_threshold: 3 },
        ];
        let ix = config_transaction_create_ix(&ms(), &key(), 1, &actions, None).unwrap();
        assert_eq!(ix.accounts.len(), 5);
    }

    // ── config_transaction_execute ────────────────────────────────────────────

    #[test]
    fn config_tx_execute_ix_builds() {
        let ix = config_transaction_execute_ix(&ms(), &key(), 1).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
        assert_eq!(ix.accounts.len(), 5);
        let expected = discriminator("config_transaction_execute");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn config_tx_execute_rejects_zero_index() {
        assert!(matches!(
            config_transaction_execute_ix(&ms(), &key(), 0),
            Err(SquadsError::InvalidTransactionIndex)
        ));
    }

    // ── Borsh conversion ─────────────────────────────────────────────────────

    #[test]
    fn config_action_set_timelock_borsh() {
        let action = ConfigAction::SetTimeLock { new_time_lock: 3600 };
        let b = config_action_to_borsh(&action).unwrap();
        let mut buf = Vec::new();
        borsh::to_writer(&mut buf, &b).unwrap();
        assert!(!buf.is_empty());
    }
}