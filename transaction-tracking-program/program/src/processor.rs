use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};
use crate::instructions::track_v1;
use crate::instructions::track_v1::TrackV1InstructionData;


#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum TrackingInstructionData {
    TrackV1(TrackV1InstructionData)
}

// set `process_instruction` as the program's main entrypoint
entrypoint!(process_instruction);
pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Deserialize the instruction data using Borsh, then execute the corresponding instruction fn
    return match TrackingInstructionData::try_from_slice(instruction_data) {
        Ok(ix_data_object) => {
            match ix_data_object {
                TrackingInstructionData::TrackV1(data) => {
                    track_v1::process_track_v1_instruction(_program_id, _accounts, data)
                }
            }
        }
        Err(err) => {
            msg!("Error: {:?}", err);
            Err(ProgramError::InvalidInstructionData)
        }
    }
}
