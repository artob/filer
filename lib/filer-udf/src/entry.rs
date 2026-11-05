// This is free and unencumbered software released into the public domain.

//! File entries, basic and extended: a file's kind, size, and times, and
//! where its data is, embedded in the entry or in extents its allocation
//! descriptors list, short, long, or extended, continued in allocation
//! extent descriptors.

use crate::{
    UdfError, tag, text,
    volume::{Partition, Volume, position},
};
use alloc::vec::Vec;
use core::ops::Range;
use filer_api::{Error, FileKind, OpKind, ReadAt, Timestamp};
use filer_fs::le;

/// How many extents a file may have, and how many allocation extent
/// descriptors its list may continue in.
const MAX_EXTENTS: usize = 1 << 20;
const MAX_CONTINUATIONS: u32 = 4096;

/// The longest symlink target, as Linux's paths are.
const MAX_TARGET: usize = 4096;

/// How data is allocated, by the low bits of the ICB tag's flags.
const SHORT: u8 = 0;
const LONG: u8 = 1;
const EXTENDED: u8 = 2;
const EMBEDDED: u8 = 3;

/// A block in a partition, by its reference: where an ICB is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Address {
    pub(crate) reference: u16,
    pub(crate) block: u32,
}

/// Returns the address of a long allocation descriptor, as file sets and
/// file identifiers point to entries.
pub(crate) fn long_ad(bytes: &[u8]) -> Option<Address> {
    Some(Address {
        block: le::u32_at(bytes, 4)?,
        reference: le::u16_at(bytes, 8)?,
    })
}

/// A file's times, as its entry records them.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Times {
    pub(crate) created: Option<Timestamp>,
    pub(crate) modified: Option<Timestamp>,
    pub(crate) accessed: Option<Timestamp>,
    /// When its attributes changed.
    pub(crate) changed: Option<Timestamp>,
}

/// A file entry.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) address: Address,
    /// Its position in the volume, which errors name.
    pub(crate) at: u64,
    pub(crate) kind: FileKind,
    /// Its file type, as recorded.
    pub(crate) file_type: u8,
    /// Its information length; a symlink's target's length.
    pub(crate) len: u64,
    pub(crate) times: Times,
    /// Whether its file identifier marks it hidden.
    pub(crate) hidden: bool,
    /// How its data is allocated.
    ads: u8,
    /// A symlink's target.
    pub(crate) target: Option<Vec<u8>>,
}

/// An extent of a file's data.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Extent {
    /// Its length in bytes.
    pub(crate) len: u64,
    pub(crate) address: Address,
    /// Whether it holds data: if not, it reads as zeros.
    pub(crate) recorded: bool,
}

impl Entry {
    /// What stands for the root while it is read.
    pub(crate) const fn placeholder() -> Self {
        Self {
            address: Address {
                reference: 0,
                block: 0,
            },
            at: 0,
            kind: FileKind::Dir,
            file_type: 4,
            len: 0,
            times: Times {
                created: None,
                modified: None,
                accessed: None,
                changed: None,
            },
            hidden: false,
            ads: EMBEDDED,
            target: None,
        }
    }

    /// Returns its id: its address, which hard links share.
    pub(crate) fn id(&self) -> u128 {
        u128::from(self.address.reference) << 32 | u128::from(self.address.block)
    }

    /// Returns whether its data is embedded in it.
    pub(crate) const fn is_embedded(&self) -> bool {
        self.ads == EMBEDDED
    }
}

/// Returns the kind of a file type: a directory, a file, a symlink, a
/// device, a FIFO, or a socket; files of no type are files.
const fn kind(file_type: u8) -> Option<FileKind> {
    Some(match file_type {
        0 | 5 => FileKind::File,
        4 => FileKind::Dir,
        6 => FileKind::BlockDevice,
        7 => FileKind::CharDevice,
        9 => FileKind::Fifo,
        10 => FileKind::Socket,
        12 => FileKind::Symlink,
        _ => return None,
    })
}

/// Parses the entry at the start of `bytes`, from `address` at `at`,
/// returning it, without its kind checked, and where its allocation
/// descriptors, or embedded data, are in `bytes`.
fn parse(bytes: &[u8], address: Address, at: u64) -> Option<(Entry, Range<usize>)> {
    let (times_at, ea_at): ([Option<usize>; 4], usize) = match tag::identifier(bytes)? {
        tag::FILE_ENTRY => ([None, Some(84), Some(72), Some(96)], 168),
        tag::EXTENDED_FILE_ENTRY => ([Some(104), Some(92), Some(80), Some(116)], 208),
        _ => return None,
    };
    let strategy = le::u16_at(bytes, 20)?;
    if !matches!(strategy, 4 | 4096) {
        return None;
    }
    let file_type = *bytes.get(27)?;
    let ads = u8::try_from(le::u16_at(bytes, 34)? & 0x7).ok()?;
    if ads > EMBEDDED {
        return None;
    }
    let time =
        |at: Option<usize>| at.and_then(|at| text::time(bytes.get(at..at.checked_add(12)?)?));
    let [created, modified, accessed, changed] = times_at;
    let extended = usize::try_from(le::u32_at(bytes, ea_at)?).ok()?;
    let allocated = usize::try_from(le::u32_at(bytes, ea_at.checked_add(4)?)?).ok()?;
    let start = ea_at.checked_add(8)?.checked_add(extended)?;
    let end = start.checked_add(allocated)?;
    bytes.get(start..end)?;
    let entry = Entry {
        address,
        at,
        kind: kind(file_type).unwrap_or(FileKind::File),
        file_type,
        len: le::u64_at(bytes, 56)?,
        times: Times {
            created: time(created),
            modified: time(modified),
            accessed: time(accessed),
            changed: time(changed),
        },
        hidden: false,
        ads,
        target: None,
    };
    Some((entry, start..end))
}

impl<S: ReadAt> Volume<S> {
    /// Reads the entry at `address`, which a file identifier, `hidden` if
    /// it says so, points to; a symlink's with its target.
    pub(crate) async fn entry(
        &self,
        address: Address,
        hidden: bool,
        op: OpKind,
    ) -> Result<Entry, Error> {
        let (mut entry, bytes, area) = self.entry_bytes(&self.partitions, address, op).await?;
        let bad = || UdfError::BadEntry { at: entry.at }.into_error(op);
        if kind(entry.file_type).is_none() {
            return Err(bad());
        }
        entry.hidden = hidden;
        if entry.kind == FileKind::Symlink {
            let data = self
                .contents_of(
                    &self.partitions,
                    &entry,
                    &bytes,
                    area,
                    MAX_TARGET.saturating_mul(2),
                    op,
                )
                .await?;
            let target = target(&data)
                .filter(|target| !target.is_empty() && target.len() <= MAX_TARGET)
                .ok_or_else(|| UdfError::BadSymlink { at: entry.at }.into_error(op))?;
            entry.len = u64::try_from(target.len()).unwrap_or(u64::MAX);
            entry.target = Some(target);
        }
        Ok(entry)
    }

    /// Reads the entry at `address` through `maps`, of any file type.
    pub(crate) async fn raw_entry(
        &self,
        maps: &[Partition],
        address: Address,
        op: OpKind,
    ) -> Result<Entry, Error> {
        Ok(self.entry_bytes(maps, address, op).await?.0)
    }

    /// Reads the entry at `address` through `maps`, with its block and
    /// where its allocation descriptors are in it.
    async fn entry_bytes(
        &self,
        maps: &[Partition],
        address: Address,
        op: OpKind,
    ) -> Result<(Entry, Vec<u8>, Range<usize>), Error> {
        let at = position(maps, self.block, address, op)?;
        let bad = || UdfError::BadEntry { at }.into_error(op);
        let len = usize::try_from(self.block).map_err(|_| bad())?;
        let bytes = self.read(at, len, op).await.map_err(|_| bad())?;
        let (entry, area) = parse(&bytes, address, at).ok_or_else(bad)?;
        Ok((entry, bytes, area))
    }

    /// Returns the extents of `entry`, read through `maps`; none if its
    /// data is embedded.
    pub(crate) async fn extents_in(
        &self,
        maps: &[Partition],
        entry: &Entry,
        op: OpKind,
    ) -> Result<Vec<Extent>, Error> {
        let (entry, bytes, area) = self.entry_bytes(maps, entry.address, op).await?;
        self.extents_of(maps, &entry, &bytes, area, op).await
    }

    /// Returns the extents of `entry`, whose block is `bytes`, its
    /// allocation descriptors at `area`, continued through `maps`.
    async fn extents_of(
        &self,
        maps: &[Partition],
        entry: &Entry,
        bytes: &[u8],
        area: Range<usize>,
        op: OpKind,
    ) -> Result<Vec<Extent>, Error> {
        let bad = || UdfError::BadExtents { at: entry.at }.into_error(op);
        let size = match entry.ads {
            SHORT => 8,
            LONG => 16,
            EXTENDED => 20,
            _ => return Ok(Vec::new()),
        };
        let mut descriptors = bytes.get(area).unwrap_or_default().to_vec();
        let (mut extents, mut continuations) = (Vec::new(), 0_u32);
        loop {
            let mut next = None;
            let mut at = 0_usize;
            while let Some(descriptor) = descriptors.get(at..at.saturating_add(size)) {
                at = at.saturating_add(size);
                let raw = le::u32_at(descriptor, 0).ok_or_else(bad)?;
                let len = u64::from(raw & 0x3FFF_FFFF);
                let kind = raw >> 30;
                if len == 0 {
                    break;
                }
                let address = match entry.ads {
                    SHORT => Address {
                        reference: entry.address.reference,
                        block: le::u32_at(descriptor, 4).ok_or_else(bad)?,
                    },
                    LONG => long_ad(descriptor).ok_or_else(bad)?,
                    _ => Address {
                        block: le::u32_at(descriptor, 12).ok_or_else(bad)?,
                        reference: le::u16_at(descriptor, 16).ok_or_else(bad)?,
                    },
                };
                if kind == 3 {
                    next = Some(address);
                    break;
                }
                if extents.len() >= MAX_EXTENTS {
                    return Err(bad());
                }
                extents.push(Extent {
                    len,
                    address,
                    recorded: kind == 0,
                });
            }
            let Some(next) = next else {
                break;
            };
            continuations = continuations.saturating_add(1);
            if continuations > MAX_CONTINUATIONS {
                return Err(bad());
            }
            let at = position(maps, self.block, next, op)?;
            let len = usize::try_from(self.block).map_err(|_| bad())?;
            let block = self.read(at, len, op).await.map_err(|_| bad())?;
            if !tag::is(&block, tag::ALLOCATION_EXTENT) {
                return Err(bad());
            }
            let allocated =
                usize::try_from(le::u32_at(&block, 20).ok_or_else(bad)?).map_err(|_| bad())?;
            descriptors = block
                .get(24..24_usize.saturating_add(allocated))
                .ok_or_else(bad)?
                .to_vec();
        }
        Ok(extents)
    }

    /// Reads the data of `entry` through `maps`, up to `max` bytes.
    pub(crate) async fn contents(
        &self,
        maps: &[Partition],
        entry: &Entry,
        max: u64,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let (entry, bytes, area) = self.entry_bytes(maps, entry.address, op).await?;
        let max = usize::try_from(max).unwrap_or(usize::MAX);
        self.contents_of(maps, &entry, &bytes, area, max, op).await
    }

    /// Reads the data of `entry`, whose block is `bytes`, its allocation
    /// descriptors at `area`, through `maps`, up to `max` bytes.
    async fn contents_of(
        &self,
        maps: &[Partition],
        entry: &Entry,
        bytes: &[u8],
        area: Range<usize>,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let len = usize::try_from(entry.len).unwrap_or(usize::MAX).min(max);
        if entry.is_embedded() {
            let data = bytes.get(area).unwrap_or_default();
            return Ok(data.get(..len.min(data.len())).unwrap_or_default().to_vec());
        }
        let extents = self.extents_of(maps, entry, bytes, area, op).await?;
        self.read_extents(maps, entry, &extents, 0, len, op).await
    }

    /// Reads `len` bytes of the data `extents` hold, from `off` on,
    /// through `maps`; zeros where they hold none.
    pub(crate) async fn read_extents(
        &self,
        maps: &[Partition],
        entry: &Entry,
        extents: &[Extent],
        off: u64,
        len: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let bad = || UdfError::BadExtents { at: entry.at }.into_error(op);
        let block = u64::from(self.block);
        let mut bytes = Vec::with_capacity(len.min(1 << 20));
        let mut start = 0_u64;
        for extent in extents {
            if bytes.len() >= len {
                break;
            }
            let end = start.saturating_add(extent.len);
            let at = off.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
            if at < end {
                let mut within = at.saturating_sub(start);
                let mut want = extent
                    .len
                    .saturating_sub(within)
                    .min(u64::try_from(len.saturating_sub(bytes.len())).unwrap_or(u64::MAX));
                if !extent.recorded {
                    let n = usize::try_from(want).map_err(|_| bad())?;
                    bytes.resize(bytes.len().saturating_add(n), 0);
                    start = end;
                    continue;
                }
                // Block by block, as each partition maps them, a run of
                // adjacent ones at a time.
                while want > 0 {
                    let index =
                        u32::try_from(within.checked_div(block).unwrap_or(0)).map_err(|_| bad())?;
                    let offset = within.checked_rem(block).unwrap_or(0);
                    let first = Address {
                        reference: extent.address.reference,
                        block: extent.address.block.checked_add(index).ok_or_else(bad)?,
                    };
                    let from = position(maps, self.block, first, op)?;
                    let mut run = block.saturating_sub(offset).min(want);
                    let mut next = first;
                    while run < want && run < (1 << 20) {
                        next.block = next.block.checked_add(1).ok_or_else(bad)?;
                        let expected = from.saturating_add(
                            u64::from(next.block.saturating_sub(first.block)).saturating_mul(block),
                        );
                        if position(maps, self.block, next, op).ok() != Some(expected) {
                            break;
                        }
                        run = run.saturating_add(block).min(want);
                    }
                    let n = usize::try_from(run).map_err(|_| bad())?;
                    let read = self
                        .read(from.saturating_add(offset), n, op)
                        .await
                        .map_err(|_| bad())?;
                    bytes.extend_from_slice(&read);
                    within = within.saturating_add(run);
                    want = want.saturating_sub(run);
                }
            }
            start = end;
        }
        if bytes.len() < len {
            // Extents shorter than the file: what is missing reads as
            // zeros, as Linux reads them.
            bytes.resize(len, 0);
        }
        Ok(bytes)
    }
}

/// Returns the target of a symlink whose data is `data`: path components,
/// each its type, its identifier's length, a version, and the identifier,
/// in compressed Unicode: the root, `..`, `.`, or a name.
fn target(data: &[u8]) -> Option<Vec<u8>> {
    let mut target = Vec::new();
    let mut rest = data;
    while let [kind, len, _, _, tail @ ..] = rest {
        let (identifier, after) = tail.split_at_checked(usize::from(*len))?;
        rest = after;
        if !target.is_empty() && !target.ends_with(b"/") {
            target.push(b'/');
        }
        match kind {
            1 | 2 => {
                target.clear();
                target.push(b'/');
            },
            3 => target.extend_from_slice(b".."),
            4 => target.push(b'.'),
            5 => target.extend_from_slice(&text::name(identifier)?),
            _ => return None,
        }
    }
    rest.is_empty().then_some(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn reads_symlink_targets() {
        let component = |kind: u8, name: &[u8]| {
            let mut bytes = vec![kind, u8::try_from(name.len()).unwrap(), 0, 0];
            bytes.extend_from_slice(name);
            bytes
        };
        let link = |parts: &[Vec<u8>]| target(&parts.concat());
        assert_eq!(
            link(&[component(5, b"\x08a"), component(5, b"\x08b")]).unwrap(),
            b"a/b"
        );
        assert_eq!(
            link(&[component(2, b""), component(5, b"\x08etc")]).unwrap(),
            b"/etc"
        );
        assert_eq!(
            link(&[component(3, b""), component(4, b""), component(5, b"\x08x")]).unwrap(),
            b".././x"
        );
        assert_eq!(link(&[component(5, b"\x10\0a")]).unwrap(), b"a");
        assert_eq!(link(&[component(9, b"")]), None);
        assert_eq!(target(&[5, 9, 0, 0, 8]), None);
    }
}
