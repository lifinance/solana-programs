use anchor_lang::prelude::*;

#[error_code]
pub enum IntentError {
    #[msg("Header bytes are malformed or non-canonical")]
    MalformedHeader,

    #[msg("v1 requires src_mint to be Some (SPL token input only)")]
    InvalidSrcMint,

    #[msg("remaining_accounts exceeds 251 (u8 index overflow with 4 virtual prefix slots)")]
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

    #[msg("receiver pubkey does not match header.receiver")]
    BadReceiver,

    #[msg("receiver_token pubkey or identity check failed")]
    BadReceiverToken,

    #[msg("source_ata.amount != header.amount_in before call loop")]
    SourceAmountMismatch,

    #[msg("source_ata.amount != 0 after call loop")]
    SourceNotDrained,

    #[msg("Receiver delta is below min_amount_out")]
    InsufficientOutput,

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

    #[msg("receiver_token must be SystemProgram when out_mint is None")]
    BadReceiverTokenNativeOutput,

    #[msg("CallSpec references virtual index 3 (receiver_token) in native-output mode")]
    ReceiverTokenRefInNativeOutput,
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
