use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token::spl_token;
use anchor_spl::token::TokenAccount;

use crate::errors::IntentError;
use crate::guards;
use crate::pda::verify_intent_pda;
use crate::state::{IntentState, IntentStatus};
use crate::wire::CallsIter;

use super::balance_outcome::{BalanceKind, BalanceOutcome};
use super::virtual_accounts::build_virtual_list;

#[derive(Accounts)]
pub struct ExecuteIntent<'info> {
    #[account(mut)]
    pub intent_pda: Account<'info, IntentState>,

    /// CHECK: pubkey + identity asserts in handler.
    #[account(mut)]
    pub source_ata: AccountInfo<'info>,

    pub clock: Sysvar<'info, Clock>,

    /// CHECK: validated against stored header.executor in handler.
    pub executor: Signer<'info>,
}

pub fn handle_execute_intent<'info>(
    ctx: Context<'_, '_, '_, 'info, ExecuteIntent<'info>>,
    calls_bytes: Vec<u8>,
) -> Result<()> {
    // --- 1. Load and validate intent state ---
    // Ensure this intent hasn't already been executed (idempotency / replay protection)
    let state = &ctx.accounts.intent_pda;
    require!(
        state.status == IntentStatus::Initialized,
        IntentError::IntentAlreadyExecuted
    );

    // Extract core parameters needed for execution validation
    let src_mint = guards::assert_src_mint_some(&state.src_mint)?;
    let bump = state.bump;
    let intent_hash = state.intent_hash;

    // Verify PDA derivation matches expected intent identity (integrity check)
    verify_intent_pda(&intent_hash, bump, &ctx.accounts.intent_pda.key())?;

    // Enforce time validity (intents can expire)
    let now = ctx.accounts.clock.unix_timestamp;
    guards::assert_deadline_not_passed(now, state.deadline)?;

    // Ensure only the authorized executor can run this intent
    guards::validate_executor(&state.executor, &ctx.accounts.executor)?;

    // --- 2. Validate funding source ---
    // Confirm the source token account belongs to the intent PDA and matches expected mint
    let pda_key = ctx.accounts.intent_pda.key();
    let source_ta = guards::validate_source_ata(&ctx.accounts.source_ata, &pda_key, &src_mint)?;

    // Ensure sufficient input funds are present before execution
    guards::assert_source_amount(source_ta.amount, state.amount_in)?;

    // Remaining accounts contain all dynamic accounts used during execution
    let remaining = ctx.remaining_accounts;
    guards::assert_tail_len(remaining.len())?;

    // --- 3. Pre-execution outcome snapshotting ---
    // Capture initial balances of all expected outcome accounts
    // This enables post-execution invariant checking (withOutcome semantics)
    let stored_outcomes = state.outcomes();

    let intent_pda_key = ctx.accounts.intent_pda.key();
    let source_ata_key = ctx.accounts.source_ata.key();

    let mut outcomes: Vec<BalanceOutcome> = Vec::with_capacity(stored_outcomes.len());

    for outcome in &stored_outcomes {
        // Prevent trivial/self-referential outcome checks
        require!(
            outcome.account != intent_pda_key && outcome.account != source_ata_key,
            IntentError::BadOutcomeAccount
        );

        // Locate the actual account in the provided account list
        let outcome_ai = remaining
            .iter()
            .find(|ai| ai.key() == outcome.account)
            .ok_or(IntentError::BadOutcomeAccount)?;

        // Determine whether this is a token balance or native SOL balance
        // and validate structure accordingly
        let kind = if let Some(ref mint) = outcome.mint {
            // SPL token account validation
            require!(
                outcome_ai.owner == &spl_token::ID,
                IntentError::BadOutcomeAccount
            );
            require!(
                outcome_ai.data_len() == TokenAccount::LEN,
                IntentError::BadOutcomeAccount
            );
            let data = outcome_ai.try_borrow_data()?;
            let ta = TokenAccount::try_deserialize(&mut &data[..])?;
            require!(ta.mint == *mint, IntentError::BadOutcomeAccount);
            BalanceKind::SplToken
        } else {
             // Native SOL balance
            BalanceKind::NativeLamports
        };

        // Record pre-execution balance snapshot
        outcomes.push(BalanceOutcome::snapshot(outcome_ai, kind, outcome.amount)?);
    }

    // --- 4. Build execution context (virtual account space) ---
    // Creates a unified indexable account list:
    // [intent_pda, source_ata, remaining_accounts...]
    // This allows compact instruction encoding using indices
    let intent_pda_ai = ctx.accounts.intent_pda.to_account_info();
    let virtual_list = build_virtual_list(&intent_pda_ai, &ctx.accounts.source_ata, remaining);

    // PDA signer seeds for CPI execution
    let pda_signer_seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];

    // --- 5. Instruction interpreter loop ---
    // Decode and execute a sequence of arbitrary instructions (CPI calls)
    let mut iter = CallsIter::new(&calls_bytes, remaining.len())?;
    while let Some(call_result) = iter.next_call() {
        let call = call_result?;

        let program_id = virtual_list[call.program_ix as usize].key();

        // Enforce deny-list restrictions (security sandboxing)
        guards::check_deny_list(&program_id, call.data)?;

        // Build CPI account metas and infos dynamically
        let acc_count = call.accounts.len();
        let mut account_metas = Vec::with_capacity(acc_count);
        let mut account_infos = Vec::with_capacity(acc_count + 1);

        for i in 0..acc_count {
            let vix = call.accounts[i];
            let (is_writable, is_signer) = call.flag_for(i);

            // Prevent privilege escalation via forged flags
            guards::check_signer_flag(vix, is_signer)?;
            guards::check_writable_intent_pda(vix, is_writable)?;

            let ai = &virtual_list[vix as usize];
            // Construct CPI metadata
            account_metas.push(if is_writable {
                AccountMeta::new(ai.key(), is_signer)
            } else {
                AccountMeta::new_readonly(ai.key(), is_signer)
            });
            account_infos.push(ai.clone());
        }

        // Append program account for CPI
        let program_ai = &virtual_list[call.program_ix as usize];
        account_infos.push(program_ai.clone());

        // Build instruction payload
        let ix = Instruction {
            program_id,
            accounts: account_metas,
            data: call.data.to_vec(),
        };

        // Execute CPI with PDA as signer (intent acts as execution authority)
        invoke_signed(&ix, &account_infos, &[pda_signer_seeds])?;
    }

    // Ensure no trailing bytes (strict decoding completeness)
    iter.assert_exhausted()?;

    // --- 6. Post-execution invariant checks ---
    // Ensure all input funds were consumed (no leftovers / leakage)
    guards::assert_source_drained(&ctx.accounts.source_ata, &src_mint)?;

    // Validate all expected balance changes occurred
    // (this enforces the "withOutcome" semantics)
    for outcome in &outcomes {
        outcome.assert_delta()?;
    }

    // --- 7. Finalize state ---
    // Mark intent as executed to prevent replay
    let state_mut = &mut ctx.accounts.intent_pda;
    state_mut.status = IntentStatus::Executed;

    Ok(())
}
