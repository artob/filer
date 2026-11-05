// This is free and unencumbered software released into the public domain.

//! Extended attributes: an inode's index names an entry of the id table,
//! which says where its pairs of names and values are, in the table of
//! them, and how many there are. A value may be stored once and referred
//! to by the pairs sharing it, so values are read one at a time, as
//! asked for.

use crate::{
    SquashfsError,
    meta::{Cursor, Reader},
    volume::Volume,
};
use alloc::{sync::Arc, vec::Vec};
use filer_api::{Error, OpKind, ReadAt, XattrNs};

/// A type's flag that the value is a reference to it.
const OUT_OF_LINE: u16 = 0x0100;

/// The longest value, as Linux's; the most bytes of names a file's
/// attributes may have, as Linux's list of them holds, prefixes aside;
/// and so the most attributes.
const MAX_VALUE: u32 = 65_536;
const MAX_NAMES: usize = 65_536;
const MAX_COUNT: u32 = 65_536;

/// Where the id table's entries are, and the pairs they point into.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Table {
    /// The table of names and values.
    pairs: u64,
    /// How many ids there are.
    ids: u32,
    /// The list of the metadata blocks of ids.
    list: u64,
}

impl Table {
    /// Reads the id table's header, at `at`.
    pub(crate) async fn read<S: ReadAt>(
        source: &S,
        at: u64,
        end: u64,
        op: OpKind,
    ) -> Result<Self, Error> {
        let bad = || SquashfsError::BadTables.into_error(op);
        let raw = filer_fs::read_exact(source, at, 16, op).await?;
        let pairs = filer_fs::le::u64_at(&raw, 0).ok_or_else(bad)?;
        let ids = filer_fs::le::u32_at(&raw, 8).ok_or_else(bad)?;
        let list = at
            .checked_add(16)
            .filter(|&list| list <= end)
            .ok_or_else(bad)?;
        if pairs >= at {
            return Err(bad());
        }
        Ok(Self { pairs, ids, list })
    }
}

/// An extended attribute: its name, and where its value is, after its
/// length.
pub(crate) struct Xattr {
    pub(crate) ns: XattrNs,
    pub(crate) name: Vec<u8>,
    value: Cursor,
}

impl<S: ReadAt> Volume<S> {
    /// Returns the names of the extended attributes of inode `inode`,
    /// which has the index `index` of them, if any, from the cache if it
    /// holds them.
    pub(crate) async fn xattrs(
        &self,
        inode: u32,
        index: Option<u32>,
        op: OpKind,
    ) -> Result<Arc<[Xattr]>, Error> {
        let Some(index) = index else {
            return Ok(Arc::from([]));
        };
        let hit = self.named.with(|cached| {
            cached
                .as_ref()
                .filter(|cached| cached.0 == index)
                .map(|cached| cached.1.clone())
        });
        if let Some(xattrs) = hit {
            return Ok(xattrs);
        }
        let xattrs: Arc<[Xattr]> = self.read_xattrs(inode, index, op).await?.into();
        self.named
            .with(|cached| *cached = Some((index, xattrs.clone())));
        Ok(xattrs)
    }

    /// Reads the names of the extended attributes of inode `inode`, which
    /// has the index `index` of them.
    async fn read_xattrs(&self, inode: u32, index: u32, op: OpKind) -> Result<Vec<Xattr>, Error> {
        let Some(table) = self.xattr_table else {
            return Ok(Vec::new());
        };
        let bad = || SquashfsError::BadXattr { inode }.into_error(op);
        if index >= table.ids {
            return Err(bad());
        }
        // Its id: in a metadata block of 512, which the list names.
        let pointer = table
            .list
            .checked_add(u64::from(index / 512).saturating_mul(8))
            .filter(|&at| at < self.sb.bytes_used)
            .ok_or_else(bad)?;
        let raw = filer_fs::read_exact(&self.meta.source, pointer, 8, op).await?;
        let block = filer_fs::le::u64_at(&raw, 0).ok_or_else(bad)?;
        let offset = usize::try_from(index % 512)
            .map_err(|_| bad())?
            .saturating_mul(16);
        let mut id = self.meta.reader(Cursor { block, offset });
        let reference = id.u64(op).await?;
        let count = id.u32(op).await?;
        if count > MAX_COUNT {
            return Err(bad());
        }
        let mut at = self
            .meta
            .reader(Cursor::of(table.pairs, reference).ok_or_else(bad)?);
        let (mut xattrs, mut names) = (Vec::new(), 0_usize);
        for _ in 0..count {
            let kind = at.u16(op).await?;
            let len = at.u16(op).await?;
            let ns = match kind & !OUT_OF_LINE {
                0 => XattrNs::User,
                1 => XattrNs::Trusted,
                2 => XattrNs::Security,
                _ => return Err(bad()),
            };
            names = names.saturating_add(usize::from(len));
            if names > MAX_NAMES {
                return Err(bad());
            }
            let name = at.read(usize::from(len), op).await?;
            let value = if kind & OUT_OF_LINE != 0 {
                // A value of 8 bytes, the reference of the value.
                if at.u32(op).await? != 8 {
                    return Err(bad());
                }
                let reference = at.u64(op).await?;
                Cursor::of(table.pairs, reference).ok_or_else(bad)?
            } else {
                // On past the value, to the next pair.
                let value = at.cursor();
                let len = Self::value_len(&mut at, op, bad).await?;
                at.skip(len, op).await?;
                value
            };
            xattrs.push(Xattr { ns, name, value });
        }
        Ok(xattrs)
    }

    /// Reads the value of `xattr`, one of inode `inode`'s.
    pub(crate) async fn xattr_value(
        &self,
        inode: u32,
        xattr: &Xattr,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let bad = || SquashfsError::BadXattr { inode }.into_error(op);
        let mut at = self.meta.reader(xattr.value);
        let len = Self::value_len(&mut at, op, bad).await?;
        at.read(len, op).await
    }

    /// Reads a value's length.
    async fn value_len(
        at: &mut Reader<'_, S>,
        op: OpKind,
        bad: impl Fn() -> Error,
    ) -> Result<usize, Error> {
        let len = at.u32(op).await?;
        if len > MAX_VALUE {
            return Err(bad());
        }
        usize::try_from(len).map_err(|_| bad())
    }
}
