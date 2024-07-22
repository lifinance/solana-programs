use std::slice::Iter;
use solana_program::account_info::{AccountInfo, next_account_info};
use solana_program::msg;
use solana_program::program_error::ProgramError;
use solana_program::pubkey::Pubkey;

pub fn get_verified_readonly_account<'a, 'b>(accounts_iter: &'a mut Iter<'a, AccountInfo<'b>>, matches_account: &Pubkey) ->  Result<&'a AccountInfo<'b>, ProgramError> {
    let account = next_account_info(accounts_iter)?;
    if account.key != matches_account {
        // Use string concatenation for efficiency
        msg!(&("Expected account '".to_owned() + &matches_account.to_string() + "', found '" + &account.key.to_string() + "'"));
        return Err(ProgramError::InvalidArgument);
    }
    if account.is_writable {
        msg!(&("Account '".to_owned() + &account.key.to_string() + "' is not readonly"));
        return Err(ProgramError::InvalidArgument);
    }
    Ok(account)
}