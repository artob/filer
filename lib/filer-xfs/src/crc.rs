// This is free and unencumbered software released into the public domain.

//! Version 5's self-describing metadata: each block's header names the
//! block's own disk address (in 512-byte units), the UUID metadata carries,
//! and the inode owning the block, and holds a CRC-32C of the whole block,
//! computed with the CRC's own four bytes zero and stored little-endian.
//! Blocks of a different shape keep these fields at different offsets.

use crate::{XfsError, superblock::Superblock};
use filer_api::checksum::Crc32c;
use filer_fs::be;

/// Where a block's header keeps its checksum, its disk address, the
/// metadata UUID, and its owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Layout {
    crc: usize,
    daddr: usize,
    uuid: usize,
    owner: usize,
}

/// A `BMA3` B+ tree block's.
pub(crate) const BMAP: Layout = Layout {
    crc: 64,
    daddr: 24,
    uuid: 40,
    owner: 56,
};

/// A directory data block's: `XDB3` or `XDD3`.
pub(crate) const DIR_DATA: Layout = Layout {
    crc: 4,
    daddr: 8,
    uuid: 24,
    owner: 40,
};

/// A leaf or node block's, of a directory or an attribute fork.
pub(crate) const DA: Layout = Layout {
    crc: 12,
    daddr: 16,
    uuid: 32,
    owner: 48,
};

/// A remote attribute value's (`XARM`) or symlink target's (`XSLM`).
pub(crate) const REMOTE: Layout = Layout {
    crc: 12,
    daddr: 40,
    uuid: 16,
    owner: 32,
};

/// Returns `true` if `bytes` hold their CRC-32C at `at`.
pub(crate) fn is_valid(bytes: &[u8], at: usize) -> bool {
    let (Some(head), Some(stored), Some(tail)) = (
        bytes.get(..at),
        bytes.get(at..at.saturating_add(4)),
        bytes.get(at.saturating_add(4)..),
    ) else {
        return false;
    };
    let crc = Crc32c::new().chain(head).chain(&[0; 4]).chain(tail);
    stored == crc.finish().to_le_bytes()
}

/// Sets the CRC-32C of `bytes`, whose four bytes at `at` hold it.
pub(crate) fn seal(bytes: &mut [u8], at: usize) {
    let (Some(head), Some(tail)) = (bytes.get(..at), bytes.get(at.saturating_add(4)..)) else {
        return;
    };
    let crc = Crc32c::new()
        .chain(head)
        .chain(&[0; 4])
        .chain(tail)
        .finish();
    if let Some(field) = bytes.get_mut(at..at.saturating_add(4)) {
        field.copy_from_slice(&crc.to_le_bytes());
    }
}

/// Checks the header of block `bytes`, read from byte `offset` of the
/// volume, which inode `owner` owns.
pub(crate) fn check(
    bytes: &[u8],
    layout: Layout,
    sb: &Superblock,
    offset: u64,
    owner: u64,
) -> Result<(), XfsError> {
    if !is_valid(bytes, layout.crc) {
        return Err(XfsError::BadChecksum { offset });
    }
    let daddr = be::u64_at(bytes, layout.daddr);
    let uuid = bytes.get(layout.uuid..layout.uuid.saturating_add(16));
    let found_owner = be::u64_at(bytes, layout.owner);
    if daddr != Some(offset >> 9) || uuid != Some(sb.meta_uuid()) || found_owner != Some(owner) {
        return Err(XfsError::Misplaced { offset });
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::superblock::tests::Fields;
    use alloc::vec;

    /// Stamps `bytes` as a version 5 block of `layout` at byte `offset`,
    /// owned by `owner`, on the volume of `Fields::v5()`, whose UUID is
    /// eight bytes of 0x11, then eight of 0x22.
    pub(crate) fn stamp(bytes: &mut [u8], layout: Layout, offset: u64, owner: u64) {
        bytes[layout.daddr..layout.daddr + 8].copy_from_slice(&(offset >> 9).to_be_bytes());
        bytes[layout.uuid..layout.uuid + 8].fill(0x11);
        bytes[layout.uuid + 8..layout.uuid + 16].fill(0x22);
        bytes[layout.owner..layout.owner + 8].copy_from_slice(&owner.to_be_bytes());
        super::seal(bytes, layout.crc);
    }

    #[test]
    fn checks_headers() {
        let sb = Superblock::parse(&Fields::v5().bytes()).unwrap();
        for layout in [BMAP, DIR_DATA, DA, REMOTE] {
            let mut block = vec![0x5A; 1024];
            stamp(&mut block, layout, 8192, 131);
            assert_eq!(check(&block, layout, &sb, 8192, 131), Ok(()));
            // Elsewhere, of another owner, or changed.
            let misplaced = Err(XfsError::Misplaced { offset: 4096 });
            assert_eq!(check(&block, layout, &sb, 4096, 131), misplaced);
            let misplaced = Err(XfsError::Misplaced { offset: 8192 });
            assert_eq!(check(&block, layout, &sb, 8192, 132), misplaced);
            block[1000] ^= 1;
            let bad = Err(XfsError::BadChecksum { offset: 8192 });
            assert_eq!(check(&block, layout, &sb, 8192, 131), bad);
        }
        // Another volume's.
        let mut block = vec![0; 512];
        stamp(&mut block, DA, 0, 1);
        block[DA.uuid] = 0;
        super::seal(&mut block, DA.crc);
        assert_eq!(
            check(&block, DA, &sb, 0, 1),
            Err(XfsError::Misplaced { offset: 0 })
        );
        assert!(!is_valid(&[0; 3], 0));
    }
}
