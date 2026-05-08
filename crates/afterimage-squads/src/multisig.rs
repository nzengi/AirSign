//! Instruction builders for Squads v4 multisig creation and PDA derivation.
//!
//! All instructions are serialised with an 8-byte Anchor discriminator prefix
//! (SHA-256("global:<instruction_name>")[0..8]) followed by Borsh-encoded
//! arguments — exactly the format the on-chain Squads v4 program expects.

use borsh::BorshSerialize;
use sha2::{Digest, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};
use std::str::FromStr;

use crate::{
    error::{SquadsError, SquadsResult},
    types::{AccountMetaJson, InstructionResult, Member, MultisigConfig, MultisigPdaInfo, SQUADS_V4_PROGRAM_ID},
};

// ─── Discriminator helper ─────────────────────────────────────────────────────

/// Compute the 8-byte Anchor instruction discriminator for `global:<name>`.
pub fn discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}"));
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

// ─── PDA derivation ───────────────────────────────────────────────────────────

/// Derive the multisig PDA from `create_key`.
///
/// Seeds: `["multisig", create_key]`
pub fn derive_multisig_pda(create_key: &Pubkey) -> SquadsResult<(Pubkey, u8)> {
    let program_id = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)
        .map_err(|e| SquadsError::InvalidPubkey(SQUADS_V4_PROGRAM_ID.into(), e.to_string()))?;
    let (pda, bump) = Pubkey::find_program_address(
        &[b"multisig", create_key.as_ref()],
        &program_id,
    );
    Ok((pda, bump))
}

/// Derive the vault PDA from the multisig PDA and `vault_index`.
///
/// Seeds: `["vault", multisig_pda, vault_index_le_bytes]`
pub fn derive_vault_pda(multisig_pda: &Pubkey, vault_index: u8) -> SquadsResult<(Pubkey, u8)> {
    let program_id = Pubkey::from_str(SQUADS_V4_PROGRAM_ID)
        .map_err(|e| SquadsError::InvalidPubkey(SQUADS_V4_PROGRAM_ID.into(), e.to_string()))?;
    let (pda, bump) = Pubkey::find_program_address(
        &[b"vault", multisig_pda.as_ref(), &[vault_index]],
        &program_id,
    );
    Ok((pda, bump))
}

/// Convenience: derive both the multisig and default vault PDAs and return a
/// [`MultisigPdaInfo`] struct.
pub fn derive_pda_info(create_key_b58: &str) -> SquadsResult<MultisigPdaInfo> {
    let create_key = parse_pubkey(create_key_b58)?;
    let (ms_pda, bump) = derive_multisig_pda(&create_key)?;
    let (vault_pda, _) = derive_vault_pda(&ms_pda, 0)?;
    Ok(MultisigPdaInfo {
        multisig_pda: ms_pda.to_string(),
        vault_pda: vault_pda.to_string(),
        bump,
    })
}

// ─── Argument structs (Borsh-serialised) ─────────────────────────────────────

/// Borsh-serialisable member struct (matches on-chain layout).
#[derive(BorshSerialize)]
struct BorshMember {
    key: [u8; 32],
    permissions: u8,
}

/// Borsh-serialisable args for `multisig_create_v2`.
#[derive(BorshSerialize)]
struct MultisigCreateArgsV2 {
    config_authority: Option<[u8; 32]>,
    threshold: u16,
    members: Vec<BorshMember>,
    time_lock: u32,
    rent_collector: Option<[u8; 32]>,
    memo: Option<String>,
}

// ─── Instruction builder: multisig_create_v2 ─────────────────────────────────

/// Validate a [`MultisigConfig`] and build the `multisig_create_v2` Squads v4
/// instruction.
///
/// Returns the built [`Instruction`] plus the derived multisig PDA.
pub fn create_multisig_ix(config: &MultisigConfig) -> SquadsResult<(Instruction, Pubkey)> {
    // ── Validation ───────────────────────────────────────────────────────────
    if config.members.is_empty() {
        return Err(SquadsError::EmptyMembers);
    }
    if config.threshold == 0 || config.threshold as usize > config.members.len() {
        return Err(SquadsError::InvalidThreshold {
            threshold: config.threshold,
            members: config.members.len(),
        });
    }
    // Check for duplicate members.
    let mut seen = std::collections::HashSet::new();
    for m in &config.members {
        if !seen.insert(m.key.clone()) {
            return Err(SquadsError::DuplicateMember(m.key.clone()));
        }
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let create_key = parse_pubkey(&config.create_key)?;

    // ── PDA derivation ───────────────────────────────────────────────────────
    let (multisig_pda, _bump) = derive_multisig_pda(&create_key)?;

    // ── Borsh args ───────────────────────────────────────────────────────────
    let borsh_members: Vec<BorshMember> = config
        .members
        .iter()
        .map(|m| -> SquadsResult<BorshMember> {
            let pk = parse_pubkey(&m.key)?;
            Ok(BorshMember { key: pk.to_bytes(), permissions: m.permissions })
        })
        .collect::<SquadsResult<_>>()?;

    let args = MultisigCreateArgsV2 {
        config_authority: None,
        threshold: config.threshold,
        members: borsh_members,
        time_lock: config.time_lock,
        rent_collector: None,
        memo: config.memo.clone(),
    };

    let mut data = discriminator("multisig_create_v2").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    // ── Accounts ─────────────────────────────────────────────────────────────
    // [multisig PDA (writable), create_key (signer), creator (writable+signer), system_program]
    // For the instruction we pass `create_key` as a signer; the actual keypair
    // must be supplied when constructing the Transaction.
    let accounts = vec![
        AccountMeta::new(multisig_pda, false),
        AccountMeta::new_readonly(create_key, true),
        AccountMeta::new(create_key, true),   // creator = create_key in offline mode
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Ok((Instruction { program_id, accounts, data }, multisig_pda))
}

/// Serialise `create_multisig_ix` output to [`InstructionResult`] JSON.
pub fn create_multisig_json(config: &MultisigConfig) -> SquadsResult<InstructionResult> {
    let (ix, _) = create_multisig_ix(config)?;
    Ok(instruction_to_json(&ix))
}

// ─── Instruction builder: multisig_create_v2 with explicit creator ────────────

/// Like [`create_multisig_ix`] but allows a separate `creator` pubkey.
pub fn create_multisig_ix_with_creator(
    config: &MultisigConfig,
    creator: &Pubkey,
) -> SquadsResult<(Instruction, Pubkey)> {
    if config.members.is_empty() {
        return Err(SquadsError::EmptyMembers);
    }
    if config.threshold == 0 || config.threshold as usize > config.members.len() {
        return Err(SquadsError::InvalidThreshold {
            threshold: config.threshold,
            members: config.members.len(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    for m in &config.members {
        if !seen.insert(m.key.clone()) {
            return Err(SquadsError::DuplicateMember(m.key.clone()));
        }
    }

    let program_id = parse_pubkey(SQUADS_V4_PROGRAM_ID)?;
    let create_key = parse_pubkey(&config.create_key)?;
    let (multisig_pda, _) = derive_multisig_pda(&create_key)?;

    let borsh_members: Vec<BorshMember> = config
        .members
        .iter()
        .map(|m| -> SquadsResult<BorshMember> {
            let pk = parse_pubkey(&m.key)?;
            Ok(BorshMember { key: pk.to_bytes(), permissions: m.permissions })
        })
        .collect::<SquadsResult<_>>()?;

    let args = MultisigCreateArgsV2 {
        config_authority: None,
        threshold: config.threshold,
        members: borsh_members,
        time_lock: config.time_lock,
        rent_collector: None,
        memo: config.memo.clone(),
    };

    let mut data = discriminator("multisig_create_v2").to_vec();
    borsh::to_writer(&mut data, &args)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;

    let accounts = vec![
        AccountMeta::new(multisig_pda, false),
        AccountMeta::new_readonly(create_key, true),
        AccountMeta::new(*creator, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Ok((Instruction { program_id, accounts, data }, multisig_pda))
}

// ─── Utility ──────────────────────────────────────────────────────────────────

/// Parse a base58 pubkey string.
pub fn parse_pubkey(s: &str) -> SquadsResult<Pubkey> {
    Pubkey::from_str(s)
        .map_err(|e| SquadsError::InvalidPubkey(s.to_owned(), e.to_string()))
}

/// Convert a `solana_sdk::instruction::Instruction` to [`InstructionResult`].
pub fn instruction_to_json(ix: &Instruction) -> InstructionResult {
    use base64::{engine::general_purpose::STANDARD, Engine};
    InstructionResult {
        program_id: ix.program_id.to_string(),
        accounts: ix
            .accounts
            .iter()
            .map(|a| AccountMetaJson {
                pubkey: a.pubkey.to_string(),
                is_signer: a.is_signer,
                is_writable: a.is_writable,
            })
            .collect(),
        data_b64: STANDARD.encode(&ix.data),
    }
}

// ─── Validate config (public helper) ─────────────────────────────────────────

/// Validate a [`MultisigConfig`] without building the instruction.
/// Returns `Ok(())` on success, or a descriptive error.
pub fn validate_config(config: &MultisigConfig) -> SquadsResult<()> {
    if config.members.is_empty() {
        return Err(SquadsError::EmptyMembers);
    }
    if config.threshold == 0 || config.threshold as usize > config.members.len() {
        return Err(SquadsError::InvalidThreshold {
            threshold: config.threshold,
            members: config.members.len(),
        });
    }
    let mut seen = std::collections::HashSet::new();
    for m in &config.members {
        parse_pubkey(&m.key)?;
        if !seen.insert(m.key.clone()) {
            return Err(SquadsError::DuplicateMember(m.key.clone()));
        }
    }
    parse_pubkey(&config.create_key)?;
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Member, MultisigConfig};
    use solana_sdk::pubkey::Pubkey;

    fn alice() -> String { Pubkey::new_unique().to_string() }
    fn bob()   -> String { Pubkey::new_unique().to_string() }
    fn carol() -> String { Pubkey::new_unique().to_string() }

    fn basic_config() -> MultisigConfig {
        MultisigConfig {
            create_key: Pubkey::new_unique().to_string(),
            members: vec![
                Member::full(alice()),
                Member::full(bob()),
                Member::full(carol()),
            ],
            threshold: 2,
            time_lock: 0,
            memo: None,
        }
    }

    // ── Discriminator ────────────────────────────────────────────────────────

    #[test]
    fn discriminator_is_8_bytes() {
        let d = discriminator("multisig_create_v2");
        assert_eq!(d.len(), 8);
    }

    #[test]
    fn discriminators_are_distinct() {
        let d1 = discriminator("multisig_create_v2");
        let d2 = discriminator("vault_transaction_create");
        let d3 = discriminator("proposal_create");
        let d4 = discriminator("proposal_approve");
        let d5 = discriminator("proposal_reject");
        let d6 = discriminator("vault_transaction_execute");
        let all = [d1, d2, d3, d4, d5, d6];
        // All 6 discriminators must be unique.
        let set: std::collections::HashSet<_> = all.iter().collect();
        assert_eq!(set.len(), 6, "discriminator collision detected");
    }

    #[test]
    fn discriminator_is_deterministic() {
        let a = discriminator("proposal_approve");
        let b = discriminator("proposal_approve");
        assert_eq!(a, b);
    }

    // ── PDA derivation ───────────────────────────────────────────────────────

    #[test]
    fn derive_multisig_pda_is_deterministic() {
        let ck = Pubkey::new_unique();
        let (pda1, bump1) = derive_multisig_pda(&ck).unwrap();
        let (pda2, bump2) = derive_multisig_pda(&ck).unwrap();
        assert_eq!(pda1, pda2);
        assert_eq!(bump1, bump2);
    }

    #[test]
    fn derive_multisig_pda_different_create_keys() {
        let ck1 = Pubkey::new_unique();
        let ck2 = Pubkey::new_unique();
        let (pda1, _) = derive_multisig_pda(&ck1).unwrap();
        let (pda2, _) = derive_multisig_pda(&ck2).unwrap();
        assert_ne!(pda1, pda2);
    }

    #[test]
    fn derive_vault_pda_is_deterministic() {
        let ms = Pubkey::new_unique();
        let (v1, b1) = derive_vault_pda(&ms, 0).unwrap();
        let (v2, b2) = derive_vault_pda(&ms, 0).unwrap();
        assert_eq!(v1, v2);
        assert_eq!(b1, b2);
    }

    #[test]
    fn derive_vault_pda_different_indices() {
        let ms = Pubkey::new_unique();
        let (v0, _) = derive_vault_pda(&ms, 0).unwrap();
        let (v1, _) = derive_vault_pda(&ms, 1).unwrap();
        assert_ne!(v0, v1);
    }

    #[test]
    fn derive_pda_info_roundtrip() {
        let ck = Pubkey::new_unique().to_string();
        let info = derive_pda_info(&ck).unwrap();
        assert!(!info.multisig_pda.is_empty());
        assert!(!info.vault_pda.is_empty());
        assert_ne!(info.multisig_pda, info.vault_pda);
    }

    // ── Instruction builder ───────────────────────────────────────────────────

    #[test]
    fn create_multisig_ix_builds_successfully() {
        let cfg = basic_config();
        let (ix, ms_pda) = create_multisig_ix(&cfg).unwrap();
        assert!(!ix.data.is_empty());
        assert!(!ms_pda.to_string().is_empty());
    }

    #[test]
    fn create_multisig_ix_has_correct_program_id() {
        let cfg = basic_config();
        let (ix, _) = create_multisig_ix(&cfg).unwrap();
        assert_eq!(ix.program_id.to_string(), SQUADS_V4_PROGRAM_ID);
    }

    #[test]
    fn create_multisig_ix_has_four_accounts() {
        let cfg = basic_config();
        let (ix, _) = create_multisig_ix(&cfg).unwrap();
        assert_eq!(ix.accounts.len(), 4);
    }

    #[test]
    fn create_multisig_ix_discriminator_prefix() {
        let cfg = basic_config();
        let (ix, _) = create_multisig_ix(&cfg).unwrap();
        let expected = discriminator("multisig_create_v2");
        assert_eq!(&ix.data[..8], &expected);
    }

    #[test]
    fn create_multisig_ix_rejects_empty_members() {
        let cfg = MultisigConfig {
            create_key: Pubkey::new_unique().to_string(),
            members: vec![],
            threshold: 1,
            time_lock: 0,
            memo: None,
        };
        assert!(matches!(create_multisig_ix(&cfg), Err(SquadsError::EmptyMembers)));
    }

    #[test]
    fn create_multisig_ix_rejects_threshold_exceeds_members() {
        let cfg = MultisigConfig {
            create_key: Pubkey::new_unique().to_string(),
            members: vec![Member::full(alice()), Member::full(bob())],
            threshold: 3,
            time_lock: 0,
            memo: None,
        };
        assert!(matches!(
            create_multisig_ix(&cfg),
            Err(SquadsError::InvalidThreshold { .. })
        ));
    }

    #[test]
    fn create_multisig_ix_rejects_zero_threshold() {
        let cfg = MultisigConfig {
            create_key: Pubkey::new_unique().to_string(),
            members: vec![Member::full(alice())],
            threshold: 0,
            time_lock: 0,
            memo: None,
        };
        assert!(matches!(
            create_multisig_ix(&cfg),
            Err(SquadsError::InvalidThreshold { .. })
        ));
    }

    #[test]
    fn create_multisig_ix_rejects_duplicate_member() {
        let key = alice();
        let cfg = MultisigConfig {
            create_key: Pubkey::new_unique().to_string(),
            members: vec![Member::full(key.clone()), Member::full(key)],
            threshold: 1,
            time_lock: 0,
            memo: None,
        };
        assert!(matches!(create_multisig_ix(&cfg), Err(SquadsError::DuplicateMember(_))));
    }

    #[test]
    fn validate_config_ok() {
        let cfg = basic_config();
        assert!(validate_config(&cfg).is_ok());
    }
}