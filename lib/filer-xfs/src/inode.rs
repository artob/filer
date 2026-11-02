// This is free and unencumbered software released into the public domain.

//! Inodes: finding them by number, and decoding their cores and forks.

use crate::{
    XfsError, be, crc, io,
    superblock::{Superblock, Version},
};
use alloc::vec::Vec;
use core::time::Duration;
use filer_api::{Error, FileAttrs, FileKind, OpKind, ReadAt, Timestamp};

/// An inode's magic number, `IN`.
const MAGIC: u16 = 0x494E;

/// The size of a version 1 or 2 inode's core, which the forks follow in
/// the literal area, and of a version 3 inode's.
const CORE_SIZE: usize = 100;
const CORE_SIZE_V3: usize = 176;

/// Where a version 3 inode keeps its checksum, its own number, and the
/// metadata UUID.
const CRC_AT: usize = 100;
const NUMBER_AT: usize = 152;
const UUID_AT: usize = 160;

/// The size of an extent record in an extent list.
pub(crate) const EXTENT_SIZE: usize = 16;

/// `di_flags`: the file cannot change, and can only be appended to.
const IMMUTABLE: u16 = 0x8;
const APPEND: u16 = 0x10;

/// `di_flags2` of version 3 inodes: the file is accessed directly; its
/// times are of the `BIGTIME` feature; and its extent counts of the
/// `NREXT64` feature.
const DAX: u64 = 0x1;
const BIGTIME: u64 = 0x8;
const NREXT64: u64 = 0x10;

/// The most extents a data fork and an attribute fork map with `NREXT64`.
const MAX_EXTENTS: u64 = (1 << 48) - 1;
const MAX_ATTR_EXTENTS: u64 = (1 << 32) - 1;

/// What a `BIGTIME` time counts from, in seconds before the Unix epoch:
/// 1901-12-13T20:45:52Z, the earliest legacy time.
const BIGTIME_EPOCH: i64 = 1 << 31;

/// Times are recorded to the nanosecond.
const NANOSECONDS: Duration = Duration::from_nanos(1);

/// How a fork stores what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    /// A device number: devices, FIFOs, and sockets.
    Device,
    /// In the inode itself: short-form directories, symlink targets, and
    /// attributes.
    Local,
    /// A list of extent records in the inode.
    Extents,
    /// The root of a B+ tree of extent records.
    Btree,
}

impl Format {
    /// Decodes `di_format` or `di_aformat`. Later formats (UUID, and the
    /// metadata B+ trees of version 5) hold no file data.
    const fn parse(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(Self::Device),
            1 => Some(Self::Local),
            2 => Some(Self::Extents),
            3 => Some(Self::Btree),
            _ => None,
        }
    }
}

/// An inode's data or attribute fork, as the inode holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fork {
    /// How the fork stores its contents.
    pub(crate) format: Format,
    /// The number of extents the fork maps, in its list or under its B+
    /// tree.
    pub(crate) extents: u64,
    /// The fork's bytes: a local data fork's contents (its file's size),
    /// an extent list's records, or the whole area for anything else.
    pub(crate) bytes: Vec<u8>,
}

/// An inode's four times. Creation is recorded only by version 3 inodes.
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
    number: u64,
    kind: FileKind,
    size: u64,
    links: u32,
    /// `di_nblocks`: blocks in use for data and its map.
    blocks: u64,
    /// `di_flags`.
    flags: u16,
    /// `di_flags2`; zero before version 3.
    flags2: u64,
    times: Times,
    data: Fork,
    attr: Option<Fork>,
}

/// Decodes a time: legacy, as signed 32-bit seconds and nanoseconds, or,
/// if `big`, as unsigned 64-bit nanoseconds since 1901-12-13T20:45:52Z.
fn timestamp(bytes: &[u8], at: usize, big: bool) -> Option<Timestamp> {
    if big {
        let since = u64::from_be_bytes(*bytes.get(at..)?.first_chunk()?);
        let secs = i64::try_from(since.checked_div(1_000_000_000)?).ok()?;
        let nanos = u32::try_from(since.checked_rem(1_000_000_000)?).ok()?;
        let secs = secs.checked_sub(BIGTIME_EPOCH)?;
        return Some(Timestamp::new(secs, nanos).with_granularity(NANOSECONDS));
    }
    let secs = i32::from_be_bytes(*bytes.get(at..)?.first_chunk()?);
    let nanos = u32::try_from(i32::from_be_bytes(
        *bytes.get(at.checked_add(4)?..)?.first_chunk()?,
    ))
    .ok()
    .filter(|&n| n < 1_000_000_000)?;
    Some(Timestamp::new(i64::from(secs), nanos).with_granularity(NANOSECONDS))
}

impl Inode {
    /// Decodes inode `number` from its record, on a volume of `version`.
    pub(crate) fn parse(bytes: &[u8], number: u64, version: Version) -> Result<Self, XfsError> {
        let bad = XfsError::BadInode { inode: number };
        let u8_at = |at: usize| bytes.get(at).copied().ok_or(bad);
        let u16_at = |at| be::u16_at(bytes, at).ok_or(bad);
        let u32_at = |at| be::u32_at(bytes, at).ok_or(bad);
        let u64_at = |at| be::u64_at(bytes, at).ok_or(bad);
        if u16_at(0)? != MAGIC {
            return Err(bad);
        }
        let inode_version = u8_at(4)?;
        let links = match (version, inode_version) {
            // Version 1 inodes count links in 16 bits.
            (Version::V4, 1) => u32::from(u16_at(6)?),
            (Version::V4, 2) | (Version::V5, 3) => u32_at(16)?,
            _ => return Err(bad),
        };
        let v3 = inode_version == 3;
        let flags2 = if v3 { u64_at(120)? } else { 0 };
        if v3 && u64_at(NUMBER_AT)? != number {
            return Err(bad);
        }
        let mode = u16_at(2)?;
        if mode == 0 || links == 0 {
            return Err(XfsError::UnusedInode { inode: number });
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
        let size = u64_at(56)?;
        if i64::try_from(size).is_err() {
            return Err(bad);
        }
        let time = |at| timestamp(bytes, at, flags2 & BIGTIME != 0).ok_or(bad);
        let times = Times {
            accessed: time(32)?,
            modified: time(40)?,
            changed: time(48)?,
            created: if v3 { Some(time(144)?) } else { None },
        };

        // The literal area after the core holds the data fork, then, from
        // `di_forkoff` 8-byte units on, the attribute fork.
        let core = if v3 { CORE_SIZE_V3 } else { CORE_SIZE };
        let literal = bytes.get(core..).ok_or(bad)?;
        let fork_off = usize::from(u8_at(82)?).checked_mul(8).ok_or(bad)?;
        let (data_area, attr_area) = match fork_off {
            0 => (literal, None),
            off if off < literal.len() => {
                let (data, attr) = literal.split_at_checked(off).ok_or(bad)?;
                (data, Some(attr))
            },
            _ => return Err(bad),
        };
        let format = Format::parse(u8_at(5)?).ok_or(bad)?;
        let fitting = match (kind, format) {
            (FileKind::File, Format::Extents | Format::Btree)
            | (FileKind::Dir, Format::Local | Format::Extents | Format::Btree)
            | (FileKind::Symlink, Format::Local | Format::Extents) => true,
            (_, Format::Device) => matches!(
                kind,
                FileKind::Fifo | FileKind::Socket | FileKind::CharDevice | FileKind::BlockDevice
            ),
            _ => false,
        };
        if !fitting {
            return Err(bad);
        }
        // Signed 32-bit and 16-bit counts, or with `NREXT64` 48-bit and
        // 32-bit ones in wider fields.
        let (extents, attr_extents) = if flags2 & NREXT64 == 0 {
            let counts = (u64::from(u32_at(76)?), u64::from(u16_at(80)?));
            let limits = (i32::MAX.unsigned_abs(), i16::MAX.unsigned_abs());
            if counts.0 > u64::from(limits.0) || counts.1 > u64::from(limits.1) {
                return Err(bad);
            }
            counts
        } else {
            let counts = (u64_at(24)?, u64::from(u32_at(76)?));
            if counts.0 > MAX_EXTENTS || counts.1 > MAX_ATTR_EXTENTS {
                return Err(bad);
            }
            counts
        };
        let data = fork(data_area, format, extents, size).ok_or(bad)?;
        let attr = match attr_area {
            None => None,
            Some(area) => {
                let format = Format::parse(u8_at(83)?)
                    .filter(|&format| format != Format::Device)
                    .ok_or(bad)?;
                Some(fork(area, format, attr_extents, u64::MAX).ok_or(bad)?)
            },
        };
        Ok(Self {
            number,
            kind,
            size,
            links,
            blocks: u64_at(64)?,
            flags: u16_at(90)?,
            flags2,
            times,
            data,
            attr,
        })
    }

    pub(crate) const fn number(&self) -> u64 {
        self.number
    }

    /// Returns the file's kind, from its mode.
    pub(crate) const fn kind(&self) -> FileKind {
        self.kind
    }

    /// Returns the size in bytes, at most `i64::MAX`.
    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn links(&self) -> u32 {
        self.links
    }

    /// Returns the number of blocks the file uses for data and its map.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn blocks(&self) -> u64 {
        self.blocks
    }

    /// Returns `di_flags`.
    pub(crate) const fn flags(&self) -> u16 {
        self.flags
    }

    /// Returns the flags Filer reports: immutable, append-only, and DAX.
    pub(crate) fn attrs(&self) -> FileAttrs {
        let mut attrs = FileAttrs::empty();
        attrs.set(FileAttrs::IMMUTABLE, self.flags & IMMUTABLE != 0);
        attrs.set(FileAttrs::APPEND_ONLY, self.flags & APPEND != 0);
        attrs.set(FileAttrs::DAX, self.flags2 & DAX != 0);
        attrs
    }

    pub(crate) const fn times(&self) -> &Times {
        &self.times
    }

    /// Returns the data fork.
    pub(crate) const fn data(&self) -> &Fork {
        &self.data
    }

    /// Returns the attribute fork, if the inode has one.
    pub(crate) const fn attr(&self) -> Option<&Fork> {
        self.attr.as_ref()
    }
}

/// Returns the fork in `area` of `format`, mapping `extents` extents, of a
/// file of `size` bytes (`u64::MAX` for an attribute fork), if it fits.
fn fork(area: &[u8], format: Format, extents: u64, size: u64) -> Option<Fork> {
    let bytes = match format {
        // Local data is exactly the file's size; local attributes say
        // their own.
        Format::Local if size != u64::MAX => area.get(..usize::try_from(size).ok()?)?,
        Format::Extents => {
            let len = usize::try_from(extents).ok()?.checked_mul(EXTENT_SIZE)?;
            area.get(..len)?
        },
        _ => area,
    };
    Some(Fork {
        format,
        extents,
        bytes: bytes.to_vec(),
    })
}

/// Reads inode `number`.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    number: u64,
    op: OpKind,
) -> Result<Inode, Error> {
    let off = sb
        .inode_offset(number)
        .ok_or_else(|| XfsError::BadInodeNumber(number).into_error(op))?;
    let len = usize::try_from(sb.inode_size()).unwrap_or(usize::MAX);
    let bytes = io::read_exact(source, off, len).await?;
    check(&bytes, sb, off).map_err(|error| error.into_error(op))?;
    Inode::parse(&bytes, number, sb.version()).map_err(|error| error.into_error(op))
}

/// Checks a version 3 inode's checksum, over the whole inode, and that it
/// carries the metadata UUID, as read from byte `offset`. An inode never
/// written has neither, but is of version 3 only once it has been.
fn check(bytes: &[u8], sb: &Superblock, offset: u64) -> Result<(), XfsError> {
    if !sb.is_v5() || bytes.get(4) != Some(&3) {
        return Ok(());
    }
    if !crc::is_valid(bytes, CRC_AT) {
        return Err(XfsError::BadChecksum { offset });
    }
    if bytes.get(UUID_AT..UUID_AT.saturating_add(16)) != Some(sb.meta_uuid()) {
        return Err(XfsError::Misplaced { offset });
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::superblock::tests::Fields;
    use alloc::vec;
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// An inode's fields, for building test volumes.
    #[derive(Clone)]
    pub(crate) struct Raw {
        pub(crate) magic: u16,
        pub(crate) mode: u16,
        pub(crate) version: u8,
        pub(crate) format: u8,
        pub(crate) links: u32,
        /// Seconds and nanoseconds of the access, modification, and
        /// change times.
        pub(crate) times: [(i32, i32); 3],
        pub(crate) size: u64,
        pub(crate) blocks: u64,
        pub(crate) extents: u32,
        pub(crate) attr_extents: u16,
        pub(crate) fork_off: u8,
        pub(crate) attr_format: u8,
        pub(crate) flags: u16,
        /// The literal area: the data fork, then the attribute fork.
        pub(crate) literal: Vec<u8>,
    }

    impl Raw {
        /// A regular file of `size` bytes, with no extents.
        pub(crate) fn file(size: u64) -> Self {
            Self {
                magic: MAGIC,
                mode: 0o100_644,
                version: 2,
                format: 2,
                links: 1,
                times: [(1_700_000_000, 1), (1_700_000_001, 2), (1_700_000_002, 3)],
                size,
                blocks: 0,
                extents: 0,
                attr_extents: 0,
                fork_off: 0,
                attr_format: 2,
                flags: 0,
                literal: Vec::new(),
            }
        }

        /// A directory or symlink of `mode` stored in the inode.
        pub(crate) fn local(mode: u16, contents: &[u8]) -> Self {
            let mut raw = Self::file(u64::try_from(contents.len()).unwrap());
            raw.mode = mode;
            raw.format = 1;
            raw.literal = contents.to_vec();
            raw
        }

        /// The record, `size` bytes long.
        pub(crate) fn bytes(&self, size: usize) -> Vec<u8> {
            let mut b = vec![0_u8; size];
            let mut put = |at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
            put(0, &self.magic.to_be_bytes());
            put(2, &self.mode.to_be_bytes());
            put(4, &[self.version, self.format]);
            let links = u16::try_from(self.links).unwrap_or(u16::MAX);
            put(6, &if self.version == 1 { links } else { 0 }.to_be_bytes());
            put(16, &self.links.to_be_bytes());
            for (i, (secs, nanos)) in self.times.iter().enumerate() {
                put(32 + 8 * i, &secs.to_be_bytes());
                put(36 + 8 * i, &nanos.to_be_bytes());
            }
            put(56, &self.size.to_be_bytes());
            put(64, &self.blocks.to_be_bytes());
            put(76, &self.extents.to_be_bytes());
            put(80, &self.attr_extents.to_be_bytes());
            put(82, &[self.fork_off, self.attr_format]);
            put(90, &self.flags.to_be_bytes());
            put(96, &u32::MAX.to_be_bytes());
            put(CORE_SIZE, &self.literal);
            b
        }

        fn parse(&self) -> Result<Inode, XfsError> {
            Inode::parse(&self.bytes(256), 128, Version::V4)
        }
    }

    #[test]
    fn decodes_a_file() {
        let mut raw = Raw::file(5000);
        raw.blocks = 2;
        raw.extents = 2;
        raw.flags = 0x18;
        raw.literal = (0..32).collect();
        let inode = raw.parse().unwrap();
        assert_eq!(inode.number(), 128);
        assert_eq!(inode.kind(), FileKind::File);
        assert_eq!((inode.size(), inode.links(), inode.blocks()), (5000, 1, 2));
        assert_eq!(inode.flags(), 0x18);
        assert_eq!(inode.attrs(), FileAttrs::IMMUTABLE | FileAttrs::APPEND_ONLY);
        let times = inode.times();
        let at = |secs, nanos| Timestamp::new(secs, nanos).with_granularity(NANOSECONDS);
        assert_eq!(times.accessed, at(1_700_000_000, 1));
        assert_eq!(times.modified, at(1_700_000_001, 2));
        assert_eq!(times.changed, at(1_700_000_002, 3));
        assert_eq!(times.created, None);
        let data = inode.data();
        assert_eq!((data.format, data.extents), (Format::Extents, 2));
        assert_eq!(data.bytes, (0..32).collect::<Vec<u8>>());
        assert_eq!(inode.attr(), None);
    }

    /// A version 3 inode of `Raw::file(size)`, as inode 128, with
    /// `flags2`, and `literal` after its 176-byte core.
    fn v3(size: u64, flags2: u64, literal: &[u8]) -> Vec<u8> {
        let mut raw = Raw::file(size);
        raw.version = 3;
        let mut b = raw.bytes(512);
        b[120..128].copy_from_slice(&flags2.to_be_bytes());
        b[152..160].copy_from_slice(&128u64.to_be_bytes());
        b[176..176 + literal.len()].copy_from_slice(literal);
        b
    }

    #[test]
    fn decodes_version_3_inodes() {
        let at = |secs, nanos| Timestamp::new(secs, nanos).with_granularity(NANOSECONDS);
        // `BIGTIME` and `NREXT64`, with times as `xfs_db` showed them, of
        // 2024-01-02T03:04:06.123456789Z, 1901-12-13T20:56:40.000000005Z,
        // and 2400-01-01T00:00:00.000000007Z; and two extents.
        let mut b = v3(8192, 0x18, &[0x11; 32]);
        b[32..40].copy_from_slice(&0x3573_CDB7_72F7_C915_u64.to_be_bytes());
        b[40..48].copy_from_slice(&0x3573_CDB7_72F7_C915_u64.to_be_bytes());
        b[48..56].copy_from_slice(&0x0096_DFCF_5005_u64.to_be_bytes());
        b[144..152].copy_from_slice(&0xDA1D_D211_AFE2_0007_u64.to_be_bytes());
        b[24..32].copy_from_slice(&2u64.to_be_bytes());
        let inode = Inode::parse(&b, 128, Version::V5).unwrap();
        let times = inode.times();
        assert_eq!(times.accessed, at(1_704_164_646, 123_456_789));
        assert_eq!(times.modified, at(1_704_164_646, 123_456_789));
        assert_eq!(times.changed, at(-2_147_483_000, 5));
        assert_eq!(times.created, Some(at(13_569_465_600, 7)));
        assert_eq!((inode.data().extents, inode.data().bytes.len()), (2, 32));
        assert_eq!(inode.attrs(), FileAttrs::empty());

        // Legacy times and counts, the creation time at 144; and DAX.
        let mut b = v3(8192, 0x1, &[0x11; 16]);
        b[76..80].copy_from_slice(&1u32.to_be_bytes());
        b[144..152].copy_from_slice(&[0, 0, 0, 5, 0, 0, 0, 6]);
        let inode = Inode::parse(&b, 128, Version::V5).unwrap();
        assert_eq!(inode.times().created, Some(at(5, 6)));
        assert_eq!(inode.data().extents, 1);
        assert_eq!(inode.attrs(), FileAttrs::DAX);

        // Another inode's number, more extents than `NREXT64` allows, or a
        // version 3 inode on version 4.
        let bad = Err(XfsError::BadInode { inode: 129 });
        assert_eq!(Inode::parse(&v3(0, 0, &[]), 129, Version::V5), bad);
        let mut b = v3(8192, 0x10, &[]);
        b[24..32].copy_from_slice(&(1u64 << 48).to_be_bytes());
        assert!(Inode::parse(&b, 128, Version::V5).is_err());
        assert!(Inode::parse(&v3(0, 0, &[]), 128, Version::V4).is_err());
    }

    #[test]
    fn decodes_kinds_and_formats() {
        let dir = Raw::local(0o40_755, b"\x00\x00\x00\x00\x00\x80").parse();
        let dir = dir.unwrap();
        assert_eq!(dir.kind(), FileKind::Dir);
        assert_eq!(dir.data().format, Format::Local);
        assert_eq!(dir.data().bytes, b"\x00\x00\x00\x00\x00\x80");

        let link = Raw::local(0o120_777, b"target").parse().unwrap();
        assert_eq!(link.kind(), FileKind::Symlink);
        assert_eq!(link.data().bytes, b"target");

        for (mode, kind) in [
            (0o10_644, FileKind::Fifo),
            (0o20_644, FileKind::CharDevice),
            (0o60_644, FileKind::BlockDevice),
            (0o140_755, FileKind::Socket),
        ] {
            let mut raw = Raw::file(0);
            raw.mode = mode;
            raw.format = 0;
            let inode = raw.parse().unwrap();
            assert_eq!(inode.kind(), kind);
            assert_eq!(inode.data().format, Format::Device);
        }

        let mut raw = Raw::file(1 << 40);
        raw.format = 3;
        assert_eq!(raw.parse().unwrap().data().format, Format::Btree);
        assert_eq!(raw.parse().unwrap().data().bytes.len(), 156);
    }

    #[test]
    fn reads_version_1_link_counts() {
        let mut raw = Raw::file(0);
        raw.version = 1;
        raw.links = 3;
        assert_eq!(raw.parse().unwrap().links(), 3);
    }

    #[test]
    fn decodes_times_before_1970() {
        let mut raw = Raw::file(0);
        raw.times[1] = (-1, 999_999_999);
        let inode = raw.parse().unwrap();
        assert_eq!(
            inode.times().modified,
            Timestamp::new(-1, 999_999_999).with_granularity(NANOSECONDS)
        );
    }

    #[test]
    fn splits_the_attribute_fork() {
        let mut raw = Raw::local(0o120_777, b"target");
        raw.literal.resize(120, 0);
        raw.literal.extend_from_slice(b"attributes");
        raw.fork_off = 15;
        raw.attr_format = 1;
        let inode = raw.parse().unwrap();
        assert_eq!(inode.data().bytes, b"target");
        let attr = inode.attr().unwrap();
        assert_eq!(attr.format, Format::Local);
        assert_eq!(attr.bytes.len(), 156 - 120);
        assert!(attr.bytes.starts_with(b"attributes"));

        raw.attr_format = 2;
        raw.attr_extents = 2;
        let inode = raw.parse().unwrap();
        assert_eq!(inode.attr().unwrap().bytes.len(), 32);
    }

    #[test]
    fn refuses_unused_inodes() {
        let mut raw = Raw::file(0);
        raw.mode = 0;
        assert_eq!(raw.parse(), Err(XfsError::UnusedInode { inode: 128 }));
        let mut raw = Raw::file(0);
        raw.links = 0;
        assert_eq!(raw.parse(), Err(XfsError::UnusedInode { inode: 128 }));
    }

    #[test]
    fn rejects_bad_inodes() {
        let check = |change: fn(&mut Raw)| {
            let mut raw = Raw::file(0);
            change(&mut raw);
            assert_eq!(raw.parse(), Err(XfsError::BadInode { inode: 128 }));
        };
        check(|r| r.magic = 0x494F);
        check(|r| r.version = 3);
        check(|r| r.version = 0);
        check(|r| r.mode = 0o170_644);
        check(|r| r.times[2].1 = 1_000_000_000);
        check(|r| r.times[0].1 = -1);
        check(|r| r.size = 1 << 63);
        // Formats that do not fit the kind, or are unknown.
        check(|r| r.format = 1);
        check(|r| r.format = 0);
        check(|r| r.format = 4);
        check(|r| {
            r.mode = 0o120_777;
            r.format = 3;
        });
        check(|r| {
            r.mode = 0o10_644;
            r.format = 2;
        });
        // Forks that do not fit the inode.
        check(|r| r.extents = 10);
        check(|r| r.extents = 1 << 31);
        check(|r| {
            r.mode = 0o40_755;
            r.format = 1;
            r.size = 157;
        });
        check(|r| r.fork_off = 20);
        check(|r| {
            r.fork_off = 15;
            r.attr_format = 0;
        });
        check(|r| {
            r.fork_off = 15;
            r.attr_extents = 3;
        });
        check(|r| {
            r.fork_off = 15;
            r.attr_extents = 0x8000;
        });
    }

    #[test]
    fn reads_from_a_source() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let mut image = vec![0; 4096 * 1024];
        // Inode 129: slot 1 of block 8.
        let at = 8 * 4096 + 256;
        image[at..at + 256].copy_from_slice(&Raw::file(7).bytes(256));
        let inode = block_on(read(&image, &sb, 129, OpKind::Stat)).unwrap();
        assert_eq!((inode.number(), inode.size()), (129, 7));

        let error = block_on(read(&image, &sb, 130, OpKind::Stat)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let error = block_on(read(&image, &sb, 4 << 12, OpKind::Stat)).unwrap_err();
        let source =
            core::error::Error::source(&error).and_then(|source| source.downcast_ref::<XfsError>());
        assert_eq!(source, Some(&XfsError::BadInodeNumber(4 << 12)));
    }
}
