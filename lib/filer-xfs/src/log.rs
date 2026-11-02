// This is free and unencumbered software released into the public domain.

//! The log: whether it holds changes not yet written to the volume.
//!
//! The log is a circle of 512-byte sectors, written in records from its
//! start round to its end and over again, each pass with a cycle number
//! one higher. Every sector starts with the cycle number of the pass that
//! wrote it (a record's header with its magic number, then the cycle), so
//! the head, where the next record goes, is where the cycle number drops.
//! A volume unmounted cleanly writes, last, an unmount record: one
//! operation of the log itself, flagged as an unmount. A log whose last
//! record, ending at the head, is anything else may hold changes the
//! volume does not have yet.

use crate::{XfsError, be, io, superblock::Superblock};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt};

/// The size of a log sector.
const SECTOR: u64 = 512;
const SECTOR_LEN: usize = 512;

/// A record header's magic number.
const RECORD_MAGIC: u32 = 0xFEED_BABE;

/// A header sector covers 32 KiB of a record's data; more takes extended
/// header sectors.
const HEADER_COVERS: u32 = 32 << 10;

/// The most sectors a record spans: 256 KiB of data, and its header's.
const MAX_RECORD_SECTORS: u64 = 512 + 8;

/// Where a record header keeps its fields.
const CYCLE: usize = 4;
const VERSION: usize = 8;
const LEN: usize = 12;
const OPS: usize = 40;
const SIZE: usize = 320;

/// An operation's client and flag for the unmount record.
const CLIENT_LOG: u8 = 0xAA;
const UNMOUNT: u8 = 0x20;

/// The internal log, as sectors of the volume.
struct Log<'a, S: ?Sized> {
    source: &'a S,
    /// The offset of the log's first sector.
    base: u64,
    /// The number of sectors.
    sectors: u64,
}

impl<S: ReadAt + ?Sized> Log<'_, S> {
    /// Reads `count` sectors from sector `first`, which must not wrap.
    async fn read(&self, first: u64, count: u64) -> Result<Vec<u8>, Error> {
        let off = first
            .checked_mul(SECTOR)
            .and_then(|off| self.base.checked_add(off));
        let len = count
            .checked_mul(SECTOR)
            .and_then(|len| usize::try_from(len).ok());
        let (Some(off), Some(len)) = (off, len) else {
            return Err(XfsError::NeedsRecovery.into_error(OpKind::Open));
        };
        io::read_exact(self.source, off, len).await
    }

    /// Returns the cycle number of sector `n`.
    async fn cycle(&self, n: u64) -> Result<u32, Error> {
        let sector = self.read(n, 1).await?;
        Ok(cycle_of(&sector))
    }
}

/// Returns the cycle number that starts a sector: after a record header's
/// magic number, or first.
fn cycle_of(sector: &[u8]) -> u32 {
    match be::u32_at(sector, 0) {
        Some(RECORD_MAGIC) => be::u32_at(sector, CYCLE).unwrap_or(0),
        word => word.unwrap_or(0),
    }
}

/// Checks that the volume's internal log was left clean: its last record
/// an unmount record, nothing after it.
pub(crate) async fn check<S: ReadAt + ?Sized>(source: &S, sb: &Superblock) -> Result<(), Error> {
    let op = OpKind::Open;
    let dirty = || XfsError::NeedsRecovery.into_error(op);
    let (start, blocks) = sb
        .internal_log()
        .ok_or_else(|| XfsError::ExternalLog.into_error(op))?;
    let log = Log {
        source,
        base: sb.block_offset(start).ok_or_else(dirty)?,
        sectors: u64::from(sb.block_size())
            .checked_div(SECTOR)
            .and_then(|per_block| per_block.checked_mul(u64::from(blocks)))
            .ok_or_else(dirty)?,
    };
    let last = log.sectors.checked_sub(1).ok_or_else(dirty)?;

    // The head: where the cycle number drops from the first sector's, or
    // the end if every sector is of one pass.
    let first = log.cycle(0).await?;
    let end = log.cycle(last).await?;
    let head = if first == end {
        log.sectors
    } else if Some(end) == first.checked_sub(1) {
        let (mut lo, mut hi) = (0, last);
        while hi.saturating_sub(lo) > 1 {
            let mid = lo.midpoint(hi);
            if log.cycle(mid).await? == first {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        hi
    } else {
        return Err(dirty());
    };

    // The last record: the nearest header before the head, which may be
    // in the previous pass, at the log's end.
    let before = head.min(MAX_RECORD_SECTORS);
    let mut window = log.read(head.saturating_sub(before), before).await?;
    let mut window_start = head.saturating_sub(before);
    let mut found = find_header(&window);
    if found.is_none() && before < MAX_RECORD_SECTORS {
        let wrapped = MAX_RECORD_SECTORS.saturating_sub(before).min(log.sectors);
        let wrap_start = log.sectors.saturating_sub(wrapped);
        let mut end = log.read(wrap_start, wrapped).await?;
        end.extend_from_slice(&window);
        window = end;
        window_start = wrap_start;
        found = find_header(&window);
    }
    let at = found.ok_or_else(dirty)?;
    let header = window.get(at..).ok_or_else(dirty)?;
    let record = window_start
        .checked_add(
            at.checked_div(SECTOR_LEN)
                .and_then(|n| u64::try_from(n).ok())
                .ok_or_else(dirty)?,
        )
        .ok_or_else(dirty)?
        .checked_rem(log.sectors)
        .ok_or_else(dirty)?;
    is_unmount(&log, header, record, head)
        .await?
        .then_some(())
        .ok_or_else(dirty)
}

/// Returns the offset in `window`, a run of sectors, of the last record
/// header in it.
fn find_header(window: &[u8]) -> Option<usize> {
    let (sectors, _) = window.as_chunks::<SECTOR_LEN>();
    let index = sectors
        .iter()
        .rposition(|sector| be::u32_at(sector, 0) == Some(RECORD_MAGIC))?;
    index.checked_mul(SECTOR_LEN)
}

/// Returns `true` if `header`, of the record at sector `record`, is an
/// unmount record that ends at sector `head`: one operation, of the log
/// itself, flagged as an unmount.
async fn is_unmount<S: ReadAt + ?Sized>(
    log: &Log<'_, S>,
    header: &[u8],
    record: u64,
    head: u64,
) -> Result<bool, Error> {
    let field = |at| be::u32_at(header, at);
    let (Some(version), Some(len), Some(ops)) = (field(VERSION), field(LEN), field(OPS)) else {
        return Ok(false);
    };
    if !matches!(version, 1 | 2) || ops != 1 {
        return Ok(false);
    }
    // Version 2 records of more than 32 KiB have extended headers.
    let size = if version == 2 {
        field(SIZE).unwrap_or(0)
    } else {
        0
    };
    let header_sectors = u64::from(size.div_ceil(HEADER_COVERS).max(1));
    let data_sectors = u64::from(len).div_ceil(SECTOR);
    let ends = record
        .checked_add(header_sectors)
        .and_then(|end| end.checked_add(data_sectors));
    let (Some(ends), Some(data)) = (ends, record.checked_add(header_sectors)) else {
        return Ok(false);
    };
    let wrap = |n: u64| n.checked_rem(log.sectors);
    if header_sectors.saturating_add(data_sectors) > MAX_RECORD_SECTORS
        || wrap(ends) != wrap(head)
        || data_sectors == 0
    {
        return Ok(false);
    }
    // The operation header starts the data; its first word, the
    // transaction id, is overwritten by the cycle number.
    let Some(data) = wrap(data) else {
        return Ok(false);
    };
    let sector = log.read(data, 1).await?;
    let (client, flags) = (sector.get(8).copied(), sector.get(9).copied());
    Ok(client == Some(CLIENT_LOG) && flags.is_some_and(|flags| flags & UNMOUNT != 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock::tests::Fields;
    use alloc::vec;
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// The log of `Fields::new()`'s volume: 200 blocks from AG 2's block 4.
    const LOG_BASE: usize = (2 * 256 + 4) * 4096;
    const LOG_SECTORS: u64 = 200 * 8;

    fn superblock() -> Superblock {
        Superblock::parse(&Fields::new().bytes()).unwrap()
    }

    /// A log being written: sectors stamped with cycle numbers, and record
    /// headers.
    struct Image(Vec<u8>);

    impl Image {
        /// A volume whose log was zeroed, then written to sector `head` in
        /// `cycle`, the rest left from the cycle before.
        fn new(cycle: u32, head: u64) -> Self {
            let mut image = Self(vec![0; 4096 * 1024]);
            for n in 0..LOG_SECTORS {
                let stamp = if n < head { cycle } else { cycle - 1 };
                image.sector(n)[..4].copy_from_slice(&stamp.to_be_bytes());
            }
            image
        }

        fn sector(&mut self, n: u64) -> &mut [u8] {
            let at = LOG_BASE + usize::try_from(n).unwrap() * 512;
            &mut self.0[at..at + 512]
        }

        /// Writes a record at sector `at` of `data` sectors holding `ops`
        /// operations, the first of `client` with `flags`.
        fn record(&mut self, at: u64, cycle: u32, data: u32, ops: u32, client: u8, flags: u8) {
            let header = self.sector(at);
            header[..4].copy_from_slice(&RECORD_MAGIC.to_be_bytes());
            header[4..8].copy_from_slice(&cycle.to_be_bytes());
            header[8..12].copy_from_slice(&2u32.to_be_bytes());
            header[12..16].copy_from_slice(&(data * 512).to_be_bytes());
            header[40..44].copy_from_slice(&ops.to_be_bytes());
            header[320..324].copy_from_slice(&HEADER_COVERS.to_be_bytes());
            let first = self.sector((at + 1) % LOG_SECTORS);
            first[8] = client;
            first[9] = flags;
        }

        fn check(&self) -> Result<(), XfsError> {
            block_on(check(&self.0, &superblock())).map_err(|error| {
                *core::error::Error::source(&error)
                    .and_then(|source| source.downcast_ref::<XfsError>())
                    .unwrap()
            })
        }
    }

    #[test]
    fn accepts_a_clean_log() {
        // As `mkfs.xfs` leaves it: one unmount record in cycle 1.
        let mut image = Image::new(1, 2);
        image.record(0, 1, 1, 1, CLIENT_LOG, UNMOUNT);
        assert_eq!(image.check(), Ok(()));
        // After transactions, unmounted cleanly.
        let mut image = Image::new(1, 12);
        image.record(0, 1, 1, 1, CLIENT_LOG, UNMOUNT);
        image.record(2, 1, 4, 14, 0x69, 0);
        image.record(7, 1, 1, 5, 0x69, 0x2);
        image.record(10, 1, 1, 1, CLIENT_LOG, UNMOUNT);
        assert_eq!(image.check(), Ok(()));
    }

    #[test]
    fn finds_the_head_across_passes() {
        // In cycle 5, written to sector 300 over cycle 4.
        let mut image = Image::new(5, 300);
        image.record(298, 5, 1, 1, CLIENT_LOG, UNMOUNT);
        assert_eq!(image.check(), Ok(()));
        // Every sector in one pass: the head is at the end.
        let mut image = Image::new(3, LOG_SECTORS);
        image.record(LOG_SECTORS - 2, 3, 1, 1, CLIENT_LOG, UNMOUNT);
        assert_eq!(image.check(), Ok(()));
        // A record of the previous pass, wrapping round to the head.
        let mut image = Image::new(4, 1);
        image.record(LOG_SECTORS - 1, 3, 1, 1, CLIENT_LOG, UNMOUNT);
        assert_eq!(image.check(), Ok(()));
    }

    #[test]
    fn refuses_a_log_needing_recovery() {
        // A transaction after the unmount record.
        let mut image = Image::new(1, 6);
        image.record(0, 1, 1, 1, CLIENT_LOG, UNMOUNT);
        image.record(2, 1, 3, 5, 0x69, 0x2);
        assert_eq!(image.check(), Err(XfsError::NeedsRecovery));
        assert_eq!(XfsError::NeedsRecovery.kind(), ErrorKind::Unsupported);
        // An unmount record not ending at the head.
        let mut image = Image::new(1, 4);
        image.record(0, 1, 1, 1, CLIENT_LOG, UNMOUNT);
        assert_eq!(image.check(), Err(XfsError::NeedsRecovery));
        // Not an unmount: another client, flag, or more operations.
        for (ops, client, flags) in [
            (1, 0x69, UNMOUNT),
            (1, CLIENT_LOG, 0x2),
            (2, CLIENT_LOG, UNMOUNT),
        ] {
            let mut image = Image::new(1, 2);
            image.record(0, 1, 1, ops, client, flags);
            assert_eq!(image.check(), Err(XfsError::NeedsRecovery));
        }
        // No header at all, or cycles that do not follow one another.
        assert_eq!(Image::new(1, 2).check(), Err(XfsError::NeedsRecovery));
        let mut image = Image::new(3, 2);
        image.record(0, 3, 1, 1, CLIENT_LOG, UNMOUNT);
        image.sector(LOG_SECTORS - 1)[..4].copy_from_slice(&1u32.to_be_bytes());
        assert_eq!(image.check(), Err(XfsError::NeedsRecovery));
    }

    #[test]
    fn refuses_an_external_log() {
        let mut f = Fields::new();
        f.log_start = 0;
        let sb = Superblock::parse(&f.bytes()).unwrap();
        let error = block_on(check(&vec![0u8; 4096 * 1024], &sb)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported);
    }
}
