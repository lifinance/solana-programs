use solana_program::program_error::ProgramError;

#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentSolverError {
    InvalidPda = 0,
    MinAmountNotMet = 1,
    WrongTokenProgram = 2,
    InvalidTransferNb = 3,
    InsufficientData = 4,
    NotEnoughAccounts = 5,
    FromAtaNotEmpty = 6,
    InvalidCpiCount = 8,
    InvalidCpiAccountSpec = 9,
    CpiToSelfNotAllowed = 10,
    CpiProgramDenied = 11,
    InnerSignerNotAllowed = 12,
    WritableEscalationNotAllowed = 13,
    /// SOL source PDA: after CPIs, lamports must equal rent-exempt minimum for 0-byte data (only rent left).
    SolSourcePdaNotDrained = 14,
    /// SPL `pda_ata` not owned by `from_program`, unpack failed, `ta.owner != pda`,
    /// or `pda_ata` is not the canonical ATA for `(pda, ta.mint, from_program)`.
    InvalidSourceAta = 15,
    /// Refund: passed `funder_ata` is not the canonical ATA for `(funder, mint, from_program)`,
    /// or the passed `mint` does not match the source ATA's mint.
    InvalidDestinationAta = 16,
}

impl From<IntentSolverError> for ProgramError {
    fn from(e: IntentSolverError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
