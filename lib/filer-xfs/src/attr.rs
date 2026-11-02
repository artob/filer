// This is free and unencumbered software released into the public domain.

//! Extended attributes, in an inode's attribute fork: in short form, in
//! the fork itself; or in blocks the fork maps, one leaf alone, or nodes
//! over leaves, which hold entries in hash order, so that one attribute is
//! found by its name's hash. A value is in its entry,
//! or remote, in blocks of the fork of its own. `attr1` and `attr2`
//! volumes differ only in how an inode's fork offset is chosen, which it
//! records either way, so they read alike.
//!
//! Flags give a name its namespace: none for `user`, `root` for `trusted`,
//! and `secure` for `security`. POSIX ACLs are `trusted` attributes named
//! `SGI_ACL_FILE` and `SGI_ACL_DEFAULT`, in a form of XFS's own; Linux
//! shows each both as it is and as `system.posix_acl_access` or
//! `system.posix_acl_default`, converted, and so does this driver.
//!
//! With version 5, leaf and node blocks are self-describing (`crc::DA`),
//! and a remote value's every block starts with a header of its own
//! (`crc::REMOTE`), naming the part of the value it holds.

use crate::{
    XfsError, crc, da,
    file_map::FileMap,
    hash,
    inode::{Format, Inode},
    superblock::Superblock,
};
use alloc::{collections::BTreeSet, vec, vec::Vec};
use filer_api::{Error, ErrorKind, OpKind, ReadAt, XattrNs};
use filer_fs::{be, posix_acl, read_exact};

/// The magic number of a leaf block, and with version 5.
const LEAF_MAGIC: u16 = 0xFBEE;
const LEAF_MAGIC_V3: u16 = 0x3BEE;

/// The magic number of a version 5 remote value's block, `XARM`, and the
/// size of its header.
const REMOTE_MAGIC: u32 = 0x5841_524D;
const REMOTE_HEADER: usize = 56;

/// The sizes of a short-form fork's header and an entry's fixed fields.
const SHORT_HEADER: usize = 4;
const SHORT_ENTRY: usize = 3;

/// The sizes of a leaf block's header, with version 5, and of an entry in
/// its table.
const LEAF_HEADER: usize = 32;
const LEAF_HEADER_V3: usize = 80;
const LEAF_ENTRY: usize = 8;

/// Entry flags: the value is in the entry; the `trusted` namespace; the
/// `security` namespace; a parent pointer, which is hidden; and an entry
/// not yet complete, which Linux skips.
const LOCAL: u8 = 0x01;
const ROOT: u8 = 0x02;
const SECURE: u8 = 0x04;
const PARENT: u8 = 0x08;
const INCOMPLETE: u8 = 0x80;

/// The longest value Linux sets.
const MAX_VALUE: usize = 65_536;

/// An extended attribute: its namespace, its name within it, and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Xattr {
    pub(crate) ns: XattrNs,
    pub(crate) name: Vec<u8>,
    pub(crate) value: Vec<u8>,
}

/// Where a leaf entry's value is: in it, or in `len` bytes from fork block
/// `block`.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Local(Vec<u8>),
    Remote { block: u32, len: usize },
}

/// An entry of a leaf block, as it is on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
struct LeafEntry {
    hash: u32,
    flags: u8,
    name: Vec<u8>,
    value: Value,
}

/// Returns the attributes an entry of `flags`, `name`, and `value` shows
/// as: none if it is hidden, and an ACL both as it is and converted.
fn named(flags: u8, name: &[u8], value: Vec<u8>, bad: XfsError) -> Result<Vec<Xattr>, XfsError> {
    if flags & (INCOMPLETE | PARENT) != 0 {
        return Ok(Vec::new());
    }
    let xattr = |ns, name: &[u8], value| Xattr {
        ns,
        name: name.to_vec(),
        value,
    };
    let converted = match (flags & (ROOT | SECURE), name) {
        (ROOT, b"SGI_ACL_FILE") => Some(&b"posix_acl_access"[..]),
        (ROOT, b"SGI_ACL_DEFAULT") => Some(&b"posix_acl_default"[..]),
        _ => None,
    };
    let ns = match flags & (ROOT | SECURE) {
        0 => XattrNs::User,
        ROOT => XattrNs::Trusted,
        SECURE => XattrNs::Security,
        _ => return Err(bad),
    };
    Ok(match converted {
        Some(system) => vec![
            xattr(XattrNs::System, system, acl(&value).ok_or(bad)?),
            xattr(ns, name, value),
        ],
        None => vec![xattr(ns, name, value)],
    })
}

/// Converts a POSIX ACL from XFS's form, big-endian: a count, then for
/// each entry its tag, id, and permissions, and two bytes of padding. To
/// the one Linux's `getxattr` returns.
fn acl(value: &[u8]) -> Option<Vec<u8>> {
    const ENTRY: usize = 12;
    let count = usize::try_from(be::u32_at(value, 0)?).ok()?;
    let entries = value.get(4..)?;
    if count.checked_mul(ENTRY)? != entries.len() {
        return None;
    }
    let mut out = posix_acl::Encoder::new();
    for entry in entries.as_chunks::<ENTRY>().0 {
        let tag = u16::try_from(be::u32_at(entry, 0)?).ok()?;
        out.push(tag, be::u16_at(entry, 8)?, be::u32_at(entry, 4)?)?;
    }
    Some(out.finish())
}

/// Parses a short-form fork: its size and count, then entries of a name's
/// length, a value's length, flags, the name, and the value, exactly
/// filling the size.
fn parse_short(fork: &[u8], inode: u64) -> Result<Vec<Xattr>, XfsError> {
    let bad = XfsError::BadXattr { inode };
    let size = usize::from(be::u16_at(fork, 0).ok_or(bad)?);
    let count = fork.get(2).copied().ok_or(bad)?;
    let region = fork.get(..size).ok_or(bad)?;
    let mut found = Vec::new();
    let mut at = SHORT_HEADER;
    for _ in 0..count {
        let header = region
            .get(at..at.checked_add(SHORT_ENTRY).ok_or(bad)?)
            .ok_or(bad)?;
        let [name_len, value_len, flags] = *header else {
            return Err(bad);
        };
        if name_len == 0 {
            return Err(bad);
        }
        let name_at = at.saturating_add(SHORT_ENTRY);
        let value_at = name_at.saturating_add(usize::from(name_len));
        let end = value_at.saturating_add(usize::from(value_len));
        let name = region.get(name_at..value_at).ok_or(bad)?;
        let value = region.get(value_at..end).ok_or(bad)?;
        found.extend(named(flags, name, value.to_vec(), bad)?);
        at = end;
    }
    if at != size {
        return Err(bad);
    }
    Ok(found)
}

/// Returns the leaf magic number of a volume, version 5 if `v5`.
const fn leaf_magic(v5: bool) -> u16 {
    if v5 { LEAF_MAGIC_V3 } else { LEAF_MAGIC }
}

/// Parses a leaf block, of version 5 if `v5`: a header holding the count
/// after the self-description, then the table of entries, each a hash,
/// the offset of its name and value, and flags. A local entry is a value's
/// length, a name's length, the name, and the value; a remote one the
/// value's first block and length, then a name's length and the name.
fn parse_leaf(block: &[u8], v5: bool, inode: u64) -> Result<Vec<LeafEntry>, XfsError> {
    let bad = XfsError::BadXattr { inode };
    if be::u16_at(block, 8) != Some(leaf_magic(v5)) {
        return Err(bad);
    }
    let (header, count_at) = if v5 {
        (LEAF_HEADER_V3, 56)
    } else {
        (LEAF_HEADER, 12)
    };
    let count = usize::from(be::u16_at(block, count_at).ok_or(bad)?);
    let table_end = count
        .checked_mul(LEAF_ENTRY)
        .and_then(|len| len.checked_add(header))
        .filter(|&end| end <= block.len())
        .ok_or(bad)?;
    let mut found = Vec::with_capacity(count);
    for i in 0..count {
        let entry = header.saturating_add(i.saturating_mul(LEAF_ENTRY));
        let hash = be::u32_at(block, entry).ok_or(bad)?;
        let at = usize::from(be::u16_at(block, entry.saturating_add(4)).ok_or(bad)?);
        let flags = block.get(entry.saturating_add(6)).copied().ok_or(bad)?;
        if at < table_end {
            return Err(bad);
        }
        let byte = |off: usize| block.get(at.saturating_add(off)).copied().ok_or(bad);
        let (name_at, name_len) = if flags & LOCAL != 0 {
            (at.saturating_add(3), byte(2)?)
        } else {
            (at.saturating_add(9), byte(8)?)
        };
        if name_len == 0 {
            return Err(bad);
        }
        let name_end = name_at.saturating_add(usize::from(name_len));
        let name = block.get(name_at..name_end).ok_or(bad)?.to_vec();
        let value = if flags & LOCAL != 0 {
            let len = usize::from(be::u16_at(block, at).ok_or(bad)?);
            let value = block
                .get(name_end..name_end.saturating_add(len))
                .ok_or(bad)?;
            Value::Local(value.to_vec())
        } else {
            let first = be::u32_at(block, at).ok_or(bad)?;
            let len = be::u32_at(block, at.saturating_add(4)).ok_or(bad)?;
            let len = usize::try_from(len).map_err(|_| bad)?;
            if len > MAX_VALUE {
                return Err(bad);
            }
            Value::Remote { block: first, len }
        };
        found.push(LeafEntry {
            hash,
            flags,
            name,
            value,
        });
    }
    Ok(found)
}

/// The blocks of an attribute fork.
struct Blocks<'a, S: ?Sized> {
    source: &'a S,
    sb: &'a Superblock,
    map: FileMap,
    inode: u64,
    op: OpKind,
}

impl<S: ReadAt + ?Sized> Blocks<'_, S> {
    fn bad(&self) -> Error {
        XfsError::BadXattr { inode: self.inode }.into_error(self.op)
    }

    /// Reads the leaf or node at fork block `logical`, which must be
    /// mapped, checking a version 5 one's header.
    async fn block(&self, logical: u32) -> Result<Vec<u8>, Error> {
        let len = usize::try_from(self.sb.block_size()).map_err(|_| self.bad())?;
        let (bytes, offset) = self.read(logical, len).await?;
        if self.sb.is_v5() {
            crc::check(&bytes, crc::DA, self.sb, offset, self.inode)
                .map_err(|error| error.into_error(self.op))?;
        }
        Ok(bytes)
    }

    /// Reads a remote value of `len` bytes from fork block `first`: with
    /// version 5, a block at a time, each holding the part its header
    /// names after the header.
    async fn value(&self, first: u32, len: usize) -> Result<Vec<u8>, Error> {
        if !self.sb.is_v5() {
            return Ok(self.read(first, len).await?.0);
        }
        let size = usize::try_from(self.sb.block_size()).map_err(|_| self.bad())?;
        let room = size.checked_sub(REMOTE_HEADER).ok_or_else(|| self.bad())?;
        let mut value = Vec::with_capacity(len);
        let mut logical = first;
        while value.len() < len {
            let (bytes, offset) = self.read(logical, size).await?;
            crc::check(&bytes, crc::REMOTE, self.sb, offset, self.inode)
                .map_err(|error| error.into_error(self.op))?;
            let part = len.saturating_sub(value.len()).min(room);
            let header = (
                be::u32_at(&bytes, 0),
                be::u32_at(&bytes, 4).and_then(|at| usize::try_from(at).ok()),
                be::u32_at(&bytes, 8).and_then(|len| usize::try_from(len).ok()),
            );
            if header != (Some(REMOTE_MAGIC), Some(value.len()), Some(part)) {
                return Err(self.bad());
            }
            let data = REMOTE_HEADER..REMOTE_HEADER.saturating_add(part);
            value.extend_from_slice(bytes.get(data).ok_or_else(|| self.bad())?);
            logical = logical.checked_add(1).ok_or_else(|| self.bad())?;
        }
        Ok(value)
    }

    /// Reads `len` bytes from fork block `first`, all of which must be
    /// mapped, and returns them and the byte offset of the first block.
    async fn read(&self, first: u32, len: usize) -> Result<(Vec<u8>, u64), Error> {
        let block = u64::from(self.sb.block_size());
        let mut out = Vec::with_capacity(len);
        let mut logical = u64::from(first);
        let mut start = None;
        while out.len() < len {
            let want = len.saturating_sub(out.len());
            let blocks = u64::try_from(want).map_err(|_| self.bad())?.div_ceil(block);
            let run = self
                .map
                .map(self.source, self.sb, logical, blocks, self.op)
                .await?;
            let physical = run.physical.ok_or_else(|| self.bad())?;
            let at = self.sb.block_offset(physical).ok_or_else(|| self.bad())?;
            start = start.or(Some(at));
            let n = run
                .len
                .checked_mul(block)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| self.bad())?
                .min(want);
            out.extend(read_exact(self.source, at, n, self.op).await?);
            logical = logical.checked_add(run.len).ok_or_else(|| self.bad())?;
        }
        Ok((out, start.unwrap_or(0)))
    }

    /// Reads every attribute, from the leaf or node at fork block 0 down.
    async fn all(&self) -> Result<Vec<Xattr>, Error> {
        let fail = |error: XfsError| error.into_error(self.op);
        let v5 = self.sb.is_v5();
        let mut found = Vec::new();
        // Blocks to read, in order, each with the level it must have, if
        // known; every block once.
        let mut stack = vec![(0, None)];
        let mut seen = BTreeSet::new();
        while let Some((logical, level)) = stack.pop() {
            if !seen.insert(logical) {
                return Err(self.bad());
            }
            let block = self.block(logical).await?;
            if da::magic(&block) == Some(da::node_magic(v5)) {
                let node = da::Node::parse(&block, v5).ok_or_else(|| self.bad())?;
                if level.is_some_and(|level| level != node.level) {
                    return Err(self.bad());
                }
                let below = node.level.saturating_sub(1);
                let children = node.entries.into_iter().rev();
                stack.extend(children.map(|(_, child)| (child, Some(below))));
                continue;
            }
            if level.is_some_and(|level| level != 0) {
                return Err(self.bad());
            }
            for entry in parse_leaf(&block, v5, self.inode).map_err(fail)? {
                let value = match entry.value {
                    Value::Local(value) => value,
                    Value::Remote { block, len } => self.value(block, len).await?,
                };
                let bad = XfsError::BadXattr { inode: self.inode };
                found.extend(named(entry.flags, &entry.name, value, bad).map_err(fail)?);
            }
        }
        Ok(found)
    }

    /// Finds the value of the attribute `name` of namespace `flags` by its
    /// hash, failing if the index is inconsistent.
    async fn find(&self, flags: u8, name: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let fail = |error: XfsError| error.into_error(self.op);
        let v5 = self.sb.is_v5();
        let hash = hash::hash(name);
        // Down the nodes, if any, to the leaf where entries of the hash
        // start; each block's level one below its parent's.
        let mut at = 0;
        let mut level = None;
        let mut block = loop {
            let block = self.block(at).await?;
            match da::magic(&block) {
                Some(magic) if magic == da::node_magic(v5) => {
                    let node = da::Node::parse(&block, v5).ok_or_else(|| self.bad())?;
                    if level.is_some_and(|level| level != node.level) {
                        return Err(self.bad());
                    }
                    let Some(child) = node.child(hash) else {
                        return Ok(None);
                    };
                    level = Some(node.level.saturating_sub(1));
                    at = child;
                },
                Some(magic) if magic == leaf_magic(v5) && level.is_none_or(|level| level == 0) => {
                    break block;
                },
                _ => return Err(self.bad()),
            }
        };
        // Along the leaves, while entries of the hash may run on.
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(at) {
                return Err(self.bad());
            }
            let entries = parse_leaf(&block, v5, self.inode).map_err(fail)?;
            if entries.is_sorted_by_key(|entry| entry.hash) {
                let start = entries.partition_point(|entry| entry.hash < hash);
                for entry in entries.into_iter().skip(start) {
                    if entry.hash != hash {
                        return Ok(None);
                    }
                    let hidden = entry.flags & (INCOMPLETE | PARENT) != 0;
                    if hidden || entry.flags & (ROOT | SECURE) != flags || entry.name != name {
                        continue;
                    }
                    return Ok(Some(match entry.value {
                        Value::Local(value) => value,
                        Value::Remote { block, len } => self.value(block, len).await?,
                    }));
                }
            } else {
                return Err(self.bad());
            }
            match da::forw(&block).ok_or_else(|| self.bad())? {
                0 => return Ok(None),
                next => at = next,
            }
            block = self.block(at).await?;
        }
    }
}

/// Returns where the attribute `name` of namespace `ns` is on disk: its
/// namespace's flags, its name there, and whether it is an ACL Linux
/// shows converted; or `None` if it cannot be on an XFS volume.
fn on_disk(ns: XattrNs, name: &[u8]) -> Option<(u8, &[u8], bool)> {
    Some(match (ns, name) {
        (XattrNs::User, name) => (0, name, false),
        (XattrNs::Trusted, name) => (ROOT, name, false),
        (XattrNs::Security, name) => (SECURE, name, false),
        (XattrNs::System, b"posix_acl_access") => (ROOT, b"SGI_ACL_FILE", true),
        (XattrNs::System, b"posix_acl_default") => (ROOT, b"SGI_ACL_DEFAULT", true),
        _ => return None,
    })
}

/// Reads the value of `inode`'s attribute `name` of namespace `ns`, if it
/// has it: in a fork of blocks, by its hash, unless the index is
/// inconsistent.
pub(crate) async fn find<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    inode: &Inode,
    ns: XattrNs,
    name: &[u8],
    op: OpKind,
) -> Result<Option<Vec<u8>>, Error> {
    let number = inode.number();
    let fail = |error: XfsError| error.into_error(op);
    let Some((flags, on_disk, convert)) = on_disk(ns, name) else {
        return Ok(None);
    };
    let blocks = inode
        .attr()
        .filter(|fork| matches!(fork.format, Format::Extents | Format::Btree) && fork.extents > 0);
    if let Some(fork) = blocks {
        let blocks = Blocks {
            source,
            sb,
            map: FileMap::new(number, fork, sb).map_err(fail)?,
            inode: number,
            op,
        };
        match blocks.find(flags, on_disk).await {
            Err(error) if error.kind() == ErrorKind::Corrupt => {},
            Ok(Some(value)) if convert => {
                let bad = XfsError::BadXattr { inode: number };
                return acl(&value).ok_or(bad).map(Some).map_err(fail);
            },
            found => return found,
        }
    }
    // Short form, or an inconsistent index: all of them.
    Ok(read(source, sb, inode, op)
        .await?
        .into_iter()
        .find(|xattr| xattr.ns == ns && xattr.name == name)
        .map(|xattr| xattr.value))
}

/// Reads all of `inode`'s attributes.
pub(crate) async fn read<S: ReadAt + ?Sized>(
    source: &S,
    sb: &Superblock,
    inode: &Inode,
    op: OpKind,
) -> Result<Vec<Xattr>, Error> {
    let number = inode.number();
    let fail = |error: XfsError| error.into_error(op);
    let Some(fork) = inode.attr() else {
        return Ok(Vec::new());
    };
    match fork.format {
        Format::Local => parse_short(&fork.bytes, number).map_err(fail),
        Format::Extents if fork.extents == 0 => Ok(Vec::new()),
        Format::Extents | Format::Btree => {
            let blocks = Blocks {
                source,
                sb,
                map: FileMap::new(number, fork, sb).map_err(fail)?,
                inode: number,
                op,
            };
            blocks.all().await
        },
        Format::Device => Err(fail(XfsError::BadXattr { inode: number })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        file_map::tests::record,
        inode::tests::Raw,
        superblock::{Version, tests::Fields},
    };
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// A short-form fork of entries of (flags, name, value).
    fn short(entries: &[(u8, &[u8], &[u8])]) -> Vec<u8> {
        let mut b = vec![0, 0, u8::try_from(entries.len()).unwrap(), 0];
        for (flags, name, value) in entries {
            b.push(u8::try_from(name.len()).unwrap());
            b.push(u8::try_from(value.len()).unwrap());
            b.push(*flags);
            b.extend_from_slice(name);
            b.extend_from_slice(value);
        }
        let size = u16::try_from(b.len()).unwrap();
        b[..2].copy_from_slice(&size.to_be_bytes());
        b
    }

    /// A 4 KiB leaf block of entries of (flags, name, value), the value
    /// local if `LOCAL` is set, else naming a block and a length; each
    /// entry's hash is its name's.
    fn leaf(entries: &[(u8, &[u8], Value)]) -> Vec<u8> {
        let mut b = vec![0; 4096];
        b[8..10].copy_from_slice(&LEAF_MAGIC.to_be_bytes());
        b[12..14].copy_from_slice(&u16::try_from(entries.len()).unwrap().to_be_bytes());
        let mut end = 4096;
        for (i, (flags, name, value)) in entries.iter().enumerate() {
            let mut bytes = Vec::new();
            match value {
                Value::Local(value) => {
                    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_be_bytes());
                    bytes.push(u8::try_from(name.len()).unwrap());
                    bytes.extend_from_slice(name);
                    bytes.extend_from_slice(value);
                },
                Value::Remote { block, len } => {
                    bytes.extend_from_slice(&block.to_be_bytes());
                    bytes.extend_from_slice(&u32::try_from(*len).unwrap().to_be_bytes());
                    bytes.push(u8::try_from(name.len()).unwrap());
                    bytes.extend_from_slice(name);
                },
            }
            end = (end - bytes.len()) & !3;
            b[end..end + bytes.len()].copy_from_slice(&bytes);
            let at = LEAF_HEADER + LEAF_ENTRY * i;
            b[at..at + 4].copy_from_slice(&hash::hash(name).to_be_bytes());
            b[at + 4..at + 6].copy_from_slice(&u16::try_from(end).unwrap().to_be_bytes());
            b[at + 6] = *flags;
        }
        b
    }

    /// A 4 KiB node block of `level` over fork blocks `children`, their
    /// hashes ascending.
    fn node(level: u16, children: &[u32]) -> Vec<u8> {
        let entries: Vec<(u32, u32)> = (0..)
            .zip(children)
            .map(|(hash, &child)| (hash, child))
            .collect();
        crate::da::tests::node(4096, level, &entries)
    }

    /// The ACL `tests/fixtures/make.sh` sets, in XFS's form.
    fn xfs_acl() -> Vec<u8> {
        let mut b = 5u32.to_be_bytes().to_vec();
        for (tag, id, perm) in [
            (1u32, u32::MAX, 6u16),
            (2, 1000, 4),
            (4, u32::MAX, 4),
            (0x10, u32::MAX, 4),
            (0x20, u32::MAX, 0),
        ] {
            b.extend(tag.to_be_bytes());
            b.extend(id.to_be_bytes());
            b.extend(perm.to_be_bytes());
            b.extend([0, 0]);
        }
        b
    }

    /// The same ACL, as Linux's `getxattr` returns it.
    fn linux_acl() -> Vec<u8> {
        let mut b = 2u32.to_le_bytes().to_vec();
        for (tag, perm, id) in [
            (1u16, 6u16, u32::MAX),
            (2, 4, 1000),
            (4, 4, u32::MAX),
            (0x10, 4, u32::MAX),
            (0x20, 0, u32::MAX),
        ] {
            b.extend(tag.to_le_bytes());
            b.extend(perm.to_le_bytes());
            b.extend(id.to_le_bytes());
        }
        b
    }

    fn xattr(ns: XattrNs, name: &[u8], value: &[u8]) -> Xattr {
        Xattr {
            ns,
            name: name.to_vec(),
            value: value.to_vec(),
        }
    }

    #[test]
    fn parses_short_form() {
        let acl = xfs_acl();
        let fork = short(&[
            (0, b"small", b"x"),
            (0, b"empty", b""),
            (ROOT, b"t", b"T"),
            (SECURE, b"s", b"S"),
            (ROOT, b"SGI_ACL_FILE", &acl),
            (ROOT, b"SGI_ACL_DEFAULT", &acl),
            (PARENT, b"hidden", b"x"),
        ]);
        assert_eq!(
            parse_short(&fork, 131).unwrap(),
            [
                xattr(XattrNs::User, b"small", b"x"),
                xattr(XattrNs::User, b"empty", b""),
                xattr(XattrNs::Trusted, b"t", b"T"),
                xattr(XattrNs::Security, b"s", b"S"),
                xattr(XattrNs::System, b"posix_acl_access", &linux_acl()),
                xattr(XattrNs::Trusted, b"SGI_ACL_FILE", &acl),
                xattr(XattrNs::System, b"posix_acl_default", &linux_acl()),
                xattr(XattrNs::Trusted, b"SGI_ACL_DEFAULT", &acl),
            ]
        );
        // As `xfs_db` showed one: three entries in 23 bytes.
        let fork = [
            0x00, 0x17, 0x03, 0x00, 0x05, 0x01, 0x00, b's', b'm', b'a', b'l', b'l', b'x', 0x01,
            0x01, 0x02, b't', b'T', 0x01, 0x01, 0x04, b's', b'S', 0, 0, 0,
        ];
        assert_eq!(parse_short(&fork, 131).unwrap().len(), 3);
        assert_eq!(parse_short(&short(&[]), 131).unwrap(), []);
    }

    #[test]
    fn rejects_bad_short_forms() {
        let bad = Err(XfsError::BadXattr { inode: 131 });
        let good = short(&[(0, b"a", b"1"), (0, b"b", b"2")]);
        // Too few bytes for the size, a size past the entries, or more
        // entries than fit.
        assert_eq!(parse_short(&good[..good.len() - 1], 131), bad);
        let mut b = good.clone();
        b.push(0);
        b[1] += 1;
        assert_eq!(parse_short(&b, 131), bad);
        let mut b = good.clone();
        b[2] = 3;
        assert_eq!(parse_short(&b, 131), bad);
        // An empty name, both namespaces, and a malformed ACL.
        assert_eq!(parse_short(&short(&[(0, b"", b"1")]), 131), bad);
        assert_eq!(
            parse_short(&short(&[(ROOT | SECURE, b"a", b"1")]), 131),
            bad
        );
        let acl = &xfs_acl()[..20];
        assert_eq!(
            parse_short(&short(&[(ROOT, b"SGI_ACL_FILE", acl)]), 131),
            bad
        );
        assert!(parse_short(&[], 131).is_err());
    }

    #[test]
    fn converts_acls() {
        assert_eq!(acl(&xfs_acl()), Some(linux_acl()));
        assert_eq!(acl(&0u32.to_be_bytes()), Some(2u32.to_le_bytes().to_vec()));
        // A count the entries do not match, or an unknown tag.
        let mut bad = xfs_acl();
        bad[3] = 6;
        assert_eq!(acl(&bad), None);
        let mut bad = xfs_acl();
        bad[7] = 0x40;
        assert_eq!(acl(&bad), None);
        assert_eq!(acl(&[]), None);
    }

    #[test]
    fn parses_leaf_blocks() {
        let block = leaf(&[
            (LOCAL, b"a", Value::Local(b"one".to_vec())),
            (LOCAL | SECURE, b"b", Value::Local(Vec::new())),
            (
                0,
                b"big",
                Value::Remote {
                    block: 1,
                    len: 5000,
                },
            ),
            (LOCAL | INCOMPLETE, b"c", Value::Local(b"x".to_vec())),
        ]);
        let entries = parse_leaf(&block, false, 131).unwrap();
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].name, b"a");
        assert_eq!(entries[0].value, Value::Local(b"one".to_vec()));
        assert_eq!(entries[1].flags, LOCAL | SECURE);
        assert_eq!(
            entries[2].value,
            Value::Remote {
                block: 1,
                len: 5000
            }
        );

        let bad = Err(XfsError::BadXattr { inode: 131 });
        let mut b = block.clone();
        b[8] = 0;
        assert_eq!(parse_leaf(&b, false, 131), bad);
        // More entries than fit, a name inside the table or past the block,
        // and a value longer than Linux sets.
        let mut b = block.clone();
        b[12..14].copy_from_slice(&600u16.to_be_bytes());
        assert_eq!(parse_leaf(&b, false, 131), bad);
        let mut b = block.clone();
        b[36..38].copy_from_slice(&40u16.to_be_bytes());
        assert_eq!(parse_leaf(&b, false, 131), bad);
        let mut b = block.clone();
        b[36..38].copy_from_slice(&4095u16.to_be_bytes());
        assert_eq!(parse_leaf(&b, false, 131), bad);
        let big = leaf(&[(
            0,
            b"big",
            Value::Remote {
                block: 1,
                len: 65_537,
            },
        )]);
        assert_eq!(parse_leaf(&big, false, 131), bad);
    }

    /// An image of `Fields::new()`'s volume, 4 KiB blocks.
    fn image() -> Vec<u8> {
        vec![0; 4096 * 1024]
    }

    fn put(image: &mut [u8], block: usize, bytes: &[u8]) {
        image[block * 4096..block * 4096 + bytes.len()].copy_from_slice(bytes);
    }

    /// A file whose attribute fork maps fork blocks to volume blocks by
    /// `records`.
    fn mapped(records: &[[u8; 16]]) -> Inode {
        let mut raw = Raw::file(0);
        raw.fork_off = 15;
        raw.attr_format = 2;
        raw.attr_extents = u16::try_from(records.len()).unwrap();
        raw.literal = vec![0; 120];
        raw.literal.extend(records.concat());
        Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap()
    }

    fn read_all(image: &[u8], inode: &Inode) -> Result<Vec<Xattr>, Error> {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        block_on(read(image, &sb, inode, OpKind::ListXattrs))
    }

    #[test]
    fn reads_short_forms_from_inodes() {
        let mut raw = Raw::file(0);
        raw.fork_off = 15;
        raw.attr_format = 1;
        raw.literal = vec![0; 120];
        raw.literal.extend(short(&[(0, b"a", b"1")]));
        let inode = Inode::parse(&raw.bytes(256), 131, Version::V4).unwrap();
        assert_eq!(
            read_all(&[], &inode).unwrap(),
            [xattr(XattrNs::User, b"a", b"1")]
        );
        // No fork, or an empty list of extents: no attributes.
        let inode = Inode::parse(&Raw::file(0).bytes(256), 131, Version::V4).unwrap();
        assert_eq!(read_all(&[], &inode).unwrap(), []);
        assert_eq!(read_all(&[], &mapped(&[])).unwrap(), []);
    }

    #[test]
    fn reads_leaves_nodes_and_remote_values() {
        let remote: Vec<u8> = (0..5000).map(|i| u8::try_from(i % 251).unwrap()).collect();
        let mut image = image();
        // Fork blocks 0-3 at volume blocks 10-13, and 4-5 at 20-21.
        put(&mut image, 10, &node(1, &[2, 1]));
        put(
            &mut image,
            11,
            &leaf(&[
                (LOCAL, b"second", Value::Local(b"2".to_vec())),
                (
                    0,
                    b"remote",
                    Value::Remote {
                        block: 4,
                        len: 5000,
                    },
                ),
            ]),
        );
        put(
            &mut image,
            12,
            &leaf(&[(LOCAL | ROOT, b"first", Value::Local(b"1".to_vec()))]),
        );
        put(&mut image, 20, &remote);
        let inode = mapped(&[record(0, 10, 4, false), record(4, 20, 2, false)]);
        assert_eq!(
            read_all(&image, &inode).unwrap(),
            [
                xattr(XattrNs::Trusted, b"first", b"1"),
                xattr(XattrNs::User, b"second", b"2"),
                xattr(XattrNs::User, b"remote", &remote),
            ]
        );

        let corrupt = |image: &[u8], inode: &Inode| {
            assert_eq!(
                read_all(image, inode).unwrap_err().kind(),
                ErrorKind::Corrupt
            );
        };
        // A remote value in a hole.
        corrupt(&image, &mapped(&[record(0, 10, 4, false)]));
        // A block that is neither a leaf nor a node.
        let mut bad = image.clone();
        put(&mut bad, 12, &[0; 16]);
        corrupt(&bad, &inode);
        // A node over a block twice, or over a node of its own level.
        let mut bad = image.clone();
        put(&mut bad, 10, &node(1, &[1, 1]));
        corrupt(&bad, &inode);
        let mut bad = image.clone();
        put(&mut bad, 10, &node(1, &[3]));
        put(&mut bad, 13, &node(1, &[1]));
        corrupt(&bad, &inode);
        // A node over itself.
        let mut bad = image.clone();
        put(&mut bad, 10, &node(1, &[0]));
        corrupt(&bad, &inode);
        // A node of level 0.
        let mut bad = image;
        put(&mut bad, 10, &node(0, &[1]));
        corrupt(&bad, &inode);
    }

    #[test]
    fn finds_attributes_by_hash() {
        let sb = Superblock::parse(&Fields::new().bytes()).unwrap();
        let remote: Vec<u8> = (0..5000).map(|i| u8::try_from(i % 251).unwrap()).collect();
        let mut image = image();
        // A node over two leaves, whose entries are in hash order: `big`
        // (0x18B4E7) and `zz` (0x3D7A) in the second; and in the first,
        // `a` (0x61), and `b`, but with `zz`'s hash, so that entries of it
        // run on into the second leaf, which `forw` names.
        let big = hash::hash(b"big");
        let zz = hash::hash(b"zz");
        assert!(zz < big);
        put(
            &mut image,
            10,
            &crate::da::tests::node(4096, 1, &[(zz, 1), (big, 2)]),
        );
        let mut first = leaf(&[
            (LOCAL, b"a", Value::Local(b"1".to_vec())),
            (LOCAL | ROOT, b"b", Value::Local(b"2".to_vec())),
        ]);
        first[LEAF_HEADER + LEAF_ENTRY..LEAF_HEADER + LEAF_ENTRY + 4]
            .copy_from_slice(&zz.to_be_bytes());
        first[..4].copy_from_slice(&2u32.to_be_bytes());
        put(&mut image, 11, &first);
        put(
            &mut image,
            12,
            &leaf(&[
                (LOCAL, b"zz", Value::Local(b"3".to_vec())),
                (
                    0,
                    b"big",
                    Value::Remote {
                        block: 3,
                        len: 5000,
                    },
                ),
            ]),
        );
        put(&mut image, 13, &remote);
        let inode = mapped(&[record(0, 10, 5, false)]);
        let find = |image: &[u8], ns, name: &[u8]| {
            block_on(super::find(image, &sb, &inode, ns, name, OpKind::GetXattr)).unwrap()
        };
        assert_eq!(find(&image, XattrNs::User, b"a"), Some(b"1".to_vec()));
        assert_eq!(find(&image, XattrNs::User, b"zz"), Some(b"3".to_vec()));
        assert_eq!(find(&image, XattrNs::User, b"big"), Some(remote.clone()));
        // `b` is not where its hash says: not found, as Linux would not.
        assert_eq!(find(&image, XattrNs::Trusted, b"b"), None);
        // Namespaces must match.
        assert_eq!(find(&image, XattrNs::Trusted, b"a"), None);
        assert_eq!(find(&image, XattrNs::System, b"other"), None);
        assert_eq!(find(&image, XattrNs::User, b"nope"), None);

        // An inconsistent index, a leaf out of hash order, falls back to
        // reading them all.
        let mut bad = image.clone();
        put(
            &mut bad,
            12,
            &leaf(&[
                (
                    0,
                    b"big",
                    Value::Remote {
                        block: 3,
                        len: 5000,
                    },
                ),
                (LOCAL, b"zz", Value::Local(b"3".to_vec())),
            ]),
        );
        assert_eq!(find(&bad, XattrNs::User, b"big"), Some(remote));
        assert_eq!(find(&bad, XattrNs::User, b"zz"), Some(b"3".to_vec()));
    }
}
