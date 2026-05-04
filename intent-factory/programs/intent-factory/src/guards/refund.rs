use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::spl_token;
use anchor_spl::token::TokenAccount;

use crate::errors::IntentError;

pub const MINT_LEN: usize = 82;

/// Validate refund-specific accounts.
pub fn validate_refund_user(user: &AccountInfo, expected: &Pubkey) -> Result<()> {
    require!(user.key() == *expected, IntentError::BadUser);
    Ok(())
}

pub fn validate_src_mint_account(src_mint_account: &AccountInfo, expected: &Pubkey) -> Result<u8> {
    require!(src_mint_account.key() == *expected, IntentError::BadSrcMint);
    require!(
        src_mint_account.owner == &spl_token::ID,
        IntentError::BadSrcMint
    );
    require!(
        src_mint_account.data_len() == MINT_LEN,
        IntentError::BadSrcMint
    );
    let data = src_mint_account.try_borrow_data()?;
    let decimals = data[44];
    Ok(decimals)
}

pub fn validate_user_source_ata(
    user_source_ata: &AccountInfo,
    user_key: &Pubkey,
    src_mint: &Pubkey,
) -> Result<()> {
    let expected = get_associated_token_address(user_key, src_mint);
    require!(
        user_source_ata.key() == expected,
        IntentError::BadUserSourceAta
    );

    if !user_source_ata.data_is_empty() {
        require!(
            user_source_ata.owner == &spl_token::ID,
            IntentError::BadUserSourceAta
        );
        require!(
            user_source_ata.data_len() == TokenAccount::LEN,
            IntentError::BadUserSourceAta
        );
        let data = user_source_ata.try_borrow_data()?;
        let ta = TokenAccount::try_deserialize(&mut &data[..])?;
        require!(ta.mint == *src_mint, IntentError::BadUserSourceAta);
        require!(ta.owner == *user_key, IntentError::BadUserSourceAta);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::system_program;

    #[test]
    fn refund_user_matches() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_refund_user(&ai, &pk).is_ok());
    }

    #[test]
    fn refund_user_mismatch_rejects() {
        let pk = Pubkey::new_unique();
        let wrong = Pubkey::new_unique();
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_refund_user(&ai, &wrong).is_err());
    }

    fn make_mint_data(decimals: u8) -> Vec<u8> {
        let mut data = vec![0u8; MINT_LEN];
        data[44] = decimals;
        data
    }

    #[test]
    fn src_mint_account_valid() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 1_000_000u64;
        let mut data = make_mint_data(6);
        let owner = spl_token::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        let result = validate_src_mint_account(&ai, &pk);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 6);
    }

    #[test]
    fn src_mint_account_wrong_pubkey_rejects() {
        let pk = Pubkey::new_unique();
        let wrong = Pubkey::new_unique();
        let lamports = &mut 1_000_000u64;
        let mut data = make_mint_data(6);
        let owner = spl_token::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_src_mint_account(&ai, &wrong).is_err());
    }

    #[test]
    fn src_mint_account_wrong_owner_rejects() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 1_000_000u64;
        let mut data = make_mint_data(6);
        let owner = system_program::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_src_mint_account(&ai, &pk).is_err());
    }

    #[test]
    fn src_mint_account_wrong_len_rejects() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 1_000_000u64;
        let mut data = vec![0u8; 50];
        let owner = spl_token::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_src_mint_account(&ai, &pk).is_err());
    }

    #[test]
    fn src_mint_account_extracts_decimals() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 1_000_000u64;
        let mut data = make_mint_data(9);
        let owner = spl_token::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert_eq!(validate_src_mint_account(&ai, &pk).unwrap(), 9);
    }

    #[test]
    fn user_source_ata_wrong_pubkey_rejects() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let wrong_key = Pubkey::new_unique();
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(
            &wrong_key, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_err());
    }

    #[test]
    fn user_source_ata_uninitialized_accepts() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let expected = get_associated_token_address(&user, &mint);
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(
            &expected, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_ok());
    }

    fn make_token_account_data(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Vec<u8> {
        let mut data = vec![0u8; TokenAccount::LEN];
        data[0..32].copy_from_slice(mint.as_ref());
        data[32..64].copy_from_slice(owner.as_ref());
        data[64..72].copy_from_slice(&amount.to_le_bytes());
        // state = Initialized (1) at offset 108
        data[108] = 1;
        data
    }

    #[test]
    fn user_source_ata_initialized_valid() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let expected = get_associated_token_address(&user, &mint);
        let lamports = &mut 1_000_000u64;
        let mut data = make_token_account_data(&mint, &user, 500);
        let owner = spl_token::ID;
        let ai = AccountInfo::new(
            &expected, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_ok());
    }

    #[test]
    fn user_source_ata_initialized_wrong_mint_rejects() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let wrong_mint = Pubkey::new_unique();
        let expected = get_associated_token_address(&user, &mint);
        let lamports = &mut 1_000_000u64;
        let mut data = make_token_account_data(&wrong_mint, &user, 500);
        let owner = spl_token::ID;
        let ai = AccountInfo::new(
            &expected, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_err());
    }

    #[test]
    fn user_source_ata_initialized_wrong_owner_rejects() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let wrong_user = Pubkey::new_unique();
        let expected = get_associated_token_address(&user, &mint);
        let lamports = &mut 1_000_000u64;
        let mut data = make_token_account_data(&mint, &wrong_user, 500);
        let owner = spl_token::ID;
        let ai = AccountInfo::new(
            &expected, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_err());
    }

    #[test]
    fn user_source_ata_initialized_wrong_program_owner_rejects() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let expected = get_associated_token_address(&user, &mint);
        let lamports = &mut 1_000_000u64;
        let mut data = make_token_account_data(&mint, &user, 500);
        let owner = system_program::ID;
        let ai = AccountInfo::new(
            &expected, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_err());
    }

    #[test]
    fn user_source_ata_initialized_wrong_data_len_rejects() {
        let user = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let expected = get_associated_token_address(&user, &mint);
        let lamports = &mut 1_000_000u64;
        let mut data = vec![0u8; 100];
        let owner = spl_token::ID;
        let ai = AccountInfo::new(
            &expected, false, false, lamports, &mut data, &owner, false, 0,
        );
        assert!(validate_user_source_ata(&ai, &user, &mint).is_err());
    }
}
