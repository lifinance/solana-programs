use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use anchor_spl::associated_token::spl_associated_token_account;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::spl_token;
use anchor_spl::token::{Token, TokenAccount};

use crate::errors::IntentError;
use crate::guards;
use crate::hash::compute_intent_hash;
use crate::state::IntentHeader;

#[derive(Accounts)]
pub struct RefundIntent<'info> {
    /// CHECK: validated by PDA recompute.
    #[account(mut)]
    pub intent_pda: AccountInfo<'info>,

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
    header_bytes: Vec<u8>,
    bump: u8,
) -> Result<()> {
    let header = IntentHeader::decode(&header_bytes)?;
    let re_encoded = header.canonical_encode();
    require!(re_encoded == header_bytes, IntentError::MalformedHeader);

    let src_mint = guards::assert_src_mint_some(&header.src_mint)?;

    let intent_hash = compute_intent_hash(&header_bytes);
    let seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];
    let derived = Pubkey::create_program_address(seeds, &crate::id())
        .map_err(|_| IntentError::BadIntentPda)?;
    require!(
        derived == ctx.accounts.intent_pda.key(),
        IntentError::BadIntentPda
    );

    let now = ctx.accounts.clock.unix_timestamp;
    guards::assert_deadline_passed(now, header.deadline)?;

    guards::validate_refund_user(&ctx.accounts.user, &header.user)?;
    let decimals = guards::validate_src_mint_account(&ctx.accounts.src_mint_account, &src_mint)?;

    let source_ata_key = ctx.accounts.source_ata.key();
    let expected_source_ata = anchor_spl::associated_token::get_associated_token_address(
        &ctx.accounts.intent_pda.key(),
        &src_mint,
    );
    require!(
        source_ata_key == expected_source_ata,
        IntentError::BadSourceAta
    );
    guards::validate_user_source_ata(
        &ctx.accounts.user_source_ata,
        &header.user,
        &src_mint,
    )?;

    let pda_signer_seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];

    if ctx.accounts.source_ata.data_is_empty() {
        return Ok(())
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
        require!(ta.mint == src_mint, IntentError::BadSourceAta);
        require!(
            ta.owner == ctx.accounts.intent_pda.key(),
            IntentError::BadSourceAta
        );
        ta.amount
    };

    if ctx.accounts.user_source_ata.data_is_empty() {
        let create_ata_ix =
            spl_associated_token_account::instruction::create_associated_token_account_idempotent(
                &ctx.accounts.payer.key(),
                &header.user,
                &src_mint,
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
            &src_mint,
            &ctx.accounts.user_source_ata.key(),
            &ctx.accounts.intent_pda.key(),
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
        &ctx.accounts.intent_pda.key(),
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
