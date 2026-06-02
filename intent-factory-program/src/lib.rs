use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg,
    program_error::ProgramError, pubkey::Pubkey,
};

#[path = "const.rs"]
pub mod constants;
pub mod errors;
pub mod helpers;
pub mod intent_hash;
pub mod instructions;

use crate::instructions::execute::execute;
use crate::instructions::refund::refund;

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!(
        "process_instruction: {}: {} accounts, data_len={}",
        program_id,
        accounts.len(),
        instruction_data.len()
    );

    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let variant = instruction_data[0];
    let rest = &instruction_data[1..];

    match variant {
        0 => execute(program_id, accounts, rest),
        1 => refund(program_id, accounts, rest),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
