// This is free and unencumbered software released into the public domain.

//! Journal replay, in memory: copying the transactions a dirty journal
//! commits to their places, as Linux does when it mounts the volume, but
//! into an [`Overlay`] that reads see instead of the volume's own bytes.
//!
//! The journal (JBD2, big-endian throughout) is a file, usually inode 8,
//! of blocks of the volume's size. Its first is its superblock: the
//! journal's length, the first block of its log, which wraps around to
//! it, and, if it needs recovery, the block where the log starts and the
//! sequence number of the transaction there. Every block of the log that
//! is not data starts with a header: the magic number, the block's type,
//! and its transaction's sequence number.
//!
//! A transaction is descriptor blocks, each followed by the data blocks
//! its tags describe, revoke blocks, and a commit block, after which the
//! next transaction, numbered one more, follows. A tag names the volume
//! block its data block replaces, with 32 more bits under the `64bit`
//! feature, and flags: the last tag; the same UUID as before, or else 16
//! bytes of it follow; and escaped, for a data block that started with
//! the magic number and was written with zeros there instead. A revoke
//! block lists volume blocks that are not to be replayed from this
//! transaction or earlier ones, as they were freed and may hold a file's
//! data now. The log ends at the first block that is not the next
//! transaction's, or whose transaction does not commit.
//!
//! Under `csum_v3`, every block is checked by CRC-32C, from a seed of the
//! journal's UUID: the superblock's own, the descriptor and revoke blocks'
//! in their last four bytes, the commit block's in its header, and each
//! data block's, over its sequence number and its bytes, in its tag. A
//! commit failing its checksum ends the log, as a commit is how a
//! transaction becomes durable; any other block of a committed transaction
//! failing its own makes the journal invalid.
//!
//! The older `csum_v2`, the asynchronous commits that only a checksum of
//! the whole transaction guards, and fast commits are not replayed: such
//! volumes are refused, unless they are read as they are.

use crate::{Ext4Error, block_map::BlockMap, inode, superblock::Superblock};
use alloc::{collections::BTreeMap, vec::Vec};
use core::future::Future;
use filer_api::{BufResult, Error, FileKind, IoBufMut, OpKind, ReadAt, checksum::Crc32c};
use filer_fs::{be, read_exact, recover::Overlay};

/// The magic number of the journal's blocks, and their types.
const MAGIC: u32 = 0xC03B_3998;
const DESCRIPTOR: u32 = 1;
const COMMIT: u32 = 2;
const SUPERBLOCK_V1: u32 = 3;
const SUPERBLOCK_V2: u32 = 4;
const REVOKE: u32 = 5;

/// The size of a block's header, and of the superblock.
const HEADER: usize = 12;
const SUPERBLOCK_LEN: usize = 1024;

/// Where the superblock keeps its checksum, the commit block its own,
/// and the size of the checksum ending descriptor and revoke blocks.
const SUPERBLOCK_CHECKSUM: usize = 0xFC;
const COMMIT_CHECKSUM: usize = 16;
const TAIL: usize = 4;

/// The size of a revoke block's header, with its count of bytes used.
const REVOKE_HEADER: usize = 16;

/// The size of a UUID after a tag.
const UUID_LEN: usize = 16;

/// The superblock's incompatible features this driver replays.
const INCOMPAT_REVOKE: u32 = 0x1;
const INCOMPAT_64BIT: u32 = 0x2;
const INCOMPAT_CSUM_V3: u32 = 0x10;
const SUPPORTED: u32 = INCOMPAT_REVOKE | INCOMPAT_64BIT | INCOMPAT_CSUM_V3;

/// The superblock's checksum type for CRC-32C.
const CRC32C: u8 = 4;

/// A tag's flags.
const FLAG_ESCAPE: u32 = 0x1;
const FLAG_SAME_UUID: u32 = 0x2;
const FLAG_LAST_TAG: u32 = 0x8;

/// Returns the CRC-32C register after `crc`, as JBD2 stores checksums:
/// not inverted at the end.
fn raw(crc: Crc32c) -> u32 {
    !crc.finish()
}

/// Returns `true` if sequence number `a` is after `b`, as they wrap.
fn after(a: u32, b: u32) -> bool {
    let distance = a.wrapping_sub(b);
    distance != 0 && distance < 0x8000_0000
}

/// The journal's superblock, validated.
struct Header {
    block_size: usize,
    /// The journal's length, in blocks.
    len: u32,
    /// The first block of the log.
    first: u32,
    /// The sequence number of the transaction at `start`.
    sequence: u32,
    /// The block where the log starts, or 0 if it holds nothing.
    start: u32,
    incompat: u32,
    /// The checksums' seed, under `csum_v3`.
    seed: Option<Crc32c>,
}

impl Header {
    /// Parses the superblock of a journal of `blocks` blocks of
    /// `block_size` bytes.
    fn parse(bytes: &[u8], block_size: usize, blocks: u64) -> Result<Self, Ext4Error> {
        let bad = Ext4Error::BadJournal { block: 0 };
        let u32_at = |at| be::u32_at(bytes, at).ok_or(bad);
        let version = u32_at(4)?;
        if u32_at(0)? != MAGIC || !matches!(version, SUPERBLOCK_V1 | SUPERBLOCK_V2) {
            return Err(bad);
        }
        let len = u32_at(0x10)?;
        let first = u32_at(0x14)?;
        let start = u32_at(0x1C)?;
        if usize::try_from(u32_at(0xC)?).ok() != Some(block_size)
            || u64::from(len) > blocks
            || first == 0
            || first >= len
            || (start != 0 && (start < first || start >= len))
        {
            return Err(bad);
        }
        let incompat = if version == SUPERBLOCK_V2 {
            u32_at(0x28)?
        } else {
            0
        };
        let unknown = incompat & !SUPPORTED;
        if unknown != 0 {
            return Err(Ext4Error::UnsupportedJournal { incompat: unknown });
        }
        let mut seed = None;
        if incompat & INCOMPAT_CSUM_V3 != 0 {
            let head = bytes.get(..SUPERBLOCK_CHECKSUM).ok_or(bad)?;
            let tail = bytes
                .get(SUPERBLOCK_CHECKSUM.saturating_add(4)..SUPERBLOCK_LEN)
                .ok_or(bad)?;
            let crc = Crc32c::new().update(head).update(&[0; 4]).update(tail);
            if bytes.get(0x50) != Some(&CRC32C) || raw(crc) != u32_at(SUPERBLOCK_CHECKSUM)? {
                return Err(bad);
            }
            seed = Some(Crc32c::new().update(bytes.get(0x30..0x40).ok_or(bad)?));
        }
        Ok(Self {
            block_size,
            len,
            first,
            sequence: u32_at(0x18)?,
            start,
            incompat,
            seed,
        })
    }

    /// Returns the log's block after `block`, wrapping around.
    fn next(&self, block: u32) -> u32 {
        match block.checked_add(1) {
            Some(next) if next < self.len => next,
            _ => self.first,
        }
    }

    /// Returns `true` if a descriptor or revoke block's checksum matches,
    /// or there is none.
    fn tail_ok(&self, bytes: &[u8]) -> bool {
        let Some(seed) = self.seed else {
            return true;
        };
        let at = bytes.len().saturating_sub(TAIL);
        let (Some(body), Some(stored)) = (bytes.get(..at), be::u32_at(bytes, at)) else {
            return false;
        };
        raw(seed.update(body).update(&[0; TAIL])) == stored
    }

    /// Returns `true` if a commit block's checksum matches, or there is
    /// none.
    fn commit_ok(&self, bytes: &[u8]) -> bool {
        let Some(seed) = self.seed else {
            return true;
        };
        let end = COMMIT_CHECKSUM.saturating_add(4);
        let (Some(head), Some(tail), Some(stored)) = (
            bytes.get(..COMMIT_CHECKSUM),
            bytes.get(end..),
            be::u32_at(bytes, COMMIT_CHECKSUM),
        ) else {
            return false;
        };
        raw(seed.update(head).update(&[0; 4]).update(tail)) == stored
    }

    /// Returns `true` if a data block's checksum matches its tag's, or
    /// there is none.
    fn data_ok(&self, sequence: u32, bytes: &[u8], checksum: u32) -> bool {
        self.seed
            .is_none_or(|seed| raw(seed.update(&sequence.to_be_bytes()).update(bytes)) == checksum)
    }

    /// Returns where a descriptor or revoke block's entries end: before
    /// its checksum, if it has one.
    fn entries_end(&self) -> usize {
        if self.seed.is_some() {
            self.block_size.saturating_sub(TAIL)
        } else {
            self.block_size
        }
    }

    /// Returns the tags of a descriptor block: the volume block each
    /// replaces, its flags, and its checksum.
    fn tags(&self, bytes: &[u8]) -> Vec<(u64, u32, u32)> {
        let wide = self.incompat & INCOMPAT_64BIT != 0;
        let size: usize = match (self.seed.is_some(), wide) {
            (true, _) => 16,
            (false, true) => 12,
            (false, false) => 8,
        };
        let body = bytes.get(..self.entries_end()).unwrap_or_default();
        let mut tags = Vec::new();
        let mut at = HEADER;
        while let Some(tag) = body.get(at..at.saturating_add(size)) {
            let field = |at| be::u32_at(tag, at).unwrap_or(0);
            let (flags, checksum) = if self.seed.is_some() {
                (field(4), field(12))
            } else {
                (u32::from(be::u16_at(tag, 6).unwrap_or(0)), 0)
            };
            let high = if wide { field(8) } else { 0 };
            let [h0, h1, h2, h3] = high.to_be_bytes();
            let [l0, l1, l2, l3] = field(0).to_be_bytes();
            let block = u64::from_be_bytes([h0, h1, h2, h3, l0, l1, l2, l3]);
            tags.push((block, flags, checksum));
            at = at.saturating_add(size);
            if flags & FLAG_SAME_UUID == 0 {
                at = at.saturating_add(UUID_LEN);
            }
            if flags & FLAG_LAST_TAG != 0 {
                break;
            }
        }
        tags
    }

    /// Returns the volume blocks a revoke block lists.
    fn revoked(&self, bytes: &[u8]) -> Option<Vec<u64>> {
        let used = usize::try_from(be::u32_at(bytes, HEADER)?).ok()?;
        let body = bytes
            .get(REVOKE_HEADER..used)
            .filter(|_| used <= self.entries_end())?;
        let blocks = if self.incompat & INCOMPAT_64BIT != 0 {
            let (records, _) = body.as_chunks::<8>();
            records.iter().map(|&r| u64::from_be_bytes(r)).collect()
        } else {
            let (records, _) = body.as_chunks::<4>();
            records
                .iter()
                .map(|&r| u64::from(u32::from_be_bytes(r)))
                .collect()
        };
        Some(blocks)
    }
}

/// A block a transaction replaces: the volume block, the journal's data
/// block, and that block's tag.
struct Tag {
    block: u64,
    data: u32,
    escaped: bool,
    checksum: u32,
}

/// A committed transaction.
struct Transaction {
    sequence: u32,
    tags: Vec<Tag>,
}

/// What the log holds: its committed transactions, and the volume blocks
/// they revoke, each with the latest transaction revoking it.
#[derive(Default)]
struct Log {
    transactions: Vec<Transaction>,
    revoked: BTreeMap<u64, u32>,
}

impl Log {
    /// Returns `true` if volume block `block` is not replayed from
    /// transaction `sequence`.
    fn is_revoked(&self, block: u64, sequence: u32) -> bool {
        self.revoked
            .get(&block)
            .is_some_and(|&revoked| !after(sequence, revoked))
    }
}

/// Reads journal block `block`, of `size` bytes.
async fn block<J: ReadAt + ?Sized>(journal: &J, size: usize, block: u32) -> Result<Vec<u8>, Error> {
    let at = u64::from(block).saturating_mul(u64::try_from(size).unwrap_or(u64::MAX));
    read_exact(journal, at, size, OpKind::Open).await
}

/// Reads the log from its start, to its end, gathering what its
/// committed transactions replace and revoke. Each block is read once at
/// most, as the log cannot be longer than the journal.
async fn scan<J: ReadAt + ?Sized>(journal: &J, header: &Header) -> Result<Log, Error> {
    let fail = |block| Ext4Error::BadJournal { block }.into_error(OpKind::Open);
    let mut log = Log::default();
    let mut at = header.start;
    let mut sequence = header.sequence;
    let mut tags = Vec::new();
    let mut revoked = Vec::new();
    // The first block of this transaction that is malformed or fails its
    // checksum, which only matters if the transaction commits.
    let mut bad = None;
    let mut left = header.len.saturating_sub(header.first);
    'log: while left > 0 {
        let bytes = block(journal, header.block_size, at).await?;
        let here = at;
        at = header.next(at);
        left = left.saturating_sub(1);
        if be::u32_at(&bytes, 0) != Some(MAGIC) || be::u32_at(&bytes, 8) != Some(sequence) {
            break;
        }
        match be::u32_at(&bytes, 4) {
            Some(DESCRIPTOR) => {
                if !header.tail_ok(&bytes) {
                    bad.get_or_insert(here);
                }
                for (block, flags, checksum) in header.tags(&bytes) {
                    if left == 0 {
                        break 'log;
                    }
                    tags.push(Tag {
                        block,
                        data: at,
                        escaped: flags & FLAG_ESCAPE != 0,
                        checksum,
                    });
                    at = header.next(at);
                    left = left.saturating_sub(1);
                }
            },
            Some(REVOKE) => match header.revoked(&bytes) {
                Some(blocks) if header.tail_ok(&bytes) => revoked.extend(blocks),
                _ => {
                    bad.get_or_insert(here);
                },
            },
            Some(COMMIT) => {
                if !header.commit_ok(&bytes) {
                    break;
                }
                if let Some(block) = bad {
                    return Err(fail(block));
                }
                for block in revoked.drain(..) {
                    log.revoked
                        .entry(block)
                        .and_modify(|latest| {
                            if after(sequence, *latest) {
                                *latest = sequence;
                            }
                        })
                        .or_insert(sequence);
                }
                log.transactions.push(Transaction {
                    sequence,
                    tags: core::mem::take(&mut tags),
                });
                sequence = sequence.wrapping_add(1);
            },
            _ => break,
        }
    }
    Ok(log)
}

/// Replays `journal`, the journal of the volume `sb` describes, into an
/// overlay: empty if it holds nothing.
async fn replay_from<J: ReadAt + ?Sized>(journal: &J, sb: &Superblock) -> Result<Overlay, Error> {
    let op = OpKind::Open;
    let fail = |block| Ext4Error::BadJournal { block }.into_error(op);
    let block_size = usize::try_from(sb.block_size()).map_err(|_| fail(0))?;
    let blocks = journal
        .size()
        .await?
        .checked_div(u64::from(sb.block_size()))
        .unwrap_or(0);
    let first = block(journal, block_size, 0).await?;
    let header = Header::parse(&first, block_size, blocks).map_err(|error| error.into_error(op))?;
    let mut overlay = Overlay::new();
    if header.start == 0 {
        return Ok(overlay);
    }
    let log = scan(journal, &header).await?;
    for transaction in &log.transactions {
        for tag in &transaction.tags {
            if log.is_revoked(tag.block, transaction.sequence) {
                continue;
            }
            let mut data = block(journal, block_size, tag.data).await?;
            if !header.data_ok(transaction.sequence, &data, tag.checksum) {
                return Err(fail(tag.data));
            }
            if tag.escaped
                && let Some(magic) = data.get_mut(..4)
            {
                magic.copy_from_slice(&MAGIC.to_be_bytes());
            }
            let at = sb.block_offset(tag.block).ok_or_else(|| fail(tag.data))?;
            if !overlay.put(at, &data) {
                return Err(fail(tag.data));
            }
        }
    }
    Ok(overlay)
}

/// The journal's file, read through its block map.
struct File<'a, S: ?Sized> {
    source: &'a S,
    sb: &'a Superblock,
    map: BlockMap,
    size: u64,
}

impl<S: ReadAt + ?Sized> ReadAt for File<'_, S> {
    async fn read_at<B: IoBufMut>(&self, mut buf: B, off: u64) -> BufResult<usize, B> {
        let read = self
            .map
            .read(
                self.source,
                self.sb,
                self.size,
                off,
                buf.spare(),
                OpKind::Open,
            )
            .await;
        match read {
            Ok(bytes) => {
                let n = buf.fill_from(&bytes);
                BufResult(Ok(n), buf)
            },
            Err(error) => BufResult(Err(error), buf),
        }
    }

    fn size(&self) -> impl Future<Output = filer_api::Result<u64>> + Send {
        core::future::ready(Ok(self.size))
    }
}

/// Replays the journal of the volume in `source`, which `sb` describes,
/// into an overlay of what recovery would write. A journal on another
/// device cannot be replayed.
pub(crate) async fn replay<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
) -> Result<Overlay, Error> {
    let op = OpKind::Open;
    let number = sb
        .journal_inode()
        .ok_or_else(|| Ext4Error::NeedsRecovery.into_error(op))?;
    let inode = inode::read(source, sb, number, op).await?;
    if inode.kind() != FileKind::File {
        return Err(Ext4Error::BadInode { inode: number }.into_error(op));
    }
    let map = BlockMap::new(&inode, sb).map_err(|error| error.into_error(op))?;
    let journal = File {
        source,
        sb,
        map,
        size: inode.size(),
    };
    replay_from(&journal, sb).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock::tests::Fields;
    use alloc::{vec, vec::Vec};
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// The journal's length, in blocks of 1 KiB, as the test volume's.
    const LEN: u32 = 64;
    const SIZE: usize = 1024;

    /// A journal, built block by block.
    struct Journal {
        incompat: u32,
        sequence: u32,
        start: u32,
        blocks: Vec<Vec<u8>>,
    }

    impl Journal {
        fn new(incompat: u32) -> Self {
            Self {
                incompat,
                sequence: 7,
                start: 1,
                blocks: vec![vec![0; SIZE]; usize::try_from(LEN).unwrap()],
            }
        }

        fn seed(&self) -> Option<Crc32c> {
            (self.incompat & INCOMPAT_CSUM_V3 != 0).then(|| Crc32c::new().update(&[0xAB; 16]))
        }

        fn header(kind: u32, sequence: u32) -> Vec<u8> {
            let mut b = vec![0; SIZE];
            b[..4].copy_from_slice(&MAGIC.to_be_bytes());
            b[4..8].copy_from_slice(&kind.to_be_bytes());
            b[8..12].copy_from_slice(&sequence.to_be_bytes());
            b
        }

        /// Seals a descriptor or revoke block's checksum.
        fn seal(&self, b: &mut [u8]) {
            if let Some(seed) = self.seed() {
                let crc = raw(seed.update(&b[..SIZE - TAIL]).update(&[0; 4]));
                b[SIZE - TAIL..].copy_from_slice(&crc.to_be_bytes());
            }
        }

        fn next(at: u32) -> u32 {
            if at + 1 == LEN { 1 } else { at + 1 }
        }

        /// Writes a transaction at `at`: a descriptor of `writes`, their
        /// data, a revoke block of `revokes`, and, if `commit`, a commit
        /// block. Returns the block after it.
        fn transaction(
            &mut self,
            mut at: u32,
            sequence: u32,
            writes: &[(u64, Vec<u8>)],
            revokes: &[u64],
            commit: bool,
        ) -> u32 {
            let wide = self.incompat & INCOMPAT_64BIT != 0;
            let mut descriptor = Self::header(DESCRIPTOR, sequence);
            let mut blocks = Vec::new();
            let mut pos = HEADER;
            for (i, (block, data)) in writes.iter().enumerate() {
                let mut data = data.clone();
                let mut flags = if i == 0 { 0 } else { FLAG_SAME_UUID };
                if data[..4] == MAGIC.to_be_bytes() {
                    data[..4].fill(0);
                    flags |= FLAG_ESCAPE;
                }
                if i + 1 == writes.len() {
                    flags |= FLAG_LAST_TAG;
                }
                let [h0, h1, h2, h3, l0, l1, l2, l3] = block.to_be_bytes();
                let tag = &mut descriptor[pos..];
                tag[..4].copy_from_slice(&[l0, l1, l2, l3]);
                if let Some(seed) = self.seed() {
                    tag[4..8].copy_from_slice(&flags.to_be_bytes());
                    tag[8..12].copy_from_slice(&[h0, h1, h2, h3]);
                    let crc = raw(seed.update(&sequence.to_be_bytes()).update(&data));
                    tag[12..16].copy_from_slice(&crc.to_be_bytes());
                    pos += 16;
                } else {
                    tag[6..8].copy_from_slice(&u16::try_from(flags).unwrap().to_be_bytes());
                    if wide {
                        tag[8..12].copy_from_slice(&[h0, h1, h2, h3]);
                    }
                    pos += if wide { 12 } else { 8 };
                }
                if flags & FLAG_SAME_UUID == 0 {
                    pos += UUID_LEN;
                }
                blocks.push(data);
            }
            self.seal(&mut descriptor);
            blocks.insert(0, descriptor);
            if !revokes.is_empty() {
                let mut revoke = Self::header(REVOKE, sequence);
                let mut pos = REVOKE_HEADER;
                for &block in revokes {
                    let record = block.to_be_bytes();
                    let record = if wide { &record[..] } else { &record[4..] };
                    revoke[pos..pos + record.len()].copy_from_slice(record);
                    pos += record.len();
                }
                revoke[12..16].copy_from_slice(&u32::try_from(pos).unwrap().to_be_bytes());
                self.seal(&mut revoke);
                blocks.push(revoke);
            }
            if commit {
                let mut b = Self::header(COMMIT, sequence);
                if let Some(seed) = self.seed() {
                    let crc = raw(seed.update(&b));
                    b[16..20].copy_from_slice(&crc.to_be_bytes());
                }
                blocks.push(b);
            }
            for b in blocks {
                self.blocks[usize::try_from(at).unwrap()] = b;
                at = Self::next(at);
            }
            at
        }

        fn bytes(&self) -> Vec<u8> {
            let mut sb = Self::header(SUPERBLOCK_V2, 0);
            let mut put = |at: usize, v: u32| sb[at..at + 4].copy_from_slice(&v.to_be_bytes());
            put(0xC, u32::try_from(SIZE).unwrap());
            put(0x10, LEN);
            put(0x14, 1);
            put(0x18, self.sequence);
            put(0x1C, self.start);
            put(0x28, self.incompat);
            sb[0x30..0x40].fill(0xAB);
            if self.incompat & INCOMPAT_CSUM_V3 != 0 {
                sb[0x50] = CRC32C;
                let crc = raw(Crc32c::new().update(&sb));
                sb[0xFC..0x100].copy_from_slice(&crc.to_be_bytes());
            }
            let mut bytes = sb;
            for b in &self.blocks[1..] {
                bytes.extend_from_slice(b);
            }
            bytes
        }

        fn replay(&self) -> Result<Overlay, Error> {
            replay_bytes(&self.bytes())
        }
    }

    /// Replays a journal, of the test volume.
    fn replay_bytes(journal: &[u8]) -> Result<Overlay, Error> {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        block_on(replay_from(journal, &sb))
    }

    /// Reads volume block `block` of a zeroed volume through `overlay`.
    fn read(overlay: &Overlay, block: u64) -> Vec<u8> {
        let mut bytes = vec![0; SIZE];
        overlay.lay(block * 1024, &mut bytes);
        bytes
    }

    fn filled(byte: u8) -> Vec<u8> {
        vec![byte; SIZE]
    }

    fn escaped() -> Vec<u8> {
        let mut b = filled(9);
        b[..4].copy_from_slice(&MAGIC.to_be_bytes());
        b
    }

    fn replays_committed_transactions_with(incompat: u32) {
        let mut j = Journal::new(incompat);
        let at = j.transaction(1, 7, &[(100, filled(1)), (101, filled(1))], &[], true);
        let at = j.transaction(at, 8, &[(100, filled(2)), (102, escaped())], &[], true);
        j.transaction(at, 9, &[(103, filled(3))], &[], false);
        let overlay = j.replay().unwrap();
        assert_eq!(read(&overlay, 100), filled(2));
        assert_eq!(read(&overlay, 101), filled(1));
        assert_eq!(read(&overlay, 102), escaped());
        assert_eq!(read(&overlay, 103), filled(0), "uncommitted");
    }

    #[test]
    fn replays_committed_transactions() {
        replays_committed_transactions_with(0);
        replays_committed_transactions_with(INCOMPAT_64BIT);
        replays_committed_transactions_with(INCOMPAT_CSUM_V3);
        replays_committed_transactions_with(INCOMPAT_CSUM_V3 | INCOMPAT_64BIT);
    }

    #[test]
    fn honors_revokes_of_the_same_and_earlier_transactions() {
        for incompat in [
            INCOMPAT_REVOKE,
            INCOMPAT_REVOKE | INCOMPAT_64BIT | INCOMPAT_CSUM_V3,
        ] {
            let mut j = Journal::new(incompat);
            let at = j.transaction(1, 7, &[(100, filled(1)), (101, filled(1))], &[], true);
            let at = j.transaction(at, 8, &[(102, filled(2))], &[100, 102], true);
            j.transaction(at, 9, &[(102, filled(3))], &[], true);
            let overlay = j.replay().unwrap();
            assert_eq!(read(&overlay, 100), filled(0));
            assert_eq!(read(&overlay, 101), filled(1));
            assert_eq!(read(&overlay, 102), filled(3));
        }
    }

    #[test]
    fn wraps_around_the_log() {
        let mut j = Journal::new(0);
        j.start = LEN - 2;
        j.transaction(LEN - 2, 7, &[(100, filled(1)), (101, filled(2))], &[], true);
        let overlay = j.replay().unwrap();
        assert_eq!(read(&overlay, 101), filled(2));
        // Transactions of the log's last cycle end it.
        j.blocks[3] = Journal::header(DESCRIPTOR, 6);
        assert!(j.replay().is_ok());
    }

    #[test]
    fn checks_checksums() {
        let v3 = INCOMPAT_CSUM_V3;
        let mut j = Journal::new(v3);
        let at = j.transaction(1, 7, &[(100, filled(1))], &[], true);
        j.transaction(at, 8, &[(101, filled(2))], &[], true);
        // A torn commit ends the log.
        j.blocks[6][100] ^= 1;
        let overlay = j.replay().unwrap();
        assert_eq!(read(&overlay, 100), filled(1));
        assert_eq!(read(&overlay, 101), filled(0));
        // A committed data block, descriptor, or superblock that fails
        // its checksum is corrupt.
        for block in [2, 1, 0] {
            let mut j = Journal::new(v3);
            j.transaction(1, 7, &[(100, filled(1))], &[], true);
            let mut bytes = j.bytes();
            bytes[usize::try_from(block).unwrap() * SIZE + 200] ^= 1;
            let error = replay_bytes(&bytes).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt);
            let source = core::error::Error::source(&error).unwrap();
            assert_eq!(
                source.downcast_ref::<Ext4Error>(),
                Some(&Ext4Error::BadJournal { block })
            );
        }
    }

    #[test]
    fn reads_clean_journals_as_nothing() {
        let mut j = Journal::new(0);
        j.transaction(1, 7, &[(100, filled(1))], &[], true);
        j.start = 0;
        assert!(j.replay().unwrap().is_empty());
    }

    #[test]
    fn refuses_bad_and_unsupported_journals() {
        let error = |j: &Journal| {
            let error = j.replay().unwrap_err();
            *core::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<Ext4Error>()
                .unwrap()
        };
        let mut j = Journal::new(0x20 | INCOMPAT_REVOKE);
        assert_eq!(error(&j), Ext4Error::UnsupportedJournal { incompat: 0x20 });
        j.incompat = 0;
        j.start = LEN;
        assert_eq!(error(&j), Ext4Error::BadJournal { block: 0 });
        // A target outside the volume.
        let mut j = Journal::new(INCOMPAT_64BIT);
        j.transaction(1, 7, &[(1 << 40, filled(1))], &[], true);
        assert_eq!(error(&j), Ext4Error::BadJournal { block: 2 });
        // A malformed revoke block: invalid if its transaction commits.
        let mut j = Journal::new(INCOMPAT_REVOKE);
        j.transaction(1, 7, &[(100, filled(1))], &[100], true);
        j.blocks[3][12..16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert_eq!(error(&j), Ext4Error::BadJournal { block: 3 });
        j.blocks[4] = vec![0; SIZE];
        assert!(j.replay().unwrap().is_empty());
    }
}
