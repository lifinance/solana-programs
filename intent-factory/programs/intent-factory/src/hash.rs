use anchor_lang::solana_program::hash::hash as sha256;

/// Compute the intent hash from canonical header bytes only.
///
/// The PDA is intent-bound (not route-bound), matching the EVM/Catapultar
/// outcome model. Route calls and remaining accounts are late-bound
/// execution inputs validated by runtime outcome checks.
///
/// ```text
/// intent_hash = sha256(header_bytes)
/// ```
pub fn compute_intent_hash(header_bytes: &[u8]) -> [u8; 32] {
    sha256(header_bytes).to_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_deterministic() {
        let h1 = compute_intent_hash(b"test_header");
        let h2 = compute_intent_hash(b"test_header");
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_changes_with_header() {
        let h1 = compute_intent_hash(b"header_a");
        let h2 = compute_intent_hash(b"header_b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_independent_of_route() {
        let h1 = compute_intent_hash(b"same_header");
        let h2 = compute_intent_hash(b"same_header");
        assert_eq!(h1, h2);
    }

    #[test]
    fn non_zero() {
        let h = compute_intent_hash(b"header");
        assert_ne!(h, [0u8; 32]);
    }
}
