use solana_program::{
    account_info::AccountInfo,
    bpf_loader, bpf_loader_upgradeable,
    entrypoint::ProgramResult,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction, system_program,
};
use crate::errors::IntentSolverError;
#[allow(deprecated)]
use spl_associated_token_account::get_associated_token_address_with_program_id;
use spl_associated_token_account::instruction::create_associated_token_account_idempotent;
use spl_token::ID as SPL_TOKEN_ID;
#[allow(deprecated)]
use spl_token_2022::instruction::transfer as spl_transfer;
use spl_token_2022::{instruction::close_account as close_token_account, state::Account as TokenAccount};
use spl_token_2022::ID as SPL_TOKEN_2022_ID;

/// CPI callee denylist (zero tx bytes): legacy + upgradeable BPF loaders.
/// Add more `bpf_*::check_id` arms or `Pubkey` comparisons here if policy tightens.
#[inline]
pub fn is_cpi_program_denied(program_id: &Pubkey) -> bool {
    bpf_loader_upgradeable::check_id(program_id) || bpf_loader::check_id(program_id)
}

/// SPL: `pda_ata` must be owned by `from_program`, unpack to a `TokenAccount` whose
/// `owner == pda`, and be the canonical ATA for `(pda, ta.mint, from_program)`. The mint
/// pubkey is read back from `ta.mint` (no separate mint account is passed); the canonical-ATA
/// check is what cryptographically binds the mint identity used in the intent preimage.
pub fn verify_spl_source_ata(
    pda: &AccountInfo,
    from_program: &AccountInfo,
    pda_ata: &AccountInfo,
) -> Result<Pubkey, ProgramError> {
    if pda_ata.owner != from_program.key {
        return Err(IntentSolverError::InvalidSourceAta.into());
    }
    let data = pda_ata.try_borrow_data()?;
    let ta = TokenAccount::unpack(&data).map_err(|_| IntentSolverError::InvalidSourceAta)?;
    if ta.owner != *pda.key {
        return Err(IntentSolverError::InvalidSourceAta.into());
    }
    let expected =
        get_associated_token_address_with_program_id(pda.key, &ta.mint, from_program.key);
    if pda_ata.key != &expected {
        return Err(IntentSolverError::InvalidSourceAta.into());
    }
    Ok(ta.mint)
}

/// Reads a little-endian `u64` at `cursor` and advances it by 8.
pub fn read_u64_le(rest: &[u8], cursor: &mut usize) -> Result<u64, ProgramError> {
    let end = *cursor + 8;
    let slice = rest
        .get(*cursor..end)
        .ok_or(IntentSolverError::InsufficientData)?;
    *cursor = end;
    Ok(u64::from_le_bytes(
        slice.try_into().map_err(|_| IntentSolverError::InsufficientData)?,
    ))
}

/// Reads a little-endian `u16` at `cursor` and advances it by 2.
pub fn read_u16_le(rest: &[u8], cursor: &mut usize) -> Result<u16, ProgramError> {
    let end = *cursor + 2;
    let slice = rest
        .get(*cursor..end)
        .ok_or(IntentSolverError::InsufficientData)?;
    *cursor = end;
    Ok(u16::from_le_bytes(
        slice.try_into().map_err(|_| IntentSolverError::InsufficientData)?,
    ))
}

/// Reads a 32-byte `Pubkey` at `cursor` and advances it by 32.
pub fn read_pubkey(rest: &[u8], cursor: &mut usize) -> Result<Pubkey, ProgramError> {
    let end = *cursor + 32;
    let slice = rest
        .get(*cursor..end)
        .ok_or(IntentSolverError::InsufficientData)?;
    *cursor = end;
    let bytes: [u8; 32] = slice
        .try_into()
        .map_err(|_| IntentSolverError::InsufficientData)?;
    Ok(Pubkey::new_from_array(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_u64_le_reads_and_advances() {
        let bytes = 0x0102_0304_0506_0708u64.to_le_bytes();
        let mut cursor = 0usize;
        assert_eq!(read_u64_le(&bytes, &mut cursor).unwrap(), 0x0102_0304_0506_0708);
        assert_eq!(cursor, 8);
    }

    #[test]
    fn read_u64_le_errors_when_short() {
        let bytes = [0u8; 4];
        let mut cursor = 0usize;
        assert!(read_u64_le(&bytes, &mut cursor).is_err());
    }

    #[test]
    fn read_u16_le_reads_and_advances() {
        let mut cursor = 0usize;
        assert_eq!(read_u16_le(&[0x34, 0x12], &mut cursor).unwrap(), 0x1234);
        assert_eq!(cursor, 2);
    }

    #[test]
    fn read_pubkey_reads_32_and_advances() {
        let mut raw = [0u8; 32];
        for (i, b) in raw.iter_mut().enumerate() {
            *b = i as u8;
        }
        let mut cursor = 0usize;
        assert_eq!(read_pubkey(&raw, &mut cursor).unwrap().to_bytes(), raw);
        assert_eq!(cursor, 32);
    }

    #[test]
    fn read_pubkey_errors_when_short() {
        let mut cursor = 0usize;
        assert!(read_pubkey(&[0u8; 31], &mut cursor).is_err());
    }

    #[test]
    fn readers_share_one_cursor() {
        let mut buf = Vec::new();
        buf.extend_from_slice(&7u64.to_le_bytes());
        buf.extend_from_slice(&[5u8; 32]);
        let mut cursor = 0usize;
        assert_eq!(read_u64_le(&buf, &mut cursor).unwrap(), 7);
        assert_eq!(read_pubkey(&buf, &mut cursor).unwrap().to_bytes(), [5u8; 32]);
        assert_eq!(cursor, buf.len());
    }
}

/// Reads SPL token balance from an ATA-shaped account.
/// Returns 0 if the account is uninitialized (e.g., ATA not yet created).
pub fn read_spl_balance(account: &AccountInfo) -> Result<u64, ProgramError> {
    if account.data_is_empty() || account.owner == &system_program::ID {
        return Ok(0);
    }
    let data = account.try_borrow_data()?;
    Ok(TokenAccount::unpack(&data)?.amount)
}

/// Reads the transfer-check balance for a destination account:
/// - system-owned account => lamports
/// - SPL token account (Token or Token-2022 owner) => token amount
/// - anything else (e.g. ATA not created yet) => `0`
pub fn read_transfer_check_balance(account: &AccountInfo) -> Result<u64, ProgramError> {
    if account.owner == &system_program::ID {
        return Ok(account.lamports());
    }
    if account.owner == &SPL_TOKEN_ID || account.owner == &SPL_TOKEN_2022_ID {
        return read_spl_balance(account);
    }
    Ok(0)
}

/// Sweeps all lamports from a system-owned PDA to `recipient` via System Program.
/// Used to close the PDA wallet in the SOL fromToken case.
pub fn close_pda_sol<'a>(
    pda: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    system_program_account: &AccountInfo<'a>,
    pda_signer_seeds: &[&[u8]],
) -> ProgramResult {
    let amount = pda.lamports();
    invoke_signed(
        &system_instruction::transfer(pda.key, recipient.key, amount),
        &[
            pda.clone(),
            recipient.clone(),
            system_program_account.clone(),
        ],
        &[pda_signer_seeds],
    )
}

/// Idempotently creates `ata` as the canonical ATA for `(wallet, mint, token_program)`.
/// `payer` funds the new account's rent; a no-op (no error) if the ATA already exists.
/// Used by `refund` to guarantee the funder's destination ATA exists before the SPL transfer.
pub fn create_ata_idempotent<'a>(
    payer: &AccountInfo<'a>,
    ata: &AccountInfo<'a>,
    wallet: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    system_program_account: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    ata_program: &AccountInfo<'a>,
) -> ProgramResult {
    invoke(
        &create_associated_token_account_idempotent(
            payer.key,
            wallet.key,
            mint.key,
            token_program.key,
        ),
        &[
            payer.clone(),
            ata.clone(),
            wallet.clone(),
            mint.clone(),
            system_program_account.clone(),
            token_program.clone(),
            ata_program.clone(),
        ],
    )
}

/// Transfers the entire token balance of `pda_ata` to `dest_ata`, signed by the vault PDA.
/// No-op when the balance is already zero. Plain (non-checked) SPL `transfer` is used so no
/// mint/decimals are needed; the source ATA is closed separately after this drains it.
pub fn transfer_pda_ata_all<'a>(
    pda: &AccountInfo<'a>,
    pda_ata: &AccountInfo<'a>,
    dest_ata: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    pda_signer_seeds: &[&[u8]],
) -> ProgramResult {
    let amount = read_spl_balance(pda_ata)?;
    if amount == 0 {
        return Ok(());
    }
    #[allow(deprecated)]
    let ix = spl_transfer(
        token_program.key,
        pda_ata.key,
        dest_ata.key,
        pda.key,
        &[],
        amount,
    )?;
    invoke_signed(
        &ix,
        &[
            pda_ata.clone(),
            dest_ata.clone(),
            pda.clone(),
            token_program.clone(),
        ],
        &[pda_signer_seeds],
    )
}

/// Closes the PDA's ATA, sending its lamports (rent + any unwrapped wSOL) to `recipient`.
/// Used in the SPL fromToken case.
pub fn close_pda_ata<'a>(
    pda: &AccountInfo<'a>,
    pda_ata: &AccountInfo<'a>,
    recipient: &AccountInfo<'a>,
    token_program: &AccountInfo<'a>,
    pda_signer_seeds: &[&[u8]],
) -> ProgramResult {
    invoke_signed(
        &close_token_account(token_program.key, pda_ata.key, recipient.key, pda.key, &[])?,
        &[
            pda_ata.clone(),
            recipient.clone(),
            pda.clone(),
            token_program.clone(),
        ],
        &[pda_signer_seeds],
    )
}
