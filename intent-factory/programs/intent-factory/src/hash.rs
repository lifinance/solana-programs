use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::hash as sha256;

use crate::errors::IntentError;

/// Compute the intent hash from canonical header bytes, wire-encoded calls
/// bytes, and the tail accounts (remaining_accounts only — the four
/// named-prefix slots are excluded).
///
/// ```text
/// acc_buf      = u8(tail.len()) || concat(pubkey for pubkey in tail)
/// calls_digest = sha256(acc_buf || calls_bytes)
/// intent_hash  = sha256(header_bytes || calls_digest)
/// ```
pub fn compute_intent_hash(
    header_bytes: &[u8],
    calls_bytes: &[u8],
    tail: &[AccountInfo],
) -> Result<[u8; 32]> {
    require!(
        tail.len() <= 251,
        IntentError::TooManyRemainingAccounts
    );

    let mut acc_buf = Vec::with_capacity(1 + tail.len() * 32);
    acc_buf.push(tail.len() as u8);
    for a in tail {
        acc_buf.extend_from_slice(a.key.as_ref());
    }

    let mut digest_input = Vec::with_capacity(acc_buf.len() + calls_bytes.len());
    digest_input.extend_from_slice(&acc_buf);
    digest_input.extend_from_slice(calls_bytes);
    let calls_digest = sha256(&digest_input);

    let mut hash_input = Vec::with_capacity(header_bytes.len() + 32);
    hash_input.extend_from_slice(header_bytes);
    hash_input.extend_from_slice(calls_digest.as_ref());
    let intent_hash = sha256(&hash_input);

    Ok(intent_hash.to_bytes())
}

/// Pure-data version for use in tests and gen_vector (no AccountInfo needed).
pub fn compute_intent_hash_from_pubkeys(
    header_bytes: &[u8],
    calls_bytes: &[u8],
    tail_pubkeys: &[Pubkey],
) -> [u8; 32] {
    let mut acc_buf = Vec::with_capacity(1 + tail_pubkeys.len() * 32);
    acc_buf.push(tail_pubkeys.len() as u8);
    for pk in tail_pubkeys {
        acc_buf.extend_from_slice(pk.as_ref());
    }

    let mut digest_input = Vec::with_capacity(acc_buf.len() + calls_bytes.len());
    digest_input.extend_from_slice(&acc_buf);
    digest_input.extend_from_slice(calls_bytes);
    let calls_digest = sha256(&digest_input);

    let mut hash_input = Vec::with_capacity(header_bytes.len() + 32);
    hash_input.extend_from_slice(header_bytes);
    hash_input.extend_from_slice(calls_digest.as_ref());
    sha256(&hash_input).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let header = b"test_header";
        let calls = b"test_calls";
        let tail = vec![Pubkey::new_unique(), Pubkey::new_unique()];

        let h1 = compute_intent_hash_from_pubkeys(header, calls, &tail);
        let h2 = compute_intent_hash_from_pubkeys(header, calls, &tail);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_with_tail_order() {
        let header = b"test_header";
        let calls = b"test_calls";
        let pk_a = Pubkey::new_unique();
        let pk_b = Pubkey::new_unique();

        let h1 = compute_intent_hash_from_pubkeys(header, calls, &[pk_a, pk_b]);
        let h2 = compute_intent_hash_from_pubkeys(header, calls, &[pk_b, pk_a]);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_header() {
        let calls = b"test_calls";
        let tail = vec![Pubkey::new_unique()];

        let h1 = compute_intent_hash_from_pubkeys(b"header_a", calls, &tail);
        let h2 = compute_intent_hash_from_pubkeys(b"header_b", calls, &tail);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_changes_with_calls() {
        let header = b"test_header";
        let tail = vec![Pubkey::new_unique()];

        let h1 = compute_intent_hash_from_pubkeys(header, b"calls_a", &tail);
        let h2 = compute_intent_hash_from_pubkeys(header, b"calls_b", &tail);
        assert_ne!(h1, h2);
    }

    #[test]
    fn empty_tail_is_valid() {
        let h = compute_intent_hash_from_pubkeys(b"header", b"calls", &[]);
        assert_ne!(h, [0u8; 32]);
    }
}
