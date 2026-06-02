use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
    system_program,
};

use crate::constants::MAX_TRANSFER_NB;
use crate::errors::IntentSolverError;
use crate::helpers::{
    close_pda_ata, close_pda_sol, create_ata_idempotent, read_pubkey, read_u64_le,
    transfer_pda_ata_all, verify_spl_source_ata,
};
use crate::intent_hash::{hash_intent, INTENT_PDA_SEED};
#[allow(deprecated)]
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_token::ID as SPL_TOKEN_ID;
use spl_token_2022::ID as SPL_TOKEN_2022_ID;

/// `refund` (variant 1): returns vault funds to `funder` and closes the vault.
///
/// Instruction data (after the variant byte), all little-endian:
///   amount_in: u64
///   salt:      [u8; 32]
///   transfer_nb: u8 (1..=8)
///   repeated transfer_nb times: destination [u8; 32] then min_amount u64
///
/// These rebuild the exact intent preimage so the vault PDA can be re-derived and verified
/// (transfer destinations are carried in data, not as accounts, since refund never touches them).
///
/// Accounts:
///   0. executor          signer (also the intent's `executor` hash input; pays SPL ATA rent)
///   1. pda               writable (vault)
///   2. funder            SOL: writable (lamport recipient) / SPL: readonly (ATA owner)
///   3. from_program      system_program (SOL) | SPL Token or Token-2022 (SPL)
///   SPL only:
///   4. pda_ata           writable (vault source ATA)
///   5. funder_ata        writable (refund destination ATA)
///   6. mint              readonly
///   7. system_program    readonly (for idempotent ATA create)
///   8. associated_token_program readonly (for idempotent ATA create)
pub fn refund(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    rest: &[u8],
) -> ProgramResult {
    // ---- 1. Parse instruction data (full intent preimage, no CPI blob) -------------------
    let mut cursor = 0usize;
    let amount_in = read_u64_le(rest, &mut cursor)?;
    let salt_slice = rest
        .get(cursor..cursor + 32)
        .ok_or(IntentSolverError::InsufficientData)?;
    let salt: [u8; 32] = salt_slice
        .try_into()
        .map_err(|_| IntentSolverError::InsufficientData)?;
    cursor += 32;

    let transfer_nb = *rest
        .get(cursor)
        .ok_or(IntentSolverError::InsufficientData)?;
    cursor += 1;
    if transfer_nb == 0 || transfer_nb > MAX_TRANSFER_NB {
        return Err(IntentSolverError::InvalidTransferNb.into());
    }

    let mut transfer_dest_pubkeys = [Pubkey::default(); MAX_TRANSFER_NB as usize];
    let mut transfer_min_amounts = [0u64; MAX_TRANSFER_NB as usize];
    for i in 0..transfer_nb as usize {
        transfer_dest_pubkeys[i] = read_pubkey(rest, &mut cursor)?;
        transfer_min_amounts[i] = read_u64_le(rest, &mut cursor)?;
    }

    if cursor != rest.len() {
        return Err(ProgramError::InvalidInstructionData);
    }

    // ---- 2. Account header ---------------------------------------------------------------
    let mut idx = 0usize;
    let executor = accounts
        .get(idx)
        .ok_or(IntentSolverError::NotEnoughAccounts)?;
    idx += 1;
    let pda = accounts
        .get(idx)
        .ok_or(IntentSolverError::NotEnoughAccounts)?;
    idx += 1;
    let funder = accounts
        .get(idx)
        .ok_or(IntentSolverError::NotEnoughAccounts)?;
    idx += 1;
    let from_program = accounts
        .get(idx)
        .ok_or(IntentSolverError::NotEnoughAccounts)?;
    idx += 1;

    if !executor.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // ---- 3. From-token path + mint for preimage (SOL vs SPL canonical ATA -> ta.mint) ----
    let (from_is_spl, pda_ata, mint_pubkey) = if from_program.key == &system_program::ID {
        (false, None, system_program::ID)
    } else if from_program.key == &SPL_TOKEN_ID || from_program.key == &SPL_TOKEN_2022_ID {
        let pa = accounts.get(idx).ok_or(IntentSolverError::NotEnoughAccounts)?;
        idx += 1;
        let mint = verify_spl_source_ata(pda, from_program, pa)?;
        (true, Some(pa), mint)
    } else {
        return Err(IntentSolverError::WrongTokenProgram.into());
    };

    // ---- 4. Intent hash, PDA verify, CPI signer seeds ------------------------------------
    let intent_hash = hash_intent(
        funder.key,
        &mint_pubkey,
        amount_in,
        &salt,
        executor.key,
        transfer_nb,
        &transfer_dest_pubkeys[..transfer_nb as usize],
        &transfer_min_amounts[..transfer_nb as usize],
    );

    let (pda_expected, bump) =
        Pubkey::find_program_address(&[INTENT_PDA_SEED, intent_hash.as_ref()], program_id);
    if pda.key != &pda_expected {
        msg!("PDA mismatch. expected={}, got={}", pda_expected, pda.key);
        return Err(IntentSolverError::InvalidPda.into());
    }
    let bump_seed = [bump];
    let pda_signer_seeds: &[&[u8]] = &[INTENT_PDA_SEED, intent_hash.as_ref(), bump_seed.as_ref()];

    // ---- 5. Refund funds to funder, then close the vault ---------------------------------
    if from_is_spl {
        let pda_ata = pda_ata.unwrap();
        let funder_ata = accounts.get(idx).ok_or(IntentSolverError::NotEnoughAccounts)?;
        idx += 1;
        let mint = accounts.get(idx).ok_or(IntentSolverError::NotEnoughAccounts)?;
        idx += 1;
        let system_program_account =
            accounts.get(idx).ok_or(IntentSolverError::NotEnoughAccounts)?;
        idx += 1;
        let ata_program = accounts.get(idx).ok_or(IntentSolverError::NotEnoughAccounts)?;

        // Bind the passed mint to the source ATA's mint, and the destination to funder's
        // canonical ATA, so the executor can't redirect the refund.
        if mint.key != &mint_pubkey {
            return Err(IntentSolverError::InvalidDestinationAta.into());
        }
        let expected_funder_ata =
            get_associated_token_address_with_program_id(funder.key, mint.key, from_program.key);
        if funder_ata.key != &expected_funder_ata {
            return Err(IntentSolverError::InvalidDestinationAta.into());
        }

        create_ata_idempotent(
            executor,
            funder_ata,
            funder,
            mint,
            system_program_account,
            from_program,
            ata_program,
        )?;
        transfer_pda_ata_all(pda, pda_ata, funder_ata, from_program, pda_signer_seeds)?;
        // Rent recipient is the caller (executor), per the refund policy.
        close_pda_ata(pda, pda_ata, executor, from_program, pda_signer_seeds)?;
    } else {
        // SOL: sweep all lamports (principal + rent) to funder, which closes the PDA.
        close_pda_sol(pda, funder, from_program, pda_signer_seeds)?;
    }

    Ok(())
}
