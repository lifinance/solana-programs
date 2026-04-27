use anchor_lang::prelude::*;

use crate::errors::WireError;

pub const MAX_CALLS: usize = 8;
pub const MAX_ACCOUNTS_PER_CALL: usize = 64;
pub const MAX_DATA_LEN: usize = 1024;
pub const NAMED_PREFIX: usize = 4;

/// Zero-copy reference into a single call's wire bytes.
pub struct CallSpecRef<'a> {
    pub program_ix: u8,
    pub accounts: &'a [u8],
    pub flags_bitmap: &'a [u8],
    pub data: &'a [u8],
}

impl<'a> CallSpecRef<'a> {
    /// Read the per-account (is_writable, is_signer) flags from the bitmap.
    /// Bit 2i+0 = is_writable, bit 2i+1 = is_signer, packed LSB-first.
    pub fn flag_for(&self, i: usize) -> (bool, bool) {
        flag_for(self.flags_bitmap, i)
    }
}

/// Read the (is_writable, is_signer) flags for account `i` from a bitmap.
pub fn flag_for(bitmap: &[u8], i: usize) -> (bool, bool) {
    let bit_offset = i * 2;
    let byte_ix = bit_offset / 8;
    let bit_ix = bit_offset % 8;
    if byte_ix >= bitmap.len() {
        return (false, false);
    }
    let byte = bitmap[byte_ix];
    let is_writable = (byte >> bit_ix) & 1 == 1;
    let is_signer = (byte >> (bit_ix + 1)) & 1 == 1;
    (is_writable, is_signer)
}

/// Zero-copy cursor-based iterator over wire-encoded `Vec<CallSpec>`.
pub struct CallsIter<'a> {
    buf: &'a [u8],
    cursor: usize,
    remaining: u8,
    virtual_list_len: usize,
}

impl<'a> CallsIter<'a> {
    /// Create a new iterator. `remaining_len` is
    /// `ctx.remaining_accounts.len()` — used for index-bound validation.
    pub fn new(buf: &'a [u8], remaining_len: usize) -> Result<Self> {
        if buf.is_empty() {
            return Err(WireError::MalformedWireBytes.into());
        }
        let num_calls = buf[0] as usize;
        if num_calls > MAX_CALLS {
            return Err(WireError::TooManyCalls.into());
        }
        Ok(Self {
            buf,
            cursor: 1,
            remaining: num_calls as u8,
            virtual_list_len: NAMED_PREFIX + remaining_len,
        })
    }

    pub fn next_call(&mut self) -> Option<Result<CallSpecRef<'a>>> {
        if self.remaining == 0 {
            return None;
        }
        self.remaining -= 1;
        Some(self.read_one())
    }

    fn read_one(&mut self) -> Result<CallSpecRef<'a>> {
        let program_ix = self.read_u8()?;
        if (program_ix as usize) < NAMED_PREFIX {
            return Err(WireError::ProgramInNamedPrefix.into());
        }
        if (program_ix as usize) >= self.virtual_list_len {
            return Err(WireError::AccountIndexOutOfBounds.into());
        }

        let acc_count = self.read_u8()? as usize;
        if acc_count > MAX_ACCOUNTS_PER_CALL {
            return Err(WireError::TooManyAccountsPerCall.into());
        }

        let accounts = self.read_slice(acc_count)?;
        for &ix in accounts {
            if (ix as usize) >= self.virtual_list_len {
                return Err(WireError::AccountIndexOutOfBounds.into());
            }
        }

        let bitmap_len = (acc_count * 2 + 7) / 8;
        let flags_bitmap = self.read_slice(bitmap_len)?;

        if acc_count > 0 {
            validate_trailing_bitmap_bits(flags_bitmap, acc_count)?;
        }

        let data_len = self.read_u16_le()? as usize;
        if data_len > MAX_DATA_LEN {
            return Err(WireError::DataTooLarge.into());
        }
        let data = self.read_slice(data_len)?;

        Ok(CallSpecRef {
            program_ix,
            accounts,
            flags_bitmap,
            data,
        })
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.cursor >= self.buf.len() {
            return Err(WireError::MalformedWireBytes.into());
        }
        let val = self.buf[self.cursor];
        self.cursor += 1;
        Ok(val)
    }

    fn read_u16_le(&mut self) -> Result<u16> {
        let end = self.cursor + 2;
        if end > self.buf.len() {
            return Err(WireError::MalformedWireBytes.into());
        }
        let val = u16::from_le_bytes(self.buf[self.cursor..end].try_into().unwrap());
        self.cursor = end;
        Ok(val)
    }

    fn read_slice(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.cursor + len;
        if end > self.buf.len() {
            return Err(WireError::MalformedWireBytes.into());
        }
        let slice = &self.buf[self.cursor..end];
        self.cursor = end;
        Ok(slice)
    }

    /// After iterating all calls, verify no trailing bytes remain.
    pub fn assert_exhausted(&self) -> Result<()> {
        if self.cursor != self.buf.len() {
            return Err(WireError::MalformedWireBytes.into());
        }
        Ok(())
    }
}

fn validate_trailing_bitmap_bits(bitmap: &[u8], acc_count: usize) -> Result<()> {
    let used_bits = acc_count * 2;
    let total_bits = bitmap.len() * 8;
    if used_bits < total_bits {
        let last_byte = bitmap[bitmap.len() - 1];
        let used_in_last = used_bits % 8;
        if used_in_last == 0 {
            return Ok(());
        }
        let mask = !((1u8 << used_in_last) - 1);
        if last_byte & mask != 0 {
            return Err(WireError::NonCanonicalBitmap.into());
        }
    }
    Ok(())
}

/// Encode a list of calls into wire bytes. Used in tests and by the
/// gen_vector example.
pub fn encode_calls(calls: &[CallSpecOwned]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(calls.len() as u8);
    for call in calls {
        buf.push(call.program_ix);
        let acc_count = call.accounts.len();
        buf.push(acc_count as u8);
        buf.extend_from_slice(&call.accounts);
        buf.extend_from_slice(&call.flags_bitmap);
        buf.extend_from_slice(&(call.data.len() as u16).to_le_bytes());
        buf.extend_from_slice(&call.data);
    }
    buf
}

/// Owned version of CallSpec for encoding and test use.
#[derive(Debug, Clone)]
pub struct CallSpecOwned {
    pub program_ix: u8,
    pub accounts: Vec<u8>,
    pub flags_bitmap: Vec<u8>,
    pub data: Vec<u8>,
}

impl CallSpecOwned {
    pub fn new(
        program_ix: u8,
        accounts: Vec<u8>,
        is_writable: Vec<bool>,
        is_signer: Vec<bool>,
        data: Vec<u8>,
    ) -> Self {
        let acc_count = accounts.len();
        let bitmap_len = (acc_count * 2 + 7) / 8;
        let mut bitmap = vec![0u8; bitmap_len];
        for i in 0..acc_count {
            let bit_offset = i * 2;
            let byte_ix = bit_offset / 8;
            let bit_ix = bit_offset % 8;
            if is_writable.get(i).copied().unwrap_or(false) {
                bitmap[byte_ix] |= 1 << bit_ix;
            }
            if is_signer.get(i).copied().unwrap_or(false) {
                bitmap[byte_ix] |= 1 << (bit_ix + 1);
            }
        }
        Self {
            program_ix,
            accounts,
            flags_bitmap: bitmap,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_decode_roundtrip_single_call() {
        let call = CallSpecOwned::new(
            4,
            vec![0, 1, 2],
            vec![true, false, true],
            vec![false, false, false],
            vec![1, 2, 3, 4],
        );
        let encoded = encode_calls(&[call]);
        let remaining_len = 10;
        let mut iter = CallsIter::new(&encoded, remaining_len).unwrap();
        let decoded = iter.next_call().unwrap().unwrap();
        assert_eq!(decoded.program_ix, 4);
        assert_eq!(decoded.accounts, &[0, 1, 2]);
        assert_eq!(decoded.data, &[1, 2, 3, 4]);
        let (w, s) = decoded.flag_for(0);
        assert!(w);
        assert!(!s);
        assert!(iter.next_call().is_none());
        iter.assert_exhausted().unwrap();
    }

    #[test]
    fn encode_decode_roundtrip_max_calls() {
        let calls: Vec<CallSpecOwned> = (0..MAX_CALLS)
            .map(|i| {
                CallSpecOwned::new(
                    (NAMED_PREFIX + i) as u8,
                    vec![0],
                    vec![true],
                    vec![false],
                    vec![i as u8],
                )
            })
            .collect();
        let encoded = encode_calls(&calls);
        let mut iter = CallsIter::new(&encoded, MAX_CALLS + 10).unwrap();
        for i in 0..MAX_CALLS {
            let c = iter.next_call().unwrap().unwrap();
            assert_eq!(c.program_ix, (NAMED_PREFIX + i) as u8);
            assert_eq!(c.data, &[i as u8]);
        }
        assert!(iter.next_call().is_none());
        iter.assert_exhausted().unwrap();
    }

    #[test]
    fn rejects_too_many_calls() {
        let mut buf = vec![9u8]; // MAX_CALLS + 1
        buf.extend_from_slice(&[0u8; 100]);
        assert!(CallsIter::new(&buf, 10).is_err());
    }

    #[test]
    fn rejects_program_in_named_prefix() {
        let call = CallSpecOwned {
            program_ix: 3, // < NAMED_PREFIX
            accounts: vec![0],
            flags_bitmap: vec![0b01],
            data: vec![],
        };
        let encoded = encode_calls(&[call]);
        let mut iter = CallsIter::new(&encoded, 10).unwrap();
        assert!(iter.next_call().unwrap().is_err());
    }

    #[test]
    fn rejects_account_index_oob() {
        let call = CallSpecOwned::new(
            4,
            vec![0, 255], // 255 is out of bounds for remaining_len=1
            vec![false, false],
            vec![false, false],
            vec![],
        );
        let encoded = encode_calls(&[call]);
        let mut iter = CallsIter::new(&encoded, 1).unwrap();
        assert!(iter.next_call().unwrap().is_err());
    }

    #[test]
    fn rejects_non_canonical_bitmap() {
        let call = CallSpecOwned {
            program_ix: 4,
            accounts: vec![0],
            flags_bitmap: vec![0b11111101], // trailing bits set
            data: vec![],
        };
        let encoded = encode_calls(&[call]);
        let mut iter = CallsIter::new(&encoded, 10).unwrap();
        assert!(iter.next_call().unwrap().is_err());
    }

    #[test]
    fn rejects_trailing_wire_bytes() {
        let call = CallSpecOwned::new(4, vec![0], vec![true], vec![false], vec![]);
        let mut encoded = encode_calls(&[call]);
        encoded.push(0xFF);
        let mut iter = CallsIter::new(&encoded, 10).unwrap();
        iter.next_call().unwrap().unwrap();
        assert!(iter.next_call().is_none());
        assert!(iter.assert_exhausted().is_err());
    }

    #[test]
    fn rejects_data_too_large() {
        let call = CallSpecOwned::new(
            4,
            vec![0],
            vec![false],
            vec![false],
            vec![0u8; MAX_DATA_LEN + 1],
        );
        let encoded = encode_calls(&[call]);
        let mut iter = CallsIter::new(&encoded, 10).unwrap();
        assert!(iter.next_call().unwrap().is_err());
    }

    #[test]
    fn empty_calls_roundtrip() {
        let encoded = encode_calls(&[]);
        let mut iter = CallsIter::new(&encoded, 0).unwrap();
        assert!(iter.next_call().is_none());
        iter.assert_exhausted().unwrap();
    }

    #[test]
    fn flag_for_multiple_accounts() {
        // 4 accounts: [writable, signer, both, neither]
        // Bits: w0=1,s0=0, w1=0,s1=1, w2=1,s2=1, w3=0,s3=0
        // Byte 0: 0b_11_10_01 = 0b00_11_10_01 = 0x39
        let bitmap = &[0x39u8];
        assert_eq!(flag_for(bitmap, 0), (true, false));
        assert_eq!(flag_for(bitmap, 1), (false, true));
        assert_eq!(flag_for(bitmap, 2), (true, true));
        assert_eq!(flag_for(bitmap, 3), (false, false));
    }
}
