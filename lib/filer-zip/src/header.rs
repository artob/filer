// This is free and unencumbered software released into the public domain.

//! Parsing of ZIP records and extra fields: pure functions over byte
//! slices, kept apart from I/O so that they can be fuzzed. Field layouts
//! follow PKWARE's APPNOTE.TXT, version 6.3.10; all numbers are
//! little-endian.

use alloc::vec::Vec;
use core::time::Duration;
use filer_api::{FileKind, Timestamp, checksum::crc32};
use filer_msft::{dos, nt};
use known_codepages::CodePage;

/// The end of central directory record, without its comment.
pub(crate) const EOCD_LEN: usize = 22;
const EOCD_SIG: u32 = 0x0605_4b50;

/// The longest archive comment, which follows the record.
pub(crate) const MAX_COMMENT: usize = 0xFFFF;

/// The ZIP64 end of central directory locator, just before the record.
pub(crate) const LOCATOR_LEN: usize = 20;
const LOCATOR_SIG: u32 = 0x0706_4b50;

/// The ZIP64 end of central directory record, without its extensible
/// data sector, which is ignored.
pub(crate) const EOCD64_LEN: usize = 56;
const EOCD64_SIG: u32 = 0x0606_4b50;

/// A central directory file header, before its name, extra field, and
/// comment.
pub(crate) const CENTRAL_LEN: usize = 46;
const CENTRAL_SIG: u32 = 0x0201_4b50;

/// A local file header, before its name and extra field.
pub(crate) const LOCAL_LEN: usize = 30;
const LOCAL_SIG: u32 = 0x0403_4b50;

/// General purpose flag bits.
const ENCRYPTED: u16 = 1 << 0;
const DATA_DESCRIPTOR: u16 = 1 << 3;
const STRONG_ENCRYPTION: u16 = 1 << 6;
/// Names and comments in UTF-8 (APPNOTE appendix D).
const UTF8: u16 = 1 << 11;

/// Extra field tags.
const ZIP64: u16 = 0x0001;
const NTFS: u16 = 0x000a;
const EXTENDED_TIMESTAMP: u16 = 0x5455;
const UNICODE_PATH: u16 = 0x7075;

/// Hosts in "version made by" whose external attributes hold a Unix mode
/// in their high 16 bits: Unix and OS X.
const UNIX_HOSTS: [u8; 2] = [3, 19];

/// Hosts in "version made by" whose names are in a DOS code page, as
/// Info-ZIP reads them: MS-DOS, OS/2's HPFS, NTFS, and VFAT.
const DOS_HOSTS: [u8; 4] = [0, 6, 11, 14];

/// The MS-DOS directory attribute.
const DOS_DIRECTORY: u32 = 0x10;

/// A little-endian reader over a slice that never panics.
struct Fields<'a>(&'a [u8]);

impl<'a> Fields<'a> {
    fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let (head, tail) = self.0.split_at_checked(n)?;
        self.0 = tail;
        Some(head)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.bytes(N)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        self.array().map(u8::from_le_bytes)
    }

    fn u16(&mut self) -> Option<u16> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Option<u64> {
        self.array().map(u64::from_le_bytes)
    }
}

/// Where the central directory lies and how many headers it holds, from
/// the end of central directory record and, for ZIP64, its locator and
/// record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Eocd {
    /// The number of this disk; nonzero only in spanned archives.
    pub(crate) disk: u32,
    /// The disk the central directory starts on.
    pub(crate) cd_disk: u32,
    /// The headers on this disk, which equals `entries` unless spanned.
    pub(crate) disk_entries: u64,
    /// The headers in the central directory.
    pub(crate) entries: u64,
    /// The central directory's size in bytes.
    pub(crate) cd_size: u64,
    /// The central directory's offset.
    pub(crate) cd_offset: u64,
}

impl Eocd {
    /// Parses an end of central directory record and returns it with its
    /// comment length, or `None` without its signature.
    pub(crate) fn parse(record: &[u8]) -> Option<(Self, usize)> {
        let mut f = Fields(record);
        if f.u32()? != EOCD_SIG {
            return None;
        }
        let eocd = Self {
            disk: f.u16()?.into(),
            cd_disk: f.u16()?.into(),
            disk_entries: f.u16()?.into(),
            entries: f.u16()?.into(),
            cd_size: f.u32()?.into(),
            cd_offset: f.u32()?.into(),
        };
        Some((eocd, f.u16()?.into()))
    }

    /// Parses a ZIP64 end of central directory record, or returns `None`
    /// without its signature.
    pub(crate) fn parse64(record: &[u8]) -> Option<Self> {
        let mut f = Fields(record);
        if f.u32()? != EOCD64_SIG {
            return None;
        }
        // The record's size, and the versions made by and needed.
        f.bytes(12)?;
        Some(Self {
            disk: f.u32()?,
            cd_disk: f.u32()?,
            disk_entries: f.u64()?,
            entries: f.u64()?,
            cd_size: f.u64()?,
            cd_offset: f.u64()?,
        })
    }

    /// Returns whether each field of this record either is the sentinel
    /// that defers to ZIP64 or agrees with `zip64`, so that readers that
    /// ignore ZIP64 see the same archive.
    pub(crate) fn agrees_with(&self, zip64: &Self) -> bool {
        let field = |value: u64, sentinel: u64, zip64: u64| value == sentinel || value == zip64;
        field(self.disk.into(), 0xFFFF, zip64.disk.into())
            && field(self.cd_disk.into(), 0xFFFF, zip64.cd_disk.into())
            && field(self.disk_entries, 0xFFFF, zip64.disk_entries)
            && field(self.entries, 0xFFFF, zip64.entries)
            && field(self.cd_size, 0xFFFF_FFFF, zip64.cd_size)
            && field(self.cd_offset, 0xFFFF_FFFF, zip64.cd_offset)
    }
}

/// Returns the position in `tail`, the end of an archive, of its end of
/// central directory record: the one candidate whose signature is there
/// and whose comment runs exactly to the end. `None` if there are none, or
/// several, since a comment holding a second plausible record would let
/// readers disagree about the archive's contents.
pub(crate) fn find_eocd(tail: &[u8]) -> Option<usize> {
    let mut found = None;
    let last = tail.len().checked_sub(EOCD_LEN)?;
    for pos in (0..=last).rev() {
        let Some((_, comment)) = tail.get(pos..).and_then(Eocd::parse) else {
            continue;
        };
        if pos.checked_add(EOCD_LEN)?.checked_add(comment)? == tail.len() {
            if found.is_some() {
                return None;
            }
            found = Some(pos);
        }
    }
    found
}

/// Parses a ZIP64 end of central directory locator, returning the offset
/// of the ZIP64 record and the total number of disks, or `None` without
/// its signature.
pub(crate) fn parse_locator(record: &[u8]) -> Option<(u64, u32)> {
    let mut f = Fields(record);
    if f.u32()? != LOCATOR_SIG {
        return None;
    }
    // The disk holding the ZIP64 record.
    f.u32()?;
    Some((f.u64()?, f.u32()?))
}

/// What a member is, from its name, host, and external attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Dir,
    File,
    Symlink,
    Special(FileKind),
}

/// The fixed part of a central directory file header, with the lengths of
/// the variable part that follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CentralFixed {
    made_by: u16,
    pub(crate) flags: u16,
    pub(crate) method: u16,
    time: u16,
    date: u16,
    pub(crate) crc: u32,
    compressed: u32,
    size: u32,
    pub(crate) name_len: usize,
    pub(crate) extra_len: usize,
    pub(crate) comment_len: usize,
    disk: u16,
    external: u32,
    local_offset: u32,
}

impl CentralFixed {
    /// Parses the fixed part, or returns `None` if it is short or lacks
    /// its signature.
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let mut f = Fields(bytes);
        if f.u32()? != CENTRAL_SIG {
            return None;
        }
        let made_by = f.u16()?;
        // The version needed to extract.
        f.u16()?;
        Some(Self {
            made_by,
            flags: f.u16()?,
            method: f.u16()?,
            time: f.u16()?,
            date: f.u16()?,
            crc: f.u32()?,
            compressed: f.u32()?,
            size: f.u32()?,
            name_len: f.u16()?.into(),
            extra_len: f.u16()?.into(),
            comment_len: f.u16()?.into(),
            disk: f.u16()?,
            // Internal attributes.
            external: {
                f.u16()?;
                f.u32()?
            },
            local_offset: f.u32()?,
        })
    }

    /// Returns the length of the variable part.
    pub(crate) fn variable_len(&self) -> usize {
        // Each is at most 0xFFFF, so this cannot overflow.
        self.name_len
            .saturating_add(self.extra_len)
            .saturating_add(self.comment_len)
    }

    /// Completes the header with its variable part, resolving ZIP64 sizes
    /// and offsets; `None` if the extra field lacks a ZIP64 value that a
    /// sentinel defers to.
    pub(crate) fn complete(self, variable: &[u8]) -> Option<Central> {
        let mut f = Fields(variable);
        let name = f.bytes(self.name_len)?.to_vec();
        let extra = f.bytes(self.extra_len)?;
        let mut zip64 = Fields(field(extra, ZIP64).unwrap_or_default());
        let mut wide = |value: u32| match value {
            0xFFFF_FFFF => zip64.u64(),
            value => Some(value.into()),
        };
        let size = wide(self.size)?;
        let compressed = wide(self.compressed)?;
        let local_offset = wide(self.local_offset)?;
        let disk = match self.disk {
            0xFFFF => zip64.u32()?,
            disk => disk.into(),
        };
        let modified = ntfs_time(extra)
            .or_else(|| unix_time(extra))
            .or_else(|| dos_time(self.date, self.time));
        let unicode_name = unicode_path(extra, &name);
        Some(Central {
            fixed: self,
            name,
            unicode_name,
            size,
            compressed,
            local_offset,
            disk,
            modified,
        })
    }
}

/// A central directory file header, with ZIP64 values resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Central {
    pub(crate) fixed: CentralFixed,
    /// The name as stored.
    pub(crate) name: Vec<u8>,
    /// The name in UTF-8 from Info-ZIP's Unicode Path extra field, if it
    /// has one that matches the name stored.
    unicode_name: Option<Vec<u8>>,
    pub(crate) size: u64,
    pub(crate) compressed: u64,
    pub(crate) local_offset: u64,
    pub(crate) disk: u32,
    /// The most precise modification time recorded: from an NTFS extra
    /// field, then an extended timestamp, then the DOS fields.
    pub(crate) modified: Option<Timestamp>,
}

impl Central {
    /// Returns whether the member is encrypted, which this crate does not
    /// support.
    pub(crate) fn encrypted(&self) -> bool {
        self.fixed.flags & (ENCRYPTED | STRONG_ENCRYPTION) != 0
    }

    /// Returns the member's name in UTF-8 where it can: as stored if the
    /// UTF-8 flag says so, or it is ASCII; from Info-ZIP's Unicode Path
    /// extra field; or decoded from `page`, if a DOS or Windows host wrote
    /// it. Otherwise, as stored, which the index keeps as a foreign name
    /// unless it is valid UTF-8, as Unix hosts mostly write without the
    /// flag.
    pub(crate) fn decoded_name(&self, page: CodePage) -> Vec<u8> {
        if self.fixed.flags & UTF8 != 0 || self.name.is_ascii() {
            return self.name.clone();
        }
        if let Some(name) = &self.unicode_name {
            return name.clone();
        }
        let [_, host] = self.fixed.made_by.to_le_bytes();
        if DOS_HOSTS.contains(&host) {
            let decoded: Option<alloc::string::String> =
                self.name.iter().map(|&byte| page.decode(byte)).collect();
            if let Some(decoded) = decoded {
                return decoded.into_bytes();
            }
        }
        self.name.clone()
    }

    /// Returns what the member is. A trailing `/` makes a directory, as do
    /// a Unix mode or the DOS directory attribute; the Unix mode also gives
    /// symlinks and special files.
    pub(crate) fn kind(&self) -> Kind {
        if self.name.ends_with(b"/") {
            return Kind::Dir;
        }
        let [_, host] = self.fixed.made_by.to_le_bytes();
        let external = self.fixed.external;
        if UNIX_HOSTS.contains(&host) {
            match FileKind::from_unix_mode(external >> 16) {
                Some(FileKind::Dir) => return Kind::Dir,
                Some(FileKind::Symlink) => return Kind::Symlink,
                // Regular files, and archivers that wrote no mode.
                Some(FileKind::File) | None => {},
                Some(kind) => return Kind::Special(kind),
            }
        }
        if external & DOS_DIRECTORY != 0 {
            Kind::Dir
        } else {
            Kind::File
        }
    }
}

/// A local file header, with ZIP64 sizes resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Local {
    pub(crate) flags: u16,
    pub(crate) method: u16,
    pub(crate) crc: u32,
    /// Absent where a sentinel defers to a ZIP64 extra field that is not
    /// there.
    pub(crate) sizes: Option<(u64, u64)>,
    pub(crate) name_len: usize,
    pub(crate) extra_len: usize,
}

/// The fixed part of a local file header.
pub(crate) struct LocalFixed {
    local: Local,
    compressed: u32,
    size: u32,
}

impl LocalFixed {
    /// Parses the fixed part, or returns `None` if it is short or lacks
    /// its signature.
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let mut f = Fields(bytes);
        if f.u32()? != LOCAL_SIG {
            return None;
        }
        // The version needed to extract.
        f.u16()?;
        let flags = f.u16()?;
        let method = f.u16()?;
        // The DOS time and date, which may legitimately differ from the
        // central header's where a writer rewrote it.
        f.u32()?;
        let crc = f.u32()?;
        let compressed = f.u32()?;
        let size = f.u32()?;
        Some(Self {
            local: Local {
                flags,
                method,
                crc,
                sizes: None,
                name_len: f.u16()?.into(),
                extra_len: f.u16()?.into(),
            },
            compressed,
            size,
        })
    }

    /// Returns the length of the name and extra field that follow.
    pub(crate) fn variable_len(&self) -> usize {
        self.local.name_len.saturating_add(self.local.extra_len)
    }

    /// Completes the header with its name and extra field, resolving ZIP64
    /// sizes, and returns it with the name.
    pub(crate) fn complete(self, variable: &[u8]) -> Option<(Local, &[u8])> {
        let mut f = Fields(variable);
        let name = f.bytes(self.local.name_len)?;
        let extra = f.bytes(self.local.extra_len)?;
        let mut zip64 = Fields(field(extra, ZIP64).unwrap_or_default());
        let mut wide = |value: u32| match value {
            0xFFFF_FFFF => zip64.u64(),
            value => Some(value.into()),
        };
        let size = wide(self.size);
        let compressed = wide(self.compressed);
        let sizes = size.zip(compressed);
        Some((
            Local {
                sizes,
                ..self.local
            },
            name,
        ))
    }
}

impl Local {
    /// Returns whether this local header agrees with `central`, its
    /// central directory header: same name, method, and encryption, and,
    /// unless a data descriptor follows the data, the same CRC and sizes.
    /// Readers that trust one over the other must see the same member.
    pub(crate) fn agrees_with(&self, name: &[u8], central: &Central) -> bool {
        let encryption = ENCRYPTED | STRONG_ENCRYPTION;
        let same = name == central.name.as_slice()
            && self.method == central.fixed.method
            && self.flags & encryption == central.fixed.flags & encryption;
        let expected = (central.size, central.compressed);
        same && if self.flags & DATA_DESCRIPTOR != 0 {
            // Sizes and CRC may be zero here, and then follow the data.
            (self.crc == 0 || self.crc == central.fixed.crc)
                && self.sizes.is_some_and(|s| s == (0, 0) || s == expected)
        } else {
            self.crc == central.fixed.crc && self.sizes == Some(expected)
        }
    }
}

/// Returns the data of the first extra field record tagged `tag`. A
/// malformed tail ends the field: some writers pad it with junk.
fn field(extra: &[u8], tag: u16) -> Option<&[u8]> {
    let mut f = Fields(extra);
    loop {
        let this = f.u16()?;
        let len = f.u16()?;
        let data = f.bytes(len.into())?;
        if this == tag {
            return Some(data);
        }
    }
}

/// Returns the name in Info-ZIP's Unicode Path extra field, version 1, if
/// the CRC-32 it records is of `name`, the name as stored, and it is valid
/// UTF-8: a later tool may have renamed the member without updating it.
fn unicode_path(extra: &[u8], name: &[u8]) -> Option<Vec<u8>> {
    let mut f = Fields(field(extra, UNICODE_PATH)?);
    if f.u8()? != 1 || f.u32()? != crc32(name) {
        return None;
    }
    core::str::from_utf8(f.0)
        .ok()
        .map(|name| name.as_bytes().to_vec())
}

/// Reads the modification time from an Info-ZIP extended timestamp extra
/// field, in whole seconds.
fn unix_time(extra: &[u8]) -> Option<Timestamp> {
    let mut f = Fields(field(extra, EXTENDED_TIMESTAMP)?);
    let flags = f.u8()?;
    if flags & 1 == 0 {
        return None;
    }
    let secs = i32::from_le_bytes(f.array()?);
    Some(Timestamp::new(secs.into(), 0).with_granularity(Duration::from_secs(1)))
}

/// Reads the modification time from an NTFS extra field, in units of 100
/// nanoseconds since 1601.
fn ntfs_time(extra: &[u8]) -> Option<Timestamp> {
    let mut f = Fields(field(extra, NTFS)?);
    // Reserved.
    f.u32()?;
    loop {
        let tag = f.u16()?;
        let len = f.u16()?;
        let mut attribute = Fields(f.bytes(len.into())?);
        if tag == 1 {
            return nt::timestamp(attribute.u64()?);
        }
    }
}

/// Returns the time of DOS date and time fields, a local time whose zone
/// ZIP does not record, read as UTC; `None` if unset or out of range.
fn dos_time(date: u16, time: u16) -> Option<Timestamp> {
    dos::Stamp::decode(date, time, 0).to_timestamp(dos::UtcOffset::UTC, dos::TWO_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn eocd(comment: &[u8]) -> Vec<u8> {
        let mut record = EOCD_SIG.to_le_bytes().to_vec();
        record.extend_from_slice(&[0; 16]);
        record.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        record.extend_from_slice(comment);
        record
    }

    #[test]
    fn finds_the_one_end_record() {
        let mut tail = b"member data".to_vec();
        tail.extend(eocd(b"a comment"));
        assert_eq!(find_eocd(&tail), Some(11));
        assert_eq!(find_eocd(&tail[..tail.len() - 1]), None);
        assert_eq!(find_eocd(b"short"), None);

        // A comment that holds a second consistent record is ambiguous.
        let nested = eocd(b"");
        let mut tail = eocd(&nested);
        assert_eq!(find_eocd(&tail), None);
        tail.push(0);
        assert_eq!(find_eocd(&tail), None);
    }

    #[test]
    fn converts_dos_times() {
        // 2024-02-29 13:45:58.
        let date = (44 << 9) | (2 << 5) | 29;
        let time = (13 << 11) | (45 << 5) | 29;
        let t = dos_time(date, time).unwrap();
        assert_eq!(t.secs(), 1_709_214_358);
        assert_eq!(t.granularity(), Some(Duration::from_secs(2)));
        assert_eq!(dos_time(0x21, 0).unwrap().secs(), 315_532_800);
        // 2023 had no February 29, and there is no month 13.
        assert_eq!(dos_time((43 << 9) | (2 << 5) | 29, 0), None);
        assert_eq!(dos_time((43 << 9) | (13 << 5) | 1, 0), None);
        assert_eq!(dos_time(0, 0), None);
    }

    #[test]
    fn reads_extended_and_ntfs_times() {
        let mut extra = vec![0x55, 0x54, 5, 0, 1];
        extra.extend_from_slice(&1_700_000_000_i32.to_le_bytes());
        assert_eq!(unix_time(&extra).unwrap().secs(), 1_700_000_000);

        let ticks: u64 = (1_700_000_000 + 11_644_473_600) * 10_000_000 + 1234;
        let mut extra = vec![0x0a, 0, 32, 0, 0, 0, 0, 0, 1, 0, 24, 0];
        extra.extend_from_slice(&ticks.to_le_bytes());
        extra.extend_from_slice(&[0; 16]);
        let t = ntfs_time(&extra).unwrap();
        assert_eq!((t.secs(), t.nanos()), (1_700_000_000, 123_400));

        // A truncated record ends the field rather than failing it.
        assert_eq!(field(&[0x55, 0x54, 9, 0, 1], EXTENDED_TIMESTAMP), None);
    }
}
