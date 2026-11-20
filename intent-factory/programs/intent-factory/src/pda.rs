use anchor_lang::prelude::*;

use crate::errors::IntentError;

pub fn verify_intent_pda(intent_hash: &[u8; 32], bump: u8, expected_key: &Pubkey) -> Result<()> {
    let seeds: &[&[u8]] = &[b"intent", intent_hash, &[bump]];
    let derived = Pubkey::create_program_address(seeds, &crate::id())
        .map_err(|_| IntentError::BadIntentPda)?;
    require!(derived == *expected_key, IntentError::BadIntentPda);
    Ok(())
}
