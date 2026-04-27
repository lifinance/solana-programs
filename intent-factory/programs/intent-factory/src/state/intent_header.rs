use anchor_lang::prelude::*;

use crate::errors::IntentError;

pub const MAX_FEES: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct IntentHeader {
    pub user: Pubkey,
    pub src_mint: Option<Pubkey>,
    pub amount_in: u64,
    pub out_mint: Option<Pubkey>,
    pub receiver: Pubkey,
    pub min_amount_out: u64,
    pub fee_recipients: Vec<(Pubkey, u64)>,
    pub deadline: i64,
    pub salt: [u8; 32],
    pub executor: Option<Pubkey>,
}

impl IntentHeader {
    /// Canonical byte encoding used as hash input. Fee recipients are sorted
    /// lexicographically by (pubkey bytes, amount LE bytes) before encoding.
    pub fn canonical_encode(&self) -> Vec<u8> {
        let mut sorted_fees = self.fee_recipients.clone();
        sorted_fees.sort_by(|a, b| {
            a.0.as_ref().cmp(b.0.as_ref()).then_with(|| a.1.cmp(&b.1))
        });

        let fee_count = sorted_fees.len();
        let capacity = 32 + 33 + 8 + 33 + 32 + 8 + 1 + fee_count * 40 + 8 + 32 + 33;
        let mut buf = Vec::with_capacity(capacity);

        buf.extend_from_slice(self.user.as_ref());
        encode_option_pubkey(&mut buf, &self.src_mint);
        buf.extend_from_slice(&self.amount_in.to_le_bytes());
        encode_option_pubkey(&mut buf, &self.out_mint);
        buf.extend_from_slice(self.receiver.as_ref());
        buf.extend_from_slice(&self.min_amount_out.to_le_bytes());
        buf.push(fee_count as u8);
        for (pk, amt) in &sorted_fees {
            buf.extend_from_slice(pk.as_ref());
            buf.extend_from_slice(&amt.to_le_bytes());
        }
        buf.extend_from_slice(&self.deadline.to_le_bytes());
        buf.extend_from_slice(&self.salt);
        encode_option_pubkey(&mut buf, &self.executor);

        buf
    }

    /// Strict decoder. Rejects non-canonical forms:
    ///   - option tags outside {0, 1}
    ///   - fee_count > MAX_FEES
    ///   - fee_recipients not strictly sorted (or duplicate pubkeys)
    ///   - cursor != bytes.len() at end
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let mut cursor = 0usize;

        let user = read_pubkey(bytes, &mut cursor)?;
        let src_mint = read_option_pubkey(bytes, &mut cursor)?;
        let amount_in = read_u64(bytes, &mut cursor)?;
        let out_mint = read_option_pubkey(bytes, &mut cursor)?;
        let receiver = read_pubkey(bytes, &mut cursor)?;
        let min_amount_out = read_u64(bytes, &mut cursor)?;

        let fee_count = read_u8(bytes, &mut cursor)? as usize;
        if fee_count > MAX_FEES {
            return Err(IntentError::MalformedHeader.into());
        }

        let mut fee_recipients = Vec::with_capacity(fee_count);
        for _ in 0..fee_count {
            let pk = read_pubkey(bytes, &mut cursor)?;
            let amt = read_u64(bytes, &mut cursor)?;
            fee_recipients.push((pk, amt));
        }

        validate_fee_sort(&fee_recipients)?;

        let deadline = read_i64(bytes, &mut cursor)?;
        let salt = read_bytes32(bytes, &mut cursor)?;
        let executor = read_option_pubkey(bytes, &mut cursor)?;

        if cursor != bytes.len() {
            return Err(IntentError::MalformedHeader.into());
        }

        Ok(Self {
            user,
            src_mint,
            amount_in,
            out_mint,
            receiver,
            min_amount_out,
            fee_recipients,
            deadline,
            salt,
            executor,
        })
    }
}

fn validate_fee_sort(fees: &[(Pubkey, u64)]) -> Result<()> {
    for w in fees.windows(2) {
        let (pk_a, _) = &w[0];
        let (pk_b, _) = &w[1];
        // Duplicate pubkeys are rejected regardless of amount ordering.
        if pk_a == pk_b {
            return Err(IntentError::MalformedHeader.into());
        }
        // Pubkeys must be strictly ascending in lex order.
        if pk_a.as_ref() >= pk_b.as_ref() {
            return Err(IntentError::MalformedHeader.into());
        }
    }
    Ok(())
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

    fn test_header() -> IntentHeader {
        IntentHeader {
            user: Pubkey::new_unique(),
            src_mint: Some(Pubkey::new_unique()),
            amount_in: 1_000_000,
            out_mint: Some(Pubkey::new_unique()),
            receiver: Pubkey::new_unique(),
            min_amount_out: 950_000,
            fee_recipients: vec![
                (Pubkey::new_from_array([1u8; 32]), 10_000),
                (Pubkey::new_from_array([2u8; 32]), 5_000),
            ],
            deadline: 1_700_000_000,
            salt: [42u8; 32],
            executor: None,
        }
    }

    #[test]
    fn encode_decode_roundtrip() {
        let header = test_header();
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.canonical_encode(), bytes);

        assert_eq!(decoded.user, header.user);
        assert_eq!(decoded.src_mint, header.src_mint);
        assert_eq!(decoded.amount_in, header.amount_in);
        assert_eq!(decoded.out_mint, header.out_mint);
        assert_eq!(decoded.receiver, header.receiver);
        assert_eq!(decoded.min_amount_out, header.min_amount_out);
        assert_eq!(decoded.deadline, header.deadline);
        assert_eq!(decoded.salt, header.salt);
        assert_eq!(decoded.executor, header.executor);
    }

    #[test]
    fn canonical_encode_sorts_fees() {
        let mut header = test_header();
        let fee_a = (Pubkey::new_from_array([1u8; 32]), 100);
        let fee_b = (Pubkey::new_from_array([2u8; 32]), 200);
        header.fee_recipients = vec![fee_b, fee_a];
        let bytes_reversed = header.canonical_encode();

        header.fee_recipients = vec![fee_a, fee_b];
        let bytes_sorted = header.canonical_encode();

        assert_eq!(bytes_reversed, bytes_sorted);
    }

    #[test]
    fn decode_rejects_bad_option_tag() {
        let header = test_header();
        let mut bytes = header.canonical_encode();
        bytes[32] = 2; // corrupt src_mint tag
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn decode_rejects_fee_count_over_max() {
        let header = test_header();
        let mut bytes = header.canonical_encode();
        // Find fee_count position: user(32) + src_mint(33) + amount_in(8) +
        // out_mint(33) + receiver(32) + min_amount_out(8) = 146
        bytes[146] = 5;
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn decode_rejects_unsorted_fees() {
        let fee_a = (Pubkey::new_from_array([1u8; 32]), 100);
        let fee_b = (Pubkey::new_from_array([2u8; 32]), 200);
        let mut header = test_header();
        header.fee_recipients = vec![fee_b, fee_a];

        // Encode with manual (wrong) sort — hack the bytes directly
        let sorted_bytes = header.canonical_encode();
        // Swap the two 40-byte fee entries in the encoded bytes
        let fee_start = 147; // 146 (fee_count byte) + 1
        let mut unsorted = sorted_bytes.clone();
        let entry_a = unsorted[fee_start..fee_start + 40].to_vec();
        let entry_b = unsorted[fee_start + 40..fee_start + 80].to_vec();
        unsorted[fee_start..fee_start + 40].copy_from_slice(&entry_b);
        unsorted[fee_start + 40..fee_start + 80].copy_from_slice(&entry_a);

        assert!(IntentHeader::decode(&unsorted).is_err());
    }

    #[test]
    fn decode_rejects_trailing_bytes() {
        let header = test_header();
        let mut bytes = header.canonical_encode();
        bytes.push(0);
        assert!(IntentHeader::decode(&bytes).is_err());
    }

    #[test]
    fn decode_rejects_truncated_bytes() {
        let header = test_header();
        let bytes = header.canonical_encode();
        assert!(IntentHeader::decode(&bytes[..bytes.len() - 1]).is_err());
    }

    #[test]
    fn none_options_roundtrip() {
        let mut header = test_header();
        header.src_mint = None;
        header.out_mint = None;
        header.executor = None;
        header.fee_recipients = vec![];
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.src_mint, None);
        assert_eq!(decoded.out_mint, None);
        assert_eq!(decoded.executor, None);
        assert!(decoded.fee_recipients.is_empty());
    }

    #[test]
    fn executor_some_roundtrip() {
        let mut header = test_header();
        header.executor = Some(Pubkey::new_unique());
        let bytes = header.canonical_encode();
        let decoded = IntentHeader::decode(&bytes).unwrap();
        assert_eq!(decoded.executor, header.executor);
    }

    #[test]
    fn decode_rejects_duplicate_fee_pubkey() {
        let pk = Pubkey::new_from_array([1u8; 32]);
        let mut header = test_header();
        header.fee_recipients = vec![(pk, 100), (pk, 200)];
        let bytes = header.canonical_encode();
        assert!(IntentHeader::decode(&bytes).is_err());
    }
}
