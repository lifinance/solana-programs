use anchor_lang::prelude::*;

#[error_code]
pub enum IntentError {
    #[msg("Header bytes are malformed or non-canonical")]
    MalformedHeader,

    #[msg("v1 requires src_mint to be Some (SPL token input only)")]
    InvalidSrcMint,

    #[msg("remaining_accounts exceeds virtual index limit")]
    TooManyRemainingAccounts,

    #[msg("Recomputed PDA does not match intent_pda account")]
    BadIntentPda,

    #[msg("Intent deadline has passed")]
    IntentExpired,

    #[msg("Intent has not expired yet (refund requires now > deadline)")]
    IntentNotExpired,

    #[msg("Executor pubkey does not match header.executor")]
    BadExecutor,

    #[msg("Required executor is not a signer")]
    ExecutorNotSigner,

    #[msg("source_ata pubkey or identity check failed")]
    BadSourceAta,

    #[msg("source_ata data is malformed")]
    SourceAccountMalformed,

    #[msg("source_ata.amount != header.amount_in before call loop")]
    SourceAmountMismatch,

    #[msg("source_ata.amount != 0 after call loop")]
    SourceNotDrained,

    #[msg("Outcome account is missing, collides with a named slot, or has wrong mint")]
    BadOutcomeAccount,

    #[msg("Outcome balance delta is below declared minimum")]
    InsufficientOutcome,

    #[msg("Inner program is on the deny-list")]
    DisallowedProgram,

    #[msg("System Program instruction is on the deny-list")]
    DisallowedSystemOp,

    #[msg("is_signer flag set on non-intent_pda account")]
    InvalidSignerFlag,

    #[msg("user pubkey does not match header.user")]
    BadUser,

    #[msg("src_mint_account pubkey does not match header.src_mint")]
    BadSrcMint,

    #[msg("user_source_ata pubkey does not match ATA(user, src_mint)")]
    BadUserSourceAta,

    #[msg("Intent PDA has already been initialized")]
    IntentAlreadyInitialized,

    #[msg("Intent has already been executed")]
    IntentAlreadyExecuted,

    #[msg("Intent has already been refunded")]
    IntentAlreadyRefunded,

    #[msg("Route CPI requested writable access to intent_pda (virtual index 0)")]
    InvalidWritableIntentPda,

    #[msg("Lamport overflow during close")]
    LamportOverflow,
}

#[error_code]
pub enum WireError {
    #[msg("num_calls exceeds MAX_CALLS")]
    TooManyCalls,

    #[msg("acc_count exceeds MAX_ACCOUNTS_PER_CALL")]
    TooManyAccountsPerCall,

    #[msg("data_len exceeds MAX_DATA_LEN")]
    DataTooLarge,

    #[msg("Non-zero trailing bits in flags bitmap")]
    NonCanonicalBitmap,

    #[msg("Wire bytes truncated or have trailing garbage")]
    MalformedWireBytes,

    #[msg("program_ix is in the named prefix (must be >= NAMED_PREFIX)")]
    ProgramInNamedPrefix,

    #[msg("Account index out of virtual list bounds")]
    AccountIndexOutOfBounds,
}
