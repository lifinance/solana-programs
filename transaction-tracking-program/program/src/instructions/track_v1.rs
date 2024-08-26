use crate::utils;
use borsh_derive::{BorshDeserialize, BorshSerialize};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::Sysvar;
use solana_program::{entrypoint::ProgramResult, sysvar};

#[derive(BorshSerialize, BorshDeserialize)]
pub struct TrackV1InstructionData {
    pub transaction_id: [u8; 8],
}

pub fn process_track_v1_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: TrackV1InstructionData,
) -> ProgramResult {
    // fetch and verify account-inputs
    let mut account_iterator = accounts.iter();
    let sysvar_clock_account =
        utils::get_verified_readonly_account(&mut account_iterator, &sysvar::clock::id())?;
    let instruction_track_account = utils::get_unverified_readonly_account(&mut account_iterator)?;

    // *** Execute Main Logic
    let clock_info = Clock::from_account_info(sysvar_clock_account)?;

    // verify that the track account passed into the instruction is valid (derived from current or previous epoch)
    let current_epoch_track_account =
        Pubkey::find_program_address(&[&clock_info.epoch.to_le_bytes()], _program_id);
    let previous_epoch_track_account =
        Pubkey::find_program_address(&[&(clock_info.epoch - 1).to_le_bytes()], _program_id);
    if !instruction_track_account
        .key
        .eq(&current_epoch_track_account.0)
        && !instruction_track_account
            .key
            .eq(&previous_epoch_track_account.0)
    {
        solana_program::log::sol_log(
            &(String::from("Invalid tracking account passed into instruction. Current Epoch: ")
                + &clock_info.epoch.to_string()),
        );
        return Err(solana_program::program_error::ProgramError::InvalidArgument);
    }

    solana_program::log::sol_log(
        &(String::from("LI.FI TX: 0x")
            + &utils::vec_u8to_hex_string(&_instruction_data.transaction_id)),
    );

    Ok(())
}
