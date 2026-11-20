use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_spl::associated_token::spl_associated_token_account;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::spl_token;
use anchor_spl::token::{Token, TokenAccount};

use crate::errors::IntentError;
use crate::guards;
use crate::pda::verify_intent_pda;
use crate::state::{IntentState, IntentStatus};

#[derive(Accounts)]
pub struct RefundIntent<'info> {
    /// The intent PDA holding stored IntentState.
    #[account(mut)]
    pub intent_pda: Account<'info, IntentState>,

    /// CHECK: pubkey + identity asserts in handler.
    #[account(mut)]
    pub source_ata: AccountInfo<'info>,

    /// CHECK: pubkey assert in handler. Receives rent lamports.
    #[account(mut)]
    pub user: AccountInfo<'info>,

    /// CHECK: pubkey + identity asserts in handler.
    #[account(mut)]
    pub user_source_ata: AccountInfo<'info>,

    /// CHECK: pubkey, SPL ownership, Mint::LEN, and decimals extraction validated in handler.
    pub src_mint_account: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub clock: Sysvar<'info, Clock>,

    #[account(mut)]
    pub payer: Signer<'info>,
}

pub fn handle_refund_intent<'info>(
    ctx: Context<'_, '_, '_, 'info, RefundIntent<'info>>,
) -> Result<()> {
    let state = &ctx.accounts.intent_pda;
    require!(
        state.status == IntentStatus::Initialized,
        if state.status == IntentStatus::Executed {
            IntentError::IntentAlreadyExecuted
        } else {
            IntentError::IntentAlreadyRefunded
        }
    );

    let src_mint = guards::assert_src_mint_some(&state.src_mint)?;
    let bump = state.bump;
    let intent_hash = state.intent_hash;

    verify_intent_pda(&intent_hash, bump, &ctx.accounts.intent_pda.key())?;

    let now = ctx.accounts.clock.unix_timestamp;
    guards::assert_deadline_passed(now, state.deadline)?;

    guards::validate_refund_user(&ctx.accounts.user, &state.user)?;
    let decimals = guards::validate_src_mint_account(&ctx.accounts.src_mint_account, &src_mint)?;

    let pda_key = ctx.accounts.intent_pda.key();
    let source_ata_key = ctx.accounts.source_ata.key();
    let expected_source_ata =
        anchor_spl::associated_token::get_associated_token_address(&pda_key, &src_mint);
    require!(
        source_ata_key == expected_source_ata,
        IntentError::BadSourceAta
    );
    guards::validate_user_source_ata(&ctx.accounts.user_source_ata, &state.user, &src_mint)?;

    let pda_signer_seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];

    refund_source_tokens(&ctx, &src_mint, &pda_key, decimals, pda_signer_seeds)?;

    let state_mut = &mut ctx.accounts.intent_pda;
    state_mut.status = IntentStatus::Refunded;

    close_intent_state(&state_mut.to_account_info(), &ctx.accounts.user)?;

    Ok(())
}

fn refund_source_tokens<'info>(
    ctx: &Context<'_, '_, '_, 'info, RefundIntent<'info>>,
    src_mint: &Pubkey,
    pda_key: &Pubkey,
    decimals: u8,
    pda_signer_seeds: &[&[u8]],
) -> Result<()> {
    if ctx.accounts.source_ata.data_is_empty() {
        return Ok(());
    }

    require!(
        ctx.accounts.source_ata.owner == &spl_token::ID,
        IntentError::BadSourceAta
    );
    require!(
        ctx.accounts.source_ata.data_len() == TokenAccount::LEN,
        IntentError::SourceAccountMalformed
    );

    let source_amount = {
        let data = ctx.accounts.source_ata.try_borrow_data()?;
        let ta = TokenAccount::try_deserialize(&mut &data[..])?;
        require!(ta.mint == *src_mint, IntentError::BadSourceAta);
        require!(ta.owner == *pda_key, IntentError::BadSourceAta);
        ta.amount
    };

    if ctx.accounts.user_source_ata.data_is_empty() {
        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &ctx.accounts.payer.key(),
                &ctx.accounts.user.key(),
                src_mint,
                &spl_token::ID,
            );
        invoke(
            &create_ata_ix,
            &[
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.user_source_ata.to_account_info(),
                ctx.accounts.user.to_account_info(),
                ctx.accounts.src_mint_account.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
            ],
        )?;
    }

    if source_amount > 0 {
        let transfer_ix = spl_token::instruction::transfer_checked(
            &spl_token::ID,
            &ctx.accounts.source_ata.key(),
            src_mint,
            &ctx.accounts.user_source_ata.key(),
            pda_key,
            &[],
            source_amount,
            decimals,
        )?;
        invoke_signed(
            &transfer_ix,
            &[
                ctx.accounts.source_ata.to_account_info(),
                ctx.accounts.src_mint_account.to_account_info(),
                ctx.accounts.user_source_ata.to_account_info(),
                ctx.accounts.intent_pda.to_account_info(),
            ],
            &[pda_signer_seeds],
        )?;
    }

    let close_ix = spl_token::instruction::close_account(
        &spl_token::ID,
        &ctx.accounts.source_ata.key(),
        &ctx.accounts.user.key(),
        pda_key,
        &[],
    )?;
    invoke_signed(
        &close_ix,
        &[
            ctx.accounts.source_ata.to_account_info(),
            ctx.accounts.user.to_account_info(),
            ctx.accounts.intent_pda.to_account_info(),
        ],
        &[pda_signer_seeds],
    )?;

    Ok(())
}

fn close_intent_state<'info>(
    intent_pda: &AccountInfo<'info>,
    user: &AccountInfo<'info>,
) -> Result<()> {
    let dest_starting_lamports = user.lamports();
    **user.lamports.borrow_mut() = dest_starting_lamports
        .checked_add(intent_pda.lamports())
        .ok_or(IntentError::LamportOverflow)?;
    **intent_pda.lamports.borrow_mut() = 0;

    intent_pda.assign(&anchor_lang::solana_program::system_program::ID);
    intent_pda.resize(0)?;

    Ok(())
}
