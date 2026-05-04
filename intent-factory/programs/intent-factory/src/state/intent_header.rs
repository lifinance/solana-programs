use anchor_lang::prelude::*;

use crate::errors::IntentError;

pub const MAX_OUTCOMES: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct IntentOutcome {
    pub mint: Option<Pubkey>,
    pub account: Pubkey,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntentHeader {
    pub user: Pubkey,
    pub src_mint: Option<Pubkey>,
    pub amount_in: u64,
    pub outcomes: Vec<IntentOutcome>,
    pub deadline: i64,
    pub salt: [u8; 32],
    pub executor: Pubkey,
}

impl IntentHeader {
    /// Canonical byte encoding used as hash input. Outcomes are encoded in
    /// declared order, matching Catapultar's order-sensitive `Outcome[]`.
    pub fn canonical_encode(&self) -> Vec<u8> {
        let outcome_bytes: usize = self.outcomes.iter().map(|o| outcome_row_len(o)).sum();
        let capacity = 32 + 33 + 8 + 1 + outcome_bytes + 8 + 32 + 32;
        let mut buf = Vec::with_capacity(capacity);

        buf.extend_from_slice(self.user.as_ref());
        encode_option_pubkey(&mut buf, &self.src_mint);
        buf.extend_from_slice(&self.amount_in.to_le_bytes());

        buf.push(self.outcomes.len() as u8);
        for outcome in &self.outcomes {
            encode_option_pubkey(&mut buf, &outcome.mint);
            buf.extend_from_slice(outcome.account.as_ref());
            buf.extend_from_slice(&outcome.amount.to_le_bytes());
        }

        buf.extend_from_slice(&self.deadline.to_le_bytes());
        buf.extend_from_slice(&self.salt);
        buf.extend_from_slice(self.executor.as_ref());

        buf
    }

    /// Strict decoder. Rejects non-canonical forms:
    ///   - option tags outside {0, 1}
    ///   - outcome_count > MAX_OUTCOMES
    ///   - cursor != bytes.len() at end
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut cursor = 0usize;

        let user = read_pubkey(bytes, &mut cursor)?;
        let src_mint = read_option_pubkey(bytes, &mut cursor)?;
        let amount_in = read_u64(bytes, &mut cursor)?;

        let outcome_count = read_u8(bytes, &mut cursor)? as usize;
        if outcome_count > MAX_OUTCOMES {
            return Err(IntentError::MalformedHeader.into());
        }

        let mut outcomes = Vec::with_capacity(outcome_count);
        for _ in 0..outcome_count {
            let mint = read_option_pubkey(bytes, &mut cursor)?;
            let account = read_pubkey(bytes, &mut cursor)?;
            let amount = read_u64(bytes, &mut cursor)?;
            outcomes.push(IntentOutcome {
                mint,
                account,
                amount,
            });
        }

        let deadline = read_i64(bytes, &mut cursor)?;
        let salt = read_bytes32(bytes, &mut cursor)?;
        let executor = read_pubkey(bytes, &mut cursor)?;

        if cursor != bytes.len() {
            return Err(IntentError::MalformedHeader.into());
        }

        Ok(Self {
            user,
            src_mint,
            amount_in,
            outcomes,
            deadline,
            salt,
            executor,
        })
    }
}

fn outcome_row_len(o: &IntentOutcome) -> usize {
    let mint_len = if o.mint.is_some() { 33 } else { 1 };
    mint_len + 32 + 8
}

fn encode_option_pubkey(buf: &mut Vec<u8>, opt: &Option<Pubkey>) {
    match opt {
        Some(pk) => {
            buf.push(1);
            buf.extend_from_slice(pk.as_ref());
        }
        None => {
            buf.push(0);
        }
    }
}

fn read_u8(bytes: &[u8], cursor: &mut usize) -> Result<u8> {
    if *cursor >= bytes.len() {
        return Err(IntentError::MalformedHeader.into());
    }
    let val = bytes[*cursor];
    *cursor += 1;
    Ok(val)
}

fn read_u64(bytes: &[u8], cursor: &mut usize) -> Result<u64> {
    let end = cursor.checked_add(8).ok_or(IntentError::MalformedHeader)?;
    if end > bytes.len() {
        return Err(IntentError::MalformedHeader.into());
    }
    let val = u64::from_le_bytes(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(val)
}

fn read_i64(bytes: &[u8], cursor: &mut usize) -> Result<i64> {
    let end = cursor.checked_add(8).ok_or(IntentError::MalformedHeader)?;
    if end > bytes.len() {
        return Err(IntentError::MalformedHeader.into());
    }
    let val = i64::from_le_bytes(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(val)
}

fn read_pubkey(bytes: &[u8], cursor: &mut usize) -> Result<Pubkey> {
    let end = cursor.checked_add(32).ok_or(IntentError::MalformedHeader)?;
    if end > bytes.len() {
        return Err(IntentError::MalformedHeader.into());
    }
    let pk = Pubkey::new_from_array(bytes[*cursor..end].try_into().unwrap());
    *cursor = end;
    Ok(pk)
}

fn read_bytes32(bytes: &[u8], cursor: &mut usize) -> Result<[u8; 32]> {
    let end = cursor.checked_add(32).ok_or(IntentError::MalformedHeader)?;
    if end > bytes.len() {
        return Err(IntentError::MalformedHeader.into());
    }
    let arr: [u8; 32] = bytes[*cursor..end].try_into().unwrap();
    *cursor = end;
    Ok(arr)
}

fn read_option_pubkey(bytes: &[u8], cursor: &mut usize) -> Result<Option<Pubkey>> {
    let tag = read_u8(bytes, cursor)?;
    match tag {
        0 => Ok(None),
        1 => Ok(Some(read_pubkey(bytes, cursor)?)),
        _ => Err(IntentError::MalformedHeader.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_header_spl_outcomes() -> IntentHeader {
        IntentHeader {
            user: Pubkey::new_unique(),
            src_mint: Some(Pubkey::new_unique()),
            amount_in: 1_000_000,
            outcomes: vec![
                IntentOutcome {
                    mint: Some(Pubkey::new_from_array([3u8; 32])),
                    account: Pubkey::new_from_array([4u8; 32]),
                    amount: 950_000,
                },
                IntentOutcome {
                    mint: Some(Pubkey::new_from_array([5u8; 32])),
                    account: Pubkey::new_from_array([6u8; 32]),
                    amount: 10_000,
                },
            ],
            deadline: 1_700_000_000,
            salt: [42u8; 32],
            executor: Pubkey::new_unique(),
        }
    }

    fn test_header_native_outcome() -> IntentHeader {
        IntentHeader {
            user: Pubkey::new_unique(),
            src_mint: Some(Pubkey::new_unique()),
            amount_in: 1_000_000,
            outcomes: vec![IntentOutcome {
                mint: None,
                account: Pubkey::new_from_array([4u8; 32]),
                amount: 950_000,
            }],
            deadline: 1_700_000_000,
            salt: [42u8; 32],
            executor: Pubkey::new_unique(),
        }
    }

    #[test]
    fn encode_decode_roundtrip_spl() {
        let header = test_header_spl_outcomes();
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.canonical_encode(), bytes);
        assert_eq!(decoded.user, header.user);
        assert_eq!(decoded.src_mint, header.src_mint);
        assert_eq!(decoded.amount_in, header.amount_in);
        assert_eq!(decoded.outcomes.len(), 2);
        assert_eq!(decoded.outcomes[0].mint, header.outcomes[0].mint);
        assert_eq!(decoded.outcomes[0].account, header.outcomes[0].account);
        assert_eq!(decoded.outcomes[0].amount, header.outcomes[0].amount);
        assert_eq!(decoded.deadline, header.deadline);
        assert_eq!(decoded.salt, header.salt);
        assert_eq!(decoded.executor, header.executor);
    }

    #[test]
    fn encode_decode_roundtrip_native() {
        let header = test_header_native_outcome();
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.canonical_encode(), bytes);
        assert_eq!(decoded.outcomes.len(), 1);
        assert_eq!(decoded.outcomes[0].mint, None);
        assert_eq!(decoded.outcomes[0].account, header.outcomes[0].account);
        assert_eq!(decoded.outcomes[0].amount, header.outcomes[0].amount);
    }

    #[test]
    fn native_outcome_is_shorter_than_spl() {
        let native = test_header_native_outcome();
        let spl = test_header_spl_outcomes();
        let native_bytes = native.canonical_encode();
        let spl_bytes = spl.canonical_encode();
        assert!(native_bytes.len() < spl_bytes.len());
    }

    #[test]
    fn decode_rejects_bad_option_tag() {
        let header = test_header_spl_outcomes();
        let mut bytes = header.canonical_encode();
        bytes[32] = 2; // corrupt src_mint tag
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn decode_rejects_outcome_count_over_max() {
        let header = test_header_spl_outcomes();
        let mut bytes = header.canonical_encode();
        // user(32) + src_mint(33) + amount_in(8) = 73
        bytes[73] = 5;
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn decode_rejects_trailing_bytes() {
        let header = test_header_spl_outcomes();
        let mut bytes = header.canonical_encode();
        bytes.push(0);
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn decode_rejects_truncated_bytes() {
        let header = test_header_spl_outcomes();
        let bytes = header.canonical_encode();
        assert!(IntentHeader::decode(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn none_options_roundtrip() {
        let mut header = test_header_native_outcome();
        header.src_mint = None;
        header.outcomes = vec![];
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.src_mint, None);
        assert!(decoded.outcomes.is_empty());
    }

    #[test]
    fn executor_roundtrip() {
        let header = test_header_spl_outcomes();
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.executor, header.executor);
    }

    #[test]
    fn outcomes_preserve_declared_order() {
        let header = test_header_spl_outcomes();
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        for (i, outcome) in decoded.outcomes.iter().enumerate() {
            assert_eq!(outcome.mint, header.outcomes[i].mint);
            assert_eq!(outcome.account, header.outcomes[i].account);
            assert_eq!(outcome.amount, header.outcomes[i].amount);
        }
    }

    #[test]
    fn decode_rejects_bad_outcome_mint_tag() {
        let header = test_header_spl_outcomes();
        let mut bytes = header.canonical_encode();
        // outcome_count is at offset 73, first outcome mint tag is at offset 74
        bytes[74] = 2;
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn zero_amount_outcome_allowed() {
        let mut header = test_header_native_outcome();
        header.outcomes[0].amount = 0;
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.outcomes[0].amount, 0);
    }

    #[test]
    fn max_outcomes_roundtrip() {
        let mut header = test_header_spl_outcomes();
        header.outcomes = (0..MAX_OUTCOMES)
            .map(|i| IntentOutcome {
                mint: Some(Pubkey::new_from_array([i as u8 + 10; 32])),
                account: Pubkey::new_from_array([i as u8 + 20; 32]),
                amount: (i as u64 + 1) * 1000,
            })
            .collect();
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.outcomes.len(), MAX_OUTCOMES);
    }
}
