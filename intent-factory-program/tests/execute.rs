//! Integration tests for the `execute` instruction (variant 0).
mod common;

use common::*;
use intent_factory_program::constants::{FLAG_INNER_SIGNER, FLAG_INNER_WRITABLE};
use intent_factory_program::errors::IntentSolverError;
use solana_program::{
    bpf_loader,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    system_instruction, system_program,
};
use solana_program_test::{tokio, ProgramTestContext};
use solana_sdk::signer::{keypair::Keypair, Signer};

const VARIANT_EXECUTE: u8 = 0;

/// Builds the `execute` (variant 0) instruction: intent prefix + per-CPI blob in `data`,
/// and the account list `[executor, pda, funder, from_program, (pda_ata), dests.., CPI slices..]`.
/// Inner signer roles are downgraded on the outer metas and re-expressed as `(pos, flags)`
/// overrides, exactly like the TS SDK's `buildExecuteIntentIx`.
fn build_execute_ix(
    program_id: Pubkey,
    intent: &Intent,
    pda: Pubkey,
    from_program: Pubkey,
    pda_ata: Option<Pubkey>,
    inners: &[Instruction],
    executor_is_signer: bool,
) -> Instruction {
    let mut data = vec![VARIANT_EXECUTE];
    data.extend_from_slice(&intent.amount_in.to_le_bytes());
    data.extend_from_slice(&intent.salt);
    data.push(intent.dests.len() as u8);
    for m in &intent.mins {
        data.extend_from_slice(&m.to_le_bytes());
    }
    data.push(inners.len() as u8);

    let mut metas = vec![
        AccountMeta {
            pubkey: intent.executor,
            is_signer: executor_is_signer,
            is_writable: true,
        },
        AccountMeta::new(pda, false),
        AccountMeta::new_readonly(intent.funder, false),
        AccountMeta::new_readonly(from_program, false),
    ];
    if let Some(pa) = pda_ata {
        metas.push(AccountMeta::new(pa, false));
    }
    for d in &intent.dests {
        metas.push(AccountMeta::new(*d, false));
    }

    for inner in inners {
        let acc_count = (1 + inner.accounts.len()) as u8;
        let mut overrides: Vec<(u8, u8)> = Vec::new();
        for (j, a) in inner.accounts.iter().enumerate() {
            if a.is_signer {
                let mut flags = FLAG_INNER_SIGNER;
                if a.is_writable {
                    flags |= FLAG_INNER_WRITABLE;
                }
                overrides.push(((j + 1) as u8, flags));
            }
        }
        data.push(acc_count);
        data.push(overrides.len() as u8);
        for (pos, flags) in &overrides {
            data.push(*pos);
            data.push(*flags);
        }
        data.extend_from_slice(&(inner.data.len() as u16).to_le_bytes());
        data.extend_from_slice(&inner.data);

        metas.push(AccountMeta::new_readonly(inner.program_id, false));
        for a in &inner.accounts {
            metas.push(AccountMeta {
                pubkey: a.pubkey,
                is_signer: false,
                is_writable: a.is_writable,
            });
        }
    }

    Instruction {
        program_id,
        accounts: metas,
        data,
    }
}

/// Default principal for SOL `execute` scenarios.
const DEFAULT_SOL_AMOUNT: u64 = 2_000_000;

/// Single-leg SOL `execute` scenario: executor (10 SOL), vault PDA (`pda_lamports`),
/// recipient (rent), started and ready to send.
struct SolLeg {
    program_id: Pubkey,
    executor: Keypair,
    recipient: Pubkey,
    amount_in: u64,
    intent: Intent,
    pda: Pubkey,
    ctx: ProgramTestContext,
}

async fn sol_leg(salt: u8, amount_in: u64, min: u64, pda_lamports: u64) -> SolLeg {
    let (mut pt, program_id) = program_test();
    let executor = Keypair::new();
    let recipient = Pubkey::new_unique();
    let intent = Intent {
        funder: Pubkey::new_unique(),
        mint: system_program::ID,
        amount_in,
        salt: [salt; 32],
        executor: executor.pubkey(),
        dests: vec![recipient],
        mins: vec![min],
    };
    let (pda, _) = intent.pda(&program_id);

    pt.add_account(executor.pubkey(), system_account(10 * LAMPORTS_PER_SOL));
    pt.add_account(pda, system_account(pda_lamports));
    pt.add_account(recipient, system_account(rent_min_for(0)));

    let ctx = pt.start_with_context().await;
    SolLeg {
        program_id,
        executor,
        recipient,
        amount_in,
        intent,
        pda,
        ctx,
    }
}

/// [`sol_leg`] with canonical defaults: amount = [`DEFAULT_SOL_AMOUNT`], min = amount,
/// vault = rent + amount.
async fn sol_leg_default(salt: u8) -> SolLeg {
    sol_leg(
        salt,
        DEFAULT_SOL_AMOUNT,
        DEFAULT_SOL_AMOUNT,
        rent_min_for(0) + DEFAULT_SOL_AMOUNT,
    )
    .await
}

impl SolLeg {
    /// Inner System transfer that drains the whole vault principal to the recipient.
    fn drain(&self) -> Instruction {
        system_instruction::transfer(&self.pda, &self.recipient, self.amount_in)
    }

    /// `execute` ix over `inners` (SOL source, executor signing).
    fn ix(&self, inners: &[Instruction]) -> Instruction {
        build_execute_ix(
            self.program_id,
            &self.intent,
            self.pda,
            system_program::ID,
            None,
            inners,
            true,
        )
    }
}

/// Single-leg SPL `execute` scenario: vault ATA holds `amount_in`, recipient ATA holds 0.
struct SplExecLeg {
    program_id: Pubkey,
    executor: Keypair,
    recipient_ata: Pubkey,
    token_program: Pubkey,
    amount_in: u64,
    intent: Intent,
    pda: Pubkey,
    pda_ata: Pubkey,
    ctx: ProgramTestContext,
}

async fn spl_exec_leg(salt: u8, token_program: Pubkey, amount_in: u64) -> SplExecLeg {
    let (mut pt, program_id) = program_test();
    let executor = Keypair::new();
    let mint = Pubkey::new_unique();
    let recipient_owner = Pubkey::new_unique();
    let recipient_ata = ata(&recipient_owner, &mint, &token_program);
    let intent = Intent {
        funder: Pubkey::new_unique(),
        mint,
        amount_in,
        salt: [salt; 32],
        executor: executor.pubkey(),
        dests: vec![recipient_ata],
        mins: vec![amount_in],
    };
    let (pda, _) = intent.pda(&program_id);
    let pda_ata = ata(&pda, &mint, &token_program);

    pt.add_account(executor.pubkey(), system_account(10 * LAMPORTS_PER_SOL));
    pt.add_account(mint, mint_account(6, token_program));
    pt.add_account(pda_ata, token_account(mint, pda, amount_in, token_program));
    pt.add_account(
        recipient_ata,
        token_account(mint, recipient_owner, 0, token_program),
    );

    let ctx = pt.start_with_context().await;
    SplExecLeg {
        program_id,
        executor,
        recipient_ata,
        token_program,
        amount_in,
        intent,
        pda,
        pda_ata,
        ctx,
    }
}

impl SplExecLeg {
    /// Inner SPL transfer of `amount` from the vault ATA to the recipient ATA.
    fn transfer(&self, amount: u64) -> Instruction {
        spl_transfer_ix(
            self.token_program,
            self.pda_ata,
            self.recipient_ata,
            self.pda,
            amount,
        )
    }

    /// `execute` ix over `inners` (SPL source, executor signing).
    fn ix(&self, inners: &[Instruction]) -> Instruction {
        build_execute_ix(
            self.program_id,
            &self.intent,
            self.pda,
            self.token_program,
            Some(self.pda_ata),
            inners,
            true,
        )
    }
}

/// Happy path, SOL source: a single inner System transfer drains the vault PDA to the
/// recipient; the PDA is then swept (rent → executor) and closed.
#[tokio::test]
async fn execute_sol_single_leg_happy() {
    let mut s = sol_leg_default(3).await;

    let ix = s.ix(&[s.drain()]);
    send(&mut s.ctx, ix, &[&s.executor]).await.unwrap();

    assert_eq!(
        lamports(&mut s.ctx, s.recipient).await,
        rent_min_for(0) + s.amount_in
    );
    assert_eq!(lamports(&mut s.ctx, s.pda).await, 0, "vault PDA closed");
}

/// Happy path, multi-leg: two destination checks satisfied by two inner CPIs, exercising
/// both the transfer-check loop (`transfer_nb = 2`) and the CPI loop (`cpi_count = 2`).
#[tokio::test]
async fn execute_sol_multi_leg_happy() {
    let (mut pt, program_id) = program_test();
    let executor = Keypair::new();
    let r1 = Pubkey::new_unique();
    let r2 = Pubkey::new_unique();
    let a1 = 1_200_000u64;
    let a2 = 800_000u64;
    let amount_in = a1 + a2;
    let rent_min = rent_min_for(0);

    let intent = Intent {
        funder: Pubkey::new_unique(),
        mint: system_program::ID,
        amount_in,
        salt: [28u8; 32],
        executor: executor.pubkey(),
        dests: vec![r1, r2],
        mins: vec![a1, a2],
    };
    let (pda, _) = intent.pda(&program_id);

    pt.add_account(executor.pubkey(), system_account(10 * LAMPORTS_PER_SOL));
    pt.add_account(pda, system_account(rent_min + amount_in));
    pt.add_account(r1, system_account(rent_min));
    pt.add_account(r2, system_account(rent_min));

    let mut ctx = pt.start_with_context().await;

    let inners = [
        system_instruction::transfer(&pda, &r1, a1),
        system_instruction::transfer(&pda, &r2, a2),
    ];
    let ix = build_execute_ix(program_id, &intent, pda, system_program::ID, None, &inners, true);

    send(&mut ctx, ix, &[&executor]).await.unwrap();

    assert_eq!(lamports(&mut ctx, r1).await, rent_min + a1);
    assert_eq!(lamports(&mut ctx, r2).await, rent_min + a2);
    assert_eq!(lamports(&mut ctx, pda).await, 0, "vault PDA closed");
}

async fn run_spl_execute_happy(token_program: Pubkey) {
    let mut s = spl_exec_leg(5, token_program, 500_000).await;

    let ix = s.ix(&[s.transfer(s.amount_in)]);
    send(&mut s.ctx, ix, &[&s.executor]).await.unwrap();

    assert_eq!(
        token_amount(&mut s.ctx, s.recipient_ata).await,
        Some(s.amount_in)
    );
    assert!(
        !account_exists(&mut s.ctx, s.pda_ata).await,
        "PDA source ATA closed"
    );
}

/// Happy path, SPL Token source.
#[tokio::test]
async fn execute_spl_token_single_leg_happy() {
    run_spl_execute_happy(spl_token::id()).await;
}

/// Happy path, SPL Token-2022 source.
#[tokio::test]
async fn execute_spl_token_2022_single_leg_happy() {
    run_spl_execute_happy(spl_token_2022::id()).await;
}

/// Tampering the salt in the instruction data (without moving the PDA) breaks the
/// intent-hash → PDA binding.
#[tokio::test]
async fn execute_rejects_pda_mismatch() {
    let mut s = sol_leg_default(3).await;

    let mut ix = s.ix(&[s.drain()]);
    ix.data[offsets::SALT] ^= 0xFF;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidPda);
}

/// The CPI moves the funds but the realized delta is below the intent's minimum.
#[tokio::test]
async fn execute_rejects_min_not_met() {
    // min is part of the hash, so the vault is derived with the inflated minimum; the drain
    // CPI passes the source-empty check but only delivers `amount_in`.
    let mut s = sol_leg(
        4,
        DEFAULT_SOL_AMOUNT,
        DEFAULT_SOL_AMOUNT + 1,
        rent_min_for(0) + DEFAULT_SOL_AMOUNT,
    )
    .await;

    let ix = s.ix(&[s.drain()]);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::MinAmountNotMet);
}

/// SOL source where the CPI leaves more than the rent-exempt minimum behind.
#[tokio::test]
async fn execute_rejects_sol_source_not_drained() {
    let leftover = 1_000_000u64;
    let mut s = sol_leg(
        6,
        DEFAULT_SOL_AMOUNT,
        DEFAULT_SOL_AMOUNT,
        rent_min_for(0) + DEFAULT_SOL_AMOUNT + leftover,
    )
    .await;

    let ix = s.ix(&[s.drain()]);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::SolSourcePdaNotDrained);
}

/// SPL source where the CPI moves only part of the balance, leaving the vault ATA non-empty.
#[tokio::test]
async fn execute_rejects_from_ata_not_empty() {
    let mut s = spl_exec_leg(29, spl_token::id(), 500_000).await;

    // Move all but one token: the source-ATA-empty check fires before the min-delta check.
    let ix = s.ix(&[s.transfer(s.amount_in - 1)]);
    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::FromAtaNotEmpty);
}

/// The executor account must sign.
#[tokio::test]
async fn execute_requires_executor_signature() {
    let mut s = sol_leg_default(7).await;

    // executor meta is_signer=false; only the payer signs.
    let ix = build_execute_ix(
        s.program_id,
        &s.intent,
        s.pda,
        system_program::ID,
        None,
        &[s.drain()],
        false,
    );
    let err = send(&mut s.ctx, ix, &[]).await.unwrap_err();
    assert_ix_error(err, InstructionError::MissingRequiredSignature);
}

#[tokio::test]
async fn execute_rejects_invalid_cpi_count_zero() {
    let mut s = sol_leg_default(14).await;

    let mut ix = s.ix(&[s.drain()]);
    ix.data[offsets::execute_cpi_count(s.intent.dests.len())] = 0;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidCpiCount);
}

#[tokio::test]
async fn execute_rejects_invalid_cpi_count_too_large() {
    let mut s = sol_leg_default(25).await;

    let mut ix = s.ix(&[s.drain()]);
    ix.data[offsets::execute_cpi_count(s.intent.dests.len())] = u8::MAX;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidCpiCount);
}

#[tokio::test]
async fn execute_rejects_invalid_transfer_count_too_large() {
    let mut s = sol_leg_default(26).await;

    let mut ix = s.ix(&[s.drain()]);
    ix.data[offsets::TRANSFER_NB] = u8::MAX;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidTransferNb);
}

#[tokio::test]
async fn execute_rejects_invalid_cpi_account_spec() {
    let mut s = sol_leg_default(15).await;

    let mut ix = s.ix(&[s.drain()]);
    ix.data[offsets::execute_first_acc_count(s.intent.dests.len())] = 0;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InvalidCpiAccountSpec);
}

#[tokio::test]
async fn execute_rejects_cpi_to_self() {
    let mut s = sol_leg_default(16).await;

    let inner = Instruction {
        program_id: s.program_id,
        accounts: vec![],
        data: vec![],
    };
    let ix = s.ix(&[inner]);

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::CpiToSelfNotAllowed);
}

#[tokio::test]
async fn execute_rejects_denied_cpi_program() {
    let mut s = sol_leg_default(17).await;

    let inner = Instruction {
        program_id: bpf_loader::id(),
        accounts: vec![],
        data: vec![],
    };
    let ix = s.ix(&[inner]);

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::CpiProgramDenied);
}

#[tokio::test]
async fn execute_rejects_inner_signer_escalation() {
    let mut s = sol_leg_default(18).await;

    let inner = Instruction {
        program_id: system_program::ID,
        accounts: vec![AccountMeta::new_readonly(s.recipient, true)],
        data: vec![],
    };
    let ix = s.ix(&[inner]);

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::InnerSignerNotAllowed);
}

#[tokio::test]
async fn execute_rejects_writable_escalation() {
    let mut s = sol_leg_default(19).await;
    let readonly_account = Pubkey::new_unique();

    let inner = Instruction {
        program_id: system_program::ID,
        // mark as signer so one override pair is emitted; we'll mutate flags to writable-only.
        accounts: vec![AccountMeta::new_readonly(readonly_account, true)],
        data: vec![],
    };
    let mut ix = s.ix(&[inner]);
    ix.data[offsets::execute_first_override_flags(s.intent.dests.len())] = 1;

    let err = send(&mut s.ctx, ix, &[&s.executor]).await.unwrap_err();
    assert_custom(err, IntentSolverError::WritableEscalationNotAllowed);
}

#[tokio::test]
async fn execute_spl_twice_second_fails() {
    let mut s = spl_exec_leg(20, spl_token::id(), 500_000).await;

    let inner = s.transfer(s.amount_in);

    let first = s.ix(std::slice::from_ref(&inner));
    send(&mut s.ctx, first, &[&s.executor]).await.unwrap();
    assert_eq!(
        token_amount(&mut s.ctx, s.recipient_ata).await,
        Some(s.amount_in)
    );

    let second = s.ix(std::slice::from_ref(&inner));
    let err = send_fresh(&mut s.ctx, second, &[&s.executor])
        .await
        .unwrap_err();
    assert_custom(err, IntentSolverError::InvalidSourceAta);
}
