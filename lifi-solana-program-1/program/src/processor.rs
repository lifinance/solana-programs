use crate::instructions::track_v1;
use crate::instructions::track_v1::TrackV1InstructionData;
use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::log::sol_log;
use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_error::ProgramError,
    pubkey::Pubkey,
};

#[derive(BorshSerialize, BorshDeserialize)]
pub enum InstructionData {
    TrackV1(TrackV1InstructionData),
}


/// The entrypoint function for the entire program - every instruction call executes this function
pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Deserialize the instruction data using Borsh, then execute the corresponding instruction fn
    match InstructionData::try_from_slice(instruction_data) {
        Ok(ix_data_object) => match ix_data_object {
            InstructionData::TrackV1(data) => {
                track_v1::process_track_v1_instruction(_program_id, _accounts, data)
            }
        },
        Err(err) => {
            sol_log(&(String::from("Error: ") + &err.to_string()));
            Err(ProgramError::InvalidInstructionData)
        }
    }
}
