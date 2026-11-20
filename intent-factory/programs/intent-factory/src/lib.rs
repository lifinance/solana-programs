//! LI.FI Intent Factory — stateful intent-binding executor for Solana.
//!
//! A generic CPI executor that binds an intent to a PDA via SHA-256 digest.
//! The lifecycle is: `init_intent` stores the canonical header once,
//! `execute_intent` reads stored state and runs route CPIs with outcome
//! enforcement (min-out, fee deltas, source drain), and `refund_intent`
//! returns remaining funds after deadline.
//!
//! See `solana-intent-factory-poc.md` for the full design.

use anchor_lang::prelude::*;

pub mod errors;
pub mod guards;
pub mod hash;
pub mod ix;
pub mod pda;
pub mod state;
pub mod wire;

pub use errors::{IntentError, WireError};
pub use ix::*;
pub use state::{IntentHeader, IntentOutcome, IntentState, IntentStatus};

declare_id!("CNHojkSgGgdQvVGRXY667mpRP6Ne2dZzBUwE4sTWMRZU");

#[program]
pub mod intent_factory {
    use super::*;

    pub fn init_intent(ctx: Context<InitIntent>, header_bytes: Vec<u8>, bump: u8) -> Result<()> {
        ix::init_intent::handle_init_intent(ctx, header_bytes, bump)
    }

    pub fn execute_intent<'info>(
        ctx: Context<'_, '_, '_, 'info, ExecuteIntent<'info>>,
        calls_bytes: Vec<u8>,
    ) -> Result<()> {
        ix::execute_intent::handle_execute_intent(ctx, calls_bytes)
    }

    pub fn refund_intent<'info>(
        ctx: Context<'_, '_, '_, 'info, RefundIntent<'info>>,
    ) -> Result<()> {
        ix::refund_intent::handle_refund_intent(ctx)
    }
}
