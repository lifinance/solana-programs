//! Shared compile-time bounds and flag bits for the intent-solver instructions.

/// Max transfer-check legs per intent (bounds the hash + account loops in `execute`/`refund`).
pub const MAX_TRANSFER_NB: u8 = 8;

/// Max inner CPIs per `execute` (bounds compute + ix data size).
pub const MAX_CPI_NB: u8 = 16;
/// Per CPI: first account is program id, rest are instruction accounts passed to `invoke_signed`.
pub const MAX_ACC_PER_CPI: u8 = 128;
pub const MAX_OVERRIDE_PER_CPI: u8 = 32; // only signer wallet pda should be concerned

/// Inner `AccountMeta` flags from override `flags` byte: bit0 = writable, bit1 = signer.
pub const FLAG_INNER_WRITABLE: u8 = 1;
pub const FLAG_INNER_SIGNER: u8 = 2;
