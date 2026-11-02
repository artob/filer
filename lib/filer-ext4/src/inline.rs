// This is free and unencumbered software released into the public domain.

//! Inline data: under `inline_data`, files, directories, and symlinks
//! small enough are kept in their inode, with the inline data flag,
//! rather than in blocks: in `i_block`'s 60 bytes, then in the value of
//! the in-inode extended attribute `system.data`.
//!
//! A file's bytes are those, to its size. A directory's begin with its
//! parent's inode number, in four bytes, where a block's `.` and `..`
//! entries would be; then entries as a block holds them, filling the rest
//! of `i_block`, and the attribute's value after it.

use crate::{
    Ext4Error,
    dir::{self, DirEntry},
    inode::{I_BLOCK_SIZE, Inode},
    xattr,
};
use alloc::vec::Vec;
use filer_api::XattrNs;

/// The name of the attribute that continues inline data, in `system`.
const DATA: &[u8] = b"data";

/// The size of an inline directory's parent inode number.
const PARENT: usize = 4;

/// Returns the value of `inode`'s `system.data` attribute, empty if it
/// has none.
fn continuation(inode: &Inode) -> Result<Vec<u8>, Ext4Error> {
    Ok(xattr::in_inode(inode)?
        .into_iter()
        .find(|xattr| xattr.ns == XattrNs::System && xattr.name == DATA)
        .map(|xattr| xattr.value)
        .unwrap_or_default())
}

/// Returns the contents of inline file `inode`: `i_block`, then the
/// attribute's value, to its size, which they must hold.
pub(crate) fn data(inode: &Inode) -> Result<Vec<u8>, Ext4Error> {
    let bad = Ext4Error::BadInlineData {
        inode: inode.number(),
    };
    let mut data = inode.block().to_vec();
    data.extend(continuation(inode)?);
    let size = usize::try_from(inode.size()).map_err(|_| bad)?;
    if size > data.len() {
        return Err(bad);
    }
    data.truncate(size);
    Ok(data)
}

/// Returns the entries of inline directory `inode`, with or without
/// kinds, by `filetype`: those in `i_block` past the parent, then those
/// in the attribute's value, with offsets in their concatenation.
pub(crate) fn entries(inode: &Inode, filetype: bool) -> Result<Vec<DirEntry>, Ext4Error> {
    let number = inode.number();
    let first = inode.block().get(PARENT..).unwrap_or_default();
    let base = u64::try_from(PARENT).unwrap_or(0);
    let mut entries = dir::parse_block(first, base, filetype, number)?;
    let rest = continuation(inode)?;
    if !rest.is_empty() {
        let base = u64::try_from(I_BLOCK_SIZE).unwrap_or(0);
        entries.extend(dir::parse_block(&rest, base, filetype, number)?);
    }
    Ok(entries)
}
