// This is free and unencumbered software released into the public domain.

//! Block group descriptors.

use crate::{Ext4Error, superblock::Superblock};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::{le, read_exact};

/// A block group's descriptor, as far as reading needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GroupDesc {
    /// The first block of the group's inode table.
    inode_table: u64,
}

impl GroupDesc {
    /// Parses the descriptor of group `group` from `bytes`, checking that
    /// its inode table lies within the volume, past the superblock.
    pub(crate) fn parse(bytes: &[u8], group: u32, sb: &Superblock) -> Result<Self, Ext4Error> {
        let bad = Ext4Error::BadGroupDescriptor { group };
        let [l0, l1, l2, l3] = le::u32_at(bytes, 0x8).ok_or(bad)?.to_le_bytes();
        // The high half, in descriptors of 64 bytes or more.
        let [h0, h1, h2, h3] = match le::u32_at(bytes, 0x28) {
            Some(high) if sb.desc_size() >= 64 => high.to_le_bytes(),
            _ => [0; 4],
        };
        let inode_table = u64::from_le_bytes([l0, l1, l2, l3, h0, h1, h2, h3]);
        let end = inode_table
            .checked_add(sb.inode_table_blocks())
            .ok_or(bad)?;
        if inode_table <= u64::from(sb.first_data_block()) || end > sb.blocks() {
            return Err(bad);
        }
        Ok(Self { inode_table })
    }

    /// Returns the first block of the group's inode table.
    pub(crate) const fn inode_table(&self) -> u64 {
        self.inode_table
    }
}

/// Reads the descriptor of group `group`.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    group: u32,
    op: OpKind,
) -> Result<GroupDesc, Error> {
    let fail = |error: Ext4Error| error.into_error(op);
    let bad = || fail(Ext4Error::BadGroupDescriptor { group });
    if group >= sb.groups() {
        return Err(bad());
    }
    let size = sb.desc_size();
    let off = sb.descriptor_offset(group).ok_or_else(bad)?;
    let bytes = read_exact(source, off, usize::from(size), op).await?;
    GroupDesc::parse(&bytes, group, sb).map_err(fail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock::tests::Fields;

    fn desc(inode_table: u32) -> [u8; 32] {
        let mut b = [0; 32];
        b[0..4].copy_from_slice(&3u32.to_le_bytes());
        b[4..8].copy_from_slice(&4u32.to_le_bytes());
        b[8..12].copy_from_slice(&inode_table.to_le_bytes());
        b
    }

    #[test]
    fn parses_and_checks_the_inode_table() {
        // 1024 inodes of 256 bytes: 256 blocks of 1 KiB, in 4096 blocks.
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        assert_eq!(sb.inode_table_blocks(), 256);
        let parse = |table| GroupDesc::parse(&desc(table), 0, &sb);
        assert_eq!(parse(5).unwrap().inode_table(), 5);
        assert_eq!(parse(4096 - 256).unwrap().inode_table(), 4096 - 256);
        let bad = Err(Ext4Error::BadGroupDescriptor { group: 0 });
        assert_eq!(parse(4096 - 255), bad);
        assert_eq!(parse(1), bad);
        assert_eq!(parse(0), bad);
        assert_eq!(parse(u32::MAX), bad);
        assert_eq!(GroupDesc::parse(&[0; 8], 0, &sb), bad);
    }

    /// Descriptors of 64 bytes or more hold the high half of the inode
    /// table's block.
    #[test]
    fn reads_64bit_inode_tables() {
        let mut f = Fields::new();
        f.features.incompat |= 0x80;
        let mut b = f.bytes();
        b[0xFE..0x100].copy_from_slice(&64u16.to_le_bytes());
        let sb = Superblock::parse(&b).unwrap();
        let mut d = [0; 64];
        d[..32].copy_from_slice(&desc(7));
        assert_eq!(GroupDesc::parse(&d, 0, &sb).unwrap().inode_table(), 7);
        d[0x28] = 1;
        assert_eq!(
            GroupDesc::parse(&d, 0, &sb),
            Err(Ext4Error::BadGroupDescriptor { group: 0 })
        );
        // In descriptors of 32 bytes, those bytes are another's.
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        assert_eq!(GroupDesc::parse(&d, 0, &sb).unwrap().inode_table(), 7);
    }

    #[test]
    fn reads_from_the_table() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let mut image = alloc::vec![0; 4096 * 1024];
        // The table follows the superblock's block, 1.
        image[2048..2080].copy_from_slice(&desc(7));
        let read = |group| filer_spec::block_on(read(&image, &sb, group, OpKind::Open));
        assert_eq!(read(0).unwrap().inode_table(), 7);
        assert_eq!(read(1).unwrap_err().kind(), filer_api::ErrorKind::Corrupt);
    }
}
