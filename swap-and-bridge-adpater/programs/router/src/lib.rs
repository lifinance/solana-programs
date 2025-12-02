use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::{self, AssociatedToken},
    token::Mint,
};

mod adapters;
mod utils;

use adapters::{across, unit};

declare_id!("75uzRTFJRYCtNHGHYiLmcL57no3DDZtvnGbqhztLJnKf");

// ------------------------------------------------------------
// PROGRAM
// ------------------------------------------------------------

#[program]
pub mod router {
    use super::*;

    pub fn swap_and_bridge<'info>(
        ctx: Context<'_, '_, '_, 'info, SwapAndBridge<'info>>,
        params: SwapAndBridgeParams,
    ) -> Result<()> {
        // 1) Derive vault authority PDA
        let (derived_vault_authority, vault_bump) = Pubkey::find_program_address(
            &[
                b"vault",
                params.route_seed.as_ref(),
                ctx.accounts.mint.key().as_ref(),
            ],
            ctx.program_id,
        );

        // Ensure the passed `vault_authority` account matches the derived PDA.
        require_keys_eq!(
            derived_vault_authority,
            ctx.accounts.vault_authority.key(),
            ErrorCode::InvalidVaultAuthority
        );

        // 3) Derive the expected vault ATA
        let expected_vault_ata =
            associated_token::get_associated_token_address_with_program_id(
                &derived_vault_authority,
                &ctx.accounts.mint.key(),
                &ctx.accounts.token_program.key(),
            );

        // 4) Validate intermediate_vault matches
        require_keys_eq!(
            expected_vault_ata,
            ctx.accounts.intermediate_vault.key(),
            ErrorCode::InvalidVaultAta
        );

        // 5) Read the vault token account amount directly
        let vault_data = ctx.accounts.intermediate_vault.try_borrow_data()
            .map_err(|_| ErrorCode::InvalidVaultAccount)?;
        
        // TokenAccount amount is at offset 64 (8 bytes u64)
        if vault_data.len() < 72 {
            return Err(ErrorCode::InvalidVaultAccount.into());
        }
        let amount = u64::from_le_bytes([
            vault_data[64], vault_data[65], vault_data[66], vault_data[67],
            vault_data[68], vault_data[69], vault_data[70], vault_data[71],
        ]);
        drop(vault_data); // Explicitly drop the borrow

        // 6) Slippage & non-empty guards.
        require!(amount > 0, ErrorCode::EmptyVault);
        require!(amount >= params.min_amount, ErrorCode::SlippageExceeded);

        // 7) Dispatch to correct adapter (transfers all tokens from vault)
        match params.adapter_id {
            0 => unit::bridge_via_unit(&ctx, &params, amount, vault_bump)?,
            1 => across::bridge_via_across(&ctx, &params, amount, vault_bump)?,
            _ => return Err(ErrorCode::UnknownAdapter.into()),
        };

        // 8) Close the intermediate vault account (returns rent to payer)
        // This will fail if balance > 0, ensuring all funds were transferred
        utils::close_vault(
            ctx.accounts.intermediate_vault.to_account_info(),
            ctx.accounts.vault_authority.to_account_info(),
            ctx.accounts.payer.to_account_info(),
            ctx.accounts.token_program.to_account_info(),
            &params.route_seed,
            &ctx.accounts.mint.key(),
            vault_bump,
        )?;

        Ok(())
    }
}

// ------------------------------------------------------------
// PARAMS & CONTEXT
// ------------------------------------------------------------

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct SwapAndBridgeParams {
    /// Unique seed per route, used to derive vault authority PDA.
    pub route_seed: [u8; 8],

    /// Min amount expected after swap (slippage guard).
    pub min_amount: u64,

    /// Adapter selector (0 = Unit, others in the future).
    pub adapter_id: u8,

    /// Opaque adapter-specific payload, Borsh-encoded.
    pub adapter_payload: Vec<u8>,
}

// ------------------------------------------------------------
// ACCOUNTS
// ------------------------------------------------------------

#[derive(Accounts)]
#[instruction(params: SwapAndBridgeParams)]
pub struct SwapAndBridge<'info> {
    /// Pays for any ATA creation (e.g. Unit deposit ATA).
    #[account(mut)]
    pub payer: Signer<'info>,

    // ---------- Vault side (swap output source) ----------

    /// PDA authority of the vault token account. Must equal:
    ///   PDA("vault", route_seed, mint).
    /// Needed as authority for CPI into the token program.
    /// CHECK: Validated on-chain by deriving PDA and comparing keys
    pub vault_authority: UncheckedAccount<'info>,

    /// Token account holding the swap output (Jupiter's `toAddress`).
    ///
    /// Backend passes ONLY this token account for the vault side.
    /// On-chain we:
    ///   - Derive the expected vault_authority PDA from (route_seed, mint).
    ///   - Derive the **expected ATA** for (vault_authority, mint, token_program/token_2022).
    ///   - Check that this derived ATA == `intermediate_vault.key()`.
    ///   - Read its amount directly from account data.
    ///
    /// CHECK: Validated on-chain by deriving expected ATA and comparing keys
    #[account(mut)]
    pub intermediate_vault: UncheckedAccount<'info>,

    pub mint: Account<'info, Mint>,

    /// Token program for this mint (either TOKEN_PROGRAM_ID or TOKEN_2022_PROGRAM_ID)
    /// Client passes the correct one based on mint.owner check
    /// CHECK: Validated by CPI during token operations
    pub token_program: UncheckedAccount<'info>,

    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,

    // Adapter-specific accounts are passed via remaining_accounts
    // and accessed dynamically based on adapter_id
}

// ------------------------------------------------------------
// ERRORS
// ------------------------------------------------------------

#[error_code]
pub enum ErrorCode {
    #[msg("Intermediate vault is empty")]
    EmptyVault,

    #[msg("Swap output is below min_amount")]
    SlippageExceeded,

    #[msg("Unknown adapter ID")]
    UnknownAdapter,

    #[msg("Invalid adapter payload encoding")]
    InvalidAdapterPayload,

    #[msg("unit_deposit_wallet does not match payload")]
    DepositWalletMismatch,

    #[msg("Invalid Unit deposit token account")]
    InvalidUnitDepositAta,

    #[msg("Invalid vault authority PDA")]
    InvalidVaultAuthority,

    #[msg("Vault token account is not a valid TokenAccount")]
    InvalidVaultAccount,

    #[msg("Invalid vault token account address (does not match derived ATA)")]
    InvalidVaultAta,

    #[msg("Vault balance changed unexpectedly")]
    InsufficientVaultBalance,

    #[msg("Insufficient adapter-specific accounts provided")]
    InsufficientAccounts,

    #[msg("Invalid Across program ID")]
    InvalidAcrossProgram,

    #[msg("Failed to create approve instruction")]
    ApproveError,
}