//! Shared harness for the `solana-program-test` integration suite.
//!
//! Programs run in-process as builtins (`cargo test`, no `.so` build): our
//! intent solver plus SPL Token, Token-2022 and the Associated-Token-Account
//! program so CPIs resolve. Helpers here mirror the on-wire layout that
//! `execute()` / `refund()` parse, so the tests exercise the real dispatch path.
#![allow(dead_code)]

use intent_factory_program::errors::IntentSolverError;
use intent_factory_program::intent_hash::{hash_intent, INTENT_PDA_SEED};

use solana_program::{
    instruction::{AccountMeta, Instruction},
    program_option::COption,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_program,
};
use solana_program_test::{processor, BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::InstructionError,
    signer::{keypair::Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use spl_token::state::{Account as TokenAccountState, AccountState, Mint as MintState};

pub use spl_associated_token_account::get_associated_token_address_with_program_id as ata;

pub const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Byte offsets into the serialized instruction `data`, kept in one place so the
/// malformed-input tests don't hand-compute (and silently drift from) the wire layout
/// produced by the per-suite execute/refund builders. Layouts share the prefix
/// `variant(1) | amount_in(8) | salt(32) | transfer_nb(1)`.
pub mod offsets {
    /// First byte of `salt` (after `variant(1) + amount_in(8)`). Shared by both variants.
    pub const SALT: usize = 1 + 8;
    /// `transfer_nb` byte (after the salt). Shared by both variants.
    pub const TRANSFER_NB: usize = 1 + 8 + 32;

    /// `execute` only: the `cpi_count` byte sits after the `mins` array (`dests_len` u64s).
    pub const fn execute_cpi_count(dests_len: usize) -> usize {
        TRANSFER_NB + 1 + dests_len * 8
    }
    /// `execute` only: the first CPI's `acc_count` byte (immediately after `cpi_count`).
    pub const fn execute_first_acc_count(dests_len: usize) -> usize {
        execute_cpi_count(dests_len) + 1
    }
    /// `execute` only: the `flags` byte of the first CPI's first `(pos, flags)` override.
    /// After `cpi_count`: `acc_count(1) | override_count(1) | pos(1) | flags(1)`.
    pub const fn execute_first_override_flags(dests_len: usize) -> usize {
        execute_cpi_count(dests_len) + 4
    }
}

/// In-process test bank with our program + the SPL programs registered as builtins.
/// Returns the freshly-minted program id so each test gets an isolated namespace.
pub fn program_test() -> (ProgramTest, Pubkey) {
    let program_id = Pubkey::new_unique();
    let mut pt = ProgramTest::new(
        "intent_factory_program",
        program_id,
        processor!(intent_factory_program::process_instruction),
    );
    pt.add_program(
        "spl_token",
        spl_token::id(),
        processor!(spl_token::processor::Processor::process),
    );
    pt.add_program(
        "spl_token_2022",
        spl_token_2022::id(),
        processor!(spl_token_2022::processor::Processor::process),
    );
    pt.add_program(
        "spl_associated_token_account",
        spl_associated_token_account::id(),
        processor!(spl_associated_token_account::processor::process_instruction),
    );
    (pt, program_id)
}

pub fn rent_min_for(len: usize) -> u64 {
    Rent::default().minimum_balance(len)
}

pub fn system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: vec![],
        owner: system_program::ID,
        executable: false,
        rent_epoch: 0,
    }
}

/// A base (extension-free) SPL token account, byte-identical for Token and Token-2022.
pub fn token_account(mint: Pubkey, owner: Pubkey, amount: u64, token_program: Pubkey) -> Account {
    let mut data = vec![0u8; TokenAccountState::LEN];
    let state = TokenAccountState {
        mint,
        owner,
        amount,
        delegate: COption::None,
        state: AccountState::Initialized,
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    TokenAccountState::pack(state, &mut data).unwrap();
    Account {
        lamports: rent_min_for(TokenAccountState::LEN),
        data,
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    }
}

pub fn mint_account(decimals: u8, token_program: Pubkey) -> Account {
    let mut data = vec![0u8; MintState::LEN];
    let state = MintState {
        mint_authority: COption::None,
        supply: 0,
        decimals,
        is_initialized: true,
        freeze_authority: COption::None,
    };
    MintState::pack(state, &mut data).unwrap();
    Account {
        lamports: rent_min_for(MintState::LEN),
        data,
        owner: token_program,
        executable: false,
        rent_epoch: 0,
    }
}

/// SPL `transfer` (tag 3) instruction, layout shared by Token and Token-2022.
/// Authority is marked as a signer so the execute builder emits the PDA signer override.
pub fn spl_transfer_ix(
    token_program: Pubkey,
    source: Pubkey,
    dest: Pubkey,
    authority: Pubkey,
    amount: u64,
) -> Instruction {
    let mut data = vec![3u8];
    data.extend_from_slice(&amount.to_le_bytes());
    Instruction {
        program_id: token_program,
        accounts: vec![
            AccountMeta::new(source, false),
            AccountMeta::new(dest, false),
            AccountMeta::new_readonly(authority, true),
        ],
        data,
    }
}

/// Canonical intent description shared by the test and the builders below.
#[derive(Clone)]
pub struct Intent {
    pub funder: Pubkey,
    pub mint: Pubkey,
    pub amount_in: u64,
    pub salt: [u8; 32],
    pub executor: Pubkey,
    pub dests: Vec<Pubkey>,
    pub mins: Vec<u64>,
}

impl Intent {
    pub fn hash(&self) -> [u8; 32] {
        hash_intent(
            &self.funder,
            &self.mint,
            self.amount_in,
            &self.salt,
            &self.executor,
            self.dests.len() as u8,
            &self.dests,
            &self.mins,
        )
    }

    pub fn pda(&self, program_id: &Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(&[INTENT_PDA_SEED, &self.hash()], program_id)
    }
}

/// Signs with `ctx.payer` plus at most one extra signer, using the cached blockhash.
pub async fn send(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
) -> Result<(), BanksClientError> {
    let blockhash = ctx.last_blockhash;
    sign_and_process(ctx, ix, extra, blockhash).await
}

/// Same as [`send`] but forces a fresh blockhash, so a byte-identical instruction
/// produces a distinct transaction signature (avoids "already processed").
pub async fn send_fresh(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
) -> Result<(), BanksClientError> {
    let blockhash = ctx.get_new_latest_blockhash().await.unwrap();
    sign_and_process(ctx, ix, extra, blockhash).await
}

async fn sign_and_process(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra: &[&Keypair],
    blockhash: solana_sdk::hash::Hash,
) -> Result<(), BanksClientError> {
    let payer_pk = ctx.payer.pubkey();
    let tx = match extra {
        [] => Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&ctx.payer], blockhash),
        [a] => {
            Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&ctx.payer, *a], blockhash)
        }
        _ => panic!("send supports at most one extra signer"),
    };
    ctx.banks_client.process_transaction(tx).await
}

pub async fn lamports(ctx: &mut ProgramTestContext, addr: Pubkey) -> u64 {
    ctx.banks_client
        .get_account(addr)
        .await
        .unwrap()
        .map(|a| a.lamports)
        .unwrap_or(0)
}

pub async fn token_amount(ctx: &mut ProgramTestContext, addr: Pubkey) -> Option<u64> {
    let acc = ctx.banks_client.get_account(addr).await.unwrap()?;
    Some(TokenAccountState::unpack(&acc.data).unwrap().amount)
}

pub async fn account_exists(ctx: &mut ProgramTestContext, addr: Pubkey) -> bool {
    ctx.banks_client.get_account(addr).await.unwrap().is_some()
}

/// Asserts the transaction failed with a specific `IntentSolverError` custom code.
pub fn assert_custom(err: BanksClientError, expected: IntentSolverError) {
    match err.unwrap() {
        TransactionError::InstructionError(_, InstructionError::Custom(code)) => assert_eq!(
            code, expected as u32,
            "expected {expected:?} ({}), got Custom({code})",
            expected as u32
        ),
        other => panic!("expected Custom({}), got {other:?}", expected as u32),
    }
}

/// Asserts the transaction failed with a specific non-custom `InstructionError`.
pub fn assert_ix_error(err: BanksClientError, expected: InstructionError) {
    match err.unwrap() {
        TransactionError::InstructionError(_, ie) => {
            assert_eq!(ie, expected, "unexpected instruction error")
        }
        other => panic!("expected {expected:?}, got {other:?}"),
    }
}
