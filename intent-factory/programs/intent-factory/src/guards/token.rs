use anchor_lang::prelude::*;
use anchor_spl::associated_token::get_associated_token_address;
use anchor_spl::token::spl_token;
use anchor_spl::token::TokenAccount;

use crate::errors::IntentError;

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

/// Read the SPL token amount from an account without full deserialization.
pub fn read_spl_amount(account: &AccountInfo) -> Result<u64> {
    let data = account.try_borrow_data()?;
    let ta = TokenAccount::try_deserialize(&mut &data[..])?;
    Ok(ta.amount)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_amount_match() {
        assert!(assert_source_amount(1000, 1000).is_ok());
        assert!(assert_source_amount(999, 1000).is_err());
        assert!(assert_source_amount(1001, 1000).is_err());
    }
}
