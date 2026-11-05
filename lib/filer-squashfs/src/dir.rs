// This is free and unencumbered software released into the public domain.

//! Directory listings, in the directory table: runs of up to 256 entries
//! whose inodes share a metadata block, each run behind a header naming
//! that block and the inode number the entries' are relative to.

use crate::{
    SquashfsError,
    inode::{self, Body, Inode},
    meta::Metadata,
};
use alloc::vec::Vec;
use filer_api::{Error, FileKind, OpKind, ReadAt};

/// The most entries a header covers.
const RUN: u32 = 256;

/// The longest name.
const MAX_NAME: usize = 256;

/// An entry of a listing.
#[derive(Clone, Debug)]
pub(crate) struct Listed {
    pub(crate) name: Vec<u8>,
    pub(crate) kind: FileKind,
    /// The reference of its inode, in the inode table.
    pub(crate) reference: u64,
    /// Its inode's number.
    pub(crate) number: u32,
}

/// Walks the listing of directory `dir`, giving each entry to `found`,
/// until it returns `true`; returns that entry, if any.
pub(crate) async fn walk<S: ReadAt>(
    meta: &Metadata<S>,
    dir: &Inode,
    mut found: impl FnMut(&Listed) -> bool,
    op: OpKind,
) -> Result<Option<Listed>, Error> {
    let bad = || SquashfsError::BadDirectory { inode: dir.number }.into_error(op);
    let Body::Dir { listing, len } = &dir.body else {
        return Err(Error::new(filer_api::ErrorKind::NotADirectory, op));
    };
    let (mut at, mut left) = (*listing, *len);
    while left > 0 {
        // The header: how many entries, less one; the metadata block of
        // their inodes; and the number theirs are relative to.
        let header = meta.read(&mut at, 12, op).await?;
        let field = |at| filer_fs::le::u32_at(&header, at).unwrap_or(0);
        let (count, block, base) = (field(0), field(4), field(8));
        if count >= RUN {
            return Err(bad());
        }
        left = left.checked_sub(12).ok_or_else(bad)?;
        for _ in 0..=count {
            let entry = meta.read(&mut at, 8, op).await?;
            let u16_at = |at| filer_fs::le::u16_at(&entry, at).unwrap_or(0);
            let (offset, delta, kind, size) = (u16_at(0), u16_at(2), u16_at(4), u16_at(6));
            let len = usize::from(size).saturating_add(1);
            if len > MAX_NAME {
                return Err(bad());
            }
            let name = meta.read(&mut at, len, op).await?;
            let used = u32::try_from(len).ok().and_then(|len| len.checked_add(8));
            left = used
                .and_then(|used| left.checked_sub(used))
                .ok_or_else(bad)?;
            // The offset of the number from the header's, signed.
            let delta = i32::from(i16::from_le_bytes(delta.to_le_bytes()));
            let number = base.checked_add_signed(delta).ok_or_else(bad)?;
            let invalid =
                name.contains(&b'/') || name.contains(&0) || name == b"." || name == b"..";
            if invalid {
                return Err(bad());
            }
            let listed = Listed {
                name,
                kind: inode::kind(kind).ok_or_else(bad)?,
                reference: u64::from(block) << 16 | u64::from(offset),
                number,
            };
            if found(&listed) {
                return Ok(Some(listed));
            }
        }
    }
    Ok(None)
}

/// Returns every entry of the listing of directory `dir`.
pub(crate) async fn entries<S: ReadAt>(
    meta: &Metadata<S>,
    dir: &Inode,
    op: OpKind,
) -> Result<Vec<Listed>, Error> {
    let mut entries = Vec::new();
    walk(
        meta,
        dir,
        |listed| {
            entries.push(listed.clone());
            false
        },
        op,
    )
    .await?;
    Ok(entries)
}
