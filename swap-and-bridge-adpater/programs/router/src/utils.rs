use anchor_lang::prelude::*;
use anchor_spl::token::{self, CloseAccount};

/// Close the intermediate vault token account, returning rent to the payer.
/// This will fail if the vault has a non-zero balance, ensuring all funds were transferred.
pub fn close_vault<'info>(
    vault: AccountInfo<'info>,
    vault_authority: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    route_seed: &[u8; 8],
    mint_key: &Pubkey,
    vault_bump: u8,
) -> Result<()> {
    let bump_slice = [vault_bump];
    let vault_seeds: &[&[u8]] = &[
        b"vault",
        route_seed.as_ref(),
        mint_key.as_ref(),
        &bump_slice,
    ];
    let signer_seeds = [vault_seeds];

    let close_accounts = CloseAccount {
        account: vault,
        destination: payer,
        authority: vault_authority,
    };

    let close_ctx = CpiContext::new_with_signer(
        token_program,
        close_accounts,
        &signer_seeds,
    );

    token::close_account(close_ctx)
}

