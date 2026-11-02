// This is free and unencumbered software released into the public domain.

//! Partitioned disk images: their partition tables, MBR or GPT, and a
//! [`Window`] that reads one partition as a source of its own, so that a
//! driver opens it as it would a bare volume.
//!
//! An MBR names four primary partitions, numbered 1 to 4, one of which
//! may be extended: a chain of boot records naming one logical partition
//! each, numbered from 5, as Linux numbers them. Its units are 512-byte
//! sectors. A GPT is found by an MBR protecting it, and is read from its
//! header after the MBR, at 512- or 4096-byte sectors, both checked by
//! their CRC-32s; if the header or its entries fail them, the backup at
//! the end of the disk is read instead.
//!
//! # Examples
//!
//! ```
//! # filer_spec::block_on(async {
//! use filer_fs::partition::{self, Window};
//!
//! // An MBR of one partition of type 0x0C, from sector 2048.
//! let mut disk = vec![0u8; 4 << 20];
//! disk[446 + 4] = 0x0C;
//! disk[446 + 8..446 + 12].copy_from_slice(&2048u32.to_le_bytes());
//! disk[446 + 12..446 + 16].copy_from_slice(&4096u32.to_le_bytes());
//! disk[510..512].copy_from_slice(&[0x55, 0xAA]);
//! let table = partition::read(&disk).await?.unwrap();
//! let first = &table.partitions()[0];
//! assert_eq!((first.number(), first.start(), first.len()), (1, 1 << 20, 2 << 20));
//! let volume = Window::of(disk, first);
//! # Ok::<_, filer_api::Error>(())
//! # }).unwrap();
//! ```

use crate::read_exact;
use alloc::{string::String, vec::Vec};
use core::future::Future;
use filer_api::{BufResult, Error, ErrorKind, IoBufMut, OpKind, ReadAt, checksum::crc32};

/// The sector size of MBRs, and of GPTs on most disks.
const SECTOR: u64 = 512;

/// The sector sizes a GPT is looked for at.
const GPT_SECTORS: [u64; 2] = [512, 4096];

/// The most bytes of GPT entries read: 256 times the usual 128 entries of
/// 128 bytes.
const MAX_GPT_ENTRIES: u64 = 4 << 20;

/// The most logical partitions an extended partition is read for.
const MAX_LOGICAL: u32 = 256;

/// MBR partition types: an extended partition (CHS, LBA, and Linux's),
/// and a protective one, which a GPT hides behind.
const EXTENDED: [u8; 3] = [0x05, 0x0F, 0x85];
const PROTECTIVE: u8 = 0xEE;

/// What is wrong with a partition table: the source of the
/// [`Corrupt`](ErrorKind::Corrupt) error returned for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PartitionError {
    /// Neither GPT header, primary or backup, is valid: by its signature,
    /// size, CRC-32, or place; or neither one's entries match their CRC-32.
    BadGpt,

    /// A partition does not lie within the disk, or ends before it starts.
    OutOfRange {
        /// The partition's number.
        number: u32,
    },

    /// An extended partition's chain of boot records loops, goes
    /// backwards, or names more logical partitions than are read.
    BadExtended,
}

impl PartitionError {
    /// Returns a [`Corrupt`](ErrorKind::Corrupt) error from `op`, with
    /// this as its source.
    #[must_use]
    pub fn into_error(self, op: OpKind) -> Error {
        Error::new(ErrorKind::Corrupt, op).with_source(self)
    }
}

impl core::fmt::Display for PartitionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("partition table: ")?;
        match self {
            Self::BadGpt => f.write_str("no valid GPT header and entries"),
            Self::OutOfRange { number } => {
                write!(f, "partition {number} does not lie within the disk")
            },
            Self::BadExtended => f.write_str("its extended partition's chain is invalid"),
        }
    }
}

impl core::error::Error for PartitionError {}

/// A GUID, as GPT stores it: its first three fields little-endian.
/// Displayed as GUIDs are written, in uppercase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Guid(pub [u8; 16]);

impl core::fmt::Display for Guid {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let [a0, a1, a2, a3, b0, b1, c0, c1, d @ ..] = self.0;
        let [d0, d1, e @ ..] = d;
        write!(
            f,
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-",
            u32::from_le_bytes([a0, a1, a2, a3]),
            u16::from_le_bytes([b0, b1]),
            u16::from_le_bytes([c0, c1]),
            d0,
            d1
        )?;
        e.iter().try_for_each(|byte| write!(f, "{byte:02X}"))
    }
}

/// The scheme of a partition table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scheme {
    /// A master boot record: the PC's, and DOS's.
    Mbr,
    /// A GUID partition table: UEFI's.
    Gpt,
}

/// What a partition is, by its table's account.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Kind {
    /// An MBR partition of this type, such as `0x0C` for FAT32 or `0x83`
    /// for Linux.
    Mbr(u8),
    /// A GPT partition.
    Gpt {
        /// Its type, such as `C12A7328-F81F-11D2-BA4B-00A0C93EC93B` for an
        /// EFI system partition.
        kind: Guid,
        /// Its own GUID.
        id: Guid,
        /// Its name, decoded from UTF-16, unpaired surrogates replaced.
        name: String,
    },
}

/// A partition: where it is on the disk, and what it is.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Partition {
    number: u32,
    start: u64,
    len: u64,
    kind: Kind,
}

impl Partition {
    /// Returns the partition's number: an MBR's primary partitions are 1
    /// to 4 and its logical ones from 5, and a GPT's entries from 1, as
    /// Linux numbers them.
    #[must_use]
    pub const fn number(&self) -> u32 {
        self.number
    }

    /// Returns the offset of the partition's first byte on the disk.
    #[must_use]
    pub const fn start(&self) -> u64 {
        self.start
    }

    /// Returns the partition's length in bytes.
    #[must_use]
    pub const fn len(&self) -> u64 {
        self.len
    }

    /// Returns `true` if the partition is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns what the partition is.
    #[must_use]
    pub const fn kind(&self) -> &Kind {
        &self.kind
    }
}

/// A partition table.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Table {
    scheme: Scheme,
    sector_size: u64,
    partitions: Vec<Partition>,
}

impl Table {
    /// Returns the table's scheme.
    #[must_use]
    pub const fn scheme(&self) -> Scheme {
        self.scheme
    }

    /// Returns the size of the sectors the table counts in.
    #[must_use]
    pub const fn sector_size(&self) -> u64 {
        self.sector_size
    }

    /// Returns the partitions, by number, without an MBR's extended
    /// partition itself, which holds no volume.
    #[must_use]
    pub fn partitions(&self) -> &[Partition] {
        &self.partitions
    }

    /// Returns partition `number`, if there is one.
    #[must_use]
    pub fn partition(&self, number: u32) -> Option<&Partition> {
        self.partitions
            .iter()
            .find(|partition| partition.number == number)
    }
}

/// Reads the partition table of the disk image in `source`, or returns
/// `None` if it has none: no MBR signature, an MBR naming no partition or
/// with invalid boot flags, or the boot sector of a bare FAT or NTFS
/// volume, which has the same signature.
///
/// # Errors
///
/// [`Corrupt`](ErrorKind::Corrupt), with a [`PartitionError`] as its
/// source, if the table is invalid; any error reading `source`.
pub async fn read<S: ReadAt + ?Sized>(source: &S) -> Result<Option<Table>, Error> {
    let op = OpKind::Open;
    let size = source.size().await?;
    if size < SECTOR {
        return Ok(None);
    }
    let mbr = read_exact(source, 0, 512, op).await?;
    let Some(entries) = mbr_entries(&mbr) else {
        return Ok(None);
    };
    if entries.iter().any(|entry| entry.kind == PROTECTIVE) {
        return gpt(source, size, op).await.map(Some);
    }
    let sectors = size / SECTOR;
    let mut partitions = Vec::new();
    for (number, entry) in (1..).zip(&entries) {
        if entry.kind == 0 {
            continue;
        }
        check(entry.start, entry.len, sectors, number)?;
        if EXTENDED.contains(&entry.kind) {
            logical(source, entry, sectors, &mut partitions, op).await?;
        } else {
            partitions.push(mbr_partition(number, entry));
        }
    }
    partitions.sort_by_key(Partition::number);
    Ok(Some(Table {
        scheme: Scheme::Mbr,
        sector_size: SECTOR,
        partitions,
    }))
}

/// An MBR entry: its type, and its first sector and count of sectors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    kind: u8,
    start: u64,
    len: u64,
}

/// Decodes the four entries of boot record `sector`, or returns `None` if
/// it is no partition table.
fn mbr_entries(sector: &[u8]) -> Option<[Entry; 4]> {
    if sector.get(510..512) != Some(&[0x55, 0xAA]) {
        return None;
    }
    // A volume's boot sector has the same signature.
    let is_volume = sector.get(3..11) == Some(b"NTFS    ")
        || sector.get(54..59).is_some_and(|id| id.starts_with(b"FAT1"))
        || sector.get(82..87) == Some(b"FAT32");
    if is_volume {
        return None;
    }
    let mut entries = [Entry {
        kind: 0,
        start: 0,
        len: 0,
    }; 4];
    for (i, entry) in entries.iter_mut().enumerate() {
        let at = i.checked_mul(16)?.checked_add(446)?;
        let raw = sector.get(at..at.checked_add(16)?)?;
        let status = *raw.first()?;
        if status != 0 && status != 0x80 {
            return None;
        }
        *entry = Entry {
            kind: *raw.get(4)?,
            start: u64::from(crate::le::u32_at(raw, 8)?),
            len: u64::from(crate::le::u32_at(raw, 12)?),
        };
    }
    entries
        .iter()
        .any(|entry| entry.kind != 0)
        .then_some(entries)
}

/// Returns partition `number` of MBR entry `entry`, at its sectors.
fn mbr_partition(number: u32, entry: &Entry) -> Partition {
    Partition {
        number,
        // Within the disk, as `check` found.
        start: entry.start.saturating_mul(SECTOR),
        len: entry.len.saturating_mul(SECTOR),
        kind: Kind::Mbr(entry.kind),
    }
}

/// Checks that partition `number` of `len` sectors from `start` lies
/// within a disk of `sectors`.
fn check(start: u64, len: u64, sectors: u64, number: u32) -> Result<(), Error> {
    let fits = start
        .checked_add(len)
        .is_some_and(|end| start > 0 && end <= sectors);
    if fits {
        Ok(())
    } else {
        Err(PartitionError::OutOfRange { number }.into_error(OpKind::Open))
    }
}

/// Reads the logical partitions of extended partition `extended`, by the
/// chain of boot records from its first sector, onto `partitions`.
async fn logical<S: ReadAt + ?Sized>(
    source: &S,
    extended: &Entry,
    sectors: u64,
    partitions: &mut Vec<Partition>,
    op: OpKind,
) -> Result<(), Error> {
    let bad = || PartitionError::BadExtended.into_error(op);
    let end = extended.start.saturating_add(extended.len);
    let mut ebr = extended.start;
    for number in 5..5_u32.saturating_add(MAX_LOGICAL) {
        let at = ebr.checked_mul(SECTOR).ok_or_else(bad)?;
        let sector = read_exact(source, at, 512, op).await?;
        let [logical, next, ..] = mbr_entries(&sector).ok_or_else(bad)?;
        if logical.kind != 0 {
            // Relative to its boot record, and within the extended one.
            let start = ebr.checked_add(logical.start).ok_or_else(bad)?;
            check(start, logical.len, sectors, number)?;
            if start.saturating_add(logical.len) > end {
                return Err(PartitionError::OutOfRange { number }.into_error(op));
            }
            let entry = Entry { start, ..logical };
            partitions.push(mbr_partition(number, &entry));
        }
        if next.kind == 0 {
            return Ok(());
        }
        // Relative to the extended partition, and further on, so that the
        // chain cannot loop.
        let following = extended.start.checked_add(next.start).ok_or_else(bad)?;
        if following <= ebr || following >= end {
            return Err(bad());
        }
        ebr = following;
    }
    Err(bad())
}

/// Reads the GPT of a disk of `size` bytes, by its primary header, or
/// else its backup.
async fn gpt<S: ReadAt + ?Sized>(source: &S, size: u64, op: OpKind) -> Result<Table, Error> {
    for sector in GPT_SECTORS {
        let sectors = size.checked_div(sector).unwrap_or(0);
        if sectors < 3 {
            continue;
        }
        for lba in [1, sectors.saturating_sub(1)] {
            if let Some(table) = gpt_at(source, sector, sectors, lba, op).await? {
                return Ok(table);
            }
        }
    }
    Err(PartitionError::BadGpt.into_error(op))
}

/// Reads the GPT whose header is at `lba`, of sectors of `sector` bytes,
/// on a disk of `sectors`, or returns `None` if the header or its entries
/// are invalid.
async fn gpt_at<S: ReadAt + ?Sized>(
    source: &S,
    sector: u64,
    sectors: u64,
    lba: u64,
    op: OpKind,
) -> Result<Option<Table>, Error> {
    let len = usize::try_from(sector).unwrap_or(usize::MAX);
    let Some(at) = lba.checked_mul(sector) else {
        return Ok(None);
    };
    let header = read_exact(source, at, len, op).await?;
    let Some(header) = GptHeader::parse(&header, lba) else {
        return Ok(None);
    };
    let Some(entries_at) = header.entries_lba.checked_mul(sector) else {
        return Ok(None);
    };
    let entries_len = u64::from(header.count).saturating_mul(u64::from(header.entry_size));
    let fits = entries_at
        .checked_add(entries_len)
        .is_some_and(|end| end <= sectors.saturating_mul(sector));
    if entries_len > MAX_GPT_ENTRIES || !fits {
        return Ok(None);
    }
    let len = usize::try_from(entries_len).unwrap_or(usize::MAX);
    let entries = read_exact(source, entries_at, len, op).await?;
    if crc32(&entries) != header.entries_crc {
        return Ok(None);
    }
    let size = usize::try_from(header.entry_size).unwrap_or(usize::MAX);
    let mut partitions = Vec::new();
    for (number, entry) in (1..).zip(entries.chunks_exact(size)) {
        let kind = Guid(entry.first_chunk().copied().unwrap_or_default());
        if kind == Guid::default() {
            continue;
        }
        let first = crate::le::u64_at(entry, 32).unwrap_or(0);
        let last = crate::le::u64_at(entry, 40).unwrap_or(0);
        let len = last
            .checked_add(1)
            .and_then(|end| end.checked_sub(first))
            .ok_or_else(|| PartitionError::OutOfRange { number }.into_error(op))?;
        check(first, len, sectors, number)?;
        let id = entry
            .get(16..32)
            .and_then(|id| id.try_into().ok())
            .map(Guid)
            .unwrap_or_default();
        let units: Vec<u16> = entry
            .get(56..128)
            .unwrap_or_default()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&unit| u16::from_le_bytes(unit))
            .take_while(|&unit| unit != 0)
            .collect();
        let name = char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect();
        partitions.push(Partition {
            number,
            start: first.saturating_mul(sector),
            len: len.saturating_mul(sector),
            kind: Kind::Gpt { kind, id, name },
        });
    }
    Ok(Some(Table {
        scheme: Scheme::Gpt,
        sector_size: sector,
        partitions,
    }))
}

/// What a GPT header says of its entries.
struct GptHeader {
    entries_lba: u64,
    count: u32,
    entry_size: u32,
    entries_crc: u32,
}

impl GptHeader {
    /// Parses the header in `sector`, which must be valid and say it is at
    /// `lba`.
    fn parse(sector: &[u8], lba: u64) -> Option<Self> {
        use crate::le::{u32_at, u64_at};
        if sector.get(..8)? != b"EFI PART" {
            return None;
        }
        let size = usize::try_from(u32_at(sector, 12)?).ok()?;
        if size < 92 || size > sector.len() {
            return None;
        }
        let mut header = sector.get(..size)?.to_vec();
        header.get_mut(16..20)?.fill(0);
        if crc32(&header) != u32_at(sector, 16)? || u64_at(sector, 24)? != lba {
            return None;
        }
        let entry_size = u32_at(sector, 84)?;
        if entry_size < 128 || !entry_size.is_multiple_of(8) {
            return None;
        }
        Some(Self {
            entries_lba: u64_at(sector, 72)?,
            count: u32_at(sector, 80)?,
            entry_size,
            entries_crc: u32_at(sector, 88)?,
        })
    }
}

/// A source that is a range of another: `len` bytes from `start`, such
/// as a partition of a disk image.
#[derive(Clone, Debug)]
pub struct Window<S> {
    source: S,
    start: u64,
    len: u64,
}

impl<S> Window<S> {
    /// Returns the `len` bytes of `source` from `start`.
    #[must_use]
    pub const fn new(source: S, start: u64, len: u64) -> Self {
        Self { source, start, len }
    }

    /// Returns partition `partition` of the disk image in `source`.
    #[must_use]
    pub const fn of(source: S, partition: &Partition) -> Self {
        Self::new(source, partition.start, partition.len)
    }

    /// Returns the source the window is of.
    pub fn into_inner(self) -> S {
        self.source
    }
}

impl<S: ReadAt> ReadAt for Window<S> {
    async fn read_at<B: IoBufMut>(&self, mut buf: B, off: u64) -> BufResult<usize, B> {
        let left = self.len.saturating_sub(off);
        let Some(at) = self.start.checked_add(off).filter(|_| left > 0) else {
            return BufResult(Ok(0), buf);
        };
        let spare = u64::try_from(buf.spare()).unwrap_or(u64::MAX);
        if spare <= left {
            return self.source.read_at(buf, at).await;
        }
        // Past the window's end: read no more than it holds.
        let len = usize::try_from(left).unwrap_or(usize::MAX);
        let BufResult(result, read) = self.source.read_at(Vec::with_capacity(len), at).await;
        BufResult(result.map(|_| buf.fill_from(&read)), buf)
    }

    fn size(&self) -> impl Future<Output = Result<u64, Error>> + Send {
        core::future::ready(Ok(self.len))
    }
}
