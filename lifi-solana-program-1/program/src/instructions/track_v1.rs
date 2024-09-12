use crate::utils;
use borsh_derive::{BorshDeserialize, BorshSerialize};
use solana_program::account_info::AccountInfo;
use solana_program::clock::Clock;
use solana_program::pubkey::Pubkey;
use solana_program::sysvar::Sysvar;
use solana_program::{entrypoint::ProgramResult, sysvar};

/**
 * Input Data for the TrackV1 Instruction
 */
#[derive(BorshSerialize, BorshDeserialize)]
pub struct TrackV1InstructionData {
    /// the unique id used to identify the transaction
    pub transaction_id: [u8; 8],
}

///
/// Executes a TrackV1 Instruction
/// * `program_id` - the public key of the lifi-solana-program-1 bytecode on-chain (referred to as __program id__)
/// * `accounts` - the input-accounts to be used in the instruction (see [README.md](../../README.md))
/// * `instruction_data` - the input-data to be used in the instruction (see [README.md](../../README.md))
pub fn process_track_v1_instruction(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    _instruction_data: TrackV1InstructionData,
) -> ProgramResult {
    // iterate over the accounts passed into the instruction
    let mut account_iterator = accounts.iter();

    // fetch and verify clock sysvar account
    let sysvar_clock_account =
        utils::get_verified_readonly_account(&mut account_iterator, &sysvar::clock::id())?;

    // parse clock sysvar account into `Clock` struct
    let clock_info = Clock::from_account_info(sysvar_clock_account)?;

    // verify that the tracking account passed into the instruction is valid
    // (i.e. that it is derived from the current or previous epoch)
    let instruction_track_account = utils::get_unverified_readonly_account(&mut account_iterator)?;
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

    // log the transaction id as hex string to stdout for debugging/readability purposes
    solana_program::log::sol_log(
        &(String::from("LI.FI TX: 0x")
            + &utils::vec_u8to_hex_string(&_instruction_data.transaction_id)),
    );

    Ok(())
}
