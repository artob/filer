// This is free and unencumbered software released into the public domain.

//! Regular files' contents: blocks of the image's block size, stored one
//! after another from the file's start, each compressed or not, or left
//! out if all zeros; and a tail of less than a block, which may instead be
//! in a fragment, a block shared by many files' tails.

use crate::{
    SquashfsError,
    inode::{Inode, Layout},
    meta::Cursor,
    volume::Volume,
};
use alloc::{sync::Arc, vec, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::Lock;

/// A block size's flag that the block is stored as is, and the bits of
/// its size on disk.
const STORED: u32 = 0x0100_0000;
const SIZE: u32 = 0x00FF_FFFF;

/// How many blocks apart the file keeps where they are.
const MARK: u64 = 256;

/// An open regular file: its layout, and what reading it has learned.
pub(crate) struct File {
    layout: Layout,
    state: Lock<State>,
}

/// What reading a file has learned: where every `MARK`th block is, and
/// where its size is listed; and the block it decoded last.
#[derive(Default)]
struct State {
    marks: Vec<(u64, Cursor)>,
    last: Option<(u64, Arc<[u8]>)>,
}

impl File {
    pub(crate) fn new(layout: Layout) -> Self {
        let marks = vec![(layout.start, layout.list)];
        Self {
            layout,
            state: Lock::new(State { marks, last: None }),
        }
    }

    /// Returns its size.
    pub(crate) fn size(&self) -> u64 {
        self.layout.size
    }
}

impl<S: ReadAt> Volume<S> {
    /// Reads up to `max` bytes of `file`, opened from `inode`, at `off`.
    pub(crate) async fn read_file(
        &self,
        inode: &Inode,
        file: &File,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let size = file.size();
        let end = off
            .saturating_add(u64::try_from(max).unwrap_or(u64::MAX))
            .min(size);
        let block = u64::from(self.sb.block_size);
        let mut bytes = Vec::with_capacity(usize::try_from(end.saturating_sub(off)).unwrap_or(0));
        let mut at = off;
        while at < end {
            let index = at.checked_div(block).unwrap_or(0);
            let within = usize::try_from(at.checked_rem(block).unwrap_or(0)).unwrap_or(0);
            let data = if index < file.layout.blocks {
                self.data_block(inode, file, index, op).await?
            } else {
                self.tail(inode, file, op).await?
            };
            let available = data.get(within..).unwrap_or_default();
            let want = usize::try_from(end.saturating_sub(at)).unwrap_or(usize::MAX);
            let n = available.len().min(want);
            if n == 0 {
                // A block shorter than its place in the file.
                let bad = SquashfsError::BadDataBlock {
                    inode: inode.number,
                    block: index,
                };
                return Err(bad.into_error(op));
            }
            bytes.extend_from_slice(available.get(..n).unwrap_or_default());
            at = at.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
        }
        Ok(bytes)
    }

    /// Returns block `index` of `file`, decoded.
    async fn data_block(
        &self,
        inode: &Inode,
        file: &File,
        index: u64,
        op: OpKind,
    ) -> Result<Arc<[u8]>, Error> {
        let hit = file.state.with(|state| {
            let (last, data) = state.last.as_ref()?;
            (*last == index).then(|| data.clone())
        });
        if let Some(data) = hit {
            return Ok(data);
        }
        let bad = || {
            SquashfsError::BadDataBlock {
                inode: inode.number,
                block: index,
            }
            .into_error(op)
        };
        let (at, word) = self.locate(inode, file, index, op).await?;
        // Every block is whole but the last, unless the tail is in a
        // fragment.
        let block = u64::from(self.sb.block_size);
        let start = index.checked_mul(block).ok_or_else(bad)?;
        let len = file.size().saturating_sub(start).min(block);
        let len = usize::try_from(len).map_err(|_| bad())?;
        let data = self.block(at, word, len, true, op, bad).await?;
        file.state
            .with(|state| state.last = Some((index, data.clone())));
        Ok(data)
    }

    /// Returns where block `index` of `file` is, and its size, as listed.
    async fn locate(
        &self,
        inode: &Inode,
        file: &File,
        index: u64,
        op: OpKind,
    ) -> Result<(u64, u32), Error> {
        let mark = index.checked_div(MARK).unwrap_or(0);
        let mark = usize::try_from(mark).unwrap_or(usize::MAX);
        // The nearest mark, and how far the marks reach.
        let (known, (mut at, mut list)) = file.state.with(|state| {
            let last = state.marks.len().saturating_sub(1).min(mark);
            let nearest = state.marks.get(last).copied();
            (
                last,
                nearest.unwrap_or((file.layout.start, file.layout.list)),
            )
        });
        let mut position = u64::try_from(known)
            .unwrap_or(u64::MAX)
            .saturating_mul(MARK);
        // Sum the sizes of the blocks before it, marking every `MARK`th.
        while position < index {
            let word = self.meta.u32(&mut list, op).await?;
            at = at.checked_add(u64::from(word & SIZE)).ok_or_else(|| {
                SquashfsError::BadDataBlock {
                    inode: inode.number,
                    block: position,
                }
                .into_error(op)
            })?;
            position = position.saturating_add(1);
            if position.checked_rem(MARK) == Some(0) {
                let mark = usize::try_from(position.checked_div(MARK).unwrap_or(0)).ok();
                file.state.with(|state| {
                    if mark == Some(state.marks.len()) {
                        state.marks.push((at, list));
                    }
                });
            }
        }
        let word = self.meta.u32(&mut list, op).await?;
        Ok((at, word))
    }

    /// Returns the tail of `file`, from its fragment.
    async fn tail(&self, inode: &Inode, file: &File, op: OpKind) -> Result<Arc<[u8]>, Error> {
        let bad = || {
            SquashfsError::BadDataBlock {
                inode: inode.number,
                block: file.layout.blocks,
            }
            .into_error(op)
        };
        let (index, offset) = file.layout.fragment.ok_or_else(bad)?;
        let fragment = self.fragment(index, op).await?;
        let block = u64::from(self.sb.block_size);
        let len = file.size().checked_rem(block).ok_or_else(bad)?;
        let start = usize::try_from(offset).map_err(|_| bad())?;
        let end = start.checked_add(usize::try_from(len).map_err(|_| bad())?);
        let tail = end
            .and_then(|end| fragment.get(start..end))
            .ok_or_else(bad)?;
        Ok(tail.into())
    }

    /// Returns fragment block `index`, decoded.
    async fn fragment(&self, index: u32, op: OpKind) -> Result<Arc<[u8]>, Error> {
        let hit = self.fragments.with(|cache| {
            let found = cache.iter().position(|(cached, _)| *cached == index)?;
            let entry = cache.remove(found);
            cache.insert(0, entry.clone());
            Some(entry.1)
        });
        if let Some(data) = hit {
            return Ok(data);
        }
        let bad = || SquashfsError::BadFragment { index }.into_error(op);
        if index >= self.sb.fragments {
            return Err(bad());
        }
        // Its entry: in a metadata block of 512, which the table lists.
        let table = self.sb.fragment_table;
        let pointer = table.checked_add(u64::from(index / 512).saturating_mul(8));
        let pointer = pointer
            .filter(|&at| at < self.sb.bytes_used)
            .ok_or_else(bad)?;
        let raw = filer_fs::read_exact(&self.meta.source, pointer, 8, op).await?;
        let block = filer_fs::le::u64_at(&raw, 0).ok_or_else(bad)?;
        let offset = usize::try_from(index % 512)
            .map_err(|_| bad())?
            .saturating_mul(16);
        let mut entry = Cursor { block, offset };
        let start = self.meta.u64(&mut entry, op).await?;
        let word = self.meta.u32(&mut entry, op).await?;
        let room = usize::try_from(self.sb.block_size).map_err(|_| bad())?;
        let data = self.block(start, word, room, false, op, bad).await?;
        self.fragments.with(|cache| {
            cache.truncate(3);
            cache.insert(0, (index, data.clone()));
        });
        Ok(data)
    }

    /// Reads the block at `at` of the size `word` lists, which decodes to
    /// `len` bytes, or if not `exact`, at most `len`: zeros if it lists no
    /// size. A malformed block is `bad`'s error.
    async fn block(
        &self,
        at: u64,
        word: u32,
        len: usize,
        exact: bool,
        op: OpKind,
        bad: impl Fn() -> Error,
    ) -> Result<Arc<[u8]>, Error> {
        let stored = word & STORED != 0;
        if word == 0 && exact {
            return Ok(vec![0; len].into());
        }
        let size = u64::from(word & SIZE);
        let end = at.checked_add(size).ok_or_else(&bad)?;
        let fits = !stored || size <= u64::try_from(len).unwrap_or(u64::MAX);
        if word == 0 || word & !(STORED | SIZE) != 0 || end > self.sb.bytes_used || !fits {
            return Err(bad());
        }
        let size = usize::try_from(size).map_err(|_| bad())?;
        let raw = filer_fs::read_exact(&self.meta.source, at, size, op).await?;
        let data = if stored {
            raw
        } else {
            let mut data = vec![0; len];
            let n = self
                .sb
                .compressor
                .decompress(&raw, &mut data)
                .ok_or_else(&bad)?;
            data.truncate(n);
            data
        };
        if exact && data.len() != len {
            return Err(bad());
        }
        Ok(data.into())
    }
}
