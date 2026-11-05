// This is free and unencumbered software released into the public domain.

//! Metadata blocks: the inode, directory, fragment, and extended attribute
//! tables are streams of them, each up to 8 KiB decoded, behind a 16-bit
//! header giving its size on disk, and whether it is stored as is.

use crate::{SquashfsError, codec::Compressor};
use alloc::{sync::Arc, vec, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::Lock;

/// The most a metadata block decodes to.
pub(crate) const BLOCK: usize = 8192;

/// The header's flag that the block is stored as is.
const STORED: u16 = 0x8000;

/// How many decoded blocks the cache keeps.
const CACHED: usize = 32;

/// A position in a stream of metadata blocks: the position of a block in
/// the image, and an offset in what it decodes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Cursor {
    pub(crate) block: u64,
    pub(crate) offset: usize,
}

impl Cursor {
    /// Returns the position that `reference` names in the table at
    /// `table`: a block's position in it, shifted 16 bits up, and an
    /// offset in that block.
    pub(crate) fn of(table: u64, reference: u64) -> Option<Self> {
        let offset = usize::from(u16::try_from(reference & 0xFFFF).ok()?);
        Some(Self {
            block: table.checked_add(reference >> 16)?,
            offset,
        })
    }
}

/// A decoded block, and where the next begins.
#[derive(Clone)]
struct Decoded {
    at: u64,
    bytes: Arc<[u8]>,
    next: u64,
}

/// What reads metadata blocks: the image, its size and compressor, and the
/// blocks most recently read: what they decode to, or `None` for those
/// that do not, which a corrupt image may point to again and again.
pub(crate) struct Metadata<S> {
    pub(crate) source: S,
    pub(crate) end: u64,
    pub(crate) compressor: Compressor,
    cache: Lock<Vec<(u64, Option<Decoded>)>>,
}

impl<S: ReadAt> Metadata<S> {
    pub(crate) fn new(source: S, end: u64, compressor: Compressor) -> Self {
        Self {
            source,
            end,
            compressor,
            cache: Lock::new(Vec::new()),
        }
    }

    /// Reads the block at `at`: what it decodes to, and where the next
    /// begins.
    async fn block(&self, at: u64, op: OpKind) -> Result<Decoded, Error> {
        let hit = self.cache.with(|cache| {
            let found = cache.iter().position(|cached| cached.0 == at)?;
            // The most recent first.
            let cached = cache.remove(found);
            cache.insert(0, cached.clone());
            Some(cached.1)
        });
        let block = match hit {
            Some(block) => block,
            None => {
                let block = self.decode(at, op).await?;
                self.cache.with(|cache| {
                    cache.truncate(CACHED.saturating_sub(1));
                    cache.insert(0, (at, block.clone()));
                });
                block
            },
        };
        block.ok_or_else(|| SquashfsError::BadMetadataBlock { at }.into_error(op))
    }

    /// Decodes the block at `at`, or returns `None` if it does not.
    async fn decode(&self, at: u64, op: OpKind) -> Result<Option<Decoded>, Error> {
        let Some(body) = at.checked_add(2).filter(|&body| body <= self.end) else {
            return Ok(None);
        };
        let header = filer_fs::read_exact(&self.source, at, 2, op).await?;
        let Some(header) = filer_fs::le::u16_at(&header, 0) else {
            return Ok(None);
        };
        let size = usize::from(header & !STORED);
        let next = body
            .checked_add(u64::from(header & !STORED))
            .filter(|&next| next <= self.end && size > 0 && size <= BLOCK);
        let Some(next) = next else {
            return Ok(None);
        };
        let raw = filer_fs::read_exact(&self.source, body, size, op).await?;
        let bytes: Arc<[u8]> = if header & STORED != 0 {
            raw.into()
        } else {
            let mut decoded = vec![0; BLOCK];
            let Some(len) = self.compressor.decompress(&raw, &mut decoded) else {
                return Ok(None);
            };
            decoded.truncate(len);
            decoded.into()
        };
        Ok(Some(Decoded { at, bytes, next }))
    }

    /// Returns a reader of the stream from `cursor` on.
    pub(crate) const fn reader(&self, cursor: Cursor) -> Reader<'_, S> {
        Reader {
            meta: self,
            cursor,
            held: None,
        }
    }
}

/// Reads a stream of metadata blocks from a cursor on, holding the block
/// it is in: reads in a row, of a listing's entries, say, decode and look
/// up each block once.
pub(crate) struct Reader<'a, S> {
    meta: &'a Metadata<S>,
    cursor: Cursor,
    held: Option<Decoded>,
}

impl<S: ReadAt> Reader<'_, S> {
    /// Returns where it is.
    pub(crate) const fn cursor(&self) -> Cursor {
        self.cursor
    }

    /// Gives the next `len` bytes to `each`, a run at a time, across
    /// blocks, and moves past them.
    async fn take(
        &mut self,
        len: usize,
        mut each: impl FnMut(&[u8]),
        op: OpKind,
    ) -> Result<(), Error> {
        let mut left = len;
        while left > 0 {
            let block = match &self.held {
                Some(block) if block.at == self.cursor.block => block,
                _ => self
                    .held
                    .insert(self.meta.block(self.cursor.block, op).await?),
            };
            let Some(available) = block.bytes.get(self.cursor.offset..) else {
                return Err(SquashfsError::BadMetadataBlock { at: block.at }.into_error(op));
            };
            if available.is_empty() {
                // On to the next block, which lies further on, so that
                // this ends.
                self.cursor = Cursor {
                    block: block.next,
                    offset: 0,
                };
                continue;
            }
            let n = available.len().min(left);
            each(available.get(..n).unwrap_or_default());
            self.cursor.offset = self.cursor.offset.saturating_add(n);
            left = left.saturating_sub(n);
        }
        Ok(())
    }

    /// Fills `out` from where it is on, and moves past it.
    pub(crate) async fn fill(&mut self, out: &mut [u8], op: OpKind) -> Result<(), Error> {
        let mut filled = 0;
        let len = out.len();
        self.take(
            len,
            |run| {
                let to = out
                    .get_mut(filled..)
                    .and_then(|rest| rest.get_mut(..run.len()));
                if let Some(to) = to {
                    to.copy_from_slice(run);
                }
                filled = filled.saturating_add(run.len());
            },
            op,
        )
        .await
    }

    /// Moves past the next `len` bytes.
    pub(crate) async fn skip(&mut self, len: usize, op: OpKind) -> Result<(), Error> {
        self.take(len, |_| {}, op).await
    }

    /// Reads `len` bytes from where it is on. Callers bound `len`.
    pub(crate) async fn read(&mut self, len: usize, op: OpKind) -> Result<Vec<u8>, Error> {
        let mut bytes = vec![0; len];
        self.fill(&mut bytes, op).await?;
        Ok(bytes)
    }

    /// Reads a little-endian `u16`.
    pub(crate) async fn u16(&mut self, op: OpKind) -> Result<u16, Error> {
        let mut bytes = [0; 2];
        self.fill(&mut bytes, op).await?;
        Ok(u16::from_le_bytes(bytes))
    }

    /// Reads a little-endian `u32`.
    pub(crate) async fn u32(&mut self, op: OpKind) -> Result<u32, Error> {
        let mut bytes = [0; 4];
        self.fill(&mut bytes, op).await?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// Reads a little-endian `u64`.
    pub(crate) async fn u64(&mut self, op: OpKind) -> Result<u64, Error> {
        let mut bytes = [0; 8];
        self.fill(&mut bytes, op).await?;
        Ok(u64::from_le_bytes(bytes))
    }
}
