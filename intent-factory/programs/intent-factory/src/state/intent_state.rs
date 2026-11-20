use anchor_lang::prelude::*;

use super::intent_header::{IntentOutcome, MAX_OUTCOMES};

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum IntentStatus {
    Initialized = 0,
    Executed = 1,
    Refunded = 2,
}

#[account]
pub struct IntentState {
    pub intent_hash: [u8; 32],
    pub status: IntentStatus,
    pub bump: u8,

    pub user: Pubkey,
    pub src_mint: Option<Pubkey>,
    pub amount_in: u64,

    pub outcome_count: u8,
    pub outcome_mint_tags: [u8; MAX_OUTCOMES],
    pub outcome_mints: [Pubkey; MAX_OUTCOMES],
    pub outcome_accounts: [Pubkey; MAX_OUTCOMES],
    pub outcome_amounts: [u64; MAX_OUTCOMES],

    pub deadline: i64,
    pub salt: [u8; 32],
    pub executor: Pubkey,
}

impl IntentState {
    // 8   anchor discriminator
    // 32  intent_hash
    // 1   status
    // 1   bump
    // 32  user
    // 33  src_mint (1 tag + 32 pubkey)
    // 8   amount_in
    // 1   outcome_count
    // 4   outcome_mint_tags
    // 128 outcome_mints (32 * 4)
    // 128 outcome_accounts (32 * 4)
    // 32  outcome_amounts (8 * 4)
    // 8   deadline
    // 32  salt
    // 32  executor
    pub const MAX_SIZE: usize = 8
        + 32
        + 1
        + 1
        + 32
        + 1 + 32
        + 8
        + 1
        + MAX_OUTCOMES
        + (32 * MAX_OUTCOMES)
        + (32 * MAX_OUTCOMES)
        + (8 * MAX_OUTCOMES)
        + 8
        + 32
        + 32;

    pub fn outcomes(&self) -> Vec<IntentOutcome> {
        (0..self.outcome_count as usize)
            .map(|i| IntentOutcome {
                mint: if self.outcome_mint_tags[i] == 1 {
                    Some(self.outcome_mints[i])
                } else {
                    None
                },
                account: self.outcome_accounts[i],
                amount: self.outcome_amounts[i],
            })
            .collect()
    }
}
