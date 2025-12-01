use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::{self},
    token::{self, TransferChecked},
};
use crate::{ErrorCode, SwapAndBridgeParams, SwapAndBridge};

// ------------------------------------------------------------
// UNIT ADAPTER
// ------------------------------------------------------------

/// Payload structure for Unit adapter
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UnitAdapterPayload {
    /// Unit deposit wallet (from getUnitDepositAddress)
    pub unit_deposit_wallet: Pubkey,
    /// Future extension: 0 = SPL / wSOL, 1 = native SOL, etc.
    pub is_native: u8,
}

/// Bridge tokens via Unit adapter
///
/// This function:
/// 1. Validates the Unit deposit wallet from the payload
/// 2. Ensures the Unit deposit ATA exists (creates idempotently if needed)
/// 3. Transfers tokens from the vault to the Unit deposit ATA
///
/// # Accounts (from remaining_accounts)
/// - Position 0 (7): unit_deposit_wallet - Unit bridge destination wallet
/// - Position 1 (8): unit_deposit_ata - Token account for Unit deposit
pub fn bridge_via_unit<'info>(
    ctx: &anchor_lang::context::Context<'_, '_, '_, 'info, SwapAndBridge<'info>>,
    params: &SwapAndBridgeParams,
    amount: u64,
    vault_bump: u8,
) -> Result<()> {
    // 1) Decode adapter payload
    let payload: UnitAdapterPayload =
        UnitAdapterPayload::try_from_slice(&params.adapter_payload)
            .map_err(|_| ErrorCode::InvalidAdapterPayload)?;

    // 2) Access Unit accounts from remaining_accounts
    let remaining = &ctx.remaining_accounts;
    require!(
        remaining.len() >= 2,
        ErrorCode::InsufficientAccounts
    );

    let unit_deposit_wallet = &remaining[0]; // Position 7
    let unit_deposit_ata = &remaining[1];     // Position 8

    // 3) Verify deposit wallet matches payload
    require_keys_eq!(
        payload.unit_deposit_wallet,
        unit_deposit_wallet.key(),
        ErrorCode::DepositWalletMismatch
    );

    //  4) Derive and validate Unit deposit ATA
    let token_program_key = ctx.accounts.token_program.key();
    let expected_ata = associated_token::get_associated_token_address_with_program_id(
        &unit_deposit_wallet.key(),
        &ctx.accounts.mint.key(),
        &token_program_key,
    );
    require_keys_eq!(
        expected_ata,
        unit_deposit_ata.key(),
        ErrorCode::InvalidUnitDepositAta
    );

    // 5) Create Unit deposit ATA idempotently if not existing
    let create_ata_accounts = associated_token::Create {
        payer: ctx.accounts.payer.to_account_info(),
        associated_token: unit_deposit_ata.clone(),
        authority: unit_deposit_wallet.clone(),
        mint: ctx.accounts.mint.to_account_info(),
        system_program: ctx.accounts.system_program.to_account_info(),
        token_program: ctx.accounts.token_program.to_account_info(),
    };
    let create_ata_ctx = CpiContext::new(
        ctx.accounts.associated_token_program.to_account_info(),
        create_ata_accounts,
    );
    associated_token::create_idempotent(create_ata_ctx)?;

    // 7) Transfer full amount from vault → Unit deposit ATA
    let cpi_accounts = TransferChecked {
        from: ctx.accounts.intermediate_vault.to_account_info(),
        mint: ctx.accounts.mint.to_account_info(),
        to: unit_deposit_ata.clone(),
        authority: ctx.accounts.vault_authority.to_account_info(),
    };

    // 8) PDA signer seeds for vault authority
    let mint_key = ctx.accounts.mint.key();
    let vault_seeds: &[&[u8]] = &[
        b"vault",
        params.route_seed.as_ref(),
        mint_key.as_ref(),
        &[vault_bump],
    ];
    let signer_seeds = &[vault_seeds];

    let cpi_ctx = CpiContext::new_with_signer(
        ctx.accounts.token_program.to_account_info(),
        cpi_accounts,
        signer_seeds
    );

    token::transfer_checked(cpi_ctx, amount, ctx.accounts.mint.decimals)?;

    Ok(())
}

