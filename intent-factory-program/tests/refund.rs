//! Integration tests for the `refund` instruction (variant 1).
//!
//! Note: the SPL cases pre-create the funder ATA, so the program's idempotent-create
//! short-circuits. The in-process builtin harness doesn't propagate `set_return_data`
//! across CPIs, which the ATA program's *creation* path needs (`GetAccountDataSize`);
//! the refund money-path (transfer + close) is exercised regardless.
mod common;

use common::*;
use intent_factory_program::errors::IntentSolverError;
use solana_program::{
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    system_program,
};
use solana_program_test::{tokio, ProgramTestContext};
use solana_sdk::signer::{keypair::Keypair, Signer};

const VARIANT_REFUND: u8 = 1;

/// Extra accounts for an SPL `refund`.
struct RefundSpl {
    pda_ata: Pubkey,
    funder_ata: Pubkey,
    mint: Pubkey,
    ata_program: Pubkey,
}

/// Builds the `refund` (variant 1) instruction. `funder` is the passed account (which the
/// test can deliberately mismatch from `intent.funder` to force `InvalidPda`).
fn build_refund_ix(
    program_id: Pubkey,
    intent: &Intent,
    pda: Pubkey,
    funder: Pubkey,
    from_program: Pubkey,
    spl: Option<RefundSpl>,
    executor_is_signer: bool,
) -> Instruction {
    let mut data = vec![VARIANT_REFUND];
    data.extend_from_slice(&intent.amount_in.to_le_bytes());
    data.extend_from_slice(&intent.salt);
    data.push(intent.dests.len() as u8);
    for i in 0..intent.dests.len() {
        data.extend_from_slice(intent.dests[i].as_ref());
        data.extend_from_slice(&intent.mins[i].to_le_bytes());
    }

    let funder_writable = spl.is_none();
    let mut metas = vec![
        AccountMeta {
            pubkey: intent.executor,
            is_signer: executor_is_signer,
            is_writable: true,
        },
        AccountMeta::new(pda, false),
        AccountMeta {
            pubkey: funder,
            is_signer: false,
            is_writable: funder_writable,
        },
        AccountMeta::new_readonly(from_program, false),
    ];
    if let Some(s) = spl {
        metas.push(AccountMeta::new(s.pda_ata, false));
        metas.push(AccountMeta::new(s.funder_ata, false));
        metas.push(AccountMeta::new_readonly(s.mint, false));
        metas.push(AccountMeta::new_readonly(system_program::ID, false));
        metas.push(AccountMeta::new_readonly(s.ata_program, false));
    }

    Instruction {
        program_id,
        accounts: metas,
        data,
    }
}

/// SOL `refund` scenario: vault PDA holds rent + `amount_in`, funder holds rent.
struct RefundSolLeg {
    program_id: Pubkey,
    executor: Keypair,
    funder: Pubkey,
    amount_in: u64,
    intent: Intent,
    pda: Pubkey,
    ctx: ProgramTestContext,
}

async fn refund_sol_leg(salt: u8, amount_in: u64) -> RefundSolLeg {
    let (mut pt, program_id) = program_test();
    let executor = Keypair::new();
    let funder = Pubkey::new_unique();
    let intent = Intent {
        funder,
        mint: system_program::ID,
        amount_in,
        salt: [salt; 32],
        executor: executor.pubkey(),
        dests: vec![Pubkey::new_unique()],
        mins: vec![1],
    };
    let (pda, _) = intent.pda(&program_id);

    pt.add_account(executor.pubkey(), system_account(10 * LAMPORTS_PER_SOL));
    pt.add_account(pda, system_account(rent_min_for(0) + amount_in));
    pt.add_account(funder, system_account(rent_min_for(0)));

    let ctx = pt.start_with_context().await;
    RefundSolLeg {
        program_id,
        executor,
        funder,
        amount_in,
        intent,
        pda,
        ctx,
    }
}

impl RefundSolLeg {
    /// `refund` ix; `funder`/`from_program`/`signer` are explicit so negative tests can deviate.
    fn ix(&self, funder: Pubkey, from_program: Pubkey, signer: bool) -> Instruction {
        build_refund_ix(
            self.program_id,
            &self.intent,
            self.pda,
            funder,
            from_program,
            None,
            signer,
        )
    }
}

/// SPL `refund` scenario: vault ATA holds `amount_in`; the funder ATA is created only when
/// `create_funder_ata` (so the harness's idempotent-create short-circuits; see file header).
struct RefundSplLeg {
    program_id: Pubkey,
    executor: Keypair,
    funder: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    amount_in: u64,
    intent: Intent,
    pda: Pubkey,
    pda_ata: Pubkey,
    funder_ata: Pubkey,
    ctx: ProgramTestContext,
}

async fn refund_spl_leg(
    salt: u8,
    token_program: Pubkey,
    amount_in: u64,
    create_funder_ata: bool,
) -> RefundSplLeg {
    let (mut pt, program_id) = program_test();
    let executor = Keypair::new();
    let funder = Pubkey::new_unique();
    let mint = Pubkey::new_unique();
    let intent = Intent {
        funder,
        mint,
        amount_in,
        salt: [salt; 32],
        executor: executor.pubkey(),
        dests: vec![Pubkey::new_unique()],
        mins: vec![1],
    };
    let (pda, _) = intent.pda(&program_id);
    let pda_ata = ata(&pda, &mint, &token_program);
    let funder_ata = ata(&funder, &mint, &token_program);

    pt.add_account(executor.pubkey(), system_account(10 * LAMPORTS_PER_SOL));
    pt.add_account(mint, mint_account(6, token_program));
    pt.add_account(pda_ata, token_account(mint, pda, amount_in, token_program));
    if create_funder_ata {
        pt.add_account(funder_ata, token_account(mint, funder, 0, token_program));
    }

    let ctx = pt.start_with_context().await;
    RefundSplLeg {
        program_id,
        executor,
        funder,
        mint,
        token_program,
        amount_in,
        intent,
        pda,
        pda_ata,
        funder_ata,
        ctx,
    }
}

impl RefundSplLeg {
    /// The canonical SPL account bundle for this scenario.
    fn spl(&self) -> RefundSpl {
        RefundSpl {
            pda_ata: self.pda_ata,
            funder_ata: self.funder_ata,
            mint: self.mint,
            ata_program: spl_associated_token_account::id(),
        }
    }

    /// `refund` ix with a caller-supplied SPL bundle (so negative tests can swap a field).
    fn ix_with(&self, spl: RefundSpl) -> Instruction {
        build_refund_ix(
            self.program_id,
            &self.intent,
            self.pda,
            self.funder,
            self.token_program,
            Some(spl),
            true,
        )
    }

    /// `refund` ix with the canonical SPL bundle.
    fn ix(&self) -> Instruction {
        self.ix_with(self.spl())
    }
}

/// Happy path, SOL vault: every lamport (principal + rent) is swept to the funder.
#[tokio::test]
async fn refund_sol_happy() {
    let mut s = refund_sol_leg(8, 3_000_000).await;
    let rent_min = rent_min_for(0);

    let ix = s.ix(s.funder, system_program::ID, true);
    send(&mut s.ctx, ix, &[&s.executor]).await.unwrap();

    assert_eq!(
        lamports(&mut s.ctx, s.funder).await,
        rent_min + rent_min + s.amount_in
    );
    assert_eq!(lamports(&mut s.ctx, s.pda).await, 0, "vault PDA closed");
}

async fn run_spl_refund_happy(token_program: Pubkey) {
    let mut s = refund_spl_leg(9, token_program, 750_000, true).await;

    let ix = s.ix();
    send(&mut s.ctx, ix, &[&s.executor]).await.unwrap();

    assert_eq!(
        token_amount(&mut s.ctx, s.funder_ata).await,
        Some(s.amount_in)
    );
    assert!(
        !account_exists(&mut s.ctx, s.pda_ata).await,
        "PDA source ATA closed"
    );
}

/// Happy path, SPL Token vault: full balance is returned to the funder ATA and the source closed.
#[tokio::test]
async fn refund_spl_token_happy() {
    run_spl_refund_happy(spl_token::id()).await;
}

/// Happy path, SPL Token-2022 vault.
#[tokio::test]
async fn refund_spl_token_2022_happy() {
    run_spl_refund_happy(spl_token_2022::id()).await;
}

/// Passing a funder other than the one bound in the intent re-derives a different hash,
/// so the PDA no longer matches.
#[tokio::test]
async fn refund_rejects_wrong_funder() {
    let mut s = refund_sol_leg(10, 3_000_000).await;
    let attacker = Pubkey::new_unique();

    let ix = s.ix(attacker, system_program::ID, true);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidPda);
}

/// The executor account must sign.
#[tokio::test]
async fn refund_requires_executor_signature() {
    let mut s = refund_sol_leg(11, 3_000_000).await;

    let ix = s.ix(s.funder, system_program::ID, false);
    let err = send(&mut s.ctx, ix, &[]).await.unwrap_err();
    assert_ix_error(err, InstructionError::MissingRequiredSignature);
}

/// A funder ATA that is not the canonical ATA for `(funder, mint, token_program)` is rejected,
/// so the executor can't redirect SPL refunds.
#[tokio::test]
async fn refund_rejects_non_canonical_funder_ata() {
    let mut s = refund_spl_leg(12, spl_token::id(), 750_000, false).await;
    let wrong_funder_ata = ata(&Pubkey::new_unique(), &s.mint, &s.token_program);

    let spl = RefundSpl {
        funder_ata: wrong_funder_ata,
        ..s.spl()
    };
    let ix = s.ix_with(spl);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidDestinationAta);
}

/// Refunding twice: the first call closes the source ATA, so the replay no longer has a
/// valid SPL source.
#[tokio::test]
async fn refund_spl_twice_second_fails() {
    let mut s = refund_spl_leg(13, spl_token::id(), 750_000, true).await;

    let first = s.ix();
    send(&mut s.ctx, first, &[&s.executor]).await.unwrap();
    assert_eq!(
        token_amount(&mut s.ctx, s.funder_ata).await,
        Some(s.amount_in)
    );

    let second = s.ix();
    let err = send_fresh(&mut s.ctx, second, &[&s.executor])
        .await
        .unwrap_err();
    assert_custom(err, IntentSolverError::InvalidSourceAta);
}

#[tokio::test]
async fn refund_rejects_truncated_payload() {
    let mut s = refund_sol_leg(21, 3_000_000).await;

    let mut ix = s.ix(s.funder, system_program::ID, true);
    ix.data.truncate(ix.data.len() - 1);

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InsufficientData);
}

#[tokio::test]
async fn refund_rejects_wrong_token_program() {
    let mut s = refund_sol_leg(22, 3_000_000).await;

    let ix = s.ix(s.funder, Pubkey::new_unique(), true);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::WrongTokenProgram);
}

#[tokio::test]
async fn refund_rejects_invalid_transfer_count() {
    let mut s = refund_sol_leg(23, 3_000_000).await;

    let mut ix = s.ix(s.funder, system_program::ID, true);
    ix.data[offsets::TRANSFER_NB] = 0;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidTransferNb);
}

#[tokio::test]
async fn refund_rejects_transfer_count_too_large() {
    let mut s = refund_sol_leg(27, 3_000_000).await;

    let mut ix = s.ix(s.funder, system_program::ID, true);
    ix.data[offsets::TRANSFER_NB] = u8::MAX;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidTransferNb);
}

#[tokio::test]
async fn refund_rejects_mint_mismatch() {
    let mut s = refund_spl_leg(24, spl_token::id(), 750_000, true).await;
    let wrong_mint = Pubkey::new_unique();

    let spl = RefundSpl {
        mint: wrong_mint,
        ..s.spl()
    };
    let ix = s.ix_with(spl);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidDestinationAta);
}
