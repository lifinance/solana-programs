use anchor_lang::prelude::*;

use crate::wire::NAMED_PREFIX;

pub(crate) fn build_virtual_list<'a, 'info>(
    intent_pda: &'a AccountInfo<'info>,
    source_ata: &'a AccountInfo<'info>,
    remaining: &'a [AccountInfo<'info>],
) -> Vec<AccountInfo<'info>> {
    let mut list = Vec::with_capacity(NAMED_PREFIX + remaining.len());
    list.push(intent_pda.clone());
    list.push(source_ata.clone());
    for a in remaining {
        list.push(a.clone());
    }
    list
}
