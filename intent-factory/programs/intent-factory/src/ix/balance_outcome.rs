use anchor_lang::prelude::*;

use crate::errors::IntentError;
use crate::guards;

pub(crate) enum BalanceKind {
    NativeLamports,
    SplToken,
}

pub(crate) struct BalanceOutcome<'a, 'info> {
    account: &'a AccountInfo<'info>,
    kind: BalanceKind,
    min_delta: u64,
    pre: u64,
}

impl<'a, 'info> BalanceOutcome<'a, 'info> {
    pub fn snapshot(
        account: &'a AccountInfo<'info>,
        kind: BalanceKind,
        min_delta: u64,
    ) -> Result<Self> {
        let pre = match &kind {
            BalanceKind::NativeLamports => account.lamports(),
            BalanceKind::SplToken => guards::read_spl_amount(account)?,
        };
        Ok(Self {
            account,
            kind,
            min_delta,
            pre,
        })
    }

    pub fn assert_delta(&self) -> Result<()> {
        let post = match &self.kind {
            BalanceKind::NativeLamports => self.account.lamports(),
            BalanceKind::SplToken => guards::read_spl_amount(self.account)?,
        };
        let delta = post
            .checked_sub(self.pre)
            .ok_or(IntentError::InsufficientOutcome)?;
        require!(delta >= self.min_delta, IntentError::InsufficientOutcome);
        Ok(())
    }
}
