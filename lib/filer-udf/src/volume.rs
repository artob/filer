// This is free and unencumbered software released into the public domain.

//! A mounted volume: its anchor and block size, its descriptors, the
//! partitions its logical volume's maps make of them, and how a block of
//! each is found in the volume.

use crate::{
    UdfError,
    dir::Listing,
    entry::{self, Address, Entry},
    tag,
};
use alloc::{sync::Arc, vec::Vec};
use filer_api::{DriverKey, Error, ErrorKind, OpKind, ReadAt};
use filer_fs::{Lock, le};

/// The block sizes tried, as no volume says its own before its anchor.
const BLOCK_SIZES: [u32; 4] = [512, 1024, 2048, 4096];

/// How many descriptors a volume descriptor sequence may hold, and how
/// many pointers to further sequences may be followed.
const MAX_DESCRIPTORS: u32 = 256;
const MAX_POINTERS: u32 = 8;

/// The largest virtual allocation table, sparing table, and metadata
/// file's list of extents read: 64 MiB of entries, beyond any disc's.
const MAX_TABLE: u64 = 64 << 20;

/// How many listings the cache keeps, and how many bytes of directories
/// they may span in all.
const LISTINGS: usize = 16;
const LISTED: u64 = 1 << 20;

/// A partition, as a map of the logical volume makes one of a partition
/// descriptor's blocks.
#[derive(Clone, Debug)]
pub(crate) enum Partition {
    /// Blocks in order from the partition's start.
    Physical { start: u32, len: u32 },
    /// The same, but for packets that a sparing table moved elsewhere:
    /// each the block it began at, and the sector it moved to.
    Sparable {
        start: u32,
        len: u32,
        packet: u32,
        moved: Vec<(u32, u32)>,
    },
    /// Blocks of a write-once partition, as its virtual allocation table
    /// maps each to a block of the physical one, `underlying`.
    Virtual { underlying: u16, table: Vec<u32> },
    /// Blocks of the metadata file, whose extents lie in `underlying`.
    Metadata {
        underlying: u16,
        extents: Vec<(u64, u32)>,
    },
}

/// A volume: what every operation reads from.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) source: S,
    /// The source's size in bytes.
    pub(crate) size: u64,
    /// The logical block size.
    pub(crate) block: u32,
    pub(crate) partitions: Vec<Partition>,
    /// The volume's id, which every file's id is in.
    pub(crate) id: u64,
    pub(crate) root: Arc<Entry>,
    listings: Lock<Vec<(Address, u64, Listing)>>,
}

impl<S: ReadAt> Volume<S> {
    /// Opens the volume in `source`: finds its anchor, reads its
    /// descriptors and partitions, and its root directory's entry.
    pub(crate) async fn open(source: S) -> Result<Self, Error> {
        let op = OpKind::Open;
        let size = source.size().await?;
        let (block, anchor) = anchor(&source, size, op).await?;
        let bad = || UdfError::BadDescriptors.into_error(op);
        // The main sequence, or else the reserve one.
        let mut found = None;
        for at in [16, 24] {
            let len = le::u32_at(&anchor, at).ok_or_else(bad)?;
            let location = le::u32_at(&anchor, at.saturating_add(4)).ok_or_else(bad)?;
            if let Ok(descriptors) = sequence(&source, block, location, len, op).await {
                found = Some(descriptors);
                break;
            }
        }
        let Descriptors { volume, partitions } = found.ok_or_else(bad)?;
        let logical_block = le::u32_at(&volume, 212).ok_or_else(bad)?;
        if logical_block != block {
            return Err(UdfError::BadBlockSize(logical_block).into_error(op));
        }
        let mut this = Self {
            key: DriverKey::new(),
            source,
            size,
            block,
            partitions: Vec::new(),
            id: identity(&volume),
            root: Arc::new(Entry::placeholder()),
            listings: Lock::new(Vec::new()),
        };
        this.partitions = this.maps(&volume, &partitions, op).await?;
        let file_set = entry::long_ad(volume.get(248..264).unwrap_or_default())
            .ok_or_else(|| UdfError::BadFileSet.into_error(op))?;
        let bytes = this
            .read_block(file_set, op)
            .await
            .map_err(|_| UdfError::BadFileSet.into_error(op))?;
        if !tag::is(&bytes, tag::FILE_SET) {
            return Err(UdfError::BadFileSet.into_error(op));
        }
        let root = entry::long_ad(bytes.get(400..416).unwrap_or_default())
            .ok_or_else(|| UdfError::BadFileSet.into_error(op))?;
        let root = this.entry(root, false, op).await.map_err(|error| {
            if error.kind() == ErrorKind::Corrupt {
                UdfError::BadRoot.into_error(op)
            } else {
                error
            }
        })?;
        if root.kind != filer_api::FileKind::Dir {
            return Err(UdfError::BadRoot.into_error(op));
        }
        this.root = Arc::new(root);
        Ok(this)
    }

    /// Returns the partitions the logical volume descriptor `volume` maps,
    /// of the partition descriptors `partitions`, by their numbers.
    async fn maps(
        &self,
        volume: &[u8],
        partitions: &[(u16, u32, u32)],
        op: OpKind,
    ) -> Result<Vec<Partition>, Error> {
        let bad = || UdfError::BadDescriptors.into_error(op);
        let count = le::u32_at(volume, 268).ok_or_else(bad)?;
        let (mut maps, mut numbers) = (Vec::new(), Vec::new());
        let mut at = 440_usize;
        for reference in 0..count.min(64) {
            let reference = u16::try_from(reference).map_err(|_| bad())?;
            let bad = || UdfError::BadPartition { reference }.into_error(op);
            let kind = *volume.get(at).ok_or_else(bad)?;
            let len = usize::from(*volume.get(at.saturating_add(1)).ok_or_else(bad)?);
            let map = volume.get(at..at.saturating_add(len)).ok_or_else(bad)?;
            at = at.saturating_add(len.max(2));
            let number = le::u16_at(map, if kind == 1 { 4 } else { 38 }).ok_or_else(bad)?;
            let &(_, start, length) = partitions
                .iter()
                .find(|(found, _, _)| *found == number)
                .ok_or_else(bad)?;
            // A type 2 map's kind is named by an identifier, after a byte
            // of flags.
            let name = map.get(5..28).unwrap_or_default();
            let named = |prefix: &[u8]| kind == 2 && name.starts_with(prefix);
            let partition = if kind == 1 && len >= 6 {
                Partition::Physical { start, len: length }
            } else if named(b"*UDF Sparable Partition") {
                self.sparable(map, start, length, reference, op).await?
            } else if named(b"*UDF Virtual Partition") {
                let underlying = physical(&maps, &numbers, number).ok_or_else(bad)?;
                let table = self.vat(&maps, underlying, reference, op).await?;
                Partition::Virtual { underlying, table }
            } else if named(b"*UDF Metadata Partition") {
                let underlying = physical(&maps, &numbers, number).ok_or_else(bad)?;
                let extents = self.metadata(&maps, map, underlying, reference, op).await?;
                Partition::Metadata {
                    underlying,
                    extents,
                }
            } else if kind == 2 {
                return Err(UdfError::UnsupportedPartition.into_error(op));
            } else {
                return Err(bad());
            };
            maps.push(partition);
            numbers.push(number);
        }
        if maps.is_empty() {
            return Err(bad());
        }
        Ok(maps)
    }

    /// Returns a sparable partition, with its first sparing table that
    /// reads.
    async fn sparable(
        &self,
        map: &[u8],
        start: u32,
        len: u32,
        reference: u16,
        op: OpKind,
    ) -> Result<Partition, Error> {
        let bad = || UdfError::BadPartition { reference }.into_error(op);
        let packet = u32::from(le::u16_at(map, 40).ok_or_else(bad)?);
        let tables = *map.get(42).ok_or_else(bad)?;
        let size = le::u32_at(map, 44).ok_or_else(bad)?;
        let size = usize::try_from(u64::from(size).min(MAX_TABLE)).map_err(|_| bad())?;
        for index in 0..usize::from(tables.min(4)) {
            let at = index.saturating_mul(4).saturating_add(48);
            let Some(sector) = le::u32_at(map, at) else {
                continue;
            };
            let position = u64::from(sector).saturating_mul(u64::from(self.block));
            let Ok(table) = self.read(position, size.max(56), op).await else {
                continue;
            };
            let named = table.get(17..35) == Some(b"*UDF Sparing Table");
            if tag::identifier(&table) != Some(0) || !named {
                continue;
            }
            let entries = usize::from(le::u16_at(&table, 48).ok_or_else(bad)?);
            let moved = (0..entries)
                .filter_map(|entry| {
                    let at = entry.checked_mul(8)?.checked_add(56)?;
                    Some((le::u32_at(&table, at)?, le::u32_at(&table, at.checked_add(4)?)?))
                })
                // Free and unusable entries move nothing.
                .filter(|&(original, _)| original < 0xFFFF_FFF0)
                .collect();
            return Ok(Partition::Sparable {
                start,
                len,
                packet: packet.max(1),
                moved,
            });
        }
        Err(bad())
    }

    /// Reads the virtual allocation table of a write-once partition,
    /// whose entry is the last block recorded: the volume's last.
    async fn vat(
        &self,
        maps: &[Partition],
        underlying: u16,
        reference: u16,
        op: OpKind,
    ) -> Result<Vec<u32>, Error> {
        let bad = || UdfError::BadPartition { reference }.into_error(op);
        let block = u64::from(self.block);
        let last = self
            .size
            .checked_div(block)
            .and_then(|n| n.checked_sub(1))
            .ok_or_else(bad)?;
        let Partition::Physical { start, .. } =
            maps.get(usize::from(underlying)).ok_or_else(bad)?
        else {
            return Err(bad());
        };
        let relative = last
            .checked_sub(u64::from(*start))
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(bad)?;
        let address = Address {
            reference: underlying,
            block: relative,
        };
        let vat = self.raw_entry(maps, address, op).await.map_err(|_| bad())?;
        let bytes = self
            .contents(maps, &vat, MAX_TABLE, op)
            .await
            .map_err(|_| bad())?;
        // UDF 2.00's: a header, then the entries; 1.50's: the entries,
        // then an identifier and the previous table's place.
        let entries = if vat.file_type == 248 {
            let header = usize::from(le::u16_at(&bytes, 0).ok_or_else(bad)?);
            bytes.get(header..).ok_or_else(bad)?
        } else {
            let end = bytes.len().checked_sub(36).ok_or_else(bad)?;
            if bytes.get(end.saturating_add(1)..end.saturating_add(23))
                != Some(b"*UDF Virtual Alloc Tbl")
            {
                return Err(bad());
            }
            bytes.get(..end).ok_or_else(bad)?
        };
        let (entries, _) = entries.as_chunks::<4>();
        Ok(entries
            .iter()
            .map(|&entry| u32::from_le_bytes(entry))
            .collect())
    }

    /// Reads the extents of the metadata file of a metadata partition, or
    /// of its mirror if it does not read.
    async fn metadata(
        &self,
        maps: &[Partition],
        map: &[u8],
        underlying: u16,
        reference: u16,
        op: OpKind,
    ) -> Result<Vec<(u64, u32)>, Error> {
        let bad = || UdfError::BadPartition { reference }.into_error(op);
        for at in [40, 44] {
            let Some(block) = le::u32_at(map, at) else {
                continue;
            };
            let address = Address {
                reference: underlying,
                block,
            };
            let Ok(file) = self.raw_entry(maps, address, op).await else {
                continue;
            };
            if !matches!(file.file_type, 250 | 251) {
                continue;
            }
            let Ok(extents) = self.extents_in(maps, &file, op).await else {
                continue;
            };
            let mut runs = Vec::new();
            for extent in extents {
                if extent.recorded {
                    runs.push((extent.len, extent.address.block));
                }
            }
            return Ok(runs);
        }
        Err(bad())
    }

    /// Returns the byte position of block `address`, through its
    /// partition's map.
    pub(crate) fn position(&self, address: Address, op: OpKind) -> Result<u64, Error> {
        position(&self.partitions, self.block, address, op)
    }

    /// Reads `len` bytes at `at`; a read past the volume's end, where only
    /// a corrupt pointer leads, is corrupt.
    pub(crate) async fn read(&self, at: u64, len: usize, op: OpKind) -> Result<Vec<u8>, Error> {
        filer_fs::read_exact(&self.source, at, len, op)
            .await
            .map_err(|error| {
                if error.kind() == ErrorKind::UnexpectedEof {
                    Error::new(ErrorKind::Corrupt, op)
                } else {
                    error
                }
            })
    }

    /// Reads block `address`.
    pub(crate) async fn read_block(&self, address: Address, op: OpKind) -> Result<Vec<u8>, Error> {
        let at = self.position(address, op)?;
        let len = usize::try_from(self.block).unwrap_or(2048);
        self.read(at, len, op).await
    }

    /// Returns the listing of directory `dir`, from the cache if it holds
    /// it.
    pub(crate) async fn listing(&self, dir: &Entry, op: OpKind) -> Result<Listing, Error> {
        let key = (dir.address, dir.len);
        let hit = self.listings.with(|cache| {
            let found = cache
                .iter()
                .position(|cached| (cached.0, cached.1) == key)?;
            let cached = cache.remove(found);
            cache.insert(0, cached.clone());
            Some(cached.2)
        });
        if let Some(listing) = hit {
            return Ok(listing);
        }
        let listing = self.list(dir, op).await?;
        if dir.len <= LISTED {
            self.listings.with(|cache| {
                let mut spanned = dir.len;
                cache.truncate(LISTINGS.saturating_sub(1));
                cache.retain(|cached| {
                    spanned = spanned.saturating_add(cached.1);
                    spanned <= LISTED
                });
                cache.insert(0, (key.0, key.1, listing.clone()));
            });
        }
        Ok(listing)
    }
}

/// Returns the byte position of block `address`, through `maps`.
pub(crate) fn position(
    maps: &[Partition],
    block: u32,
    address: Address,
    op: OpKind,
) -> Result<u64, Error> {
    let Address {
        reference,
        block: at,
    } = address;
    let bad = || {
        UdfError::BadBlock {
            reference,
            block: at,
        }
        .into_error(op)
    };
    let size = u64::from(block);
    let sector = match maps.get(usize::from(reference)).ok_or_else(bad)? {
        Partition::Physical { start, len } => {
            if at >= *len {
                return Err(bad());
            }
            u64::from(*start).saturating_add(u64::from(at))
        },
        Partition::Sparable {
            start,
            len,
            packet,
            moved,
        } => {
            if at >= *len {
                return Err(bad());
            }
            let first = at.checked_div(*packet).unwrap_or(0).saturating_mul(*packet);
            match moved.iter().find(|&&(original, _)| original == first) {
                Some(&(_, sector)) => {
                    u64::from(sector).saturating_add(u64::from(at.saturating_sub(first)))
                },
                None => u64::from(*start).saturating_add(u64::from(at)),
            }
        },
        Partition::Virtual { underlying, table } => {
            let mapped = *table
                .get(usize::try_from(at).map_err(|_| bad())?)
                .filter(|&&mapped| mapped != u32::MAX)
                .ok_or_else(bad)?;
            let address = Address {
                reference: *underlying,
                block: mapped,
            };
            return position(maps, block, address, op);
        },
        Partition::Metadata {
            underlying,
            extents,
        } => {
            let mut offset = u64::from(at).saturating_mul(size);
            for &(len, first) in extents {
                if offset < len {
                    let within =
                        u32::try_from(offset.checked_div(size).unwrap_or(0)).map_err(|_| bad())?;
                    let address = Address {
                        reference: *underlying,
                        block: first.checked_add(within).ok_or_else(bad)?,
                    };
                    return position(maps, block, address, op);
                }
                offset = offset.saturating_sub(len);
            }
            return Err(bad());
        },
    };
    sector.checked_mul(size).ok_or_else(bad)
}

/// Returns the reference of the physical or sparable map of partition
/// `number`, among `maps` so far, whose partitions' numbers are
/// `numbers`: what a virtual or metadata partition lies in.
fn physical(maps: &[Partition], numbers: &[u16], number: u16) -> Option<u16> {
    let index = maps.iter().zip(numbers).position(|(map, &found)| {
        found == number && matches!(map, Partition::Physical { .. } | Partition::Sparable { .. })
    })?;
    u16::try_from(index).ok()
}

/// Finds the anchor volume descriptor pointer: at sector 256, at 256
/// before the last, or at the last, of each block size, by its tag, which
/// gives its own place. Returns the block size and the anchor.
async fn anchor<S: ReadAt>(source: &S, size: u64, op: OpKind) -> Result<(u32, Vec<u8>), Error> {
    for block in BLOCK_SIZES {
        let blocks = size.checked_div(u64::from(block)).unwrap_or(0);
        let last = blocks.saturating_sub(1);
        for sector in [256, last.saturating_sub(256), last] {
            let Some(at) = sector
                .checked_mul(u64::from(block))
                .filter(|&at| at.saturating_add(u64::from(block)) <= size && sector >= 256)
            else {
                continue;
            };
            let len = usize::try_from(block).unwrap_or(2048);
            let Ok(bytes) = filer_fs::read_exact(source, at, len, op).await else {
                continue;
            };
            if tag::is(&bytes, tag::ANCHOR) && tag::location(&bytes).map(u64::from) == Some(sector)
            {
                return Ok((block, bytes));
            }
        }
    }
    Err(UdfError::NoAnchor.into_error(op))
}

/// What a volume descriptor sequence holds: its logical volume
/// descriptor, and its partition descriptors' numbers, starts, and
/// lengths.
struct Descriptors {
    volume: Vec<u8>,
    partitions: Vec<(u16, u32, u32)>,
}

/// Reads the volume descriptor sequence of `len` bytes at `location`,
/// following pointers to others.
async fn sequence<S: ReadAt>(
    source: &S,
    block: u32,
    location: u32,
    len: u32,
    op: OpKind,
) -> Result<Descriptors, Error> {
    let bad = || UdfError::BadDescriptors.into_error(op);
    let (mut volume, mut partitions) = (None, Vec::new());
    let (mut location, mut left, mut pointers, mut read) = (location, len, 0, 0);
    let size = usize::try_from(block).map_err(|_| bad())?;
    'sequence: while left >= block && read < MAX_DESCRIPTORS {
        let at = u64::from(location).saturating_mul(u64::from(block));
        let bytes = filer_fs::read_exact(source, at, size, op)
            .await
            .map_err(|_| bad())?;
        read = read.saturating_add(1);
        match tag::identifier(&bytes) {
            Some(tag::PARTITION) => {
                let number = le::u16_at(&bytes, 22).ok_or_else(bad)?;
                let start = le::u32_at(&bytes, 188).ok_or_else(bad)?;
                let length = le::u32_at(&bytes, 192).ok_or_else(bad)?;
                if partitions.iter().all(|&(known, _, _)| known != number) {
                    partitions.push((number, start, length));
                }
            },
            Some(tag::LOGICAL_VOLUME) if volume.is_none() => volume = Some(bytes),
            Some(tag::POINTER) if pointers < MAX_POINTERS => {
                pointers = pointers.saturating_add(1);
                left = le::u32_at(&bytes, 20).ok_or_else(bad)?;
                location = le::u32_at(&bytes, 24).ok_or_else(bad)?;
                continue 'sequence;
            },
            Some(tag::TERMINATING) | None => break,
            Some(_) => {},
        }
        location = location.saturating_add(1);
        left = left.saturating_sub(block);
    }
    match volume {
        Some(volume) if !partitions.is_empty() => Ok(Descriptors { volume, partitions }),
        _ => Err(bad()),
    }
}

/// Returns the 64-bit FNV-1a hash of what identifies a volume in its
/// logical volume descriptor: its identifier, and its file set's place.
fn identity(volume: &[u8]) -> u64 {
    [84..212, 248..264]
        .into_iter()
        .filter_map(|range| volume.get(range))
        .flatten()
        .fold(0xCBF2_9CE4_8422_2325, |hash, &byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01B3)
        })
}
