use anchor_lang::prelude::*;
use anchor_lang::solana_program::instruction::{AccountMeta, Instruction};
use anchor_lang::solana_program::program::invoke_signed;
use anchor_spl::token::TokenAccount;

use crate::errors::IntentError;
use crate::guards;
use crate::hash::compute_intent_hash;
use crate::state::IntentHeader;
use crate::wire::{CallsIter, NAMED_PREFIX};

#[derive(Accounts)]
pub struct ExecuteIntent<'info> {
    /// CHECK: validated against compute_intent_hash + create_program_address.
    #[account(mut)]
    pub intent_pda: AccountInfo<'info>,

    /// CHECK: pubkey + identity asserts in handler.
    #[account(mut)]
    pub source_ata: AccountInfo<'info>,

    /// CHECK: pubkey assert in handler.
    pub receiver: AccountInfo<'info>,

    /// CHECK: handler validates based on out_mint.
    #[account(mut)]
    pub receiver_token: AccountInfo<'info>,

    pub clock: Sysvar<'info, Clock>,
}

pub fn handle_execute_intent<'info>(
    ctx: Context<'_, '_, '_, 'info, ExecuteIntent<'info>>,
    header_bytes: Vec<u8>,
    calls_bytes: Vec<u8>,
    bump: u8,
) -> Result<()> {
    let header = IntentHeader::decode(&header_bytes)?;
    let re_encoded = header.canonical_encode();
    require!(re_encoded == header_bytes, IntentError::MalformedHeader);

    let src_mint = guards::assert_src_mint_some(&header.src_mint)?;

    let remaining = ctx.remaining_accounts;
    guards::assert_tail_len(remaining.len())?;

    let intent_hash = compute_intent_hash(&header_bytes, &calls_bytes, remaining)?;
    let seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];
    let derived = Pubkey::create_program_address(seeds, &crate::id())
        .map_err(|_| IntentError::BadIntentPda)?;
    require!(
        derived == ctx.accounts.intent_pda.key(),
        IntentError::BadIntentPda
    );

    let now = ctx.accounts.clock.unix_timestamp;
    guards::assert_deadline_not_passed(now, header.deadline)?;

    guards::assert_executor_signer(
        &header.executor,
        ctx.remaining_accounts,
    )?;

    let source_ta = guards::validate_source_ata(
        &ctx.accounts.source_ata,
        &ctx.accounts.intent_pda.key(),
        &src_mint,
    )?;

    let is_native_output = header.out_mint.is_none();
    if let Some(out_mint) = &header.out_mint {
        guards::validate_receiver_token_spl(
            &ctx.accounts.receiver_token,
            &header.receiver,
            out_mint,
        )?;
    } else {
        guards::validate_receiver_token_native(&ctx.accounts.receiver_token)?;
    }
    guards::validate_receiver(&ctx.accounts.receiver, &header.receiver)?;

    guards::assert_source_amount(source_ta.amount, header.amount_in)?;

    let pre_balance = read_receiver_balance(
        &header.out_mint,
        &ctx.accounts.receiver,
        &ctx.accounts.receiver_token,
    )?;

    let virtual_list = build_virtual_list(
        &ctx.accounts.intent_pda,
        &ctx.accounts.source_ata,
        &ctx.accounts.receiver,
        &ctx.accounts.receiver_token,
        remaining,
    );

    let pda_signer_seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];

    let mut iter = CallsIter::new(&calls_bytes, remaining.len())?;
    while let Some(call_result) = iter.next_call() {
        let call = call_result?;

        let program_id = virtual_list[call.program_ix as usize].key();

        guards::check_deny_list(&program_id, call.data)?;

        let acc_count = call.accounts.len();
        let mut account_metas = Vec::with_capacity(acc_count);
        let mut account_infos = Vec::with_capacity(acc_count + 1);

        for i in 0..acc_count {
            let vix = call.accounts[i];
            let (is_writable, is_signer) = call.flag_for(i);

            guards::check_signer_flag(vix, is_signer)?;
            guards::check_native_output_index(vix, is_native_output)?;

            let ai = &virtual_list[vix as usize];
            account_metas.push(if is_writable {
                AccountMeta::new(ai.key(), is_signer)
            } else {
                AccountMeta::new_readonly(ai.key(), is_signer)
            });
            account_infos.push(ai.clone());
        }

        let program_ai = &virtual_list[call.program_ix as usize];
        account_infos.push(program_ai.clone());

        let ix = Instruction {
            program_id,
            accounts: account_metas,
            data: call.data.to_vec(),
        };

        invoke_signed(&ix, &account_infos, &[pda_signer_seeds])?;
    }
    iter.assert_exhausted()?;

    guards::assert_source_drained(&ctx.accounts.source_ata, &src_mint)?;

    let post_balance = read_receiver_balance(
        &header.out_mint,
        &ctx.accounts.receiver,
        &ctx.accounts.receiver_token,
    )?;

    guards::assert_min_out(pre_balance, post_balance, header.min_amount_out)?;

    Ok(())
}

fn build_virtual_list<'a, 'info>(
    intent_pda: &'a AccountInfo<'info>,
    source_ata: &'a AccountInfo<'info>,
    receiver: &'a AccountInfo<'info>,
    receiver_token: &'a AccountInfo<'info>,
    remaining: &'a [AccountInfo<'info>],
) -> Vec<AccountInfo<'info>> {
    let mut list = Vec::with_capacity(NAMED_PREFIX + remaining.len());
    list.push(intent_pda.clone());
    list.push(source_ata.clone());
    list.push(receiver.clone());
    list.push(receiver_token.clone());
    for a in remaining {
        list.push(a.clone());
    }
    list
}

fn read_receiver_balance(
    out_mint: &Option<Pubkey>,
    receiver: &AccountInfo,
    receiver_token: &AccountInfo,
) -> Result<u64> {
    match out_mint {
        None => Ok(receiver.lamports()),
        Some(_) => {
            let data = receiver_token.try_borrow_data()?;
            let ta = TokenAccount::try_deserialize(&mut &data[..])?;
            Ok(ta.amount)
        }
    }
}
