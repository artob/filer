// This is free and unencumbered software released into the public domain.

//! Update sequence arrays ("fixups"), which protect MFT and index records
//! against torn writes.
//!
//! The last two bytes of every 512-byte stride of a record hold the
//! record's update sequence number, and the bytes they replaced are kept
//! in the update sequence array, after the number itself. A stride whose
//! last two bytes differ was not written with the rest: the record is
//! torn, and must not be used.

/// The stride the update sequence protects, whatever the sector size.
pub(crate) const STRIDE: usize = 512;

/// Checks and removes the update sequence of `record`, whose array is at
/// `offset` and has `count` entries, the number included. Returns `false`
/// (leaving `record` partly restored) if the array does not fit the
/// record or any stride is torn.
#[must_use]
pub(crate) fn apply(record: &mut [u8], offset: u16, count: u16) -> bool {
    let strides = record.len() / STRIDE;
    let offset = usize::from(offset);
    let count = usize::from(count);
    // One entry for the number, one per stride, all within the first
    // stride, before its own protected bytes, and aligned.
    let fits = record.len().is_multiple_of(STRIDE)
        && strides > 0
        && count == strides.saturating_add(1)
        && offset.is_multiple_of(2)
        && offset
            .checked_add(count.saturating_mul(2))
            .is_some_and(|end| end <= STRIDE - 2);
    if !fits {
        return false;
    }
    let Some(array) = record
        .get(offset..)
        .and_then(|rest| rest.get(..count.saturating_mul(2)))
    else {
        return false;
    };
    let array: alloc::vec::Vec<[u8; 2]> = array.as_chunks::<2>().0.to_vec();
    let Some((&number, saved)) = array.split_first() else {
        return false;
    };
    for (stride, saved) in record.as_chunks_mut::<STRIDE>().0.iter_mut().zip(saved) {
        let Some(last) = stride.last_chunk_mut::<2>() else {
            return false;
        };
        if *last != number {
            return false;
        }
        *last = *saved;
    }
    true
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    /// Protects `record` with an update sequence at `offset`, numbered
    /// `number`, as a writer does.
    pub(crate) fn protect(record: &mut [u8], offset: usize, number: u16) {
        let strides = record.len() / STRIDE;
        record[offset..offset + 2].copy_from_slice(&number.to_le_bytes());
        for i in 0..strides {
            let end = (i + 1) * STRIDE;
            let saved = [record[end - 2], record[end - 1]];
            record[offset + 2 + 2 * i..offset + 4 + 2 * i].copy_from_slice(&saved);
            record[end - 2..end].copy_from_slice(&number.to_le_bytes());
        }
    }

    #[test]
    fn restores_protected_records() {
        let mut record = vec![0u8; 1024];
        record[510] = 0xAB;
        record[1023] = 0xCD;
        let original = record.clone();
        protect(&mut record, 0x30, 7);
        assert_eq!(&record[510..512], &7u16.to_le_bytes());
        assert!(apply(&mut record, 0x30, 3));
        assert_eq!(record[510], 0xAB);
        assert_eq!(record[1023], 0xCD);
        // Everything past the array is as it was.
        assert_eq!(&record[0x36..], &original[0x36..]);
    }

    #[test]
    fn detects_torn_records_and_bad_arrays() {
        let mut fresh = vec![0u8; 1024];
        protect(&mut fresh, 0x30, 7);
        let mut torn = fresh.clone();
        torn[1022] = 8;
        assert!(!apply(&mut torn, 0x30, 3));
        for (offset, count) in [(0x30, 2), (0x30, 4), (0x31, 3), (509, 3), (0xFFFF, 3)] {
            assert!(
                !apply(&mut fresh.clone(), offset, count),
                "{offset} {count}"
            );
        }
        assert!(!apply(&mut [0u8; 1000], 0x30, 3));
        assert!(!apply(&mut [], 0x30, 1));
    }
}
