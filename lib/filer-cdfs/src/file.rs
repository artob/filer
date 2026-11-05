// This is free and unencumbered software released into the public domain.

//! Files' data: their extents, one after another, and, for a file zisofs
//! compresses, the blocks they hold. Such a file begins with a header, of
//! a magic number, its size, and its block size, then lists where each
//! block begins, and where the last ends; each block is a zlib stream,
//! but for an empty one, which stands for zeros.

use crate::{CdfsError, node::Data, read, volume::Volume};
use alloc::{sync::Arc, vec, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::{Lock, le};

/// What begins a file zisofs compresses.
#[cfg(feature = "zisofs")]
const MAGIC: [u8; 8] = [0x37, 0xE4, 0x53, 0x96, 0xC9, 0xDB, 0xD6, 0x07];

/// An open file: for one zisofs compresses, its layout, and the block
/// last decoded.
pub(crate) struct File {
    zisofs: Option<Layout>,
    last: Lock<Option<(u64, Arc<[u8]>)>>,
}

/// The layout of a file zisofs compresses.
#[derive(Clone, Copy, Debug)]
struct Layout {
    /// Its size, uncompressed.
    size: u64,
    /// The base-2 logarithm of its block size.
    log: u8,
    /// How many blocks it has.
    blocks: u64,
    /// Where its pointers to them begin.
    table: u64,
}

impl<S: ReadAt> Volume<S> {
    /// Opens the file whose data is `data`, of `len` bytes.
    pub(crate) async fn open_data(&self, data: &Data, len: u64, op: OpKind) -> Result<File, Error> {
        if let Some(refusal) = data.refusal {
            return Err(refusal.into_error(op));
        }
        let zisofs = match data.zisofs {
            Some((log, header)) => Some(self.layout(data, len, log, header, op).await?),
            None => None,
        };
        Ok(File {
            zisofs,
            last: Lock::new(None),
        })
    }

    /// Reads up to `max` bytes of `file`, whose data is `data`, of `len`
    /// bytes, at `off`.
    pub(crate) async fn read_file(
        &self,
        data: &Data,
        len: u64,
        file: &File,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let end = off
            .saturating_add(u64::try_from(max).unwrap_or(u64::MAX))
            .min(len);
        let Some(layout) = file.zisofs else {
            let n = usize::try_from(end.saturating_sub(off)).unwrap_or(usize::MAX);
            return self.stored(data, off, n, op).await;
        };
        let mut bytes = Vec::with_capacity(usize::try_from(end.saturating_sub(off)).unwrap_or(0));
        let mut at = off;
        while at < end {
            let index = at.checked_shr(u32::from(layout.log)).unwrap_or(0);
            let start = index.checked_shl(u32::from(layout.log)).unwrap_or(0);
            let block = self.block(data, file, layout, index, op).await?;
            let within = usize::try_from(at.saturating_sub(start)).unwrap_or(usize::MAX);
            let want = usize::try_from(end.saturating_sub(at)).unwrap_or(usize::MAX);
            let available = block.get(within..).unwrap_or_default();
            let n = available.len().min(want);
            if n == 0 {
                return Err(CdfsError::BadZisofs { at: data.at }.into_error(op));
            }
            bytes.extend_from_slice(available.get(..n).unwrap_or_default());
            at = at.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
        }
        Ok(bytes)
    }

    /// Reads `len` bytes of `data`, as stored, from `off` on, across its
    /// extents.
    async fn stored(
        &self,
        data: &Data,
        off: u64,
        len: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let mut bytes = Vec::with_capacity(len.min(1 << 20));
        let mut start = 0_u64;
        for &(position, extent) in &data.extents {
            let have = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            if bytes.len() >= len {
                break;
            }
            let at = off.saturating_add(have);
            let end = start.saturating_add(extent);
            if at < end {
                let within = at.saturating_sub(start);
                let n = extent
                    .saturating_sub(within)
                    .min(u64::try_from(len.saturating_sub(bytes.len())).unwrap_or(u64::MAX));
                let n = usize::try_from(n).unwrap_or(usize::MAX);
                let bad = || CdfsError::BadRecord { at: data.at }.into_error(op);
                let at = position.saturating_add(within);
                let read = read::read(&self.source, at, n, op, bad).await?;
                bytes.extend_from_slice(&read);
            }
            start = end;
        }
        if bytes.len() < len {
            return Err(CdfsError::BadRecord { at: data.at }.into_error(op));
        }
        Ok(bytes)
    }

    /// Reads and checks the header of a file zisofs compresses.
    #[cfg(feature = "zisofs")]
    async fn layout(
        &self,
        data: &Data,
        len: u64,
        log: u8,
        header: u8,
        op: OpKind,
    ) -> Result<Layout, Error> {
        let bad = || CdfsError::BadZisofs { at: data.at }.into_error(op);
        let bytes = self.stored(data, 0, 16, op).await.map_err(|_| bad())?;
        let valid = bytes.get(..8) == Some(&MAGIC[..])
            && le::u32_at(&bytes, 8).map(u64::from) == Some(len)
            && bytes.get(12..14) == Some(&[header, log][..]);
        if !valid {
            return Err(bad());
        }
        let block = 1_u64.checked_shl(u32::from(log)).ok_or_else(bad)?;
        let blocks = len.div_ceil(block);
        let table = u64::from(header).checked_mul(4).ok_or_else(bad)?;
        let table_end = blocks
            .checked_add(1)
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(table))
            .ok_or_else(bad)?;
        if table_end > data.stored {
            return Err(bad());
        }
        Ok(Layout {
            size: len,
            log,
            blocks,
            table,
        })
    }

    /// Without the `zisofs` feature, refuses what it compresses; files
    /// are refused before this is called.
    #[cfg(not(feature = "zisofs"))]
    async fn layout(&self, data: &Data, _: u64, _: u8, _: u8, op: OpKind) -> Result<Layout, Error> {
        Err(CdfsError::UnsupportedCompression { at: data.at }.into_error(op))
    }

    /// Returns block `index` of a file zisofs compresses, decoded.
    async fn block(
        &self,
        data: &Data,
        file: &File,
        layout: Layout,
        index: u64,
        op: OpKind,
    ) -> Result<Arc<[u8]>, Error> {
        let hit = file.last.with(|last| {
            last.as_ref()
                .filter(|(cached, _)| *cached == index)
                .map(|(_, block)| block.clone())
        });
        if let Some(block) = hit {
            return Ok(block);
        }
        let bad = || CdfsError::BadZisofs { at: data.at }.into_error(op);
        if index >= layout.blocks {
            return Err(bad());
        }
        let size = 1_u64.checked_shl(u32::from(layout.log)).ok_or_else(bad)?;
        let start = index.checked_mul(size).ok_or_else(bad)?;
        let expected =
            usize::try_from(layout.size.saturating_sub(start).min(size)).map_err(|_| bad())?;
        let pointer = index
            .checked_mul(4)
            .and_then(|at| at.checked_add(layout.table))
            .ok_or_else(bad)?;
        let pointers = self.stored(data, pointer, 8, op).await?;
        let (from, to) = (
            le::u32_at(&pointers, 0).map(u64::from).ok_or_else(bad)?,
            le::u32_at(&pointers, 4).map(u64::from).ok_or_else(bad)?,
        );
        let first = layout
            .blocks
            .checked_add(1)
            .and_then(|n| n.checked_mul(4))
            .and_then(|n| n.checked_add(layout.table))
            .ok_or_else(bad)?;
        // A zlib stream of a block is at most a little longer than it.
        let bound = size.checked_mul(2).ok_or_else(bad)?;
        if from < first || to < from || to > data.stored || to.saturating_sub(from) > bound {
            return Err(bad());
        }
        let block: Arc<[u8]> = if from == to {
            vec![0; expected].into()
        } else {
            let len = usize::try_from(to.saturating_sub(from)).map_err(|_| bad())?;
            let compressed = self.stored(data, from, len, op).await?;
            let mut decoded = vec![0; expected];
            if inflate(&compressed, &mut decoded) != Some(expected) {
                return Err(bad());
            }
            decoded.into()
        };
        file.last.with(|last| *last = Some((index, block.clone())));
        Ok(block)
    }
}

/// Decompresses `input`, a whole zlib stream, into `output`, and returns
/// how many bytes it decompressed to, or `None` if it is malformed, or
/// decompresses to more than `output` holds.
#[cfg(feature = "zisofs")]
fn inflate(input: &[u8], output: &mut [u8]) -> Option<usize> {
    let mut decoder = decode_zlib::Decoder::new();
    let (mut consumed, mut produced) = (0_usize, 0_usize);
    loop {
        let (taken, made) = decoder
            .decode(input.get(consumed..)?, output.get_mut(produced..)?)
            .ok()?;
        consumed = consumed.checked_add(taken)?;
        produced = produced.checked_add(made)?;
        if (taken, made) == (0, 0) {
            break;
        }
    }
    (consumed == input.len() && decoder.finish().is_ok()).then_some(produced)
}

/// Without the `zisofs` feature, decompresses nothing.
#[cfg(not(feature = "zisofs"))]
fn inflate(_: &[u8], _: &mut [u8]) -> Option<usize> {
    None
}
