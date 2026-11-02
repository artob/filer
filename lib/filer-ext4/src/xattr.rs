// This is free and unencumbered software released into the public domain.

//! Extended attributes: in the inode, after its extra fields, and in one
//! block named by `i_file_acl`. Both hold entries naming a prefix by index,
//! and values elsewhere in the same region, or under `ea_inode`, in the
//! data of an inode of their own, which no directory names.

use crate::{
    Ext4Error,
    file_map::FileMap,
    inode::{self, Inode},
    superblock::Superblock,
};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt, XattrNs};
use filer_fs::{le, posix_acl, read_exact};

/// The magic number of both an attribute block and in-inode attributes.
const MAGIC: u32 = 0xEA02_0000;

/// The size of an attribute block's header, and of an entry's fixed fields.
const BLOCK_HEADER: usize = 32;
const ENTRY_HEADER: usize = 16;

/// The name indexes of the POSIX ACLs, whose values are converted.
const ACL_ACCESS: u8 = 2;
const ACL_DEFAULT: u8 = 3;

/// The largest value an inode of its own holds, as Linux allows.
const MAX_INODE_VALUE: u32 = 1 << 24;

/// An extended attribute: its namespace, its name within it, and its value,
/// or, until it is read, the inode holding it and its size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Xattr {
    pub(crate) ns: XattrNs,
    pub(crate) name: Vec<u8>,
    pub(crate) value: Vec<u8>,
    value_inode: Option<(u32, u32)>,
}

/// Returns the namespace of name index `index`, and what the name starts
/// with in it, or `None` for an index Filer has no namespace for.
fn prefix(index: u8) -> Option<(XattrNs, &'static [u8])> {
    Some(match index {
        1 => (XattrNs::User, b""),
        ACL_ACCESS => (XattrNs::System, b"posix_acl_access"),
        ACL_DEFAULT => (XattrNs::System, b"posix_acl_default"),
        4 => (XattrNs::Trusted, b""),
        6 => (XattrNs::Security, b""),
        7 => (XattrNs::System, b""),
        8 => (XattrNs::System, b"richacl"),
        _ => return None,
    })
}

/// Parses the entries of `region` from `entries`, whose values are at
/// offsets from `values`, until an entry whose first four bytes are zero,
/// or the region's end.
fn parse(
    region: &[u8],
    entries: usize,
    values: usize,
    inode: u32,
) -> Result<Vec<Xattr>, Ext4Error> {
    let bad = Ext4Error::BadXattr { inode };
    let mut found = Vec::new();
    let mut at = entries;
    loop {
        match le::u32_at(region, at) {
            None | Some(0) => return Ok(found),
            Some(_) => {},
        }
        let header = region.get(at..at.saturating_add(ENTRY_HEADER)).ok_or(bad)?;
        let (Some(&name_len), Some(&index)) = (header.first(), header.get(1)) else {
            return Err(bad);
        };
        let value_offs = usize::from(le::u16_at(header, 2).ok_or(bad)?);
        let value_inum = le::u32_at(header, 4).ok_or(bad)?;
        let value_size = le::u32_at(header, 8).ok_or(bad)?;
        let name_at = at.saturating_add(ENTRY_HEADER);
        let name = region
            .get(name_at..name_at.saturating_add(usize::from(name_len)))
            .ok_or(bad)?;
        // A value in an inode of its own is read later; an ACL never is.
        let (value, value_inode) = if value_inum != 0 {
            if matches!(index, ACL_ACCESS | ACL_DEFAULT) || value_size > MAX_INODE_VALUE {
                return Err(bad);
            }
            (&[][..], Some((value_inum, value_size)))
        } else {
            let value_at = values.checked_add(value_offs).ok_or(bad)?;
            let value_size = usize::try_from(value_size).map_err(|_| bad)?;
            let value = region
                .get(value_at..value_at.checked_add(value_size).ok_or(bad)?)
                .ok_or(bad)?;
            (value, None)
        };
        if let Some((ns, start)) = prefix(index) {
            let mut full = start.to_vec();
            full.extend_from_slice(name);
            let value = match index {
                ACL_ACCESS | ACL_DEFAULT if name.is_empty() => acl(value).ok_or(bad)?,
                ACL_ACCESS | ACL_DEFAULT => return Err(bad),
                _ => value.to_vec(),
            };
            found.push(Xattr {
                ns,
                name: full,
                value,
                value_inode,
            });
        }
        at = ENTRY_HEADER
            .saturating_add(usize::from(name_len))
            .next_multiple_of(4)
            .checked_add(at)
            .ok_or(bad)?;
    }
}

/// Converts a POSIX ACL from its form on disk, little-endian: version 1,
/// then for each entry its tag and permissions, and an id only for a named
/// user or group. To the one Linux's `getxattr` returns.
fn acl(value: &[u8]) -> Option<Vec<u8>> {
    if le::u32_at(value, 0)? != 1 {
        return None;
    }
    let mut out = posix_acl::Encoder::new();
    let mut at = 4usize;
    while at < value.len() {
        let tag = le::u16_at(value, at)?;
        let perm = le::u16_at(value, at.checked_add(2)?)?;
        let id = if posix_acl::is_named(tag) {
            let id = le::u32_at(value, at.checked_add(4)?)?;
            at = at.checked_add(8)?;
            id
        } else {
            at = at.checked_add(4)?;
            0
        };
        out.push(tag, perm, id)?;
    }
    Some(out.finish())
}

/// Parses the attributes in `inode` itself, if it has any.
pub(crate) fn in_inode(inode: &Inode) -> Result<Vec<Xattr>, Ext4Error> {
    let tail = inode.tail();
    if le::u32_at(tail, 0) != Some(MAGIC) {
        return Ok(Vec::new());
    }
    parse(tail, 4, 4, inode.number())
}

/// Parses an attribute block of `inode`.
pub(crate) fn in_block(block: &[u8], inode: u32) -> Result<Vec<Xattr>, Ext4Error> {
    let bad = Ext4Error::BadXattr { inode };
    // One block: no attribute spans more.
    if le::u32_at(block, 0) != Some(MAGIC) || le::u32_at(block, 8) != Some(1) {
        return Err(bad);
    }
    parse(block, BLOCK_HEADER, 0, inode)
}

/// Returns `true` for `system.data`, which holds inline data's bytes past
/// `i_block` rather than an attribute, and which Linux does not list.
fn is_inline_data(xattr: &Xattr) -> bool {
    xattr.ns == XattrNs::System && xattr.name == b"data"
}

/// Reads all of `inode`'s attributes: those in it, then its block's, but
/// for `system.data`.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    inode: &Inode,
    op: OpKind,
) -> Result<Vec<Xattr>, Error> {
    let fail = |error: Ext4Error| error.into_error(op);
    let mut xattrs = in_inode(inode).map_err(fail)?;
    let block = inode.file_acl(sb);
    if block != 0 {
        let bad = || {
            fail(Ext4Error::BadBlockNumber {
                inode: inode.number(),
                block,
            })
        };
        if block <= u64::from(sb.first_data_block()) {
            return Err(bad());
        }
        let at = sb.block_offset(block).ok_or_else(bad)?;
        let len = usize::try_from(sb.block_size()).map_err(|_| bad())?;
        let bytes = read_exact(source, at, len, op).await?;
        if !sb.checksums().xattr_block_ok(block, &bytes) {
            return Err(fail(Ext4Error::BadXattrChecksum {
                inode: inode.number(),
            }));
        }
        xattrs.extend(in_block(&bytes, inode.number()).map_err(fail)?);
    }
    xattrs.retain(|xattr| !is_inline_data(xattr));
    for xattr in &mut xattrs {
        if let Some((number, size)) = xattr.value_inode.take() {
            xattr.value = inode_value(source, sb, inode.number(), number, size, op).await?;
        }
    }
    Ok(xattrs)
}

/// Reads the value of `size` bytes that inode `number` holds for
/// `owner`'s attribute: a file with the attribute inode flag, of that
/// size, under `ea_inode`.
async fn inode_value<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    owner: u32,
    number: u32,
    size: u32,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let bad = || Ext4Error::BadXattr { inode: owner }.into_error(op);
    if !sb.has_ea_inodes() {
        return Err(bad());
    }
    let holder = inode::read(source, sb, number, op).await?;
    if holder.kind() != filer_api::FileKind::File
        || !holder.is_ea_inode()
        || holder.size() != u64::from(size)
    {
        return Err(bad());
    }
    let map = FileMap::new(&holder, sb).map_err(|error| error.into_error(op))?;
    let len = usize::try_from(size).map_err(|_| bad())?;
    let value = map.read(source, sb, holder.size(), 0, len, op).await?;
    if value.len() != len {
        return Err(bad());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inode::tests::Raw;
    use alloc::vec;

    /// Lays out entries of (index, name, value) from `entries`, values
    /// from the region's end, at offsets from `values`.
    fn region(len: usize, entries: usize, values: usize, items: &[(u8, &[u8], &[u8])]) -> Vec<u8> {
        let mut b = vec![0; len];
        let mut at = entries;
        let mut end = len;
        for (index, name, value) in items {
            end = (end - value.len()) & !3;
            b[end..end + value.len()].copy_from_slice(value);
            b[at] = u8::try_from(name.len()).unwrap();
            b[at + 1] = *index;
            b[at + 2..at + 4].copy_from_slice(&u16::try_from(end - values).unwrap().to_le_bytes());
            b[at + 8..at + 12].copy_from_slice(&u32::try_from(value.len()).unwrap().to_le_bytes());
            b[at + 16..at + 16 + name.len()].copy_from_slice(name);
            at += (16 + name.len()).next_multiple_of(4);
        }
        b
    }

    fn block(items: &[(u8, &[u8], &[u8])]) -> Vec<u8> {
        let mut b = region(1024, 32, 0, items);
        b[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        b[4..8].copy_from_slice(&1u32.to_le_bytes());
        b[8..12].copy_from_slice(&1u32.to_le_bytes());
        b
    }

    fn xattr(ns: XattrNs, name: &[u8], value: &[u8]) -> Xattr {
        Xattr {
            ns,
            name: name.to_vec(),
            value: value.to_vec(),
            value_inode: None,
        }
    }

    #[test]
    fn parses_every_namespace() {
        let b = block(&[
            (1, b"comment", b"a comment"),
            (1, b"empty", b""),
            (4, b"secret", b"x"),
            (6, b"selinux", b"label"),
            (7, b"data", b"inline"),
            (8, b"", b"rich"),
            // An index Filer has no namespace for.
            (5, b"lustre", b"skipped"),
        ]);
        assert_eq!(
            in_block(&b, 12).unwrap(),
            [
                xattr(XattrNs::User, b"comment", b"a comment"),
                xattr(XattrNs::User, b"empty", b""),
                xattr(XattrNs::Trusted, b"secret", b"x"),
                xattr(XattrNs::Security, b"selinux", b"label"),
                xattr(XattrNs::System, b"data", b"inline"),
                xattr(XattrNs::System, b"richacl", b"rich"),
            ]
        );
    }

    #[test]
    fn converts_posix_acls() {
        // The owner rw-, user 1000 r--, the group r--, the mask r--, and
        // others ---, as stored.
        let mut stored = 1u32.to_le_bytes().to_vec();
        for (tag, perm, id) in [
            (1u16, 6u16, None),
            (2, 4, Some(1000u32)),
            (4, 4, None),
            (0x10, 4, None),
            (0x20, 0, None),
        ] {
            stored.extend(tag.to_le_bytes());
            stored.extend(perm.to_le_bytes());
            if let Some(id) = id {
                stored.extend(id.to_le_bytes());
            }
        }
        let mut expected = 2u32.to_le_bytes().to_vec();
        for (tag, perm, id) in [
            (1u16, 6u16, u32::MAX),
            (2, 4, 1000),
            (4, 4, u32::MAX),
            (0x10, 4, u32::MAX),
            (0x20, 0, u32::MAX),
        ] {
            expected.extend(tag.to_le_bytes());
            expected.extend(perm.to_le_bytes());
            expected.extend(id.to_le_bytes());
        }
        let b = block(&[(ACL_ACCESS, b"", &stored), (ACL_DEFAULT, b"", &stored)]);
        let xattrs = in_block(&b, 12).unwrap();
        assert_eq!(
            xattrs[0],
            xattr(XattrNs::System, b"posix_acl_access", &expected)
        );
        assert_eq!(xattrs[1].name, b"posix_acl_default");
        // Bad versions, tags, and lengths; and names on ACLs.
        let bad = Err(Ext4Error::BadXattr { inode: 12 });
        let mut wrong = stored.clone();
        wrong[0] = 2;
        assert_eq!(in_block(&block(&[(ACL_ACCESS, b"", &wrong)]), 12), bad);
        let mut wrong = stored.clone();
        wrong[4] = 3;
        assert_eq!(in_block(&block(&[(ACL_ACCESS, b"", &wrong)]), 12), bad);
        assert_eq!(
            in_block(&block(&[(ACL_ACCESS, b"", &stored[..10])]), 12),
            bad
        );
        assert_eq!(in_block(&block(&[(ACL_ACCESS, b"x", &stored)]), 12), bad);
    }

    #[test]
    fn parses_in_inode_attributes() {
        // Values are at offsets from the first entry, after the magic.
        let mut tail = region(96, 4, 4, &[(1, b"small", b"in the inode")]);
        tail[0..4].copy_from_slice(&MAGIC.to_le_bytes());
        // 32 bytes of extra fields, of which 28 follow `i_extra_isize`.
        let mut raw = Raw::file(0);
        let mut fields = vec![0; 28];
        fields.extend(&tail);
        raw.extra = Some((32, fields));
        let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
        assert_eq!(
            in_inode(&inode).unwrap(),
            [xattr(XattrNs::User, b"small", b"in the inode")]
        );
        // Without the magic, there are none.
        let mut fields = vec![0; 28];
        fields.extend(&tail[4..]);
        raw.extra = Some((32, fields));
        let inode = Inode::parse(&raw.bytes(256), 12).unwrap();
        assert!(in_inode(&inode).unwrap().is_empty());
        // Nor in 128-byte inodes.
        let inode = Inode::parse(&Raw::file(0).bytes(128), 12).unwrap();
        assert!(in_inode(&inode).unwrap().is_empty());
    }

    #[test]
    fn refuses_malformed_blocks_and_entries() {
        let bad = Err(Ext4Error::BadXattr { inode: 12 });
        let good = block(&[(1, b"name", b"value")]);
        let mut b = good.clone();
        b[3] = 0;
        assert_eq!(in_block(&b, 12), bad);
        // More than one block.
        let mut b = good.clone();
        b[8] = 2;
        assert_eq!(in_block(&b, 12), bad);
        // A value past the block.
        let mut b = good.clone();
        b[32 + 2..32 + 4].copy_from_slice(&1022u16.to_le_bytes());
        assert_eq!(in_block(&b, 12), bad);
        // A value in another inode is read from it later, but never an
        // ACL's, nor one past 16 MiB.
        let mut b = good.clone();
        b[32 + 4] = 1;
        assert_eq!(in_block(&b, 12).unwrap()[0].value_inode, Some((1, 5)));
        let mut acl = b.clone();
        acl[32 + 1] = ACL_ACCESS;
        assert_eq!(in_block(&acl, 12), bad);
        b[32 + 8..32 + 12].copy_from_slice(&(MAX_INODE_VALUE + 1).to_le_bytes());
        assert_eq!(in_block(&b, 12), bad);
        // A name past the block.
        assert_eq!(in_block(&good[..34 + 16], 12), bad);
    }
}
