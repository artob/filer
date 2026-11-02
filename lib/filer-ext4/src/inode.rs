// This is free and unencumbered software released into the public domain.

//! Inodes: finding them in their group's table, and decoding them.

use crate::{
    Ext4Error, group, io, le,
    superblock::{GOOD_OLD_INODE_SIZE, Superblock},
};
use alloc::vec::Vec;
use core::time::Duration;
use filer_api::{Error, FileAttrs, FileKind, OpKind, ReadAt, Timestamp};

/// The size of `i_block`: block numbers, an extent tree's root, inline
/// data, a fast symlink's target, or a device number.
pub(crate) const I_BLOCK_SIZE: usize = 60;

/// The granularity of times without, and with, the extra fields.
const SECONDS: Duration = Duration::from_secs(1);
const NANOSECONDS: Duration = Duration::from_nanos(1);

/// An inode's four times. Creation is recorded only in the extra fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Times {
    pub(crate) accessed: Timestamp,
    pub(crate) changed: Timestamp,
    pub(crate) modified: Timestamp,
    pub(crate) created: Option<Timestamp>,
}

/// A decoded inode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Inode {
    number: u32,
    kind: FileKind,
    size: u64,
    links: u16,
    /// `i_blocks`: 512-byte sectors in use, data and metadata.
    sectors: u32,
    flags: u32,
    block: [u8; I_BLOCK_SIZE],
    generation: u32,
    file_acl: u32,
    times: Times,
    /// What follows the base and its extra fields in the record: in-inode
    /// extended attributes.
    tail: Vec<u8>,
}

/// Decodes a time from its seconds, a signed 32-bit field, and its extra
/// field, if any: two epoch bits extending the seconds, then nanoseconds.
fn timestamp(secs: u32, extra: Option<u32>) -> Option<Timestamp> {
    let secs = i64::from(i32::from_le_bytes(secs.to_le_bytes()));
    let Some(extra) = extra else {
        return Some(Timestamp::new(secs, 0).with_granularity(SECONDS));
    };
    let epoch = i64::from(extra & 0b11) << 32;
    let nanos = extra >> 2;
    (nanos < 1_000_000_000)
        .then(|| Timestamp::new(secs.saturating_add(epoch), nanos).with_granularity(NANOSECONDS))
}

impl Inode {
    /// Decodes inode `number` from its record in the inode table.
    pub(crate) fn parse(bytes: &[u8], number: u32) -> Result<Self, Ext4Error> {
        let bad = Ext4Error::BadInode { inode: number };
        let u16_at = |at| le::u16_at(bytes, at).ok_or(bad);
        let u32_at = |at| le::u32_at(bytes, at).ok_or(bad);
        let mode = u16_at(0x0)?;
        let links = u16_at(0x1A)?;
        if mode == 0 || links == 0 {
            return Err(Ext4Error::UnusedInode { inode: number });
        }
        let kind = match mode & 0xF000 {
            0x1000 => FileKind::Fifo,
            0x2000 => FileKind::CharDevice,
            0x4000 => FileKind::Dir,
            0x6000 => FileKind::BlockDevice,
            0x8000 => FileKind::File,
            0xA000 => FileKind::Symlink,
            0xC000 => FileKind::Socket,
            _ => return Err(bad),
        };
        let size = u64::from(u32_at(0x4)?) | u64::from(u32_at(0x6C)?) << 32;
        let mut block = [0; I_BLOCK_SIZE];
        block.copy_from_slice(bytes.get(0x28..0x64).ok_or(bad)?);
        // The extra fields, `i_extra_isize` bytes from 0x80, if the record
        // is larger than the original 128 bytes.
        let base = usize::from(GOOD_OLD_INODE_SIZE);
        let extra = if bytes.len() > base {
            usize::from(u16_at(base)?)
        } else {
            0
        };
        let end = base.checked_add(extra).filter(|&end| end <= bytes.len());
        let end = end.ok_or(bad)?;
        // An extra field is present if the extra size covers it.
        let field = |at: usize| {
            at.checked_add(4)
                .filter(|&field_end| field_end <= end)
                .and_then(|_| le::u32_at(bytes, at))
        };
        let time = |at, extra_at| timestamp(u32_at(at)?, field(extra_at)).ok_or(bad);
        let times = Times {
            accessed: time(0x8, 0x8C)?,
            changed: time(0xC, 0x84)?,
            modified: time(0x10, 0x88)?,
            created: match field(0x90) {
                Some(secs) => Some(timestamp(secs, field(0x94)).ok_or(bad)?),
                None => None,
            },
        };
        Ok(Self {
            number,
            kind,
            size,
            links,
            sectors: u32_at(0x1C)?,
            flags: u32_at(0x20)?,
            block,
            generation: u32_at(0x64)?,
            file_acl: u32_at(0x68)?,
            times,
            tail: bytes.get(end..).unwrap_or_default().to_vec(),
        })
    }

    pub(crate) const fn number(&self) -> u32 {
        self.number
    }

    /// Returns the file's kind, from its mode.
    pub(crate) const fn kind(&self) -> FileKind {
        self.kind
    }

    /// Returns the size in bytes.
    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn links(&self) -> u16 {
        self.links
    }

    /// Returns the number of 512-byte sectors the file uses.
    pub(crate) const fn sectors(&self) -> u32 {
        self.sectors
    }

    /// Returns `i_block`.
    pub(crate) const fn block(&self) -> &[u8; I_BLOCK_SIZE] {
        &self.block
    }

    #[allow(dead_code, reason = "for checksums, later")]
    pub(crate) const fn generation(&self) -> u32 {
        self.generation
    }

    /// Returns the block of the file's extended attributes, or 0.
    pub(crate) const fn file_acl(&self) -> u32 {
        self.file_acl
    }

    /// Returns the inode flags that are [`FileAttrs`].
    pub(crate) fn attrs(&self) -> FileAttrs {
        const FLAGS: [(u32, FileAttrs); 5] = [
            (0x0000_0004, FileAttrs::COMPRESSED),
            (0x0000_0010, FileAttrs::IMMUTABLE),
            (0x0000_0020, FileAttrs::APPEND_ONLY),
            (0x0000_0800, FileAttrs::ENCRYPTED),
            (0x0010_0000, FileAttrs::VERITY),
        ];
        let mut attrs = FileAttrs::empty();
        for (bit, attr) in FLAGS {
            if self.flags & bit != 0 {
                attrs |= attr;
            }
        }
        attrs
    }

    pub(crate) const fn times(&self) -> &Times {
        &self.times
    }

    /// Returns what follows the inode's fields in its record.
    pub(crate) fn tail(&self) -> &[u8] {
        &self.tail
    }
}

/// Reads inode `number`.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    number: u32,
    op: OpKind,
) -> Result<Inode, Error> {
    let fail = |error: Ext4Error| error.into_error(op);
    let bad = || fail(Ext4Error::BadInodeNumber(number));
    if number == 0 || number > sb.inodes() {
        return Err(bad());
    }
    let index = number.checked_sub(1).ok_or_else(bad)?;
    let per_group = sb.inodes_per_group();
    let group = index.checked_div(per_group).ok_or_else(bad)?;
    let slot = index.checked_rem(per_group).ok_or_else(bad)?;
    let desc = group::read(source, sb, group, op).await?;
    // Within the table, which the descriptor checked is in the volume.
    let size = sb.inode_size();
    let off = sb
        .block_offset(desc.inode_table())
        .and_then(|table| table.checked_add(u64::from(slot).checked_mul(u64::from(size))?))
        .ok_or_else(bad)?;
    let bytes = io::read_exact(source, off, usize::from(size)).await?;
    Inode::parse(&bytes, number).map_err(fail)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::superblock::tests::Fields;
    use alloc::vec;

    /// An inode's fields, for building test volumes.
    #[derive(Clone)]
    pub(crate) struct Raw {
        pub(crate) mode: u16,
        pub(crate) size: u64,
        pub(crate) links: u16,
        pub(crate) sectors: u32,
        pub(crate) flags: u32,
        pub(crate) block: [u8; I_BLOCK_SIZE],
        pub(crate) file_acl: u32,
        /// Seconds of the access, change, and modification times.
        pub(crate) times: [u32; 3],
        /// `i_extra_isize`, and the extra fields after it.
        pub(crate) extra: Option<(u16, Vec<u8>)>,
    }

    impl Raw {
        pub(crate) fn file(size: u64) -> Self {
            Self {
                mode: 0o100_644,
                size,
                links: 1,
                sectors: 0,
                flags: 0,
                block: [0; I_BLOCK_SIZE],
                file_acl: 0,
                times: [1_700_000_003, 1_700_000_002, 1_700_000_001],
                extra: None,
            }
        }

        /// Returns the record, `len` bytes long.
        pub(crate) fn bytes(&self, len: usize) -> Vec<u8> {
            let mut b = vec![0; len];
            let mut put = |at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
            put(0x0, &self.mode.to_le_bytes());
            put(0x4, &self.size.to_le_bytes()[..4]);
            put(0x8, &self.times[0].to_le_bytes());
            put(0xC, &self.times[1].to_le_bytes());
            put(0x10, &self.times[2].to_le_bytes());
            put(0x1A, &self.links.to_le_bytes());
            put(0x1C, &self.sectors.to_le_bytes());
            put(0x20, &self.flags.to_le_bytes());
            put(0x28, &self.block);
            put(0x64, &7u32.to_le_bytes());
            put(0x68, &self.file_acl.to_le_bytes());
            put(0x6C, &self.size.to_le_bytes()[4..]);
            if let Some((extra_isize, fields)) = &self.extra {
                put(0x80, &extra_isize.to_le_bytes());
                put(0x84, fields);
            }
            b
        }
    }

    #[test]
    fn decodes_an_original_inode() {
        let mut raw = Raw::file(0x1_2345_6789);
        raw.flags = 0x10;
        raw.block[0] = 9;
        let inode = Inode::parse(&raw.bytes(128), 12).unwrap();
        assert_eq!(inode.number(), 12);
        assert_eq!(inode.kind(), FileKind::File);
        assert_eq!(inode.size(), 0x1_2345_6789);
        assert_eq!((inode.links(), inode.generation()), (1, 7));
        assert_eq!(inode.block()[0], 9);
        assert_eq!(inode.attrs(), FileAttrs::IMMUTABLE);
        let times = inode.times();
        assert_eq!(times.accessed.secs(), 1_700_000_003);
        assert_eq!(times.changed.secs(), 1_700_000_002);
        assert_eq!(times.modified.secs(), 1_700_000_001);
        assert_eq!(times.modified.granularity(), Some(SECONDS));
        assert_eq!(times.created, None);
        assert!(inode.tail().is_empty());
    }

    #[test]
    fn decodes_every_kind() {
        for (mode, kind) in [
            (0o010_000, FileKind::Fifo),
            (0o020_000, FileKind::CharDevice),
            (0o040_755, FileKind::Dir),
            (0o060_000, FileKind::BlockDevice),
            (0o100_000, FileKind::File),
            (0o120_777, FileKind::Symlink),
            (0o140_000, FileKind::Socket),
        ] {
            let mut raw = Raw::file(0);
            raw.mode = mode;
            assert_eq!(Inode::parse(&raw.bytes(128), 12).unwrap().kind(), kind);
        }
        let mut raw = Raw::file(0);
        raw.mode = 0o170_000;
        assert_eq!(
            Inode::parse(&raw.bytes(128), 12),
            Err(Ext4Error::BadInode { inode: 12 })
        );
    }

    #[test]
    fn refuses_unused_inodes() {
        let mut raw = Raw::file(0);
        raw.links = 0;
        let unused = Err(Ext4Error::UnusedInode { inode: 3 });
        assert_eq!(Inode::parse(&raw.bytes(128), 3), unused);
        assert_eq!(Inode::parse(&[0; 256], 3), unused);
    }

    #[test]
    fn decodes_extra_times() {
        let mut fields = Vec::new();
        // Change: epoch 1 and 5 ns; modification: 7 ns; access: epoch 3.
        for extra in [5 << 2 | 1, 7 << 2, 3u32] {
            fields.extend(extra.to_le_bytes());
        }
        // Creation: before 1970, at 999,999,999 ns.
        fields.extend((-10i32).to_le_bytes());
        fields.extend((999_999_999u32 << 2).to_le_bytes());
        let mut raw = Raw::file(0);
        raw.times = [0x8000_0000, 0x7FFF_FFFF, 0];
        raw.extra = Some((32, fields));
        let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
        let t = inode.times();
        assert_eq!((t.changed.secs(), t.changed.nanos()), (0x1_7FFF_FFFF, 5));
        assert_eq!((t.modified.secs(), t.modified.nanos()), (0, 7));
        // 0x80000000 is -2^31, plus 3 << 32.
        assert_eq!(t.accessed.secs(), 0x2_8000_0000);
        let created = t.created.unwrap();
        assert_eq!((created.secs(), created.nanos()), (-10, 999_999_999));
        assert_eq!(created.granularity(), Some(NANOSECONDS));
        // In-inode extended attributes follow the 32 bytes.
        assert_eq!(inode.tail().len(), 256 - 160);
    }

    #[test]
    fn reads_only_the_extra_fields_present() {
        // Room for the change and modification times' extra fields only.
        let mut raw = Raw::file(0);
        raw.times = [0x8000_0000; 3];
        raw.extra = Some((12, vec![0x55; 28]));
        let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
        let t = inode.times();
        assert_eq!(t.changed.granularity(), Some(NANOSECONDS));
        assert_eq!(t.modified.granularity(), Some(NANOSECONDS));
        assert_eq!(t.accessed.granularity(), Some(SECONDS));
        assert_eq!(t.accessed.secs(), -0x8000_0000);
        assert_eq!(t.created, None);
        assert_eq!(inode.tail().len(), 256 - 140);
        // No extra fields at all.
        raw.extra = Some((0, Vec::new()));
        let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
        assert_eq!(inode.times().changed.granularity(), Some(SECONDS));
        assert_eq!(inode.tail().len(), 128);
    }

    #[test]
    fn refuses_bad_extra_fields() {
        let bad = Err(Ext4Error::BadInode { inode: 12 });
        let mut raw = Raw::file(0);
        raw.extra = Some((129, Vec::new()));
        assert_eq!(Inode::parse(&raw.bytes(256), 12), bad);
        // Nanoseconds of a second or more.
        raw.extra = Some((8, (1_000_000_000u32 << 2).to_le_bytes().to_vec()));
        assert_eq!(Inode::parse(&raw.bytes(256), 12), bad);
        assert_eq!(Inode::parse(&raw.bytes(256)[..100], 12), bad);
    }

    #[test]
    fn reads_from_the_inode_table() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let mut image = vec![0; 4096 * 1024];
        // Group 0's inode table at block 5; inode 12 is its twelfth slot.
        image[2048 + 8..2048 + 12].copy_from_slice(&5u32.to_le_bytes());
        let at = 5 * 1024 + 11 * 256;
        image[at..at + 256].copy_from_slice(&Raw::file(42).bytes(256));
        let read = |n| filer_spec::block_on(read(&image, &sb, n, OpKind::Open));
        assert_eq!(read(12).unwrap().size(), 42);
        for n in [0, 1025] {
            let error = read(n).unwrap_err();
            assert_eq!(error.kind(), filer_api::ErrorKind::Corrupt);
        }
        assert_eq!(read(13).unwrap_err().kind(), filer_api::ErrorKind::Corrupt);
    }
}
