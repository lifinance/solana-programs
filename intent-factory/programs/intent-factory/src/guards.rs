use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::system_program;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::spl_token;
use anchor_spl::token::TokenAccount;

use crate::errors::IntentError;

pub const RECEIVER_TOKEN_VINDEX: u8 = 3;

pub fn assert_src_mint_some(src_mint: &Option<Pubkey>) -> Result<Pubkey> {
    src_mint.ok_or_else(|| IntentError::InvalidSrcMint.into())
}

pub fn assert_tail_len(remaining_len: usize) -> Result<()> {
    require!(remaining_len <= 251, IntentError::TooManyRemainingAccounts);
    Ok(())
}

pub fn assert_deadline_not_passed(now: i64, deadline: i64) -> Result<()> {
    require!(now <= deadline, IntentError::IntentExpired);
    Ok(())
}

pub fn assert_deadline_passed(now: i64, deadline: i64) -> Result<()> {
    require!(now > deadline, IntentError::IntentNotExpired);
    Ok(())
}

pub fn assert_executor_signer(
    executor: &Option<Pubkey>,
    accounts: &[AccountInfo],
) -> Result<()> {
    if let Some(exec) = executor {
        let found = accounts.iter().any(|a| a.is_signer && a.key == exec);
        require!(found, IntentError::ExecutorNotSigner);
    }
    Ok(())
}

/// Validate source_ata: pubkey must be the canonical ATA, SPL token-owned,
/// correct mint+owner. Returns the deserialized TokenAccount.
pub fn validate_source_ata(
    source_ata: &AccountInfo,
    intent_pda_key: &Pubkey,
    src_mint: &Pubkey,
) -> Result<TokenAccount> {
    let expected = get_associated_token_address(intent_pda_key, src_mint);
    require!(source_ata.key() == expected, IntentError::BadSourceAta);
    require!(
        source_ata.owner == &spl_token::ID,
        IntentError::BadSourceAta
    );
    require!(
        source_ata.data_len() == TokenAccount::LEN,
        IntentError::SourceAccountMalformed
    );

    let data = source_ata.try_borrow_data()?;
    let ta = TokenAccount::try_deserialize(&mut &data[..])?;
    require!(ta.mint == *src_mint, IntentError::BadSourceAta);
    require!(ta.owner == *intent_pda_key, IntentError::BadSourceAta);

    Ok(ta)
}

pub fn validate_receiver(receiver: &AccountInfo, expected: &Pubkey) -> Result<()> {
    require!(receiver.key() == *expected, IntentError::BadReceiver);
    Ok(())
}

/// Validate receiver_token when out_mint is Some: pubkey must be
/// ATA(receiver, out_mint), initialized, correct mint+owner.
pub fn validate_receiver_token_spl(
    receiver_token: &AccountInfo,
    receiver_key: &Pubkey,
    out_mint: &Pubkey,
) -> Result<()> {
    let expected = get_associated_token_address(receiver_key, out_mint);
    require!(
        receiver_token.key() == expected,
        IntentError::BadReceiverToken
    );
    require!(
        receiver_token.data_len() == TokenAccount::LEN,
        IntentError::BadReceiverToken
    );

    let data = receiver_token.try_borrow_data()?;
    let ta = TokenAccount::try_deserialize(&mut &data[..])?;
    require!(ta.mint == *out_mint, IntentError::BadReceiverToken);
    require!(ta.owner == *receiver_key, IntentError::BadReceiverToken);

    Ok(())
}

pub fn assert_source_amount(actual: u64, expected: u64) -> Result<()> {
    require!(actual == expected, IntentError::SourceAmountMismatch);
    Ok(())
}

pub fn assert_source_drained(source_ata: &AccountInfo, src_mint: &Pubkey) -> Result<()> {
    require!(
        source_ata.data_len() == TokenAccount::LEN,
        IntentError::SourceAccountMalformed
    );
    let data = source_ata.try_borrow_data()?;
    let ta = TokenAccount::try_deserialize(&mut &data[..])?;
    require!(ta.mint == *src_mint, IntentError::BadSourceAta);
    require!(ta.amount == 0, IntentError::SourceNotDrained);
    Ok(())
}

pub fn assert_min_out(pre: u64, post: u64, min_amount_out: u64) -> Result<()> {
    let delta = post
        .checked_sub(pre)
        .ok_or(IntentError::InsufficientOutput)?;
    require!(delta >= min_amount_out, IntentError::InsufficientOutput);
    Ok(())
}

/// Deny-list check for inner program IDs.
pub fn check_deny_list(program_id: &Pubkey, data: &[u8]) -> Result<()> {
    require!(
        program_id != &crate::id(),
        IntentError::DisallowedProgram
    );
    require!(
        program_id != &bpf_loader_upgradeable::ID,
        IntentError::DisallowedProgram
    );

    if program_id == &system_program::ID && data.len() >= 4 {
        let disc = u32::from_le_bytes(data[0..4].try_into().unwrap());
        // Assign = 1, Allocate = 8, AllocateWithSeed = 9, AssignWithSeed = 10
        if matches!(disc, 1 | 8 | 9 | 10) {
            return Err(IntentError::DisallowedSystemOp.into());
        }
    }

    Ok(())
}

/// When out_mint is None (native SOL output), require receiver_token to be
/// the System Program to prevent an unbound indexable account.
pub fn validate_receiver_token_native(
    receiver_token: &AccountInfo,
) -> Result<()> {
    require!(
        receiver_token.key() == system_program::ID,
        IntentError::BadReceiverTokenNativeOutput
    );
    Ok(())
}

/// Reject any CallSpec account reference to virtual index 3 (receiver_token)
/// in native-output mode. The slot is constrained to SystemProgram and must
/// not be used as an inner CPI account.
pub fn check_native_output_index(account_ix: u8, is_native_output: bool) -> Result<()> {
    if is_native_output && account_ix == RECEIVER_TOKEN_VINDEX {
        return Err(IntentError::ReceiverTokenRefInNativeOutput.into());
    }
    Ok(())
}

/// Only virtual index 0 (intent_pda) may have is_signer = true in inner CPIs.
pub fn check_signer_flag(account_ix: u8, is_signer: bool) -> Result<()> {
    if is_signer && account_ix != 0 {
        return Err(IntentError::InvalidSignerFlag.into());
    }
    Ok(())
}

/// Validate refund-specific accounts.
pub fn validate_refund_user(user: &AccountInfo, expected: &Pubkey) -> Result<()> {
    require!(user.key() == *expected, IntentError::BadUser);
    Ok(())
}

pub const MINT_LEN: usize = 82;

pub fn validate_src_mint_account(
    src_mint_account: &AccountInfo,
    expected: &Pubkey,
) -> Result<u8> {
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

    #[test]
    fn deny_list_rejects_self_program() {
        let self_id = crate::id();
        assert!(check_deny_list(&self_id, &[]).is_err());
    }

    #[test]
    fn deny_list_rejects_bpf_loader() {
        assert!(check_deny_list(&bpf_loader_upgradeable::ID, &[]).is_err());
    }

    #[test]
    fn deny_list_rejects_system_assign() {
        let data = 1u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_rejects_system_allocate() {
        let data = 8u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_rejects_system_allocate_with_seed() {
        let data = 9u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_rejects_system_assign_with_seed() {
        let data = 10u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_allows_system_transfer() {
        let data = 2u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_ok());
    }

    #[test]
    fn deny_list_allows_system_create_account() {
        let data = 0u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_ok());
    }

    #[test]
    fn deny_list_allows_unrelated_program() {
        let pk = Pubkey::new_unique();
        assert!(check_deny_list(&pk, &[0xAA, 0xBB]).is_ok());
    }

    #[test]
    fn deny_list_allows_system_short_data() {
        assert!(check_deny_list(&system_program::ID, &[1, 0, 0]).is_ok());
    }

    #[test]
    fn signer_flag_allows_intent_pda() {
        assert!(check_signer_flag(0, true).is_ok());
    }

    #[test]
    fn signer_flag_rejects_non_pda() {
        assert!(check_signer_flag(1, true).is_err());
        assert!(check_signer_flag(3, true).is_err());
        assert!(check_signer_flag(5, true).is_err());
    }

    #[test]
    fn signer_flag_allows_non_signer() {
        assert!(check_signer_flag(1, false).is_ok());
        assert!(check_signer_flag(5, false).is_ok());
    }

    #[test]
    fn native_output_index_rejects_vindex_3() {
        assert!(check_native_output_index(3, true).is_err());
    }

    #[test]
    fn native_output_index_allows_vindex_3_when_spl() {
        assert!(check_native_output_index(3, false).is_ok());
    }

    #[test]
    fn native_output_index_allows_other_indexes() {
        assert!(check_native_output_index(0, true).is_ok());
        assert!(check_native_output_index(1, true).is_ok());
        assert!(check_native_output_index(2, true).is_ok());
        assert!(check_native_output_index(4, true).is_ok());
    }

    #[test]
    fn src_mint_some_returns_value() {
        let pk = Pubkey::new_unique();
        assert_eq!(assert_src_mint_some(&Some(pk)).unwrap(), pk);
    }

    #[test]
    fn src_mint_none_rejects() {
        assert!(assert_src_mint_some(&None).is_err());
    }

    #[test]
    fn deadline_not_passed_at_boundary() {
        assert!(assert_deadline_not_passed(100, 100).is_ok());
        assert!(assert_deadline_not_passed(99, 100).is_ok());
        assert!(assert_deadline_not_passed(101, 100).is_err());
    }

    #[test]
    fn deadline_passed_at_boundary() {
        assert!(assert_deadline_passed(101, 100).is_ok());
        assert!(assert_deadline_passed(100, 100).is_err());
        assert!(assert_deadline_passed(99, 100).is_err());
    }

    #[test]
    fn tail_len_boundary() {
        assert!(assert_tail_len(251).is_ok());
        assert!(assert_tail_len(252).is_err());
        assert!(assert_tail_len(0).is_ok());
    }

    #[test]
    fn source_amount_match() {
        assert!(assert_source_amount(1000, 1000).is_ok());
        assert!(assert_source_amount(999, 1000).is_err());
        assert!(assert_source_amount(1001, 1000).is_err());
    }

    #[test]
    fn min_out_enforcement() {
        assert!(assert_min_out(100, 200, 100).is_ok());
        assert!(assert_min_out(100, 200, 101).is_err());
        assert!(assert_min_out(100, 199, 100).is_err());
        assert!(assert_min_out(100, 200, 0).is_ok());
    }

    #[test]
    fn min_out_overflow_protection() {
        assert!(assert_min_out(200, 100, 1).is_err());
    }

    // --- refund-specific guard tests ---

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

    #[test]
    fn deadline_passed_strict_gt() {
        assert!(assert_deadline_passed(100, 100).is_err());
        assert!(assert_deadline_passed(101, 100).is_ok());
    }
}
