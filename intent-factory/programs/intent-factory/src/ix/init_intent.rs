use anchor_lang::prelude::*;

use crate::errors::IntentError;
use crate::guards;
use crate::hash::compute_intent_hash;
use crate::pda::verify_intent_pda;
use crate::state::intent_header::MAX_OUTCOMES;
use crate::state::{IntentHeader, IntentState, IntentStatus};

#[derive(Accounts)]
pub struct InitIntent<'info> {
    /// CHECK: validated by PDA recompute from header_bytes.
    /// Initialized via manual realloc+assign so we can use
    /// create_program_address validation (not init+seeds).
    #[account(mut)]
    pub intent_pda: AccountInfo<'info>,

    /// CHECK: validated against header.executor in handler.
    pub executor: Signer<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_init_intent(ctx: Context<InitIntent>, header_bytes: Vec<u8>, bump: u8) -> Result<()> {
    let header = IntentHeader::decode(&header_bytes)?;
    let re_encoded = header.canonical_encode();
    require!(re_encoded == header_bytes, IntentError::MalformedHeader);

    guards::assert_src_mint_some(&header.src_mint)?;

    guards::validate_executor(&header.executor, &ctx.accounts.executor)?;

    let intent_hash = compute_intent_hash(&header_bytes);
    verify_intent_pda(&intent_hash, bump, &ctx.accounts.intent_pda.key())?;

    require!(
        ctx.accounts.intent_pda.data_is_empty(),
        IntentError::IntentAlreadyInitialized
    );

    let space = IntentState::MAX_SIZE;
    let rent = Rent::get()?.minimum_balance(space);
    let current_lamports = ctx.accounts.intent_pda.lamports();
    let needed = rent.saturating_sub(current_lamports);

    if needed > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.payer.to_account_info(),
                    to: ctx.accounts.intent_pda.to_account_info(),
                },
            ),
            needed,
        )?;
    }

    let pda_signer_seeds: &[&[u8]] = &[b"intent", &intent_hash, &[bump]];
    anchor_lang::system_program::allocate(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Allocate {
                account_to_allocate: ctx.accounts.intent_pda.to_account_info(),
            },
            &[pda_signer_seeds],
        ),
        space as u64,
    )?;

    anchor_lang::system_program::assign(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::Assign {
                account_to_assign: ctx.accounts.intent_pda.to_account_info(),
            },
            &[pda_signer_seeds],
        ),
        &crate::id(),
    )?;

    let mut outcome_mint_tags = [0u8; MAX_OUTCOMES];
    let mut outcome_mints = [Pubkey::default(); MAX_OUTCOMES];
    let mut outcome_accounts = [Pubkey::default(); MAX_OUTCOMES];
    let mut outcome_amounts = [0u64; MAX_OUTCOMES];
    for (i, o) in header.outcomes.iter().enumerate() {
        if let Some(mint) = o.mint {
            outcome_mint_tags[i] = 1;
            outcome_mints[i] = mint;
        }
        outcome_accounts[i] = o.account;
        outcome_amounts[i] = o.amount;
    }

    let state = IntentState {
        intent_hash,
        status: IntentStatus::Initialized,
        bump,
        user: header.user,
        src_mint: header.src_mint,
        amount_in: header.amount_in,
        outcome_count: header.outcomes.len() as u8,
        outcome_mint_tags,
        outcome_mints,
        outcome_accounts,
        outcome_amounts,
        deadline: header.deadline,
        salt: header.salt,
        executor: header.executor,
    };

    let mut data = ctx.accounts.intent_pda.try_borrow_mut_data()?;
    let mut writer = &mut data[..];
    state.try_serialize(&mut writer)?;

    Ok(())
}
