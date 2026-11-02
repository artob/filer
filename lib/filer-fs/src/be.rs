// This is free and unencumbered software released into the public domain.

//! Big-endian fields of on-disk structures, read without panicking:
//! each returns `None` if the field does not fit in the bytes.
//!
//! # Examples
//!
//! ```
//! use filer_fs::be;
//!
//! let bytes = [0x12, 0x34, 0x56, 0x78];
//! assert_eq!(be::u16_at(&bytes, 0), Some(0x1234));
//! assert_eq!(be::u16_at(&bytes, 3), None);
//! ```

/// Returns the `u16` at byte `at` of `bytes`, if it fits.
#[must_use]
pub fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..)?
        .first_chunk()
        .map(|b| u16::from_be_bytes(*b))
}

/// Returns the `u32` at byte `at` of `bytes`, if it fits.
#[must_use]
pub fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..)?
        .first_chunk()
        .map(|b| u32::from_be_bytes(*b))
}

/// Returns the `u64` at byte `at` of `bytes`, if it fits.
#[must_use]
pub fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    bytes
        .get(at..)?
        .first_chunk()
        .map(|b| u64::from_be_bytes(*b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_fields_that_fit() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        assert_eq!(u16_at(&bytes, 7), Some(0x0809));
        assert_eq!(u32_at(&bytes, 1), Some(0x0203_0405));
        assert_eq!(u64_at(&bytes, 1), Some(0x0203_0405_0607_0809));
        assert_eq!(u16_at(&bytes, 8), None);
        assert_eq!(u32_at(&bytes, 6), None);
        assert_eq!(u64_at(&bytes, 2), None);
        assert_eq!(u64_at(&bytes, usize::MAX), None);
    }
}
