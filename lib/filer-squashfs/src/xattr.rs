// This is free and unencumbered software released into the public domain.

//! Extended attributes: an inode's index names an entry of the id table,
//! which says where its pairs of names and values are, in the table of
//! them, and how many there are. A value may be stored once and referred
//! to by the pairs sharing it.

use crate::{SquashfsError, meta::Cursor, volume::Volume};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt, XattrNs};

/// A type's flag that the value is a reference to it.
const OUT_OF_LINE: u16 = 0x0100;

/// The longest value, as Linux's, and the most attributes a file may
/// have, as Linux's list of their names holds.
const MAX_VALUE: u32 = 65_536;
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

/// An extended attribute.
pub(crate) struct Xattr {
    pub(crate) ns: XattrNs,
    pub(crate) name: Vec<u8>,
    pub(crate) value: Vec<u8>,
}

impl<S: ReadAt> Volume<S> {
    /// Reads the extended attributes of inode `inode`, which has the index
    /// `index` of them, if any.
    pub(crate) async fn xattrs(
        &self,
        inode: u32,
        index: Option<u32>,
        op: OpKind,
    ) -> Result<Vec<Xattr>, Error> {
        let (Some(index), Some(table)) = (index, self.xattr_table) else {
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
        let mut id = Cursor { block, offset };
        let reference = self.meta.u64(&mut id, op).await?;
        let count = self.meta.u32(&mut id, op).await?;
        if count > MAX_COUNT {
            return Err(bad());
        }
        let mut at = Cursor::of(table.pairs, reference).ok_or_else(bad)?;
        let mut xattrs = Vec::new();
        for _ in 0..count {
            let kind = self.meta.u16(&mut at, op).await?;
            let len = self.meta.u16(&mut at, op).await?;
            let ns = match kind & !OUT_OF_LINE {
                0 => XattrNs::User,
                1 => XattrNs::Trusted,
                2 => XattrNs::Security,
                _ => return Err(bad()),
            };
            let name = self.meta.read(&mut at, usize::from(len), op).await?;
            let value = if kind & OUT_OF_LINE != 0 {
                // A value of 8 bytes, the reference of the value.
                if self.meta.u32(&mut at, op).await? != 8 {
                    return Err(bad());
                }
                let reference = self.meta.u64(&mut at, op).await?;
                let mut value = Cursor::of(table.pairs, reference).ok_or_else(bad)?;
                self.value(&mut value, op, bad).await?
            } else {
                self.value(&mut at, op, bad).await?
            };
            xattrs.push(Xattr { ns, name, value });
        }
        Ok(xattrs)
    }

    /// Reads a value: its length, and its bytes.
    async fn value(
        &self,
        at: &mut Cursor,
        op: OpKind,
        bad: impl Fn() -> Error,
    ) -> Result<Vec<u8>, Error> {
        let len = self.meta.u32(at, op).await?;
        if len > MAX_VALUE {
            return Err(bad());
        }
        let len = usize::try_from(len).map_err(|_| bad())?;
        self.meta.read(at, len, op).await
    }
}
