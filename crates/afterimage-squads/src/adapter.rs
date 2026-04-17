//! AirSign ↔ Squads v4 adapter layer.
//!
//! Converts an [`ApprovalRequest`] (everything known from the off-line signer
//! side) into a complete `solana_sdk::transaction::Transaction` containing a
//! `proposal_approve` instruction, ready to be serialised and injected into
//! the AirSign air-gap signing flow.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use solana_sdk::{
    hash::Hash,
    message::Message,
    transaction::Transaction,
};

use crate::{
    error::{SquadsError, SquadsResult},
    multisig::parse_pubkey,
    types::{ApprovalRequest, InstructionResult},
    vault_tx::proposal_approve_ix,
};

// ─── AirSignSquadsPayload ────────────────────────────────────────────────────

/// A self-contained JSON payload that can be passed through the AirSign QR
/// tunnel.  The online machine builds this; the offline signer decodes it,
/// signs the inner transaction, and passes the signature back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AirSignSquadsPayload {
    /// Human-readable label displayed on the air-gap device.
    pub label: String,
    /// Squads multisig PDA (base58).
    pub multisig_pda: String,
    /// Index of the proposal being approved.
    pub transaction_index: u64,
    /// The serialised unsigned `Transaction` (base64-encoded bincode).
    pub transaction_b64: String,
    /// The signing pubkey expected to sign (approver).
    pub signer: String,
}

// ─── build_approval_transaction ──────────────────────────────────────────────

/// Build an unsigned Solana `Transaction` containing a single
/// `proposal_approve` instruction.
///
/// The `recent_blockhash` can be `Hash::default()` for offline inspection;
/// supply a real blockhash before broadcasting.
pub fn build_approval_transaction(
    req: &ApprovalRequest,
    recent_blockhash: Hash,
) -> SquadsResult<Transaction> {
    if req.transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }

    let ix = proposal_approve_ix(
        &req.multisig_pda,
        &req.approver,
        req.transaction_index,
        req.memo.clone(),
    )?;

    let approver = parse_pubkey(&req.approver)?;
    let message = Message::new_with_blockhash(&[ix], Some(&approver), &recent_blockhash);
    let tx = Transaction::new_unsigned(message);
    Ok(tx)
}

/// Serialise an unsigned approval `Transaction` to base64-encoded bincode.
pub fn transaction_to_b64(tx: &Transaction) -> SquadsResult<String> {
    let bytes = bincode::serialize(tx)
        .map_err(|e| SquadsError::Serialization(e.to_string()))?;
    Ok(STANDARD.encode(bytes))
}

/// Deserialise a base64-encoded bincode `Transaction`.
pub fn transaction_from_b64(b64: &str) -> SquadsResult<Transaction> {
    let bytes = STANDARD
        .decode(b64)
        .map_err(|e| SquadsError::Base64(e.to_string()))?;
    bincode::deserialize(&bytes)
        .map_err(|e| SquadsError::Serialization(e.to_string()))
}

// ─── build_airsign_payload ───────────────────────────────────────────────────

/// Build a complete [`AirSignSquadsPayload`] for the given approval request.
///
/// Pass `recent_blockhash = Hash::default()` when building the payload
/// offline; replace with a live blockhash just before sending through the
/// QR tunnel.
pub fn build_airsign_payload(
    req: &ApprovalRequest,
    recent_blockhash: Hash,
) -> SquadsResult<AirSignSquadsPayload> {
    let tx = build_approval_transaction(req, recent_blockhash)?;
    let tx_b64 = transaction_to_b64(&tx)?;

    Ok(AirSignSquadsPayload {
        label: format!(
            "Squads approve: multisig {} tx #{}",
            shorten(&req.multisig_pda),
            req.transaction_index
        ),
        multisig_pda: req.multisig_pda.clone(),
        transaction_index: req.transaction_index,
        transaction_b64: tx_b64,
        signer: req.approver.clone(),
    })
}

/// Serialise an [`AirSignSquadsPayload`] to a compact JSON string.
pub fn payload_to_json(payload: &AirSignSquadsPayload) -> SquadsResult<String> {
    serde_json::to_string(payload).map_err(SquadsError::Json)
}

/// Deserialise an [`AirSignSquadsPayload`] from JSON.
pub fn payload_from_json(json: &str) -> SquadsResult<AirSignSquadsPayload> {
    serde_json::from_str(json).map_err(SquadsError::Json)
}

// ─── Instruction-level adapter (no Transaction wrapping) ─────────────────────

/// Build *only* the `proposal_approve` instruction (as [`InstructionResult`])
/// without wrapping it in a `Transaction`.  Useful when the caller wants to
/// batch multiple instructions.
pub fn approval_instruction_json(req: &ApprovalRequest) -> SquadsResult<InstructionResult> {
    if req.transaction_index == 0 {
        return Err(SquadsError::InvalidTransactionIndex);
    }
    crate::vault_tx::proposal_approve_json(
        &req.multisig_pda,
        &req.approver,
        req.transaction_index,
        req.memo.clone(),
    )
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Shorten a base58 address to `XXXX…XXXX` for display.
fn shorten(addr: &str) -> String {
    if addr.len() <= 12 {
        return addr.to_string();
    }
    format!("{}…{}", &addr[..6], &addr[addr.len() - 4..])
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ApprovalRequest;
    use solana_sdk::pubkey::Pubkey;

    fn req() -> ApprovalRequest {
        ApprovalRequest {
            multisig_pda: Pubkey::new_unique().to_string(),
            transaction_index: 1,
            approver: Pubkey::new_unique().to_string(),
            memo: None,
        }
    }

    // ── build_approval_transaction ───────────────────────────────────────────

    #[test]
    fn build_approval_transaction_creates_transaction() {
        let tx = build_approval_transaction(&req(), Hash::default()).unwrap();
        // Must contain exactly one instruction.
        assert_eq!(tx.message.instructions.len(), 1);
    }

    #[test]
    fn build_approval_transaction_has_correct_signer() {
        let r = req();
        let approver = parse_pubkey(&r.approver).unwrap();
        let tx = build_approval_transaction(&r, Hash::default()).unwrap();
        // The fee payer (account index 0) should be the approver.
        assert_eq!(tx.message.account_keys[0], approver);
    }

    #[test]
    fn build_approval_transaction_rejects_zero_index() {
        let mut r = req();
        r.transaction_index = 0;
        assert!(matches!(
            build_approval_transaction(&r, Hash::default()),
            Err(SquadsError::InvalidTransactionIndex)
        ));
    }

    // ── serialisation roundtrip ──────────────────────────────────────────────

    #[test]
    fn transaction_b64_roundtrip() {
        let tx = build_approval_transaction(&req(), Hash::default()).unwrap();
        let b64 = transaction_to_b64(&tx).unwrap();
        let decoded = transaction_from_b64(&b64).unwrap();
        // Message bytes should be identical after roundtrip.
        assert_eq!(tx.message.serialize(), decoded.message.serialize());
    }

    #[test]
    fn transaction_b64_is_valid_base64() {
        let tx = build_approval_transaction(&req(), Hash::default()).unwrap();
        let b64 = transaction_to_b64(&tx).unwrap();
        assert!(STANDARD.decode(&b64).is_ok());
    }

    // ── AirSignSquadsPayload ─────────────────────────────────────────────────

    #[test]
    fn build_airsign_payload_roundtrip_json() {
        let r = req();
        let payload = build_airsign_payload(&r, Hash::default()).unwrap();
        let json = payload_to_json(&payload).unwrap();
        let decoded = payload_from_json(&json).unwrap();
        assert_eq!(decoded.multisig_pda, r.multisig_pda);
        assert_eq!(decoded.transaction_index, 1);
        assert_eq!(decoded.signer, r.approver);
    }

    #[test]
    fn build_airsign_payload_label_contains_index() {
        let r = req();
        let payload = build_airsign_payload(&r, Hash::default()).unwrap();
        assert!(payload.label.contains("1"), "label should mention tx index 1");
    }

    #[test]
    fn build_airsign_payload_transaction_b64_non_empty() {
        let payload = build_airsign_payload(&req(), Hash::default()).unwrap();
        assert!(!payload.transaction_b64.is_empty());
    }

    // ── approval_instruction_json ────────────────────────────────────────────

    #[test]
    fn approval_instruction_json_has_program_id() {
        let r = req();
        let result = approval_instruction_json(&r).unwrap();
        assert_eq!(result.program_id, crate::types::SQUADS_V4_PROGRAM_ID);
    }

    #[test]
    fn approval_instruction_json_has_three_accounts() {
        let r = req();
        let result = approval_instruction_json(&r).unwrap();
        assert_eq!(result.accounts.len(), 3);
    }

    #[test]
    fn approval_instruction_json_rejects_zero_index() {
        let mut r = req();
        r.transaction_index = 0;
        assert!(matches!(
            approval_instruction_json(&r),
            Err(SquadsError::InvalidTransactionIndex)
        ));
    }

    // ── shorten ──────────────────────────────────────────────────────────────

    #[test]
    fn shorten_long_address() {
        let addr = "SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf";
        let short = shorten(addr);
        assert!(short.contains('…'));
        assert!(short.len() < addr.len());
    }

    #[test]
    fn shorten_short_address_unchanged() {
        let addr = "short";
        assert_eq!(shorten(addr), "short");
    }

    // ── with memo ────────────────────────────────────────────────────────────

    #[test]
    fn build_approval_with_memo() {
        let mut r = req();
        r.memo = Some("approved".into());
        let tx = build_approval_transaction(&r, Hash::default()).unwrap();
        assert_eq!(tx.message.instructions.len(), 1);
    }
}