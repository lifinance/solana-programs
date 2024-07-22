use solana_program::{entrypoint::ProgramResult, msg, program_error::ProgramError};
use solana_program::account_info::AccountInfo;
use solana_program::pubkey::Pubkey;

#[derive(Debug)]
pub struct TrackV1InstructionData {

}

pub fn process_instruction(_program_id: &Pubkey,
                           _accounts: &[AccountInfo],
                           instruction_data: TrackV1InstructionData) -> ProgramResult {
    msg!("LI.FI Track V1 instruction");
    return Ok(());

    Err(ProgramError::InvalidInstructionData)
}
