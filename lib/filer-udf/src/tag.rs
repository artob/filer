// This is free and unencumbered software released into the public domain.

//! Descriptor tags: the 16 bytes that begin every UDF descriptor, of its
//! identifier, its version, a checksum of the tag, its serial number, a
//! CRC of what follows, and the block it says it is at.

use filer_fs::le;

/// The identifiers of the descriptors read.
pub(crate) const ANCHOR: u16 = 2;
pub(crate) const POINTER: u16 = 3;
pub(crate) const PARTITION: u16 = 5;
pub(crate) const LOGICAL_VOLUME: u16 = 6;
pub(crate) const TERMINATING: u16 = 8;
pub(crate) const FILE_SET: u16 = 256;
pub(crate) const FILE_IDENTIFIER: u16 = 257;
pub(crate) const ALLOCATION_EXTENT: u16 = 258;
pub(crate) const FILE_ENTRY: u16 = 261;
pub(crate) const EXTENDED_FILE_ENTRY: u16 = 266;

/// Returns the identifier of the tag at the start of `bytes`, if its
/// checksum, the sum of its other bytes, holds. Its CRC is not checked.
pub(crate) fn identifier(bytes: &[u8]) -> Option<u16> {
    let tag = bytes.get(..16)?;
    let sum = tag
        .iter()
        .enumerate()
        .filter(|&(at, _)| at != 4)
        .fold(0_u8, |sum, (_, &byte)| sum.wrapping_add(byte));
    (Some(&sum) == tag.get(4)).then(|| le::u16_at(tag, 0))?
}

/// Returns `true` if `bytes` begin with a valid tag of `id`.
pub(crate) fn is(bytes: &[u8], id: u16) -> bool {
    identifier(bytes) == Some(id)
}

/// Returns the block the tag at the start of `bytes` says it is at.
pub(crate) fn location(bytes: &[u8]) -> Option<u32> {
    le::u32_at(bytes, 12)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_tags() {
        let mut tag = [0_u8; 16];
        tag[0] = 2;
        tag[12] = 1;
        tag[4] = 3;
        assert_eq!(identifier(&tag), Some(ANCHOR));
        assert!(is(&tag, ANCHOR) && !is(&tag, POINTER));
        assert_eq!(location(&tag), Some(1));
        tag[4] = 4;
        assert_eq!(identifier(&tag), None);
        assert_eq!(identifier(&tag[..15]), None);
    }
}
