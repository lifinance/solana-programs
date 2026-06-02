//! Canonical intent preimage → SHA-256 → PDA seeds (`ICV1`).

use sha2::{Digest, Sha256};
use solana_program::pubkey::Pubkey;

/// First seed for `find_program_address` / `invoke_signed` (vault PDAs).
pub const INTENT_PDA_SEED: &[u8] = b"intent";

/// Domain separator prepended to every preimage (version bump = new string).
/// isv1 = intent solver version 1
pub const PREIMAGE_DOMAIN: &[u8] = b"isv1";

pub fn hash_intent(
    funder: &Pubkey,
    mint: &Pubkey,
    amount_in: u64,
    salt: &[u8; 32],
    executor: &Pubkey,
    transfer_nb: u8,
    transfer_destinations: &[Pubkey],
    transfer_min_amounts: &[u64],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(PREIMAGE_DOMAIN);
    hasher.update(funder.as_ref());
    hasher.update(mint.as_ref());
    hasher.update(amount_in.to_le_bytes());
    hasher.update(salt.as_slice());
    hasher.update(executor.as_ref());
    hasher.update([transfer_nb]);
    let n = transfer_nb as usize;
    for i in 0..n {
        hasher.update(transfer_destinations[i].as_ref());
        hasher.update(transfer_min_amounts[i].to_le_bytes());
    }
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic_and_field_sensitive() {
        let funder = Pubkey::new_from_array([1u8; 32]);
        let mint = Pubkey::new_from_array([2u8; 32]);
        let executor = Pubkey::new_from_array([3u8; 32]);
        let dest = Pubkey::new_from_array([4u8; 32]);
        let salt = [9u8; 32];
        let other = Pubkey::new_from_array([0xAAu8; 32]);

        let base = hash_intent(&funder, &mint, 100, &salt, &executor, 1, &[dest], &[50]);

        // Deterministic for identical inputs.
        assert_eq!(
            base,
            hash_intent(&funder, &mint, 100, &salt, &executor, 1, &[dest], &[50])
        );

        // Every hashed field perturbs the digest.
        assert_ne!(
            base,
            hash_intent(&other, &mint, 100, &salt, &executor, 1, &[dest], &[50])
        );
        assert_ne!(
            base,
            hash_intent(&funder, &other, 100, &salt, &executor, 1, &[dest], &[50])
        );
        assert_ne!(
            base,
            hash_intent(&funder, &mint, 101, &salt, &executor, 1, &[dest], &[50])
        );
        let mut salt2 = salt;
        salt2[0] ^= 1;
        assert_ne!(
            base,
            hash_intent(&funder, &mint, 100, &salt2, &executor, 1, &[dest], &[50])
        );
        assert_ne!(
            base,
            hash_intent(&funder, &mint, 100, &salt, &other, 1, &[dest], &[50])
        );
        assert_ne!(
            base,
            hash_intent(&funder, &mint, 100, &salt, &executor, 1, &[other], &[50])
        );
        assert_ne!(
            base,
            hash_intent(&funder, &mint, 100, &salt, &executor, 1, &[dest], &[51])
        );
    }
}
