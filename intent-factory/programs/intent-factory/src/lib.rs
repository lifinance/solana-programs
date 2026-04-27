//! LI.FI Intent Factory — stateless intent-binding executor for Solana.
//!
//! A generic CPI executor that binds an intent (header + calls + accounts)
//! to a PDA via SHA-256 digest. Supports `execute_intent` (deadline-gated,
//! with min-out enforcement and source-drain invariant) and `refund_intent`
//! (post-deadline, scoped to canonical source ATA).
//!
//! See `solana-intent-factory-poc.md` for the full design.

use anchor_lang::prelude::*;

pub mod errors;
pub mod guards;
pub mod hash;
pub mod ix;
pub mod state;
pub mod wire;

pub use errors::{IntentError, WireError};
pub use ix::*;
pub use state::IntentHeader;

declare_id!("Co8vRsjgnGS62NkoJS7nWpzFfNLzZsKViuxarnU4h8gc");

#[program]
pub mod intent_factory {
    use super::*;

    pub fn execute_intent<'info>(
        ctx: Context<'_, '_, '_, 'info, ExecuteIntent<'info>>,
        header_bytes: Vec<u8>,
        calls_bytes: Vec<u8>,
        bump: u8,
    ) -> Result<()> {
        ix::execute_intent::handle_execute_intent(ctx, header_bytes, calls_bytes, bump)
    }

    pub fn refund_intent<'info>(
        ctx: Context<'_, '_, '_, 'info, RefundIntent<'info>>,
        header_bytes: Vec<u8>,
        calls_bytes: Vec<u8>,
        bump: u8,
    ) -> Result<()> {
        ix::refund_intent::handle_refund_intent(ctx, header_bytes, calls_bytes, bump)
    }
}
