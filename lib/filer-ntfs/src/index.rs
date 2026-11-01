// This is free and unencumbered software released into the public domain.

//! Directory indexes (`$I30`): B+ trees of `$FILE_NAME` keys, whose root
//! is the resident `$INDEX_ROOT` and whose other nodes are `INDX` records
//! in `$INDEX_ALLOCATION`.

// The driver (NTFS-05) uses the rest of this module.
#![allow(dead_code)]

use crate::{
    NtfsError,
    file_name::FileName,
    fixup,
    geometry::Geometry,
    record::{AttrType, FileRef},
    stream::Stream,
};
use alloc::{collections::BTreeSet, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(*bytes.get(at..)?.first_chunk()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(*bytes.get(at..)?.first_chunk()?))
}

fn u64_at(bytes: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(*bytes.get(at..)?.first_chunk()?))
}

/// An index entry: a name of a file, or the node's last entry, which has
/// none; either may lead to a subnode of lesser keys.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    file: FileRef,
    name: Option<FileName>,
    subnode: Option<u64>,
}

/// Parses the entries of a node, from its index header in `bytes`;
/// `None` if any does not fit or the node lacks its last entry.
fn entries(bytes: &[u8]) -> Option<Vec<Entry>> {
    const SUBNODE: u16 = 0x01;
    const LAST: u16 = 0x02;
    let first = usize::try_from(u32_at(bytes, 0)?).ok()?;
    let used = usize::try_from(u32_at(bytes, 4)?).ok()?;
    let node = bytes.get(..used)?;
    let mut entries = Vec::new();
    let mut at = first;
    loop {
        let entry = node.get(at..)?;
        let len = usize::from(u16_at(entry, 8)?);
        let key_len = usize::from(u16_at(entry, 0x0A)?);
        let flags = u16_at(entry, 0x0C)?;
        let entry = entry.get(..len)?;
        if len < 0x10 || !len.is_multiple_of(8) {
            return None;
        }
        let subnode = match flags & SUBNODE {
            0 => None,
            _ => Some(u64_at(entry, len.checked_sub(8)?)?),
        };
        let last = flags & LAST != 0;
        let name = match last {
            true => None,
            false => Some(FileName::parse(entry.get(0x10..)?.get(..key_len)?)?),
        };
        entries.push(Entry {
            file: FileRef::from_u64(u64_at(entry, 0)?),
            name,
            subnode,
        });
        if last {
            return Some(entries);
        }
        at = at.checked_add(len)?;
    }
}

/// A directory's index: its root, and its other nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Index {
    /// The number of the directory's record, for errors.
    record: u64,
    root: Vec<Entry>,
    allocation: Option<Stream>,
    record_size: u32,
    /// The bytes a VCN of the index counts: a cluster, or 512 bytes if
    /// index records are smaller than clusters.
    unit: u64,
}

impl Index {
    /// Builds the index of directory `record` from its `$INDEX_ROOT`
    /// value and its `$INDEX_ALLOCATION`, if it has one.
    pub(crate) fn new(
        root: &[u8],
        allocation: Option<Stream>,
        geometry: &Geometry,
        record: u64,
    ) -> Result<Self, NtfsError> {
        let bad = NtfsError::BadIndex { record };
        if u32_at(root, 0) != Some(AttrType::FILE_NAME.0) {
            return Err(bad);
        }
        let record_size = u32_at(root, 8).ok_or(bad)?;
        if !record_size.is_power_of_two() || !(512..=65536).contains(&record_size) {
            return Err(bad);
        }
        let entries = entries(root.get(0x10..).ok_or(bad)?).ok_or(bad)?;
        let cluster_size = geometry.cluster_size();
        Ok(Self {
            record,
            root: entries,
            allocation,
            record_size,
            unit: u64::from(if record_size >= cluster_size {
                cluster_size
            } else {
                512
            }),
        })
    }

    /// Returns a walk over the index's names, in order.
    pub(crate) fn walk(&self) -> Walk {
        Walk {
            stack: alloc::vec![Frame {
                entries: self.root.clone(),
                at: 0,
                descended: false,
            }],
            visited: BTreeSet::new(),
        }
    }

    /// Reads and parses the node at `vcn`.
    async fn node<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        geometry: &Geometry,
        vcn: u64,
        op: OpKind,
    ) -> Result<Vec<Entry>, Error> {
        let record = self.record;
        let bad = || NtfsError::BadIndexRecord { record, vcn }.into_error(op);
        let allocation = self.allocation.as_ref().ok_or_else(bad)?;
        let off = vcn.checked_mul(self.unit).ok_or_else(bad)?;
        let len = usize::try_from(self.record_size).map_err(|_| bad())?;
        let mut bytes = allocation
            .read_exact(source, geometry, off, len, op)
            .await?;
        if bytes.get(..4) != Some(b"INDX".as_slice()) {
            return Err(bad());
        }
        let usa_offset = u16_at(&bytes, 4).ok_or_else(bad)?;
        let usa_count = u16_at(&bytes, 6).ok_or_else(bad)?;
        if !fixup::apply(&mut bytes, usa_offset, usa_count) || u64_at(&bytes, 0x10) != Some(vcn) {
            return Err(bad());
        }
        bytes.get(0x18..).and_then(entries).ok_or_else(bad)
    }
}

/// A node being walked: its entries, the next one, and whether that
/// one's subnode was walked.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Frame {
    entries: Vec<Entry>,
    at: usize,
    descended: bool,
}

/// An in-order walk of an [`Index`]. Each node is read when reached, and
/// at most once: a node reached again means the index loops, an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Walk {
    stack: Vec<Frame>,
    visited: BTreeSet<u64>,
}

impl Walk {
    /// Returns the next name in the index, with the file it names, or
    /// `None` at the end.
    pub(crate) async fn next<S: ReadAt + ?Sized>(
        &mut self,
        index: &Index,
        source: &S,
        geometry: &Geometry,
        op: OpKind,
    ) -> Result<Option<(FileRef, FileName)>, Error> {
        loop {
            let Some(frame) = self.stack.last_mut() else {
                return Ok(None);
            };
            let Some(entry) = frame.entries.get(frame.at) else {
                // Parsing ends every node with its last entry, so this is
                // only reached past one.
                self.stack.pop();
                continue;
            };
            if let (Some(vcn), false) = (entry.subnode, frame.descended) {
                frame.descended = true;
                if !self.visited.insert(vcn) {
                    let record = index.record;
                    return Err(NtfsError::IndexLoop { record, vcn }.into_error(op));
                }
                let entries = index.node(source, geometry, vcn, op).await?;
                self.stack.push(Frame {
                    entries,
                    at: 0,
                    descended: false,
                });
                continue;
            }
            match &entry.name {
                None => {
                    self.stack.pop();
                },
                Some(name) => {
                    let found = (entry.file, name.clone());
                    frame.at = frame.at.saturating_add(1);
                    frame.descended = false;
                    return Ok(Some(found));
                },
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        file_name::tests::file_name, fixup::tests::protect, mft::tests::Volume, runs::Run,
    };
    use alloc::{string::String, vec, vec::Vec};
    use core::num::NonZeroU64;
    use filer_spec::block_on;

    /// An entry to encode: a name of record `file`, or the last entry
    /// (`None`), with an optional subnode.
    pub(crate) type EntrySpec<'a> = (Option<(u64, &'a str)>, Option<u64>);

    /// Encodes a node: its index header, then its entries.
    pub(crate) fn node(entries: &[EntrySpec<'_>], header_to_entries: u32) -> Vec<u8> {
        let mut body = Vec::new();
        for &(name, subnode) in entries {
            let key = name.map_or_else(Vec::new, |(_, name)| file_name(5 | 1 << 48, name, 1, 0));
            let mut len = (0x10 + key.len()).div_ceil(8) * 8;
            if subnode.is_some() {
                len += 8;
            }
            let mut entry = vec![0u8; len];
            let file = name.map_or(0, |(file, _)| file | 1 << 48);
            entry[0..8].copy_from_slice(&file.to_le_bytes());
            entry[8..10].copy_from_slice(&u16::try_from(len).unwrap().to_le_bytes());
            entry[0x0A..0x0C].copy_from_slice(&u16::try_from(key.len()).unwrap().to_le_bytes());
            let flags = u16::from(subnode.is_some()) | if name.is_none() { 2 } else { 0 };
            entry[0x0C..0x0E].copy_from_slice(&flags.to_le_bytes());
            entry[0x10..0x10 + key.len()].copy_from_slice(&key);
            if let Some(vcn) = subnode {
                entry[len - 8..].copy_from_slice(&vcn.to_le_bytes());
            }
            body.extend(entry);
        }
        let used = header_to_entries + u32::try_from(body.len()).unwrap();
        let mut header = vec![0u8; usize::try_from(header_to_entries).unwrap()];
        header[0..4].copy_from_slice(&header_to_entries.to_le_bytes());
        header[4..8].copy_from_slice(&used.to_le_bytes());
        header[8..12].copy_from_slice(&used.to_le_bytes());
        header.extend(body);
        header
    }

    /// Encodes an `$INDEX_ROOT` value of 4 KiB index records.
    pub(crate) fn root(entries: &[EntrySpec<'_>]) -> Vec<u8> {
        let mut value = vec![0u8; 0x10];
        value[0..4].copy_from_slice(&0x30u32.to_le_bytes());
        value[4..8].copy_from_slice(&1u32.to_le_bytes());
        value[8..12].copy_from_slice(&4096u32.to_le_bytes());
        value[12] = 1;
        value.extend(node(entries, 0x10));
        value
    }

    /// Encodes a 4 KiB `INDX` record at `vcn`.
    pub(crate) fn index_record(vcn: u64, entries: &[EntrySpec<'_>]) -> Vec<u8> {
        let mut record = vec![0u8; 4096];
        record[0..4].copy_from_slice(b"INDX");
        record[4..6].copy_from_slice(&0x28u16.to_le_bytes());
        record[6..8].copy_from_slice(&9u16.to_le_bytes());
        record[0x10..0x18].copy_from_slice(&vcn.to_le_bytes());
        let node = node(entries, 0x28);
        record[0x18..0x18 + node.len()].copy_from_slice(&node);
        protect(&mut record, 0x28, 3);
        record
    }

    /// An allocation of `clusters` clusters at cluster 200.
    fn allocation(clusters: u64) -> Stream {
        Stream::NonResident {
            runs: vec![Run {
                vcn: 0,
                len: NonZeroU64::new(clusters).unwrap(),
                lcn: Some(200),
            }],
            size: clusters * 4096,
            initialized: clusters * 4096,
            compression: None,
        }
    }

    fn walk(volume: &Volume, index: &Index) -> Result<Vec<String>, NtfsError> {
        let mut walk = index.walk();
        let mut names = Vec::new();
        loop {
            let next = block_on(walk.next(
                index,
                volume.bytes.as_slice(),
                &volume.geometry,
                OpKind::ReadDir,
            ));
            match next {
                Ok(Some((_, name))) => names.push(String::from_utf16(&name.name).unwrap()),
                Ok(None) => return Ok(names),
                Err(error) => {
                    return Err(*core::error::Error::source(&error)
                        .unwrap()
                        .downcast_ref::<NtfsError>()
                        .unwrap());
                },
            }
        }
    }

    #[test]
    fn walks_indexes_in_order() {
        let mut volume = Volume::new(&[(4, 4)]);
        volume
            .write(
                200,
                &index_record(
                    0,
                    &[
                        (Some((20, "a")), None),
                        (Some((21, "b")), None),
                        (None, None),
                    ],
                ),
            )
            .write(
                201,
                &index_record(
                    1,
                    &[
                        (Some((23, "x")), None),
                        (Some((24, "y")), None),
                        (None, None),
                    ],
                ),
            );
        let root = root(&[(Some((22, "m")), Some(0)), (None, Some(1))]);
        let index = Index::new(&root, Some(allocation(2)), &volume.geometry, 5).unwrap();
        assert_eq!(walk(&volume, &index).unwrap(), ["a", "b", "m", "x", "y"]);

        // A root alone.
        let root = super::tests::root(&[(Some((20, "only")), None), (None, None)]);
        let index = Index::new(&root, None, &volume.geometry, 5).unwrap();
        assert_eq!(walk(&volume, &index).unwrap(), ["only"]);
    }

    #[test]
    fn detects_loops_and_bad_nodes() {
        let mut volume = Volume::new(&[(4, 4)]);
        // Node 0 leads back to itself.
        volume.write(
            200,
            &index_record(0, &[(Some((20, "a")), Some(0)), (None, None)]),
        );
        let root = root(&[(None, Some(0))]);
        let index = Index::new(&root, Some(allocation(2)), &volume.geometry, 5).unwrap();
        assert_eq!(
            walk(&volume, &index),
            Err(NtfsError::IndexLoop { record: 5, vcn: 0 })
        );

        // No allocation, an unwritten node, and a node of another VCN.
        let index = Index::new(&root, None, &volume.geometry, 5).unwrap();
        assert_eq!(
            walk(&volume, &index),
            Err(NtfsError::BadIndexRecord { record: 5, vcn: 0 })
        );
        let root = super::tests::root(&[(None, Some(1))]);
        let index = Index::new(&root, Some(allocation(2)), &volume.geometry, 5).unwrap();
        assert_eq!(
            walk(&volume, &index),
            Err(NtfsError::BadIndexRecord { record: 5, vcn: 1 })
        );
        volume.write(201, &index_record(0, &[(None, None)]));
        assert_eq!(
            walk(&volume, &index),
            Err(NtfsError::BadIndexRecord { record: 5, vcn: 1 })
        );
    }

    #[test]
    fn rejects_malformed_roots() {
        let volume = Volume::new(&[(4, 4)]);
        let bad = Err(NtfsError::BadIndex { record: 5 });
        let good = root(&[(Some((20, "a")), None), (None, None)]);
        let new = |root: &[u8]| Index::new(root, None, &volume.geometry, 5).map(|_| ());
        assert_eq!(new(&good), Ok(()));
        let mut other = good.clone();
        other[0] = 0x80;
        assert_eq!(new(&other), bad);
        let mut size = good.clone();
        size[8..12].copy_from_slice(&3000u32.to_le_bytes());
        assert_eq!(new(&size), bad);
        // Without its last entry.
        assert_eq!(new(&root(&[(Some((20, "a")), None)])), bad);
        assert_eq!(new(&good[..good.len() - 8]), bad);
    }
}
