use borsh_derive::{BorshDeserialize, BorshSerialize};
use solana_program::{entrypoint::ProgramResult, msg, sysvar};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::Sysvar;
use crate::utils;

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub struct TrackV1InstructionData {

}

pub fn process_instruction(_program_id: &Pubkey,
                           accounts: &[AccountInfo],
                           _instruction_data: TrackV1InstructionData) -> ProgramResult {
    msg!("LI.FI Track V1 instruction");

    // fetch and verify account-inputs
    let account_iterator = &mut accounts.iter();
    let sysvar_clock_account = utils::get_verified_readonly_account(account_iterator, &sysvar::clock::id())?;

    // *** Execute Main Logic
    let clock_info = Clock::from_account_info(&sysvar_clock_account)?;
    msg!("Current Slot: {}", clock_info.slot);
    msg!("Current Epoch: {}", clock_info.epoch);
    
    msg!("LI.FI Track V1 instruction completed");

    return Ok(());
}
