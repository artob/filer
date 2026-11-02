// This is free and unencumbered software released into the public domain.

//! Log recovery, in memory: replaying the transactions a dirty log holds
//! over what the volume has, as Linux does when it mounts it, but into an
//! overlay that reads see instead of the volume's own bytes.
//!
//! A log record is a header, extended by a header sector for each 32 KiB
//! of data past the first, and data whose sectors' first words are saved
//! in the headers, the sectors stamped with the cycle number instead. The
//! data is operations: a header of the transaction's id, the length, the
//! client, and flags (big-endian), then their bytes, which may continue in
//! the next record. A transaction's operations are a start, a header of
//! its own, the regions of its items, and a commit. Items and their
//! regions are as the writing machine held them in memory, which their
//! type numbers, little-endian or big-endian, tell; buffers' data and
//! inodes' forks are as on disk.
//!
//! A buffer item names a block by its disk address and length, and which
//! 128-byte chunks of it are logged, each run of them in a region of its
//! own. A buffer freed later is cancelled: logged with the cancel flag,
//! which earlier items of it are not replayed past, nor items of the
//! inodes in it, as a freed chunk of inodes may now hold a file's data.
//! An inode item names where the inode is, its cluster's buffer too, and
//! which parts of it are logged: the core, in
//! memory's order, and the forks. With version 5, a block or an inode
//! whose own LSN is the transaction's or later is not replayed over, and
//! each replayed is resealed, as Linux writes them.
//!
//! Intents are deferred work, done in later transactions, whose items
//! name them. Those unfinished at the log's end are done by recovery:
//! freeing extents and updating reverse maps and reference counts, which
//! change nothing a reader sees; but mapping extents into files,
//! exchanging their ranges, and setting attributes, which this driver
//! does not do, so volumes needing them are refused.

use crate::{
    XfsError, crc,
    log::{Dirty, Log},
    superblock::Superblock,
};
use alloc::{collections::BTreeMap, vec, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::{
    be, le, read_exact,
    recover::{CHUNK, Overlay},
};

/// The size of a sector, of which buffers are logged in 128-byte chunks.
const SECTOR: u64 = 512;
const SECTOR_LEN: usize = 512;

/// A record header's magic number, and where it keeps its fields.
const RECORD_MAGIC: u32 = 0xFEED_BABE;
const VERSION: usize = 8;
const LEN: usize = 12;
const LSN: usize = 16;
const OPS: usize = 40;
const CYCLE_DATA: usize = 44;
const SIZE: usize = 320;

/// A header sector covers 32 KiB of a record's data, a word for each
/// sector; an extended header sector's words follow its cycle.
const HEADER_COVERS: u32 = 32 << 10;
const WORDS: usize = 64;
const EXT_CYCLE_DATA: usize = 4;

/// The largest record read: 256 KiB of data.
const MAX_RECORD: u32 = 256 << 10;

/// An operation's header, and its clients' and flags' values.
const OP_HEADER: usize = 12;
const CLIENT_TRANSACTION: u8 = 0x69;
const START: u8 = 0x1;
const COMMIT: u8 = 0x2;
const CONTINUE: u8 = 0x4;
const WAS_CONTINUED: u8 = 0x8;

/// A transaction header's magic number.
const TRANS_MAGIC: u32 = 0x5452_414E;

/// Log item types.
mod item {
    pub(super) const EFI: u16 = 0x1236;
    pub(super) const EFD: u16 = 0x1237;
    pub(super) const INODE: u16 = 0x123B;
    pub(super) const BUF: u16 = 0x123C;
    pub(super) const DQUOT: u16 = 0x123D;
    pub(super) const QUOTAOFF: u16 = 0x123E;
    pub(super) const ICREATE: u16 = 0x123F;
    pub(super) const RUI: u16 = 0x1240;
    pub(super) const RUD: u16 = 0x1241;
    pub(super) const CUI: u16 = 0x1242;
    pub(super) const CUD: u16 = 0x1243;
    pub(super) const BUI: u16 = 0x1244;
    pub(super) const BUD: u16 = 0x1245;
    pub(super) const ATTRI: u16 = 0x1246;
    pub(super) const ATTRD: u16 = 0x1247;
    pub(super) const XMI: u16 = 0x1248;
    pub(super) const XMD: u16 = 0x1249;
    pub(super) const EFI_RT: u16 = 0x124A;
    pub(super) const EFD_RT: u16 = 0x124B;
    pub(super) const RUI_RT: u16 = 0x124C;
    pub(super) const RUD_RT: u16 = 0x124D;
    pub(super) const CUI_RT: u16 = 0x124E;
    pub(super) const CUD_RT: u16 = 0x124F;
}

/// Buffer item flags: an inode buffer, of which only unlinked list
/// pointers are replayed, which reads never follow; a cancelled buffer;
/// and quota buffers, which reads never use.
const BUF_INODE: u16 = 0x1;
const BUF_CANCEL: u16 = 0x2;
const BUF_QUOTA: u16 = 0x1C;

/// Inode item fields: the core is logged, and which of each fork's forms.
const LOG_DATA: u32 = 0x2;
const LOG_EXTENTS: u32 = 0x4;
const LOG_DATA_ROOT: u32 = 0x8;
const LOG_DEV: u32 = 0x10;
const LOG_ATTR_DATA: u32 = 0x40;
const LOG_ATTR_EXTENTS: u32 = 0x80;
const LOG_ATTR_ROOT: u32 = 0x100;

/// Where a version 3 inode keeps its CRC and LSN; and where version 2
/// keeps its flush count.
const INODE_CRC: usize = 100;
const INODE_LSN: usize = 112;
const INODE_FLUSHITER: usize = 30;

/// The sizes of a logged core and of an inode's core, by version.
const CORE_V2: usize = 96;
const CORE_V3: usize = 176;
const DISK_CORE_V2: usize = 100;

/// `di_flags2`: 64-bit times, and wide extent counts.
const BIGTIME: u64 = 0x8;
const NREXT64: u64 = 0x10;

/// The size of a B+ tree block's header in a logged root, and of the
/// root's on disk.
const BMAP_HEADER: usize = 24;
const BMAP_HEADER_V3: usize = 72;
const ROOT_HEADER: usize = 4;

/// The largest buffer replayed: twice the largest directory block.
const MAX_BUFFER: u64 = 128 << 10;

/// How the machine that wrote items ordered their bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    Little,
    Big,
}

impl Order {
    fn u16_at(self, bytes: &[u8], at: usize) -> Option<u16> {
        match self {
            Self::Little => le::u16_at(bytes, at),
            Self::Big => be::u16_at(bytes, at),
        }
    }

    fn u32_at(self, bytes: &[u8], at: usize) -> Option<u32> {
        match self {
            Self::Little => le::u32_at(bytes, at),
            Self::Big => be::u32_at(bytes, at),
        }
    }

    fn u64_at(self, bytes: &[u8], at: usize) -> Option<u64> {
        match self {
            Self::Little => le::u64_at(bytes, at),
            Self::Big => be::u64_at(bytes, at),
        }
    }

    /// Returns the order in which `magic` reads as `TRANS_MAGIC`.
    fn of_transaction(header: &[u8]) -> Option<Self> {
        [Self::Little, Self::Big]
            .into_iter()
            .find(|order| order.u32_at(header, 0) == Some(TRANS_MAGIC))
    }

    /// Copies a `size`-byte integer at `at` of `from`, in this order, to
    /// `to` big-endian.
    fn to_disk(self, from: &[u8], to: &mut [u8], at: usize, size: usize) -> Option<()> {
        let field = from.get(at..at.checked_add(size)?)?;
        let target = to.get_mut(at..at.checked_add(size)?)?;
        target.copy_from_slice(field);
        if self == Self::Little {
            target.reverse();
        }
        Some(())
    }
}

/// A committed transaction: the LSN of the record it started in, the byte
/// order of its items, and its regions, its header first.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Transaction {
    lsn: u64,
    regions: Vec<Vec<u8>>,
}

/// A transaction being read: its LSN, its regions so far, and an operation
/// continued in the next record.
#[derive(Default)]
struct Pending {
    lsn: u64,
    regions: Vec<Vec<u8>>,
    partial: Option<Vec<u8>>,
}

fn bad() -> XfsError {
    XfsError::BadLogRecord
}

/// Reads the committed transactions of `dirty`, in order.
async fn transactions<S: ReadAt + ?Sized>(
    log: &Log<'_, S>,
    dirty: Dirty,
) -> Result<Vec<Transaction>, Error> {
    let op = OpKind::Open;
    let fail = |error: XfsError| error.into_error(op);
    let mut pending: BTreeMap<u32, Pending> = BTreeMap::new();
    let mut done = Vec::new();
    let mut at = dirty.tail;
    let mut walked = 0u64;
    while at != dirty.head {
        let header = log.read_wrapping(at, 1).await?;
        let field = |at| be::u32_at(&header, at);
        let (Some(RECORD_MAGIC), Some(version), Some(len), Some(ops)) =
            (field(0), field(VERSION), field(LEN), field(OPS))
        else {
            return Err(fail(bad()));
        };
        let lsn = be::u64_at(&header, LSN).ok_or_else(|| fail(bad()))?;
        let size = if version == 2 {
            field(SIZE).unwrap_or(0)
        } else {
            0
        };
        if !matches!(version, 1 | 2) || len > MAX_RECORD || size > MAX_RECORD {
            return Err(fail(bad()));
        }
        let header_sectors = u64::from(size.div_ceil(HEADER_COVERS).max(1));
        let data_sectors = u64::from(len).div_ceil(SECTOR);
        let headers = log.read_wrapping(at, header_sectors).await?;
        let mut data = log
            .read_wrapping(at.saturating_add(header_sectors), data_sectors)
            .await?;
        restore(&headers, &mut data).ok_or_else(|| fail(bad()))?;
        data.truncate(usize::try_from(len).map_err(|_| fail(bad()))?);
        operations(&data, ops, lsn, &mut pending, &mut done).ok_or_else(|| fail(bad()))?;
        let sectors = header_sectors.saturating_add(data_sectors);
        walked = walked.saturating_add(sectors);
        if walked > log.sectors {
            return Err(fail(bad()));
        }
        at = at
            .saturating_add(sectors)
            .checked_rem(log.sectors)
            .ok_or_else(|| fail(bad()))?;
    }
    Ok(done)
}

/// Puts back the first word of each sector of a record's `data`, which
/// the record's `headers` saved.
fn restore(headers: &[u8], data: &mut [u8]) -> Option<()> {
    for (i, sector) in data.chunks_mut(SECTOR_LEN).enumerate() {
        let header = i.checked_div(WORDS)?;
        let word = i.checked_rem(WORDS)?;
        let base = if header == 0 {
            CYCLE_DATA
        } else {
            header
                .checked_mul(SECTOR_LEN)?
                .checked_add(EXT_CYCLE_DATA)?
        };
        let saved = headers.get(base.checked_add(word.checked_mul(4)?)?..)?;
        let saved = saved.first_chunk::<4>()?;
        sector.get_mut(..4)?.copy_from_slice(saved);
    }
    Some(())
}

/// Reads the `count` operations of a record's `data`, of LSN `lsn`, into
/// the transactions `pending`, moving those committed to `done`.
fn operations(
    data: &[u8],
    count: u32,
    lsn: u64,
    pending: &mut BTreeMap<u32, Pending>,
    done: &mut Vec<Transaction>,
) -> Option<()> {
    let mut at = 0usize;
    for _ in 0..count {
        let tid = be::u32_at(data, at)?;
        let len = usize::try_from(be::u32_at(data, at.checked_add(4)?)?).ok()?;
        let client = *data.get(at.checked_add(8)?)?;
        let flags = *data.get(at.checked_add(9)?)?;
        let start = at.checked_add(OP_HEADER)?;
        let bytes = data.get(start..start.checked_add(len)?)?;
        at = start.checked_add(len)?;
        if client != CLIENT_TRANSACTION {
            continue;
        }
        if flags & START != 0 {
            pending.insert(
                tid,
                Pending {
                    lsn,
                    ..Pending::default()
                },
            );
            continue;
        }
        // Operations of a transaction whose start is before the tail are
        // of work already done.
        let Some(transaction) = pending.get_mut(&tid) else {
            continue;
        };
        if flags & COMMIT != 0 {
            let transaction = pending.remove(&tid)?;
            done.push(Transaction {
                lsn: transaction.lsn,
                regions: transaction.regions,
            });
            continue;
        }
        let mut region = if flags & WAS_CONTINUED != 0 {
            transaction.partial.take().unwrap_or_default()
        } else {
            Vec::new()
        };
        region.extend_from_slice(bytes);
        if flags & CONTINUE != 0 {
            transaction.partial = Some(region);
        } else if !region.is_empty() {
            transaction.regions.push(region);
        }
    }
    Some(())
}

/// An item of a transaction: its type, and its regions, its format first.
struct Item<'a> {
    kind: u16,
    regions: &'a [Vec<u8>],
}

/// Splits a transaction's regions, after its header, into items, each of
/// as many regions as its format says.
fn items(transaction: &Transaction, order: Order) -> Option<Vec<Item<'_>>> {
    let mut items = Vec::new();
    let mut rest = transaction.regions.get(1..)?;
    while let Some(format) = rest.first() {
        let kind = order.u16_at(format, 0)?;
        let count = usize::from(order.u16_at(format, 2)?).max(1);
        let (regions, after) = rest.split_at_checked(count)?;
        items.push(Item { kind, regions });
        rest = after;
    }
    Some(items)
}

/// Replays `dirty`, the log of the volume in `source`, returning the
/// overlay to read it through.
pub(crate) async fn replay<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    log: &Log<'_, S>,
    dirty: Dirty,
) -> Result<Overlay, Error> {
    let op = OpKind::Open;
    let fail = |error: XfsError| error.into_error(op);
    let transactions = transactions(log, dirty).await?;

    // The first pass: cancelled buffers, and unfinished intents.
    let mut cancels: BTreeMap<(u64, u16), u32> = BTreeMap::new();
    let mut intents: BTreeMap<u64, u16> = BTreeMap::new();
    let mut parsed = Vec::with_capacity(transactions.len());
    for transaction in &transactions {
        let header = transaction.regions.first().ok_or_else(|| fail(bad()))?;
        let order = Order::of_transaction(header).ok_or_else(|| fail(bad()))?;
        let items = items(transaction, order).ok_or_else(|| fail(bad()))?;
        for item in &items {
            let format = item.regions.first().ok_or_else(|| fail(bad()))?;
            match item.kind {
                item::BUF => {
                    let buffer = BufFormat::parse(format, order).ok_or_else(|| fail(bad()))?;
                    if buffer.flags & BUF_CANCEL != 0 {
                        let count = cancels.entry((buffer.daddr, buffer.len)).or_default();
                        *count = count.saturating_add(1);
                    }
                },
                item::EFI | item::RUI | item::CUI | item::BUI | item::ATTRI | item::XMI => {
                    let id = order.u64_at(format, 8).ok_or_else(|| fail(bad()))?;
                    intents.insert(id, item.kind);
                },
                item::EFI_RT | item::RUI_RT | item::CUI_RT => {
                    let id = order.u64_at(format, 8).ok_or_else(|| fail(bad()))?;
                    intents.insert(id, item.kind);
                },
                item::EFD
                | item::RUD
                | item::CUD
                | item::BUD
                | item::ATTRD
                | item::XMD
                | item::EFD_RT
                | item::RUD_RT
                | item::CUD_RT => {
                    let id = order.u64_at(format, 8).ok_or_else(|| fail(bad()))?;
                    intents.remove(&id);
                },
                item::INODE | item::DQUOT | item::QUOTAOFF | item::ICREATE => {},
                kind => return Err(fail(XfsError::UnsupportedLogItem(kind))),
            }
        }
        parsed.push((transaction.lsn, order, items));
    }
    if let Some(&kind) = intents
        .values()
        .find(|&&kind| matches!(kind, item::BUI | item::ATTRI | item::XMI))
    {
        return Err(fail(XfsError::UnsupportedLogItem(kind)));
    }

    // The second pass: buffers and inodes not cancelled later.
    let mut overlay = Overlay::default();
    for (lsn, order, items) in parsed {
        for item in items {
            match item.kind {
                item::BUF => {
                    let replayer = Replayer {
                        source,
                        sb,
                        overlay: &mut overlay,
                        lsn,
                        order,
                    };
                    replayer.buffer(item.regions, &mut cancels).await?;
                },
                item::INODE => {
                    let replayer = Replayer {
                        source,
                        sb,
                        overlay: &mut overlay,
                        lsn,
                        order,
                    };
                    replayer.inode(item.regions, &cancels).await?;
                },
                _ => {},
            }
        }
    }
    Ok(overlay)
}

/// A buffer item's format: its flags, the disk address and length (in
/// 512-byte units) of the buffer, and the bitmap of its logged chunks.
struct BufFormat {
    flags: u16,
    len: u16,
    daddr: u64,
    map: Vec<u32>,
}

impl BufFormat {
    fn parse(format: &[u8], order: Order) -> Option<Self> {
        let words = usize::try_from(order.u32_at(format, 16)?).ok()?;
        let map = (0..words)
            .map(|i| order.u32_at(format, i.checked_mul(4)?.checked_add(20)?))
            .collect::<Option<Vec<u32>>>()?;
        Some(Self {
            flags: order.u16_at(format, 4)?,
            len: order.u16_at(format, 6)?,
            daddr: order.u64_at(format, 8)?,
            map,
        })
    }

    /// Returns the runs of logged chunks: their first, and their count.
    fn runs(&self) -> Vec<(usize, usize)> {
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let bits = self.map.len().saturating_mul(32);
        for bit in 0..bits {
            let word = self.map.get(bit / 32).copied().unwrap_or(0);
            if word & (1 << (bit % 32)) == 0 {
                continue;
            }
            match runs.last_mut() {
                Some((first, count)) if first.saturating_add(*count) == bit => {
                    *count = count.saturating_add(1);
                },
                _ => runs.push((bit, 1)),
            }
        }
        runs
    }
}

/// What replays one transaction's items.
struct Replayer<'a, S: ?Sized> {
    source: &'a S,
    sb: &'a Superblock,
    overlay: &'a mut Overlay,
    lsn: u64,
    order: Order,
}

impl<S: ReadAt + ?Sized> Replayer<'_, S> {
    fn fail(&self) -> Error {
        bad().into_error(OpKind::Open)
    }

    /// Reads `len` bytes at `at`, through what is replayed so far.
    async fn read(&self, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let mut bytes = read_exact(self.source, at, len, OpKind::Open).await?;
        self.overlay.lay(at, &mut bytes);
        Ok(bytes)
    }

    /// Replays a buffer item, unless it is cancelled later.
    async fn buffer(
        self,
        regions: &[Vec<u8>],
        cancels: &mut BTreeMap<(u64, u16), u32>,
    ) -> Result<(), Error> {
        let format = regions.first().ok_or_else(|| self.fail())?;
        let buffer = BufFormat::parse(format, self.order).ok_or_else(|| self.fail())?;
        let key = (buffer.daddr, buffer.len);
        if buffer.flags & BUF_CANCEL != 0 {
            if let Some(count) = cancels.get_mut(&key) {
                *count = count.saturating_sub(1);
            }
            return Ok(());
        }
        if cancels.get(&key).is_some_and(|&count| count > 0)
            || buffer.flags & (BUF_INODE | BUF_QUOTA) != 0
        {
            return Ok(());
        }
        let at = buffer
            .daddr
            .checked_mul(SECTOR)
            .ok_or_else(|| self.fail())?;
        let size = u64::from(buffer.len).saturating_mul(SECTOR);
        if size == 0 || size > MAX_BUFFER || at.saturating_add(size) > self.sb.volume_len() {
            return Err(self.fail());
        }
        let len = usize::try_from(size).map_err(|_| self.fail())?;
        let mut bytes = self.read(at, len).await?;
        if self.sb.is_v5() && block_lsn(&bytes).is_some_and(|lsn| newer(lsn, self.lsn)) {
            return Ok(());
        }
        let mut data = regions.get(1..).unwrap_or_default().iter();
        for (first, count) in buffer.runs() {
            let region = data.next().ok_or_else(|| self.fail())?;
            let start = first.checked_mul(CHUNK).ok_or_else(|| self.fail())?;
            let n = count
                .checked_mul(CHUNK)
                .ok_or_else(|| self.fail())?
                .min(region.len());
            let target = bytes
                .get_mut(start..start.saturating_add(n))
                .ok_or_else(|| self.fail())?;
            target.copy_from_slice(region.get(..n).ok_or_else(|| self.fail())?);
        }
        if self.sb.is_v5() {
            reseal(&mut bytes);
        }
        if !self.overlay.put(at, &bytes) {
            return Err(self.fail());
        }
        Ok(())
    }

    /// Replays an inode item: its core, and the forks it logs; unless its
    /// cluster's buffer is cancelled later.
    async fn inode(
        self,
        regions: &[Vec<u8>],
        cancels: &BTreeMap<(u64, u16), u32>,
    ) -> Result<(), Error> {
        let order = self.order;
        let format = regions.first().ok_or_else(|| self.fail())?;
        let field16 = |at| order.u16_at(format, at).ok_or_else(|| self.fail());
        let field32 = |at| order.u32_at(format, at).ok_or_else(|| self.fail());
        let field64 = |at| order.u64_at(format, at).ok_or_else(|| self.fail());
        let fields = field32(4)?;
        let attr_size = usize::from(field16(8)?);
        let data_size = usize::from(field16(10)?);
        let number = field64(16)?;
        let dev = field32(24)?;
        let daddr = field64(40)?;
        let cluster = field32(48)?;
        let offset = u64::from(field32(52)?);
        // A cluster longer than any buffer is cancelled by none.
        if let Ok(cluster) = u16::try_from(cluster)
            && cancels
                .get(&(daddr, cluster))
                .is_some_and(|&count| count > 0)
        {
            return Ok(());
        }
        let at = daddr
            .checked_mul(SECTOR)
            .and_then(|at| at.checked_add(offset))
            .ok_or_else(|| self.fail())?;
        if self.sb.inode_offset(number) != Some(at) {
            return Err(self.fail());
        }
        let size = usize::try_from(self.sb.inode_size()).map_err(|_| self.fail())?;
        let mut inode = self.read(at, size).await?;
        let core = regions.get(1).ok_or_else(|| self.fail())?;
        let v3 = core.get(4) == Some(&3);
        let on_disk_magic = be::u16_at(&inode, 0) == Some(0x494E);
        if on_disk_magic && v3 {
            if be::u64_at(&inode, INODE_LSN).is_some_and(|lsn| newer(lsn, self.lsn)) {
                return Ok(());
            }
        } else if on_disk_magic {
            let logged = order.u16_at(core, INODE_FLUSHITER);
            let disk = be::u16_at(&inode, INODE_FLUSHITER);
            if let (Some(logged), Some(disk)) = (logged, disk)
                && logged < disk
                && disk != u16::MAX
            {
                return Ok(());
            }
        }
        core_to_disk(core, &mut inode, order, v3).ok_or_else(|| self.fail())?;

        // The forks, in the literal area after the core.
        let literal = if v3 { CORE_V3 } else { DISK_CORE_V2 };
        let fork_off = usize::from(*inode.get(82).ok_or_else(|| self.fail())?)
            .checked_mul(8)
            .ok_or_else(|| self.fail())?;
        let area = size.checked_sub(literal).ok_or_else(|| self.fail())?;
        let data_area = if fork_off == 0 { area } else { fork_off };
        let attr_area = area.saturating_sub(fork_off);
        let mut next = regions.get(2..).unwrap_or_default().iter();
        if fields & (LOG_DATA | LOG_EXTENTS | LOG_DATA_ROOT) != 0 {
            let region = next.next().ok_or_else(|| self.fail())?;
            let region = region.get(..data_size).ok_or_else(|| self.fail())?;
            let fork = inode
                .get_mut(literal..literal.saturating_add(data_area))
                .ok_or_else(|| self.fail())?;
            fork_to_disk(region, fork, fields & LOG_DATA_ROOT != 0, self.sb)
                .ok_or_else(|| self.fail())?;
        } else if fields & LOG_DEV != 0 {
            let fork = inode
                .get_mut(literal..literal.saturating_add(4))
                .ok_or_else(|| self.fail())?;
            fork.copy_from_slice(&dev.to_be_bytes());
        }
        if fields & (LOG_ATTR_DATA | LOG_ATTR_EXTENTS | LOG_ATTR_ROOT) != 0 {
            let region = next.next().ok_or_else(|| self.fail())?;
            let region = region.get(..attr_size).ok_or_else(|| self.fail())?;
            let start = literal.saturating_add(fork_off);
            let fork = inode
                .get_mut(start..start.saturating_add(attr_area))
                .ok_or_else(|| self.fail())?;
            fork_to_disk(region, fork, fields & LOG_ATTR_ROOT != 0, self.sb)
                .ok_or_else(|| self.fail())?;
        }
        if v3 {
            let lsn = inode
                .get_mut(INODE_LSN..INODE_LSN.saturating_add(8))
                .ok_or_else(|| self.fail())?;
            lsn.copy_from_slice(&self.lsn.to_be_bytes());
            crc::seal(&mut inode, INODE_CRC);
        }
        if !self.overlay.put(at, &inode) {
            return Err(self.fail());
        }
        Ok(())
    }
}

/// Returns `true` if a block's or an inode's LSN `found` is that of the
/// transaction, `lsn`, or later: its cycle, then its block.
fn newer(found: u64, lsn: u64) -> bool {
    found != u64::MAX && found >= lsn
}

/// Where a version 5 block of each kind keeps its LSN and its CRC, by its
/// magic number: at its start, or at 8.
fn block_fields(bytes: &[u8]) -> Option<(usize, usize)> {
    match (be::u32_at(bytes, 0)?, be::u16_at(bytes, 8)?) {
        // Directory data and free-space blocks.
        (0x5844_4233 | 0x5844_4433 | 0x5844_4633, _) => Some((16, 4)),
        (0x424D_4133, _) => Some((32, 64)),
        (0x5853_4C4D, _) => Some((48, 12)),
        (_, 0x3DF1 | 0x3DFF | 0x3EBE | 0x3BEE) => Some((24, 12)),
        _ => None,
    }
}

/// Returns a version 5 block's LSN, if it is of a kind this driver reads.
fn block_lsn(bytes: &[u8]) -> Option<u64> {
    let (lsn, _) = block_fields(bytes)?;
    be::u64_at(bytes, lsn)
}

/// Seals a replayed version 5 block with its CRC, if it is of a kind this
/// driver reads; or the superblock.
fn reseal(bytes: &mut [u8]) {
    if let Some((_, crc)) = block_fields(bytes) {
        crc::seal(bytes, crc);
    } else if be::u32_at(bytes, 0) == Some(0x5846_5342) {
        crc::seal(bytes, 224);
    }
}

/// Writes a logged core, in memory's `order`, to the start of `inode` as
/// on disk: every field byte-swapped as need be, of version 3 if `v3`.
fn core_to_disk(core: &[u8], inode: &mut [u8], order: Order, v3: bool) -> Option<()> {
    let len = if v3 { CORE_V3 } else { CORE_V2 };
    let core = core.get(..len)?;
    let flags2 = if v3 { order.u64_at(core, 120)? } else { 0 };
    let mut disk = inode.get(..len)?.to_vec();
    disk.copy_from_slice(core);
    let mut swap = |at: usize, size: usize| order.to_disk(core, &mut disk, at, size);
    // Magic, mode, version, format, links, owner, group, link count, and
    // project id.
    for (at, size) in [
        (0, 2),
        (2, 2),
        (6, 2),
        (8, 4),
        (12, 4),
        (16, 4),
        (20, 2),
        (22, 2),
    ] {
        swap(at, size)?;
    }
    if flags2 & NREXT64 != 0 {
        swap(24, 8)?;
    } else {
        swap(INODE_FLUSHITER, 2)?;
    }
    let mut times = vec![32, 40, 48];
    if v3 {
        times.push(144);
    }
    for at in times {
        if flags2 & BIGTIME != 0 {
            swap(at, 8)?;
        } else {
            swap(at, 4)?;
            swap(at.checked_add(4)?, 4)?;
        }
    }
    // Size, blocks, extent size, extent counts, fork offset and format,
    // DMAPI fields, flags, and generation.
    for (at, size) in [
        (56, 8),
        (64, 8),
        (72, 4),
        (76, 4),
        (80, 2),
        (84, 4),
        (88, 2),
        (90, 2),
        (92, 4),
    ] {
        swap(at, size)?;
    }
    if v3 {
        // Next unlinked, CRC, change count, LSN, flags, CoW extent size,
        // and number; the UUID is as it is.
        for (at, size) in [
            (96, 4),
            (100, 4),
            (104, 8),
            (112, 8),
            (120, 8),
            (128, 4),
            (152, 8),
        ] {
            swap(at, size)?;
        }
    }
    // Version 2's logged core ends before the next unlinked inode, which
    // stays as it is.
    inode.get_mut(..len)?.copy_from_slice(&disk);
    Some(())
}

/// Writes a logged fork, `region`, to `fork`: as it is, or for a B+ tree
/// root (`root`), from a block's form, its header and as many entries as
/// the region holds, to the root's, sized to the fork.
fn fork_to_disk(region: &[u8], fork: &mut [u8], root: bool, sb: &Superblock) -> Option<()> {
    if !root {
        fork.get_mut(..region.len())?.copy_from_slice(region);
        return Some(());
    }
    let header = if sb.is_v5() {
        BMAP_HEADER_V3
    } else {
        BMAP_HEADER
    };
    let level = be::u16_at(region, 4)?;
    let count = usize::from(be::u16_at(region, 6)?);
    let logged = region.len().checked_sub(header)?.checked_div(16)?;
    let fits = fork.len().checked_sub(ROOT_HEADER)?.checked_div(16)?;
    if count > logged || count > fits {
        return None;
    }
    fork.fill(0);
    fork.get_mut(..2)?.copy_from_slice(&level.to_be_bytes());
    fork.get_mut(2..4)?
        .copy_from_slice(&u16::try_from(count).ok()?.to_be_bytes());
    for i in 0..count {
        let key = header.checked_add(i.checked_mul(8)?)?;
        let ptr = header
            .checked_add(logged.checked_mul(8)?)?
            .checked_add(i.checked_mul(8)?)?;
        let to_key = ROOT_HEADER.checked_add(i.checked_mul(8)?)?;
        let to_ptr = ROOT_HEADER
            .checked_add(fits.checked_mul(8)?)?
            .checked_add(i.checked_mul(8)?)?;
        let key = *region.get(key..)?.first_chunk::<8>()?;
        let ptr = *region.get(ptr..)?.first_chunk::<8>()?;
        fork.get_mut(to_key..to_key.checked_add(8)?)?
            .copy_from_slice(&key);
        fork.get_mut(to_ptr..to_ptr.checked_add(8)?)?
            .copy_from_slice(&ptr);
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{file_map::tests::record, log, superblock::tests::Fields};
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// The log of `Fields::new()`'s volume: from AG 2's block 4.
    const LOG_BASE: usize = (2 * 256 + 4) * 4096;

    /// A volume whose log is being written, record by record, in cycle 1.
    struct Image {
        bytes: Vec<u8>,
        /// The next record's sector.
        at: usize,
        order: Order,
    }

    impl Image {
        fn new(order: Order) -> Self {
            let mut bytes = vec![0; 4096 * 1024];
            bytes[..512].copy_from_slice(&Fields::new().bytes());
            Self {
                bytes,
                at: 0,
                order,
            }
        }

        fn u16(&self, n: u16) -> [u8; 2] {
            match self.order {
                Order::Little => n.to_le_bytes(),
                Order::Big => n.to_be_bytes(),
            }
        }

        fn u32(&self, n: u32) -> [u8; 4] {
            match self.order {
                Order::Little => n.to_le_bytes(),
                Order::Big => n.to_be_bytes(),
            }
        }

        fn u64(&self, n: u64) -> [u8; 8] {
            match self.order {
                Order::Little => n.to_le_bytes(),
                Order::Big => n.to_be_bytes(),
            }
        }

        /// Writes a record of `ops`, operations of (flags, bytes) of
        /// transaction 7, its first sector's words saved in its header.
        fn record(&mut self, ops: &[(u8, Vec<u8>)]) {
            let mut data = Vec::new();
            for (flags, bytes) in ops {
                data.extend(7u32.to_be_bytes());
                data.extend(u32::try_from(bytes.len()).unwrap().to_be_bytes());
                data.extend([CLIENT_TRANSACTION, *flags, 0, 0]);
                data.extend(bytes);
            }
            let len = data.len();
            data.resize(len.next_multiple_of(512), 0);
            let sectors = data.len() / 512;
            let header = LOG_BASE + self.at * 512;
            let h = &mut self.bytes[header..header + 512];
            h.fill(0);
            h[..4].copy_from_slice(&RECORD_MAGIC.to_be_bytes());
            h[4..8].copy_from_slice(&1u32.to_be_bytes());
            h[8..12].copy_from_slice(&2u32.to_be_bytes());
            h[12..16].copy_from_slice(&u32::try_from(len).unwrap().to_be_bytes());
            let lsn = (1u64 << 32) | u64::try_from(self.at).unwrap();
            h[16..24].copy_from_slice(&lsn.to_be_bytes());
            h[24..32].copy_from_slice(&(1u64 << 32).to_be_bytes());
            h[40..44].copy_from_slice(&u32::try_from(ops.len()).unwrap().to_be_bytes());
            h[320..324].copy_from_slice(&HEADER_COVERS.to_be_bytes());
            for i in 0..sectors {
                let sector = &mut data[i * 512..(i + 1) * 512];
                h[44 + 4 * i..48 + 4 * i].copy_from_slice(&sector[..4]);
                sector[..4].copy_from_slice(&1u32.to_be_bytes());
            }
            let at = header + 512;
            self.bytes[at..at + data.len()].copy_from_slice(&data);
            self.at += 1 + sectors;
            // Past the head, the previous cycle.
            let head = LOG_BASE + self.at * 512;
            self.bytes[head..head + 4].copy_from_slice(&0u32.to_be_bytes());
        }

        /// A transaction's start and header, of `items` items.
        fn start(&self, items: u32) -> Vec<(u8, Vec<u8>)> {
            let mut header = self.u32(TRANS_MAGIC).to_vec();
            header.extend(self.u32(0x28));
            header.extend(self.u32(7));
            header.extend(self.u32(items));
            vec![(START, Vec::new()), (0, header)]
        }

        /// A buffer item's format, of `flags`, at `daddr`, of `len` sectors,
        /// logging chunks `map`.
        fn buffer(&self, flags: u16, daddr: u64, len: u16, map: u32, regions: u16) -> Vec<u8> {
            let mut b = self.u16(item::BUF).to_vec();
            b.extend(self.u16(regions));
            b.extend(self.u16(flags));
            b.extend(self.u16(len));
            b.extend(self.u64(daddr));
            b.extend(self.u32(1));
            b.extend(self.u32(map));
            b
        }

        /// An intent item or a done item of `kind`, of id `id`.
        fn intent(&self, kind: u16, id: u64) -> Vec<u8> {
            let mut b = self.u16(kind).to_vec();
            b.extend(self.u16(1));
            b.extend(self.u32(0));
            b.extend(self.u64(id));
            b
        }

        /// An inode item for inode `number`, of `fields`, whose data fork
        /// is `dsize` bytes.
        fn inode_format(&self, number: u64, fields: u32, dsize: u16) -> Vec<u8> {
            let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
            let at = sb.inode_offset(number).unwrap();
            let mut b = self.u16(item::INODE).to_vec();
            b.extend(self.u16(3));
            b.extend(self.u32(fields));
            b.extend(self.u16(0));
            b.extend(self.u16(dsize));
            b.extend([0; 4]);
            b.extend(self.u64(number));
            b.extend([0; 16]);
            b.extend(self.u64(at / 512 / 16 * 16));
            b.extend(self.u32(16));
            b.extend(self.u32(u32::try_from(at % (512 * 16)).unwrap()));
            b
        }

        /// A version 2 core, in memory's order: a regular file of `size`
        /// bytes, of one extent, at 2024-01-02T03:04:06.000000005Z.
        fn core(&self, size: u64) -> Vec<u8> {
            let mut b = vec![0; CORE_V2];
            let mut put = |at: usize, bytes: &[u8]| b[at..at + bytes.len()].copy_from_slice(bytes);
            put(0, &self.u16(0x494E));
            put(2, &self.u16(0o100_644));
            put(4, &[2, 2]);
            put(16, &self.u32(1));
            for at in [32, 40, 48] {
                put(at, &self.u32(1_704_164_646));
                put(at + 4, &self.u32(5));
            }
            put(56, &self.u64(size));
            put(64, &self.u64(1));
            put(76, &self.u32(1));
            put(83, &[2]);
            b
        }

        fn replay(&self) -> Result<Overlay, Error> {
            let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
            let dirty = block_on(log::check(&self.bytes, &sb))?.expect("dirty");
            let log = log::of(&self.bytes[..], &sb)?;
            block_on(replay(&self.bytes[..], &sb, &log, dirty))
        }

        /// Reads `len` bytes at `at` through what is replayed.
        fn read(&self, overlay: &Overlay, at: usize, len: usize) -> Vec<u8> {
            let mut bytes = self.bytes[at..at + len].to_vec();
            overlay.lay(u64::try_from(at).unwrap(), &mut bytes);
            bytes
        }
    }

    #[test]
    fn replays_inodes_but_not_in_cancelled_clusters() {
        let image = |cancel: bool, after: bool| {
            let mut image = Image::new(Order::Little);
            let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
            let cluster = sb.inode_offset(131).unwrap() / 512 / 16 * 16;
            let mut ops = image.start(1 + u32::from(cancel) + u32::from(after));
            ops.push((0, image.inode_format(131, 0x1 | LOG_EXTENTS, 16)));
            ops.push((0, image.core(5000)));
            ops.push((0, record(0, 20, 2, false).to_vec()));
            if cancel {
                let flags = BUF_CANCEL | BUF_INODE;
                ops.push((0, image.buffer(flags, cluster, 16, 0, 1)));
            }
            if after {
                ops.push((0, image.inode_format(131, 0x1 | LOG_EXTENTS, 16)));
                ops.push((0, image.core(6000)));
                ops.push((0, record(0, 20, 2, false).to_vec()));
            }
            ops.push((COMMIT, Vec::new()));
            image.record(&ops);
            let overlay = image.replay().unwrap();
            let at = usize::try_from(sb.inode_offset(131).unwrap()).unwrap();
            let inode = image.read(&overlay, at, 256);
            crate::inode::Inode::parse(&inode, 131, crate::superblock::Version::V4)
                .ok()
                .map(|inode| inode.size())
        };
        assert_eq!(image(false, false), Some(5000));
        // Freed with its chunk: what was the inode is left as it is.
        assert_eq!(image(true, false), None);
        // Allocated again, after the cancel.
        assert_eq!(image(true, true), Some(6000));
    }

    #[test]
    fn replays_buffers_and_inodes() {
        for order in [Order::Little, Order::Big] {
            let mut image = Image::new(order);
            // Block 10 (sector 80): its second chunk.
            let mut ops = image.start(2);
            ops.push((0, image.buffer(0, 80, 8, 0b10, 2)));
            ops.push((0, vec![0xAB; 128]));
            // Inode 131: its core and extent list.
            ops.push((0, image.inode_format(131, 0x1 | LOG_EXTENTS, 16)));
            ops.push((0, image.core(5000)));
            ops.push((0, record(0, 20, 2, false).to_vec()));
            ops.push((COMMIT, Vec::new()));
            image.record(&ops);
            let overlay = image.replay().unwrap();
            assert_eq!(image.read(&overlay, 40 * 1024, 128), [0; 128]);
            assert_eq!(image.read(&overlay, 40 * 1024 + 128, 128), [0xAB; 128]);
            let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
            let at = usize::try_from(sb.inode_offset(131).unwrap()).unwrap();
            let inode = image.read(&overlay, at, 256);
            let inode =
                crate::inode::Inode::parse(&inode, 131, crate::superblock::Version::V4).unwrap();
            assert_eq!((inode.size(), inode.data().extents), (5000, 1));
            assert_eq!(inode.data().bytes, record(0, 20, 2, false));
            assert_eq!(inode.times().modified.secs(), 1_704_164_646);
            assert_eq!(inode.times().modified.nanos(), 5);
        }
    }

    #[test]
    fn replays_operations_across_records_but_not_cancelled_buffers() {
        let mut image = Image::new(Order::Little);
        // A buffer whose region the next record continues.
        let mut ops = image.start(1);
        ops.push((0, image.buffer(0, 80, 8, 0b11, 2)));
        ops.push((CONTINUE, vec![1; 100]));
        image.record(&ops);
        image.record(&[(WAS_CONTINUED | 0x10, vec![2; 156]), (COMMIT, Vec::new())]);
        // One logged, then cancelled; then one logged after the cancel.
        let mut ops = image.start(3);
        ops.push((0, image.buffer(0, 96, 8, 0b1, 2)));
        ops.push((0, vec![3; 128]));
        ops.push((0, image.buffer(BUF_CANCEL, 96, 8, 0, 1)));
        ops.push((0, image.buffer(0, 104, 8, 0b1, 2)));
        ops.push((0, vec![4; 128]));
        ops.push((COMMIT, Vec::new()));
        image.record(&ops);
        let overlay = image.replay().unwrap();
        let joined = image.read(&overlay, 40 * 1024, 256);
        assert_eq!(joined[..100], [1; 100]);
        assert_eq!(joined[100..], [2; 156]);
        assert_eq!(image.read(&overlay, 48 * 1024, 128), [0; 128]);
        assert_eq!(image.read(&overlay, 52 * 1024, 128), [4; 128]);
        // An uncommitted transaction is not replayed.
        let mut image = Image::new(Order::Little);
        let mut ops = image.start(1);
        ops.push((0, image.buffer(0, 80, 8, 0b1, 2)));
        ops.push((0, vec![5; 128]));
        image.record(&ops);
        assert!(image.replay().unwrap().is_empty());
    }

    #[test]
    fn refuses_unfinished_work_it_cannot_do() {
        let unsupported = |image: &Image| {
            let error = image.replay().unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Unsupported);
        };
        // Mapping extents into a file, unfinished; and finished.
        let mut image = Image::new(Order::Little);
        let mut ops = image.start(1);
        ops.push((0, image.intent(item::BUI, 9)));
        ops.push((COMMIT, Vec::new()));
        image.record(&ops);
        unsupported(&image);
        let mut ops = image.start(1);
        ops.push((0, image.intent(item::BUD, 9)));
        ops.push((COMMIT, Vec::new()));
        image.record(&ops);
        assert!(image.replay().unwrap().is_empty());
        // Freeing extents, unfinished, changes nothing a reader sees.
        let mut image = Image::new(Order::Little);
        let mut ops = image.start(1);
        ops.push((0, image.intent(item::EFI, 3)));
        ops.push((COMMIT, Vec::new()));
        image.record(&ops);
        assert!(image.replay().unwrap().is_empty());
        // An item of a type not known.
        let mut image = Image::new(Order::Little);
        let mut ops = image.start(1);
        ops.push((0, image.intent(0x1250, 1)));
        ops.push((COMMIT, Vec::new()));
        image.record(&ops);
        unsupported(&image);
    }

    #[test]
    fn rejects_bad_records() {
        let mut image = Image::new(Order::Little);
        let mut ops = image.start(1);
        // A buffer of more chunks than regions.
        ops.push((0, image.buffer(0, 80, 8, 0b101, 2)));
        ops.push((0, vec![1; 128]));
        ops.push((COMMIT, Vec::new()));
        image.record(&ops);
        assert_eq!(image.replay().unwrap_err().kind(), ErrorKind::Corrupt);
        // A transaction header of neither order.
        let mut image = Image::new(Order::Little);
        image.record(&[(START, Vec::new()), (0, vec![0; 16]), (COMMIT, Vec::new())]);
        assert_eq!(image.replay().unwrap_err().kind(), ErrorKind::Corrupt);
    }
}
