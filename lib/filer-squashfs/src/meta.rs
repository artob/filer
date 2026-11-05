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
/// blocks most recently decoded.
pub(crate) struct Metadata<S> {
    pub(crate) source: S,
    pub(crate) end: u64,
    pub(crate) compressor: Compressor,
    cache: Lock<Vec<Decoded>>,
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
            let found = cache.iter().position(|block| block.at == at)?;
            // The most recent first.
            let block = cache.remove(found);
            cache.insert(0, block.clone());
            Some(block)
        });
        if let Some(block) = hit {
            return Ok(block);
        }
        let bad = || SquashfsError::BadMetadataBlock { at }.into_error(op);
        let body = at
            .checked_add(2)
            .filter(|&body| body <= self.end)
            .ok_or_else(bad)?;
        let header = filer_fs::read_exact(&self.source, at, 2, op).await?;
        let header = filer_fs::le::u16_at(&header, 0).ok_or_else(bad)?;
        let size = usize::from(header & !STORED);
        let next = body
            .checked_add(u64::from(header & !STORED))
            .filter(|&next| next <= self.end && size > 0 && size <= BLOCK)
            .ok_or_else(bad)?;
        let raw = filer_fs::read_exact(&self.source, body, size, op).await?;
        let bytes: Arc<[u8]> = if header & STORED != 0 {
            raw.into()
        } else {
            let mut decoded = vec![0; BLOCK];
            let len = self
                .compressor
                .decompress(&raw, &mut decoded)
                .ok_or_else(bad)?;
            decoded.truncate(len);
            decoded.into()
        };
        let block = Decoded { at, bytes, next };
        self.cache.with(|cache| {
            cache.truncate(CACHED.saturating_sub(1));
            cache.insert(0, block.clone());
        });
        Ok(block)
    }

    /// Reads `len` bytes from `cursor` on, across blocks, and moves it past
    /// them. Callers bound `len`.
    pub(crate) async fn read(
        &self,
        cursor: &mut Cursor,
        len: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::with_capacity(len);
        while bytes.len() < len {
            let block = self.block(cursor.block, op).await?;
            let Some(available) = block.bytes.get(cursor.offset..) else {
                return Err(SquashfsError::BadMetadataBlock { at: block.at }.into_error(op));
            };
            if available.is_empty() {
                // On to the next block, which lies further on, so that
                // this ends.
                *cursor = Cursor {
                    block: block.next,
                    offset: 0,
                };
                continue;
            }
            let n = available.len().min(len.saturating_sub(bytes.len()));
            bytes.extend_from_slice(available.get(..n).unwrap_or_default());
            cursor.offset = cursor.offset.saturating_add(n);
        }
        Ok(bytes)
    }

    /// Reads a little-endian `u16` from `cursor` on.
    pub(crate) async fn u16(&self, cursor: &mut Cursor, op: OpKind) -> Result<u16, Error> {
        let bytes = self.read(cursor, 2, op).await?;
        Ok(filer_fs::le::u16_at(&bytes, 0).unwrap_or(0))
    }

    /// Reads a little-endian `u32` from `cursor` on.
    pub(crate) async fn u32(&self, cursor: &mut Cursor, op: OpKind) -> Result<u32, Error> {
        let bytes = self.read(cursor, 4, op).await?;
        Ok(filer_fs::le::u32_at(&bytes, 0).unwrap_or(0))
    }

    /// Reads a little-endian `u64` from `cursor` on.
    pub(crate) async fn u64(&self, cursor: &mut Cursor, op: OpKind) -> Result<u64, Error> {
        let bytes = self.read(cursor, 8, op).await?;
        Ok(filer_fs::le::u64_at(&bytes, 0).unwrap_or(0))
    }
}
