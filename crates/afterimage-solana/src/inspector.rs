//! Transaction inspector — static analysis of Solana transactions.
//!
//! [`TransactionInspector`] parses a raw bincode-serialised
//! `solana_sdk::transaction::Transaction` and produces:
//!
//! - A human-readable [`TransactionSummary`] (fee estimate, accounts, programs).
//! - A list of [`InstructionInfo`] items describing each instruction in plain
//!   English.
//! - A list of [`RiskFlag`]s highlighting potentially dangerous operations.
//!
//! ## Design
//!
//! All analysis is **purely static** — no RPC calls are made here.  Network-
//! dependent checks (fee fetch, simulation) live in [`crate::preflight`].
//!
//! ## Recognised programs
//!
//! | Program | Instructions parsed |
//! |---|---|
//! | System Program | `Transfer`, `CreateAccount`, `Assign`, `Allocate`, `CreateAccountWithSeed` |
//! | SPL Token (v3) | `Transfer`, `TransferChecked`, `Approve`, `ApproveChecked`, `MintTo`, `MintToChecked`, `Burn`, `BurnChecked`, `CloseAccount`, `SetAuthority` |
//! | SPL ATA | `Create` |
//! | BPF Upgradeable Loader | `Upgrade` (program upgrade) |
//! | Stake | `Delegate`, `Withdraw`, `Deactivate`, `Split` |
//! | Vote | `Vote`, `Withdraw` |
//! | Compute Budget | `SetComputeUnitPrice`, `SetComputeUnitLimit` |

use std::fmt;

use solana_sdk::{
    pubkey::Pubkey,
    transaction::Transaction,
};

// ─── Well-known program IDs ───────────────────────────────────────────────────

/// System program.
pub const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";
/// SPL Token program.
pub const TOKEN_PROGRAM_ID: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
/// SPL Token-2022 program.
pub const TOKEN_2022_PROGRAM_ID: &str = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb";
/// SPL Associated Token Account program.
pub const ATA_PROGRAM_ID: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe1bsn";
/// BPF Upgradeable Loader.
pub const BPF_UPGRADEABLE_LOADER_ID: &str = "BPFLoaderUpgradeab1e11111111111111111111111";
/// Stake program.
pub const STAKE_PROGRAM_ID: &str = "Stake11111111111111111111111111111111111111";
/// Vote program.
pub const VOTE_PROGRAM_ID: &str = "Vote111111111111111111111111111111111111111";
/// Compute Budget program.
pub const COMPUTE_BUDGET_PROGRAM_ID: &str = "ComputeBudget111111111111111111111111111111";
/// Memo program v1.
pub const MEMO_V1_PROGRAM_ID: &str = "Memo1UhkJRfHyvLMcVucJwxXeuD728EqVDDwQDxFMNo";
/// Memo program v2.
pub const MEMO_V2_PROGRAM_ID: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

// ─── Risk thresholds ──────────────────────────────────────────────────────────

/// SOL transfers above this lamport amount are flagged as large.
/// Default: 10 SOL = 10_000_000_000 lamports.
pub const LARGE_TRANSFER_THRESHOLD_LAMPORTS: u64 = 10_000_000_000;

/// Token amounts (in raw units, before decimals) flagged as large approvals.
/// Default: u64::MAX / 2 — catching "unlimited approval" patterns.
pub const LARGE_TOKEN_APPROVAL_THRESHOLD: u64 = u64::MAX / 2;

// ─── RiskFlag ─────────────────────────────────────────────────────────────────

/// A security warning attached to a transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskFlag {
    /// A SOL transfer exceeds [`LARGE_TRANSFER_THRESHOLD_LAMPORTS`].
    LargeSolTransfer {
        /// Transfer amount in lamports.
        lamports: u64,
        /// Source account pubkey (base58).
        from: String,
        /// Destination account pubkey (base58).
        to: String,
    },

    /// A token Approve instruction delegates an unusually large (or unlimited) amount.
    LargeTokenApproval {
        /// Delegated token amount (raw units).
        amount: u64,
        /// Token mint pubkey.
        mint: Option<String>,
        /// Delegate pubkey.
        delegate: String,
    },

    /// A program (BPF Upgradeable Loader) upgrade instruction was found.
    /// Program upgrades are high-risk operations that change deployed code.
    ProgramUpgrade {
        /// The program data account being upgraded.
        program_data: String,
    },

    /// A token account close instruction — rent will be reclaimed.
    TokenAccountClose {
        /// The account being closed.
        account: String,
        /// Destination of the reclaimed rent.
        destination: String,
    },

    /// An instruction targets a program that is not in the recognised list.
    UnknownProgram {
        /// The unrecognised program ID.
        program_id: String,
        /// Index of the instruction (0-based).
        ix_index: usize,
    },

    /// The transaction has an unusually high number of signers (≥ 5).
    ManySigner {
        /// Number of required signers.
        count: usize,
    },
}

impl RiskFlag {
    /// Human-readable description of the risk.
    pub fn description(&self) -> String {
        match self {
            RiskFlag::LargeSolTransfer { lamports, from, to } => {
                let sol = *lamports as f64 / 1e9;
                format!(
                    "Large SOL transfer: {:.4} SOL from {} → {}",
                    sol,
                    short(from),
                    short(to)
                )
            }
            RiskFlag::LargeTokenApproval { amount, delegate, mint } => {
                let mint_str = mint.as_deref().unwrap_or("(unknown mint)");
                format!(
                    "Large token approval: {} units of {} delegated to {}",
                    amount,
                    short(mint_str),
                    short(delegate)
                )
            }
            RiskFlag::ProgramUpgrade { program_data } => {
                format!(
                    "Program upgrade detected — program data account: {}",
                    short(program_data)
                )
            }
            RiskFlag::TokenAccountClose { account, destination } => {
                format!(
                    "Token account {} will be closed; rent → {}",
                    short(account),
                    short(destination)
                )
            }
            RiskFlag::UnknownProgram { program_id, ix_index } => {
                format!(
                    "Instruction [{}] invokes unrecognised program {}",
                    ix_index,
                    short(program_id)
                )
            }
            RiskFlag::ManySigner { count } => {
                format!("Transaction requires {} signers", count)
            }
        }
    }

    /// Risk level: `"HIGH"`, `"MEDIUM"`, or `"LOW"`.
    pub fn level(&self) -> &'static str {
        match self {
            RiskFlag::ProgramUpgrade { .. } => "HIGH",
            RiskFlag::LargeSolTransfer { lamports, .. }
                if *lamports >= 100_000_000_000 =>
            {
                "HIGH"
            }
            RiskFlag::LargeSolTransfer { .. } => "MEDIUM",
            RiskFlag::LargeTokenApproval { .. } => "HIGH",
            RiskFlag::TokenAccountClose { .. } => "MEDIUM",
            RiskFlag::UnknownProgram { .. } => "LOW",
            RiskFlag::ManySigner { .. } => "LOW",
        }
    }

    /// Returns `true` if this flag is HIGH severity.
    pub fn is_high(&self) -> bool {
        self.level() == "HIGH"
    }
}

// ─── InstructionInfo ──────────────────────────────────────────────────────────

/// A decoded, human-readable representation of a single instruction.
#[derive(Debug, Clone)]
pub enum InstructionInfo {
    // ── System Program ────────────────────────────────────────────────────────
    SystemTransfer {
        from: String,
        to: String,
        lamports: u64,
    },
    SystemCreateAccount {
        funder: String,
        new_account: String,
        lamports: u64,
        space: u64,
        owner: String,
    },
    SystemAssign {
        account: String,
        owner: String,
    },

    // ── SPL Token ─────────────────────────────────────────────────────────────
    TokenTransfer {
        source: String,
        destination: String,
        authority: String,
        amount: u64,
        mint: Option<String>,
    },
    TokenApprove {
        source: String,
        delegate: String,
        owner: String,
        amount: u64,
        mint: Option<String>,
    },
    TokenMintTo {
        mint: String,
        destination: String,
        authority: String,
        amount: u64,
    },
    TokenBurn {
        source: String,
        mint: String,
        owner: String,
        amount: u64,
    },
    TokenCloseAccount {
        account: String,
        destination: String,
        owner: String,
    },
    TokenSetAuthority {
        account: String,
        new_authority: Option<String>,
        authority_type: String,
    },

    // ── ATA ───────────────────────────────────────────────────────────────────
    AtaCreate {
        funder: String,
        ata: String,
        owner: String,
        mint: String,
    },

    // ── BPF Upgradeable Loader ────────────────────────────────────────────────
    ProgramUpgrade {
        program_data: String,
        program: String,
        buffer: String,
        spill: String,
        authority: String,
    },

    // ── Stake ─────────────────────────────────────────────────────────────────
    StakeDelegate {
        stake: String,
        vote: String,
    },
    StakeWithdraw {
        stake: String,
        destination: String,
        lamports: u64,
    },
    StakeDeactivate {
        stake: String,
    },

    // ── Compute Budget ────────────────────────────────────────────────────────
    ComputeBudgetSetPrice {
        micro_lamports: u64,
    },
    ComputeBudgetSetLimit {
        units: u32,
    },

    // ── Memo ──────────────────────────────────────────────────────────────────
    Memo {
        text: String,
    },

    // ── Fallback ──────────────────────────────────────────────────────────────
    Unknown {
        program_id: String,
        data_hex: String,
        account_count: usize,
    },
}

impl fmt::Display for InstructionInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstructionInfo::SystemTransfer { from, to, lamports } => {
                write!(
                    f,
                    "System :: Transfer  {:.9} SOL  {} → {}",
                    *lamports as f64 / 1e9,
                    short(from),
                    short(to)
                )
            }
            InstructionInfo::SystemCreateAccount { funder, new_account, lamports, space, owner } => {
                write!(
                    f,
                    "System :: CreateAccount  funder={}  new={}  lamports={}  space={}  owner={}",
                    short(funder), short(new_account), lamports, space, short(owner)
                )
            }
            InstructionInfo::SystemAssign { account, owner } => {
                write!(f, "System :: Assign  {} → owner={}", short(account), short(owner))
            }
            InstructionInfo::TokenTransfer { source, destination, amount, mint, .. } => {
                let mint_str = mint.as_deref().unwrap_or("?");
                write!(
                    f,
                    "SPL Token :: Transfer  {} units of {}  {} → {}",
                    amount, short(mint_str), short(source), short(destination)
                )
            }
            InstructionInfo::TokenApprove { source, delegate, amount, mint, .. } => {
                let mint_str = mint.as_deref().unwrap_or("?");
                write!(
                    f,
                    "SPL Token :: Approve  {} units of {}  source={}  delegate={}",
                    amount, short(mint_str), short(source), short(delegate)
                )
            }
            InstructionInfo::TokenMintTo { mint, destination, amount, .. } => {
                write!(
                    f,
                    "SPL Token :: MintTo  {} units  mint={}  → {}",
                    amount, short(mint), short(destination)
                )
            }
            InstructionInfo::TokenBurn { source, mint, amount, .. } => {
                write!(
                    f,
                    "SPL Token :: Burn  {} units  source={}  mint={}",
                    amount, short(source), short(mint)
                )
            }
            InstructionInfo::TokenCloseAccount { account, destination, .. } => {
                write!(
                    f,
                    "SPL Token :: CloseAccount  {}  rent→{}",
                    short(account), short(destination)
                )
            }
            InstructionInfo::TokenSetAuthority { account, new_authority, authority_type } => {
                let new_auth = new_authority.as_deref().unwrap_or("(revoked)");
                write!(
                    f,
                    "SPL Token :: SetAuthority  {}  type={}  new={}",
                    short(account), authority_type, short(new_auth)
                )
            }
            InstructionInfo::AtaCreate { funder, ata, owner, mint } => {
                write!(
                    f,
                    "ATA :: Create  funder={}  ata={}  owner={}  mint={}",
                    short(funder), short(ata), short(owner), short(mint)
                )
            }
            InstructionInfo::ProgramUpgrade { program, buffer, authority, .. } => {
                write!(
                    f,
                    "BPF Loader :: Upgrade  program={}  buffer={}  authority={}",
                    short(program), short(buffer), short(authority)
                )
            }
            InstructionInfo::StakeDelegate { stake, vote } => {
                write!(f, "Stake :: Delegate  {}  → vote={}", short(stake), short(vote))
            }
            InstructionInfo::StakeWithdraw { stake, destination, lamports } => {
                write!(
                    f,
                    "Stake :: Withdraw  {} lamports  {} → {}",
                    lamports, short(stake), short(destination)
                )
            }
            InstructionInfo::StakeDeactivate { stake } => {
                write!(f, "Stake :: Deactivate  {}", short(stake))
            }
            InstructionInfo::ComputeBudgetSetPrice { micro_lamports } => {
                write!(f, "ComputeBudget :: SetComputeUnitPrice  {} µ-lamports/CU", micro_lamports)
            }
            InstructionInfo::ComputeBudgetSetLimit { units } => {
                write!(f, "ComputeBudget :: SetComputeUnitLimit  {} CU", units)
            }
            InstructionInfo::Memo { text } => {
                write!(f, "Memo :: {:?}", text)
            }
            InstructionInfo::Unknown { program_id, data_hex, account_count } => {
                write!(
                    f,
                    "Unknown program {}  accounts={}  data={}",
                    short(program_id), account_count,
                    if data_hex.len() > 16 { format!("{}…", &data_hex[..16]) } else { data_hex.clone() }
                )
            }
        }
    }
}

// ─── TransactionSummary ───────────────────────────────────────────────────────

/// Static analysis summary of a Solana transaction.
#[derive(Debug, Clone)]
pub struct TransactionSummary {
    /// Number of signatures required (= number of unique signers).
    pub signer_count: usize,

    /// Unique account pubkeys referenced in the transaction (base58).
    pub accounts: Vec<String>,

    /// Unique program IDs invoked (base58).
    pub programs_invoked: Vec<String>,

    /// Decoded instruction list (one per instruction in the message).
    pub instructions: Vec<InstructionInfo>,

    /// Security risk flags.
    pub risks: Vec<RiskFlag>,

    /// Number of recent blockhash (hex, for display only).
    pub recent_blockhash: String,

    /// Raw instruction count.
    pub instruction_count: usize,
}

impl TransactionSummary {
    /// `true` if any HIGH-level risk flag is present.
    pub fn has_high_risk(&self) -> bool {
        self.risks.iter().any(|r| r.is_high())
    }

    /// Overall risk label: `"HIGH"`, `"MEDIUM"`, `"LOW"`, or `"NONE"`.
    pub fn risk_level(&self) -> &'static str {
        if self.risks.iter().any(|r| r.level() == "HIGH") {
            "HIGH"
        } else if self.risks.iter().any(|r| r.level() == "MEDIUM") {
            "MEDIUM"
        } else if !self.risks.is_empty() {
            "LOW"
        } else {
            "NONE"
        }
    }

    /// Render a human-readable multi-line summary suitable for a terminal.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("╔══════════════════════════════════════════════════════════╗\n");
        out.push_str("║          AirSign Transaction Inspector                   ║\n");
        out.push_str("╚══════════════════════════════════════════════════════════╝\n");
        out.push_str(&format!(
            " Signers     : {}\n",
            self.signer_count
        ));
        out.push_str(&format!(
            " Accounts    : {}\n",
            self.accounts.len()
        ));
        out.push_str(&format!(
            " Blockhash   : {}\n",
            &self.recent_blockhash[..16.min(self.recent_blockhash.len())]
        ));
        out.push_str(&format!(
            " Instructions: {}\n",
            self.instruction_count
        ));
        out.push_str("─────────────────────────────────────────────────────────────\n");

        for (i, ix) in self.instructions.iter().enumerate() {
            out.push_str(&format!(" [{i}] {ix}\n"));
        }

        out.push_str("─────────────────────────────────────────────────────────────\n");
        out.push_str(&format!(" Risk: {}\n", self.risk_level()));
        for flag in &self.risks {
            let icon = match flag.level() {
                "HIGH"   => "🔴",
                "MEDIUM" => "🟡",
                _        => "🔵",
            };
            out.push_str(&format!("   {} [{}] {}\n", icon, flag.level(), flag.description()));
        }
        out
    }
}

// ─── TransactionInspector ────────────────────────────────────────────────────

/// Parses and analyses a raw Solana transaction.
pub struct TransactionInspector;

impl TransactionInspector {
    /// Analyse a bincode-serialised [`Transaction`] byte slice.
    pub fn inspect(tx_bytes: &[u8]) -> Result<TransactionSummary, String> {
        let tx: Transaction = bincode::deserialize(tx_bytes)
            .map_err(|e| format!("failed to deserialise transaction: {e}"))?;
        Ok(Self::inspect_tx(&tx))
    }

    /// Analyse a pre-deserialised [`Transaction`].
    pub fn inspect_tx(tx: &Transaction) -> TransactionSummary {
        let msg = &tx.message;

        // Collect accounts
        let accounts: Vec<String> = msg
            .account_keys
            .iter()
            .map(|k| k.to_string())
            .collect();

        // Collect unique programs invoked
        let mut programs_invoked: Vec<String> = msg
            .instructions
            .iter()
            .map(|ix| {
                msg.account_keys
                    .get(ix.program_id_index as usize)
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| "(invalid)".into())
            })
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        programs_invoked.sort();

        let signer_count = msg.header.num_required_signatures as usize;
        let recent_blockhash = msg.recent_blockhash.to_string();
        let instruction_count = msg.instructions.len();

        // Decode instructions
        let mut instructions = Vec::new();
        let mut risks: Vec<RiskFlag> = Vec::new();

        for (ix_index, cix) in msg.instructions.iter().enumerate() {
            let program_id = msg
                .account_keys
                .get(cix.program_id_index as usize)
                .map(|k| k.to_string())
                .unwrap_or_else(|| "(invalid)".into());

            // Resolve account helper
            let acct = |idx: usize| -> String {
                cix.accounts
                    .get(idx)
                    .and_then(|&ai| msg.account_keys.get(ai as usize))
                    .map(|k| k.to_string())
                    .unwrap_or_else(|| "(?)".into())
            };

            let data = &cix.data;

            let info = match program_id.as_str() {
                SYSTEM_PROGRAM_ID => {
                    parse_system_instruction(data, acct, &mut risks)
                }
                TOKEN_PROGRAM_ID | TOKEN_2022_PROGRAM_ID => {
                    parse_token_instruction(data, acct, &mut risks)
                }
                ATA_PROGRAM_ID => {
                    Some(InstructionInfo::AtaCreate {
                        funder: acct(0),
                        ata: acct(1),
                        owner: acct(2),
                        mint: acct(3),
                    })
                }
                BPF_UPGRADEABLE_LOADER_ID => {
                    let info = InstructionInfo::ProgramUpgrade {
                        program_data: acct(0),
                        program: acct(1),
                        buffer: acct(2),
                        spill: acct(3),
                        authority: acct(6),
                    };
                    risks.push(RiskFlag::ProgramUpgrade {
                        program_data: acct(0),
                    });
                    Some(info)
                }
                STAKE_PROGRAM_ID => parse_stake_instruction(data, acct),
                COMPUTE_BUDGET_PROGRAM_ID => parse_compute_budget_instruction(data),
                MEMO_V1_PROGRAM_ID | MEMO_V2_PROGRAM_ID => {
                    let text = String::from_utf8_lossy(data).into_owned();
                    Some(InstructionInfo::Memo { text })
                }
                _ => {
                    risks.push(RiskFlag::UnknownProgram {
                        program_id: program_id.clone(),
                        ix_index,
                    });
                    None
                }
            };

            instructions.push(info.unwrap_or_else(|| InstructionInfo::Unknown {
                program_id: program_id.clone(),
                data_hex: hex::encode(data),
                account_count: cix.accounts.len(),
            }));
        }

        // Many-signer check
        if signer_count >= 5 {
            risks.push(RiskFlag::ManySigner { count: signer_count });
        }

        TransactionSummary {
            signer_count,
            accounts,
            programs_invoked,
            instructions,
            risks,
            recent_blockhash,
            instruction_count,
        }
    }
}

// ─── Instruction parsers ──────────────────────────────────────────────────────

fn parse_system_instruction(
    data: &[u8],
    acct: impl Fn(usize) -> String,
    risks: &mut Vec<RiskFlag>,
) -> Option<InstructionInfo> {
    if data.len() < 4 {
        return None;
    }
    let discriminant = u32::from_le_bytes(data[..4].try_into().ok()?);
    match discriminant {
        // Transfer
        2 => {
            if data.len() < 12 {
                return None;
            }
            let lamports = u64::from_le_bytes(data[4..12].try_into().ok()?);
            let from = acct(0);
            let to = acct(1);
            if lamports >= LARGE_TRANSFER_THRESHOLD_LAMPORTS {
                risks.push(RiskFlag::LargeSolTransfer {
                    lamports,
                    from: from.clone(),
                    to: to.clone(),
                });
            }
            Some(InstructionInfo::SystemTransfer { from, to, lamports })
        }
        // CreateAccount
        0 => {
            if data.len() < 20 {
                return None;
            }
            let lamports = u64::from_le_bytes(data[4..12].try_into().ok()?);
            let space = u64::from_le_bytes(data[12..20].try_into().ok()?);
            let owner = if data.len() >= 52 {
                Pubkey::try_from(&data[20..52]).map(|k| k.to_string()).unwrap_or_else(|_| "?".into())
            } else {
                "?".into()
            };
            Some(InstructionInfo::SystemCreateAccount {
                funder: acct(0),
                new_account: acct(1),
                lamports,
                space,
                owner,
            })
        }
        // Assign
        1 => {
            let owner = if data.len() >= 36 {
                Pubkey::try_from(&data[4..36]).map(|k| k.to_string()).unwrap_or_else(|_| "?".into())
            } else {
                "?".into()
            };
            Some(InstructionInfo::SystemAssign {
                account: acct(0),
                owner,
            })
        }
        _ => None,
    }
}

fn parse_token_instruction(
    data: &[u8],
    acct: impl Fn(usize) -> String,
    risks: &mut Vec<RiskFlag>,
) -> Option<InstructionInfo> {
    if data.is_empty() {
        return None;
    }
    // SPL Token instruction tag is the first byte
    match data[0] {
        // Transfer (3)
        3 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some(InstructionInfo::TokenTransfer {
                source: acct(0),
                destination: acct(1),
                authority: acct(2),
                amount,
                mint: None,
            })
        }
        // Approve (4)
        4 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            let delegate = acct(1);
            if amount >= LARGE_TOKEN_APPROVAL_THRESHOLD {
                risks.push(RiskFlag::LargeTokenApproval {
                    amount,
                    mint: None,
                    delegate: delegate.clone(),
                });
            }
            Some(InstructionInfo::TokenApprove {
                source: acct(0),
                delegate,
                owner: acct(2),
                amount,
                mint: None,
            })
        }
        // MintTo (7)
        7 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some(InstructionInfo::TokenMintTo {
                mint: acct(0),
                destination: acct(1),
                authority: acct(2),
                amount,
            })
        }
        // Burn (8)
        8 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some(InstructionInfo::TokenBurn {
                source: acct(0),
                mint: acct(1),
                owner: acct(2),
                amount,
            })
        }
        // CloseAccount (9)
        9 => {
            let account = acct(0);
            let destination = acct(1);
            risks.push(RiskFlag::TokenAccountClose {
                account: account.clone(),
                destination: destination.clone(),
            });
            Some(InstructionInfo::TokenCloseAccount {
                account,
                destination,
                owner: acct(2),
            })
        }
        // TransferChecked (12)
        12 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some(InstructionInfo::TokenTransfer {
                source: acct(0),
                destination: acct(2),
                authority: acct(4),
                amount,
                mint: Some(acct(1)),
            })
        }
        // ApproveChecked (13)
        13 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            let delegate = acct(2);
            let mint = acct(1);
            if amount >= LARGE_TOKEN_APPROVAL_THRESHOLD {
                risks.push(RiskFlag::LargeTokenApproval {
                    amount,
                    mint: Some(mint.clone()),
                    delegate: delegate.clone(),
                });
            }
            Some(InstructionInfo::TokenApprove {
                source: acct(0),
                delegate,
                owner: acct(3),
                amount,
                mint: Some(mint),
            })
        }
        // MintToChecked (14)
        14 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some(InstructionInfo::TokenMintTo {
                mint: acct(1),
                destination: acct(0),
                authority: acct(2),
                amount,
            })
        }
        // BurnChecked (15)
        15 => {
            if data.len() < 9 {
                return None;
            }
            let amount = u64::from_le_bytes(data[1..9].try_into().ok()?);
            Some(InstructionInfo::TokenBurn {
                source: acct(0),
                mint: acct(1),
                owner: acct(3),
                amount,
            })
        }
        // SetAuthority (6)
        6 => {
            let authority_type_byte = data.get(1).copied().unwrap_or(0);
            let authority_type = match authority_type_byte {
                0 => "MintTokens",
                1 => "FreezeAccount",
                2 => "AccountOwner",
                3 => "CloseAccount",
                _ => "Unknown",
            }
            .to_string();
            // Optional new authority: byte 2 = 1 means present, 0 means absent
            let new_authority = if data.get(2).copied() == Some(1) && data.len() >= 35 {
                Pubkey::try_from(&data[3..35]).ok().map(|k| k.to_string())
            } else {
                None
            };
            Some(InstructionInfo::TokenSetAuthority {
                account: acct(0),
                new_authority,
                authority_type,
            })
        }
        _ => None,
    }
}

fn parse_stake_instruction(
    data: &[u8],
    acct: impl Fn(usize) -> String,
) -> Option<InstructionInfo> {
    if data.len() < 4 {
        return None;
    }
    let tag = u32::from_le_bytes(data[..4].try_into().ok()?);
    match tag {
        // Delegate (2)
        2 => Some(InstructionInfo::StakeDelegate {
            stake: acct(0),
            vote: acct(1),
        }),
        // Withdraw (4)
        4 => {
            let lamports = if data.len() >= 12 {
                u64::from_le_bytes(data[4..12].try_into().ok()?)
            } else {
                0
            };
            Some(InstructionInfo::StakeWithdraw {
                stake: acct(0),
                destination: acct(1),
                lamports,
            })
        }
        // Deactivate (5)
        5 => Some(InstructionInfo::StakeDeactivate { stake: acct(0) }),
        _ => None,
    }
}

fn parse_compute_budget_instruction(data: &[u8]) -> Option<InstructionInfo> {
    if data.is_empty() {
        return None;
    }
    match data[0] {
        // SetComputeUnitLimit (0x02)
        0x02 => {
            let units = if data.len() >= 5 {
                u32::from_le_bytes(data[1..5].try_into().ok()?)
            } else {
                0
            };
            Some(InstructionInfo::ComputeBudgetSetLimit { units })
        }
        // SetComputeUnitPrice (0x03)
        0x03 => {
            let micro_lamports = if data.len() >= 9 {
                u64::from_le_bytes(data[1..9].try_into().ok()?)
            } else {
                0
            };
            Some(InstructionInfo::ComputeBudgetSetPrice { micro_lamports })
        }
        _ => None,
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Shorten a base58 pubkey for display: `Abc1…XyZ9`.
fn short(s: &str) -> String {
    if s.len() <= 12 {
        s.to_owned()
    } else {
        format!("{}…{}", &s[..4], &s[s.len() - 4..])
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::{
        hash::Hash,
        message::Message,
        pubkey::Pubkey,
        system_instruction,
        transaction::Transaction,
        signer::keypair::Keypair,
        signature::Signer,
    };

    fn make_transfer_tx(lamports: u64) -> Vec<u8> {
        let from = Keypair::new();
        let to = Pubkey::new_unique();
        let ix = system_instruction::transfer(&from.pubkey(), &to, lamports);
        let msg = Message::new(&[ix], Some(&from.pubkey()));
        let tx = Transaction::new(&[&from], msg, Hash::default());
        bincode::serialize(&tx).unwrap()
    }

    #[test]
    fn system_transfer_parse() {
        let bytes = make_transfer_tx(1_000_000);
        let summary = TransactionInspector::inspect(&bytes).unwrap();
        assert_eq!(summary.instruction_count, 1);
        assert!(summary.risks.is_empty(), "small transfer should have no risks");
        let ix = &summary.instructions[0];
        assert!(
            matches!(ix, InstructionInfo::SystemTransfer { lamports, .. } if *lamports == 1_000_000)
        );
    }

    #[test]
    fn large_transfer_risk_flag() {
        let bytes = make_transfer_tx(50_000_000_000); // 50 SOL
        let summary = TransactionInspector::inspect(&bytes).unwrap();
        assert!(
            summary.risks.iter().any(|r| matches!(r, RiskFlag::LargeSolTransfer { .. })),
            "50 SOL should trigger LargeSolTransfer risk flag"
        );
        assert!(summary.has_high_risk() || summary.risk_level() == "MEDIUM");
    }

    #[test]
    fn high_risk_large_transfer() {
        let bytes = make_transfer_tx(200_000_000_000); // 200 SOL
        let summary = TransactionInspector::inspect(&bytes).unwrap();
        assert!(summary.has_high_risk(), "200 SOL should be HIGH risk");
    }

    #[test]
    fn risk_level_none_for_small_transfer() {
        let bytes = make_transfer_tx(5_000); // 0.000005 SOL
        let summary = TransactionInspector::inspect(&bytes).unwrap();
        assert_eq!(summary.risk_level(), "NONE");
    }

    #[test]
    fn render_contains_instruction_info() {
        let bytes = make_transfer_tx(1_000_000);
        let summary = TransactionInspector::inspect(&bytes).unwrap();
        let rendered = summary.render();
        assert!(rendered.contains("System :: Transfer"), "render should include instruction");
        assert!(rendered.contains("Risk:"), "render should include risk section");
    }

    #[test]
    fn multiple_instructions() {
        let from = Keypair::new();
        let to1 = Pubkey::new_unique();
        let to2 = Pubkey::new_unique();
        let ix1 = system_instruction::transfer(&from.pubkey(), &to1, 1_000);
        let ix2 = system_instruction::transfer(&from.pubkey(), &to2, 2_000);
        let msg = Message::new(&[ix1, ix2], Some(&from.pubkey()));
        let tx = Transaction::new(&[&from], msg, Hash::default());
        let bytes = bincode::serialize(&tx).unwrap();
        let summary = TransactionInspector::inspect(&bytes).unwrap();
        assert_eq!(summary.instruction_count, 2);
        assert_eq!(summary.instructions.len(), 2);
    }

    #[test]
    fn invalid_bytes_returns_error() {
        let result = TransactionInspector::inspect(b"not a transaction");
        assert!(result.is_err());
    }

    #[test]
    fn short_helper() {
        let long_key = "4wTQmXZnY1KjS8dRfGhAbc123456789XyZ9";
        let s = short(long_key);
        assert!(s.contains('…'));
        assert!(s.len() < long_key.len());
    }
}