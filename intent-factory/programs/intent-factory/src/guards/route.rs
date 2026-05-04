use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;
use anchor_lang::solana_program::system_program;

use crate::errors::IntentError;

/// Deny-list check for inner program IDs.
pub fn check_deny_list(program_id: &Pubkey, data: &[u8]) -> Result<()> {
    require!(program_id != &crate::id(), IntentError::DisallowedProgram);
    require!(
        program_id != &bpf_loader_upgradeable::ID,
        IntentError::DisallowedProgram
    );

    if program_id == &system_program::ID && data.len() >= 4 {
        let disc = u32::from_le_bytes(data[0..4].try_into().unwrap());
        // Assign = 1, Allocate = 8, AllocateWithSeed = 9, AssignWithSeed = 10
        if matches!(disc, 1 | 8 | 9 | 10) {
            return Err(IntentError::DisallowedSystemOp.into());
        }
    }

    Ok(())
}

/// Only virtual index 0 (intent_pda) may have is_signer = true in inner CPIs.
pub fn check_signer_flag(account_ix: u8, is_signer: bool) -> Result<()> {
    if is_signer && account_ix != 0 {
        return Err(IntentError::InvalidSignerFlag.into());
    }
    Ok(())
}

/// Reject writable route metas for virtual index 0 (intent_pda) so generic
/// CPIs cannot request mutable access to the state-bearing signer.
pub fn check_writable_intent_pda(account_ix: u8, is_writable: bool) -> Result<()> {
    if is_writable && account_ix == 0 {
        return Err(IntentError::InvalidWritableIntentPda.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_list_rejects_self_program() {
        let self_id = crate::id();
        assert!(check_deny_list(&self_id, &[]).is_err());
    }

    #[test]
    fn deny_list_rejects_bpf_loader() {
        assert!(check_deny_list(&bpf_loader_upgradeable::ID, &[]).is_err());
    }

    #[test]
    fn deny_list_rejects_system_assign() {
        let data = 1u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_rejects_system_allocate() {
        let data = 8u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_rejects_system_allocate_with_seed() {
        let data = 9u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_rejects_system_assign_with_seed() {
        let data = 10u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_err());
    }

    #[test]
    fn deny_list_allows_system_transfer() {
        let data = 2u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_ok());
    }

    #[test]
    fn deny_list_allows_system_create_account() {
        let data = 0u32.to_le_bytes();
        assert!(check_deny_list(&system_program::ID, &data).is_ok());
    }

    #[test]
    fn deny_list_allows_unrelated_program() {
        let pk = Pubkey::new_unique();
        assert!(check_deny_list(&pk, &[0xAA, 0xBB]).is_ok());
    }

    #[test]
    fn deny_list_allows_system_short_data() {
        assert!(check_deny_list(&system_program::ID, &[1, 0, 0]).is_ok());
    }

    #[test]
    fn signer_flag_allows_intent_pda() {
        assert!(check_signer_flag(0, true).is_ok());
    }

    #[test]
    fn signer_flag_rejects_non_pda() {
        assert!(check_signer_flag(1, true).is_err());
        assert!(check_signer_flag(3, true).is_err());
        assert!(check_signer_flag(5, true).is_err());
    }

    #[test]
    fn signer_flag_allows_non_signer() {
        assert!(check_signer_flag(1, false).is_ok());
        assert!(check_signer_flag(5, false).is_ok());
    }
}
