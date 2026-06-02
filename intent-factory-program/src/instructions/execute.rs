use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    msg,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvar::{rent::Rent, Sysvar},
    system_program,
};

use crate::constants::{
    FLAG_INNER_SIGNER, FLAG_INNER_WRITABLE, MAX_ACC_PER_CPI, MAX_CPI_NB, MAX_OVERRIDE_PER_CPI,
    MAX_TRANSFER_NB,
};
use crate::errors::IntentSolverError;
use crate::helpers::{
    close_pda_ata, close_pda_sol, is_cpi_program_denied, read_spl_balance,
    read_transfer_check_balance, read_u16_le, read_u64_le, verify_spl_source_ata,
};
use crate::intent_hash::{hash_intent, INTENT_PDA_SEED};
use spl_token::ID as SPL_TOKEN_ID;
use spl_token_2022::ID as SPL_TOKEN_2022_ID;

/// Builds `AccountMeta` for one account in an inner CPI slice.
///
/// Inner **`is_writable`** starts from the **outer** account meta, then any sparse **`(pos,
/// flags)`** override for this **`slice_idx`** sets **`is_writable`** from **`flags & 1`**.
///
/// Inner **`is_signer`** is **only** taken from overrides: when an override targets this index,
/// **`is_signer = flags & 2`**. There is no implicit signer for the vault PDA or any other key;
/// clients that omit the PDA as a transaction signer must supply a signer override for its CPI
/// index (the TS SDK generates these from inner `Instruction` account roles).
fn inner_account_meta(
    account: &AccountInfo,
    slice_idx: u8,
    overrides: &[(u8, u8)],
    override_len: usize,
    pda: &Pubkey,
    executor: &Pubkey,
) -> Result<AccountMeta, ProgramError> {
    let mut is_signer = false;
    let mut is_writable = account.is_writable;
    for k in 0..override_len {
        let (pos, flags) = overrides[k];
        if pos == slice_idx {
            is_writable = (flags & FLAG_INNER_WRITABLE) != 0;
            is_signer = (flags & FLAG_INNER_SIGNER) != 0;
        }
    }
    if is_signer && account.key != pda && account.key != executor {
        return Err(IntentSolverError::InnerSignerNotAllowed.into());
    }
    if is_writable && !account.is_writable {
        return Err(IntentSolverError::WritableEscalationNotAllowed.into());
    }
    Ok(AccountMeta {
        pubkey: *account.key,
        is_signer,
        is_writable,
    })
}

pub fn execute(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    rest: &[u8],
) -> ProgramResult {
    // ---- 1. Parse instruction data (intent prefix; CPI blob follows) --------------------
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

    let mut transfer_min_amounts = [0u64; MAX_TRANSFER_NB as usize];
    for i in 0..transfer_nb as usize {
        transfer_min_amounts[i] = read_u64_le(rest, &mut cursor)?;
    }

    let mut cpi_cursor = cursor;
    let cpi_count = *rest
        .get(cpi_cursor)
        .ok_or(IntentSolverError::InsufficientData)?;
    cpi_cursor += 1;
    if cpi_count == 0 || cpi_count > MAX_CPI_NB {
        return Err(IntentSolverError::InvalidCpiCount.into());
    }

    // ---- 2. Account list (executor, pda, funder, from_program, [pda_ata], checks, CPIs) ---
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

    // ---- 3. From-token path + mint for preimage (SOL vs SPL canonical ATA -> ta.mint) -----
    let (from_is_spl, token_program_from, pda_ata, mint_pubkey) =
        if from_program.key == &system_program::ID {
            (false, None, None, system_program::ID)
        } else if from_program.key == &SPL_TOKEN_ID || from_program.key == &SPL_TOKEN_2022_ID {
            let pa = accounts.get(idx).ok_or(IntentSolverError::NotEnoughAccounts)?;
            idx += 1;
            let mint = verify_spl_source_ata(pda, from_program, pa)?;
            (true, Some(from_program), Some(pa), mint)
        } else {
            return Err(IntentSolverError::WrongTokenProgram.into());
        };

    let transfer_section_start = idx;
    let mut transfer_dest_pubkeys = [Pubkey::default(); MAX_TRANSFER_NB as usize];
    for i in 0..transfer_nb as usize {
        let dest = accounts
            .get(transfer_section_start + i)
            .ok_or(IntentSolverError::NotEnoughAccounts)?;
        transfer_dest_pubkeys[i] = *dest.key;
    }
    idx += transfer_nb as usize;

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

    // ---- 5. Pre-snapshot balances at transfer-check destinations -------------------------
    let mut pre_balances = [0u64; MAX_TRANSFER_NB as usize];
    for i in 0..transfer_nb as usize {
        let dest = &accounts[transfer_section_start + i];
        pre_balances[i] = read_transfer_check_balance(dest)?;
    }

    // ---- 6. Multi-CPI loop (wire after intent prefix: cpi_count + repeated CPI specs) -----
    for _ in 0..cpi_count {
        let acc_count = *rest
            .get(cpi_cursor)
            .ok_or(IntentSolverError::InsufficientData)?;
        cpi_cursor += 1;
        if acc_count < 1 || acc_count > MAX_ACC_PER_CPI {
            return Err(IntentSolverError::InvalidCpiAccountSpec.into());
        }

        let override_count = *rest
            .get(cpi_cursor)
            .ok_or(IntentSolverError::InsufficientData)?;
        cpi_cursor += 1;
        if override_count > MAX_OVERRIDE_PER_CPI {
            return Err(IntentSolverError::InvalidCpiAccountSpec.into());
        }

        let mut override_pairs = [(0u8, 0u8); MAX_OVERRIDE_PER_CPI as usize];
        for k in 0..override_count as usize {
            let pos = *rest
                .get(cpi_cursor)
                .ok_or(IntentSolverError::InsufficientData)?;
            cpi_cursor += 1;
            let flags = *rest
                .get(cpi_cursor)
                .ok_or(IntentSolverError::InsufficientData)?;
            cpi_cursor += 1;
            if pos >= acc_count {
                return Err(IntentSolverError::InvalidCpiAccountSpec.into());
            }
            override_pairs[k] = (pos, flags);
        }

        let inner_data_len = read_u16_le(rest, &mut cpi_cursor)? as usize;
        let inner_data_end = cpi_cursor
            .checked_add(inner_data_len)
            .ok_or(IntentSolverError::InsufficientData)?;
        let inner_ix_data = rest
            .get(cpi_cursor..inner_data_end)
            .ok_or(IntentSolverError::InsufficientData)?;
        cpi_cursor = inner_data_end;

        let acc_count_usize = acc_count as usize;
        let cpi_slice = accounts
            .get(idx..idx + acc_count_usize)
            .ok_or(IntentSolverError::NotEnoughAccounts)?;
        idx += acc_count_usize;

        let program_id_inner = *cpi_slice[0].key;
        if program_id_inner == *program_id {
            return Err(IntentSolverError::CpiToSelfNotAllowed.into());
        }
        if is_cpi_program_denied(&program_id_inner) {
            return Err(IntentSolverError::CpiProgramDenied.into());
        }
        let inner_accounts = &cpi_slice[1..];

        let mut cpi_metas = Vec::with_capacity(inner_accounts.len());
        for (j, a) in inner_accounts.iter().enumerate() {
            let slice_idx = (j + 1) as u8;
            cpi_metas.push(inner_account_meta(
                a,
                slice_idx,
                &override_pairs,
                override_count as usize,
                pda.key,
                executor.key,
            )?);
        }

        let inner_ix = Instruction {
            program_id: program_id_inner,
            accounts: cpi_metas,
            data: inner_ix_data.to_vec(),
        };
        invoke_signed(&inner_ix, inner_accounts, &[pda_signer_seeds])?;
    }

    if cpi_cursor != rest.len() {
        return Err(ProgramError::InvalidInstructionData);
    }

    // ---- 7. Post-snapshot balances at transfer-check destinations -------------------------
    let mut post_balances = [0u64; MAX_TRANSFER_NB as usize];
    for i in 0..transfer_nb as usize {
        let dest = &accounts[transfer_section_start + i];
        post_balances[i] = read_transfer_check_balance(dest)?;
    }

    // ---- 8. Outcome checks: (a) source wallet empty, (b) transfer min deltas ------------
    if from_is_spl {
        let pa = pda_ata.unwrap();
        if read_spl_balance(pa)? != 0 {
            msg!("pda_ata token amount must be 0 after CPIs");
            return Err(IntentSolverError::FromAtaNotEmpty.into());
        }
    } else {
        // assumption: wallet pda isn't allocated any data
        let rent_min = Rent::get()?.minimum_balance(0);
        if pda.lamports() != rent_min {
            msg!(
                "SOL PDA must hold only rent-exempt lamports for 0 data: have={}, need={}",
                pda.lamports(),
                rent_min
            );
            return Err(IntentSolverError::SolSourcePdaNotDrained.into());
        }
    }

    for i in 0..transfer_nb as usize {
        let delta = post_balances[i].saturating_sub(pre_balances[i]);
        if delta < transfer_min_amounts[i] {
            msg!(
                "check[{}] failed: delta={} min={}",
                i,
                delta,
                transfer_min_amounts[i]
            );
            return Err(IntentSolverError::MinAmountNotMet.into());
        }
        msg!("check[{}] ok: delta={}", i, delta);
    }

    // ---- 9. Cleanup (close SPL ATA or sweep SOL PDA) ------------------------------------
    if from_is_spl {
        let tp = token_program_from.unwrap();
        let pa = pda_ata.unwrap();
        close_pda_ata(pda, pa, executor, tp, pda_signer_seeds)?;
    } else {
        close_pda_sol(pda, executor, from_program, pda_signer_seeds)?;
    }

    Ok(())
}
