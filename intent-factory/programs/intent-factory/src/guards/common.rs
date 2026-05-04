use anchor_lang::prelude::*;

use crate::errors::IntentError;
use crate::wire::NAMED_PREFIX;

pub fn assert_src_mint_some(src_mint: &Option<Pubkey>) -> Result<Pubkey> {
    src_mint.ok_or_else(|| IntentError::InvalidSrcMint.into())
}

pub fn assert_tail_len(remaining_len: usize) -> Result<()> {
    let max_tail = u8::MAX as usize - NAMED_PREFIX;
    require!(remaining_len <= max_tail, IntentError::TooManyRemainingAccounts);
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

pub fn validate_executor(expected_executor: &Pubkey, executor_account: &AccountInfo) -> Result<()> {
    require!(
        executor_account.key() == *expected_executor,
        IntentError::BadExecutor
    );
    require!(executor_account.is_signer, IntentError::ExecutorNotSigner);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::solana_program::system_program;

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
    fn deadline_passed_strict_gt() {
        assert!(assert_deadline_passed(100, 100).is_err());
        assert!(assert_deadline_passed(101, 100).is_ok());
    }

    #[test]
    fn tail_len_boundary() {
        let max_tail = u8::MAX as usize - NAMED_PREFIX;
        assert!(assert_tail_len(max_tail).is_ok());
        assert!(assert_tail_len(max_tail + 1).is_err());
        assert!(assert_tail_len(0).is_ok());
    }

    #[test]
    fn validate_executor_matching_signer_passes() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(&pk, true, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_executor(&pk, &ai).is_ok());
    }

    #[test]
    fn validate_executor_matching_non_signer_rejects() {
        let pk = Pubkey::new_unique();
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(&pk, false, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_executor(&pk, &ai).is_err());
    }

    #[test]
    fn validate_executor_wrong_pubkey_signer_rejects() {
        let pk = Pubkey::new_unique();
        let wrong = Pubkey::new_unique();
        let lamports = &mut 0u64;
        let mut data = vec![];
        let owner = system_program::ID;
        let ai = AccountInfo::new(&wrong, true, false, lamports, &mut data, &owner, false, 0);
        assert!(validate_executor(&pk, &ai).is_err());
    }
}
