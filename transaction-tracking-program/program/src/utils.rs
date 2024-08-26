use solana_program::account_info::{next_account_info, AccountInfo};
use solana_program::log::sol_log;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;
use std::slice::Iter;

/// Returns the next account of an AccountInfo iterator if and only if it matches
/// the correct public key and is readonly. Otherwise, it returns an error.
pub fn get_verified_readonly_account<'a, 'b>(
    accounts_iter: &'_ mut Iter<'a, AccountInfo<'b>>,
    matches_account: &Pubkey,
) -> Result<&'a AccountInfo<'b>, ProgramError> {
    let account = next_account_info(accounts_iter)?;
    if account.key != matches_account {
        sol_log(
            &(String::from("Expected account '")
                + &matches_account.to_string()
                + "', found '"
                + &account.key.to_string()
                + "'"),
        );
        return Err(ProgramError::InvalidArgument);
    }
    assert_account_is_readonly(account)?;

    Ok(account)
}

/// Returns the next account of an AccountInfo iterator if and only if it is readonly.
/// Otherwise, it returns an error.
pub fn get_unverified_readonly_account<'a, 'b>(
    accounts_iter: &'a mut Iter<'a, AccountInfo<'b>>,
) -> Result<&'a AccountInfo<'b>, ProgramError> {
    let account = next_account_info(accounts_iter)?;
    assert_account_is_readonly(account)?;

    Ok(account)
}

/**
 * Asserts that an account is readonly.
 * If the account is writable, it logs an error and returns an error.
 */
pub fn assert_account_is_readonly(account: &AccountInfo) -> Result<(), ProgramError> {
    if account.is_writable {
        sol_log(&(String::from("Account '") + &account.key.to_string() + "' is not readonly"));
        return Err(ProgramError::InvalidArgument);
    }
    Ok(())
}

const HEX_CHAR_LOOKUP: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
];

// this avoids using the `format!` macro which would cost more compute units
pub fn vec_u8to_hex_string(array: &[u8]) -> String {
    let mut hex_string = String::new();
    for byte in array {
        hex_string.push(HEX_CHAR_LOOKUP[(byte >> 4) as usize]);
        hex_string.push(HEX_CHAR_LOOKUP[(byte & 0xF) as usize]);
    }
    hex_string
}
