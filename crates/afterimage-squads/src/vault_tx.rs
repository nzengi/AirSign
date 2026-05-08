//! Instruction builders for Squads v4 vault transactions and proposals.

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
    types::{InstructionResult, VaultTransactionRequest, SQUADS_V4_PROGRAM_ID},
};

// ─── PDA derivation ───────────────────────────────────────────────────────────

/// Derive the vault transaction PDA.
///
/// Seeds: `["vault_transaction", multisig_pda, &tx_index.to_le_bytes()]`
pub fn derive_transaction_pda(multisig_pda: &Pubkey, tx_index: u64) -> SquadsResult<(Pubkey, u8)> {
    let program_id = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)
        .map_err(|e| SquadsError::InvalidPubkey(SQUADS_V4_PROGRAM_ID.into(), e.to_string()))?;
    let (pda, bump) = Pubkey::find_program_address(
        &[b"vault_transaction", multisig_pda.as_ref(), &tx_index.to_le_bytes()],
        &program_id,
    );
    Ok((pda, bump))
}

/// Derive the proposal PDA.
///
/// Seeds: `["proposal", multisig_pda, &tx_index.to_le_bytes()]`
pub fn derive_proposal_pda(multisig_pda: &Pubkey, tx_index: u64) -> SquadsResult<(Pubkey, u8)> {
    let program_id = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)
        .map_err(|e| SquadsError::InvalidPubkey(SQUADS_V4_PROGRAM_ID.into(), e.to_string()))?;
    let (pda, bump) = Pubkey::find_program_address(
        &[b"proposal", multisig_pda.as_ref(), &tx_index.to_le_bytes()],
        &program_id,
    );
    Ok((pda, bump))
}

// ─── Borsh arg structs ────────────────────────────────────────────────────────

#[derive(BorshSerialize)]
struct VaultTransactionCreateArgs {
    vault_index: u8,
    ephemeral_signers: u8,
    transaction_message: Vec<u8>,
    memo: Option<String>,
}

#[derive(BorshSerialize)]
struct ProposalCreateArgs {
    transaction_index: u64,
    draft: bool,
}

#[derive(BorshSerialize)]
struct ProposalVoteArgs {
    memo: Option<String>,
}

// ─── vault_transaction_create ────────────────────────────────────────────────

/// Build a `vault_transaction_create` instruction.
///
/// `transaction_message_bytes` must be a serialised `TransactionMessage`
/// (the inner transaction the vault will execute once the proposal passes).
pub fn vault_transaction_create_ix(
    req: &VaultTransactionRequest,
    multisig_transaction_index: u64,
) -> SquadsResult<Instruction> {
    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(&req.multisig_pda)?;
    let creator = parse_pubkey(&req.creator)?;

    // Decode the base64 transaction message.
    use base64::{engine::general_purpose::STANDARD, Engine};
    let tx_msg_bytes = STANDARD
        .decode(&req.transaction_message_b64)
        .map_err(|e| SquadsError::Base64(e.to_string()))?;

    if tx_msg_bytes.is_empty() {
        return Err(SquadsError::EmptyTransactionMessage);
    }

    let (tx_pda, _) = derive_transaction_pda(&multisig_pda, multisig_transaction_index)?;

    let args = VaultTransactionCreateArgs {
        vault_index: req.vault_index,
        ephemeral_signers: req.ephemeral_signers,
        transaction_message: tx_msg_bytes,
        memo: req.memo.clone(),
    };

    let mut data = discriminator("vault_transaction_create").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    // Accounts: [multisig(r), transaction(w,init), creator(w,s), rent_payer(w,s), system_program]
    let accounts = vec![
        AccountMeta::new_readonly(multisig_pda, false),
        AccountMeta::new(tx_pda, false),
        AccountMeta::new_readonly(creator, true),
        AccountMeta::new(creator, true),   // rent_payer = creator
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Ok(Instruction { program_id, accounts, data })
}

/// JSON wrapper for [`vault_transaction_create_ix`].
pub fn vault_transaction_create_json(
    req: &VaultTransactionRequest,
    tx_index: u64,
) -> SquadsResult<InstructionResult> {
    let ix = vault_transaction_create_ix(req, tx_index)?;
    Ok(instruction_to_json(&ix))
}

// ─── proposal_create ─────────────────────────────────────────────────────────

/// Build a `proposal_create` instruction.
///
/// `draft = false` immediately activates the proposal for voting.
pub fn proposal_create_ix(
    multisig_pda_b58: &str,
    creator_b58: &str,
    transaction_index: u64,
    draft: bool,
) -> SquadsResult<Instruction> {
    if transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(multisig_pda_b58)?;
    let creator = parse_pubkey(creator_b58)?;
    let (proposal_pda, _) = derive_proposal_pda(&multisig_pda, transaction_index)?;

    let args = ProposalCreateArgs { transaction_index, draft };
    let mut data = discriminator("proposal_create").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    // Accounts: [multisig(r), proposal(w,init), creator(w,s), rent_payer(w,s), system_program]
    let accounts = vec![
        AccountMeta::new_readonly(multisig_pda, false),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new_readonly(creator, true),
        AccountMeta::new(creator, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Ok(Instruction { program_id, accounts, data })
}

/// JSON wrapper for [`proposal_create_ix`].
pub fn proposal_create_json(
    multisig_pda: &str,
    creator: &str,
    tx_index: u64,
    draft: bool,
) -> SquadsResult<InstructionResult> {
    let ix = proposal_create_ix(multisig_pda, creator, tx_index, draft)?;
    Ok(instruction_to_json(&ix))
}

// ─── proposal_approve ────────────────────────────────────────────────────────

/// Build a `proposal_approve` instruction.
///
/// The `member` must have **Vote** permission on the multisig.
pub fn proposal_approve_ix(
    multisig_pda_b58: &str,
    member_b58: &str,
    transaction_index: u64,
    memo: Option<String>,
) -> SquadsResult<Instruction> {
    if transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(multisig_pda_b58)?;
    let member = parse_pubkey(member_b58)?;
    let (proposal_pda, _) = derive_proposal_pda(&multisig_pda, transaction_index)?;

    let args = ProposalVoteArgs { memo };
    let mut data = discriminator("proposal_approve").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    // Accounts: [multisig(r), member(s), proposal(w)]
    let accounts = vec![
        AccountMeta::new_readonly(multisig_pda, false),
        AccountMeta::new_readonly(member, true),
        AccountMeta::new(proposal_pda, false),
    ];

    Ok(Instruction { program_id, accounts, data })
}

/// JSON wrapper for [`proposal_approve_ix`].
pub fn proposal_approve_json(
    multisig_pda: &str,
    member: &str,
    tx_index: u64,
    memo: Option<String>,
) -> SquadsResult<InstructionResult> {
    let ix = proposal_approve_ix(multisig_pda, member, tx_index, memo)?;
    Ok(instruction_to_json(&ix))
}

// ─── proposal_reject ─────────────────────────────────────────────────────────

/// Build a `proposal_reject` instruction.
pub fn proposal_reject_ix(
    multisig_pda_b58: &str,
    member_b58: &str,
    transaction_index: u64,
    memo: Option<String>,
) -> SquadsResult<Instruction> {
    if transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(multisig_pda_b58)?;
    let member = parse_pubkey(member_b58)?;
    let (proposal_pda, _) = derive_proposal_pda(&multisig_pda, transaction_index)?;

    let args = ProposalVoteArgs { memo };
    let mut data = discriminator("proposal_reject").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    let accounts = vec![
        AccountMeta::new_readonly(multisig_pda, false),
        AccountMeta::new_readonly(member, true),
        AccountMeta::new(proposal_pda, false),
    ];

    Ok(Instruction { program_id, accounts, data })
}

// ─── vault_transaction_execute ───────────────────────────────────────────────

/// Build a `vault_transaction_execute` instruction.
///
/// `remaining_accounts` are the additional accounts that the inner transaction
/// reads or writes (fetched from the on-chain `VaultTransaction` account).
pub fn vault_transaction_execute_ix(
    multisig_pda_b58: &str,
    member_b58: &str,
    transaction_index: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> SquadsResult<Instruction> {
    if transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let multisig_pda = parse_pubkey(multisig_pda_b58)?;
    let member = parse_pubkey(member_b58)?;
    let (proposal_pda, _) = derive_proposal_pda(&multisig_pda, transaction_index)?;
    let (tx_pda, _) = derive_transaction_pda(&multisig_pda, transaction_index)?;

    let data = discriminator("vault_transaction_execute").to_vec();

    // Accounts: [multisig(r), member(s), proposal(w), transaction(w), ...remaining]
    let mut accounts = vec![
        AccountMeta::new_readonly(multisig_pda, false),
        AccountMeta::new_readonly(member, true),
        AccountMeta::new(proposal_pda, false),
        AccountMeta::new(tx_pda, false),
    ];
    accounts.extend(remaining_accounts);

    Ok(Instruction { program_id, accounts, data })
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::pubkey::Pubkey;

    fn ms()     -> String { Pubkey::new_unique().to_string() }
    fn member() -> String { Pubkey::new_unique().to_string() }

    // ── PDA derivation ───────────────────────────────────────────────────────

    #[test]
    fn derive_transaction_pda_is_deterministic() {
        let ms_pda = Pubkey::new_unique();
        let (pda1, b1) = derive_transaction_pda(&ms_pda, 1).unwrap();
        let (pda2, b2) = derive_transaction_pda(&ms_pda, 1).unwrap();
        assert_eq!(pda1, pda2);
        assert_eq!(b1, b2);
    }

    #[test]
    fn derive_proposal_pda_is_deterministic() {
        let ms_pda = Pubkey::new_unique();
        let (pda1, _) = derive_proposal_pda(&ms_pda, 5).unwrap();
        let (pda2, _) = derive_proposal_pda(&ms_pda, 5).unwrap();
        assert_eq!(pda1, pda2);
    }

    #[test]
    fn transaction_and_proposal_pdas_are_distinct() {
        let ms_pda = Pubkey::new_unique();
        let (tx_pda, _)  = derive_transaction_pda(&ms_pda, 1).unwrap();
        let (prop_pda, _) = derive_proposal_pda(&ms_pda, 1).unwrap();
        assert_ne!(tx_pda, prop_pda);
    }

    #[test]
    fn different_tx_indices_give_different_pdas() {
        let ms_pda = Pubkey::new_unique();
        let (pda1, _) = derive_transaction_pda(&ms_pda, 1).unwrap();
        let (pda2, _) = derive_transaction_pda(&ms_pda, 2).unwrap();
        assert_ne!(pda1, pda2);
    }

    // ── vault_transaction_create ─────────────────────────────────────────────

    #[test]
    fn vault_transaction_create_ix_builds() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let req = VaultTransactionRequest {
            multisig_pda: ms(),
            creator: member(),
            vault_index: 0,
            ephemeral_signers: 0,
            transaction_message_b64: STANDARD.encode(b"dummy_message"),
            memo: None,
        };
        let ix = vault_transaction_create_ix(&req, 1).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
        assert_eq!(ix.accounts.len(), 5);
    }

    #[test]
    fn vault_transaction_create_ix_discriminator() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let req = VaultTransactionRequest {
            multisig_pda: ms(),
            creator: member(),
            vault_index: 0,
            ephemeral_signers: 0,
            transaction_message_b64: STANDARD.encode(b"dummy"),
            memo: None,
        };
        let ix = vault_transaction_create_ix(&req, 1).unwrap();
        let expected = discriminator("vault_transaction_create");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn vault_transaction_create_rejects_empty_message() {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let req = VaultTransactionRequest {
            multisig_pda: ms(),
            creator: member(),
            vault_index: 0,
            ephemeral_signers: 0,
            transaction_message_b64: STANDARD.encode(b""),
            memo: None,
        };
        assert!(matches!(
            vault_transaction_create_ix(&req, 1),
            Err(SquadsError::EmptyTransactionMessage)
        ));
    }

    // ── proposal_create ──────────────────────────────────────────────────────

    #[test]
    fn proposal_create_ix_builds() {
        let ix = proposal_create_ix(&ms(), &member(), 1, false).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
        assert_eq!(ix.accounts.len(), 5);
    }

    #[test]
    fn proposal_create_ix_discriminator() {
        let ix = proposal_create_ix(&ms(), &member(), 1, false).unwrap();
        let expected = discriminator("proposal_create");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn proposal_create_rejects_zero_index() {
        assert!(matches!(
            proposal_create_ix(&ms(), &member(), 0, false),
            Err(SquadsError::InvalidTransactionIndex)
        ));
    }

    // ── proposal_approve ────────────────────────────────────────────────────

    #[test]
    fn proposal_approve_ix_builds() {
        let ix = proposal_approve_ix(&ms(), &member(), 1, None).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
        assert_eq!(ix.accounts.len(), 3);
    }

    #[test]
    fn proposal_approve_ix_discriminator() {
        let ix = proposal_approve_ix(&ms(), &member(), 1, None).unwrap();
        let expected = discriminator("proposal_approve");
        assert_eq!(&ix.data[..8], &expected);
    }

    // ── proposal_reject ─────────────────────────────────────────────────────

    #[test]
    fn proposal_reject_ix_builds() {
        let ix = proposal_reject_ix(&ms(), &member(), 2, Some("no".into())).unwrap();
        assert_eq!(ix.accounts.len(), 3);
        let expected = discriminator("proposal_reject");
        assert_eq!(&ix.data[..8], &expected);
    }

    // ── vault_transaction_execute ────────────────────────────────────────────

    #[test]
    fn vault_transaction_execute_ix_builds() {
        let ix = vault_transaction_execute_ix(&ms(), &member(), 1, vec![]).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
        // base 4 accounts, no remaining
        assert_eq!(ix.accounts.len(), 4);
        let expected = discriminator("vault_transaction_execute");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn vault_transaction_execute_rejects_zero_index() {
        assert!(matches!(
            vault_transaction_execute_ix(&ms(), &member(), 0, vec![]),
            Err(SquadsError::InvalidTransactionIndex)
        ));
    }
}