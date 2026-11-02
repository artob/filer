// This is free and unencumbered software released into the public domain.

//! Little-endian fields of on-disk structures, read without panicking.

/// Returns the `u16` at byte `at` of `bytes`, if it fits.
pub(crate) fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    bytes
        .get(at..)?
        .first_chunk()
        .map(|b| u16::from_le_bytes(*b))
}

/// Returns the `u32` at byte `at` of `bytes`, if it fits.
pub(crate) fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    bytes
        .get(at..)?
        .first_chunk()
        .map(|b| u32::from_le_bytes(*b))
}
