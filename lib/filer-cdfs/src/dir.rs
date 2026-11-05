// This is free and unencumbered software released into the public domain.

//! Directories: their extents hold records, which end in the sector they
//! begin in, a zero byte ending each sector's; `.` and `..` first. Each
//! record names its file, and says, with its System Use entries, all that
//! is reported of it, so a listing holds every entry's node.

use crate::{
    CdfsError,
    descriptor::{Names, SECTOR},
    node::{Body, Data, Node},
    read,
    record::{self, Record},
    susp::{self, Rock},
    time,
    volume::Volume,
};
use alloc::{collections::BTreeSet, sync::Arc, vec::Vec};
use filer_api::{Error, FileKind, OpKind, ReadAt};

/// A directory's entry: its name, as shown, and its node.
#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) name: Vec<u8>,
    pub(crate) node: Arc<Node>,
}

/// A directory's entries, in the order of their records.
#[derive(Clone, Debug)]
pub(crate) struct Listing {
    pub(crate) entries: Arc<[Entry]>,
    /// Whether their names ascend, so that a lookup may search them by
    /// halves: ISO 9660 sorts records by their own names, which Rock Ridge
    /// and Joliet names need not follow.
    sorted: bool,
}

impl Listing {
    /// Finds the entry `name`, matching bytes exactly.
    pub(crate) fn find(&self, name: &[u8]) -> Option<&Entry> {
        if self.sorted {
            let found = self
                .entries
                .binary_search_by(|entry| entry.name.as_slice().cmp(name));
            found.ok().and_then(|at| self.entries.get(at))
        } else {
            self.entries.iter().find(|entry| entry.name == name)
        }
    }
}

/// A record, copied out of its sector, with its position.
struct Owned {
    at: u64,
    data: Option<u64>,
    len: u32,
    date: [u8; 7],
    flags: u8,
    interleaved: bool,
    name: Vec<u8>,
    system_use: Vec<u8>,
}

impl Owned {
    fn new(record: &Record<'_>, at: u64, block: u32) -> Self {
        Self {
            at,
            data: record.data(block),
            len: record.len,
            date: <[u8; 7]>::try_from(record.date).unwrap_or_default(),
            flags: record.flags,
            interleaved: record.interleaved,
            name: record.name.to_vec(),
            system_use: record.system_use.to_vec(),
        }
    }
}

impl<S: ReadAt> Volume<S> {
    /// Reads the directory whose extent is at `at`, of `len` bytes.
    pub(crate) async fn list(&self, at: u64, len: u64, op: OpKind) -> Result<Listing, Error> {
        let bad = || CdfsError::BadDirectory { at }.into_error(op);
        if at.checked_add(len).is_none_or(|end| end > self.size) {
            return Err(bad());
        }
        let sector = u64::try_from(SECTOR).unwrap_or(u64::MAX);
        let (mut entries, mut parts) = (Vec::<Entry>::new(), Vec::<Owned>::new());
        let mut seen = BTreeSet::new();
        let mut done = 0_u64;
        while done < len {
            let start = at.saturating_add(done);
            let size = usize::try_from(len.saturating_sub(done).min(sector)).unwrap_or(SECTOR);
            let bytes = read::read(&self.source, start, size, op, bad).await?;
            let mut off = 0_usize;
            while let Some(&record_len) = bytes.get(off).filter(|&&len| len != 0) {
                let record = bytes.get(off..).and_then(Record::parse).ok_or_else(bad)?;
                let position = start.saturating_add(u64::try_from(off).unwrap_or(u64::MAX));
                off = off.saturating_add(usize::from(record_len));
                if record.is_dot() {
                    continue;
                }
                parts.push(Owned::new(&record, position, self.block));
                if record.flags & record::MULTI_EXTENT != 0 {
                    continue;
                }
                // A file's extents, each in a record of its name.
                let group = core::mem::take(&mut parts);
                let name = group.last().map(|part| part.name.as_slice());
                if group.iter().any(|part| Some(part.name.as_slice()) != name) {
                    return Err(bad());
                }
                let Some(entry) = self.entry(&group, op).await? else {
                    continue;
                };
                // Of several versions of a name, the first is the latest;
                // a corrupt directory's names may repeat anywhere, and
                // then only the first is found, so only it is listed.
                if seen.insert(entry.name.clone()) {
                    entries.push(entry);
                }
            }
            done = done.saturating_add(sector);
        }
        if !parts.is_empty() {
            return Err(bad());
        }
        Ok(Listing {
            sorted: entries.is_sorted_by(|a, b| a.name < b.name),
            entries: entries.into(),
        })
    }

    /// Returns the entry the records `group` make, those of one file, or
    /// `None` for one that is not shown: an associated file, or a
    /// directory relocated to where its `CL` record stands.
    async fn entry(&self, group: &[Owned], op: OpKind) -> Result<Option<Entry>, Error> {
        let Some(first) = group.first() else {
            return Ok(None);
        };
        let bad = || CdfsError::BadRecord { at: first.at }.into_error(op);
        if first.flags & record::ASSOCIATED != 0 {
            return Ok(None);
        }
        let rock = self.rock(&first.system_use, first.at, op).await?;
        if rock.relocated {
            return Ok(None);
        }
        let name = match &rock.name {
            Some(name) => name.clone(),
            None => self.name(&first.name).ok_or_else(bad)?,
        };
        let invalid = name.is_empty()
            || name.contains(&b'/')
            || name.contains(&0)
            || name == b"."
            || name == b"..";
        if invalid {
            return Err(bad());
        }
        let node = match rock.child {
            Some(child) => {
                let at = u64::from(child)
                    .checked_mul(u64::from(self.block))
                    .ok_or_else(bad)?;
                self.directory_at(at, self.sharing, first.flags & record::HIDDEN != 0, op)
                    .await?
            },
            None => self.node(group, rock, op)?,
        };
        Ok(Some(Entry {
            name,
            node: Arc::new(node),
        }))
    }

    /// Returns the name a record's identifier shows as: without its
    /// version, `;1`, and the `.` before it if it has no extension; and
    /// Joliet's as WTF-8.
    fn name(&self, identifier: &[u8]) -> Option<Vec<u8>> {
        if self.names == Names::Joliet {
            let (units, rest) = identifier.as_chunks::<2>();
            if !rest.is_empty() {
                return None;
            }
            let units: Vec<u16> = units.iter().map(|&pair| u16::from_be_bytes(pair)).collect();
            let units = unversioned(&units, |unit| u8::try_from(unit).ok());
            Some(filer_msft::wtf8::encode(units))
        } else {
            Some(unversioned(identifier, Some).to_vec())
        }
    }

    /// Reads a record's Rock Ridge entries, if the image has them.
    async fn rock(&self, field: &[u8], at: u64, op: OpKind) -> Result<Rock, Error> {
        match self.sharing {
            Some(skip) => susp::read(&self.source, self.block, field, skip, at, op).await,
            None => Ok(Rock::default()),
        }
    }

    /// Returns the directory whose extent is at `at`, as the `.` record it
    /// begins with has it, with Rock Ridge entries after `sharing`'s bytes
    /// if any; the root's, or a relocated one's.
    pub(crate) async fn directory_at(
        &self,
        at: u64,
        sharing: Option<u8>,
        hidden: bool,
        op: OpKind,
    ) -> Result<Node, Error> {
        let bad = || CdfsError::BadDirectory { at }.into_error(op);
        let len = self
            .size
            .checked_sub(at)
            .filter(|&len| len > 0)
            .ok_or_else(bad)?;
        let len = usize::try_from(len).unwrap_or(SECTOR).min(SECTOR);
        let bytes = read::read(&self.source, at, len, op, bad).await?;
        let dot = Record::parse(&bytes)
            .filter(|dot| dot.name == [0] && dot.flags & record::DIRECTORY != 0)
            .ok_or_else(bad)?;
        let rock = match sharing {
            Some(skip) => {
                susp::read(&self.source, self.block, dot.system_use, skip, at, op).await?
            },
            None => Rock::default(),
        };
        let data = dot.data(self.block).ok_or_else(bad)?;
        Ok(Node {
            kind: FileKind::Dir,
            id: Node::dir_id(data),
            len: u64::from(dot.len),
            times: times(&rock, dot.date),
            hidden,
            body: Body::Dir {
                at: data,
                len: u64::from(dot.len),
            },
        })
    }

    /// Returns the node the records `group` make, with what `rock` says.
    fn node(&self, group: &[Owned], rock: Rock, op: OpKind) -> Result<Node, Error> {
        let first = group
            .first()
            .ok_or_else(|| Error::new(filer_api::ErrorKind::Corrupt, op))?;
        let at = first.at;
        let bad = || CdfsError::BadRecord { at }.into_error(op);
        let directory = first.flags & record::DIRECTORY != 0;
        let kind = match rock.mode {
            Some(mode) => FileKind::from_unix_mode(mode).ok_or_else(bad)?,
            None if directory => FileKind::Dir,
            None => FileKind::File,
        };
        if (kind == FileKind::Dir) != directory {
            return Err(bad());
        }
        let times = times(&rock, &first.date);
        let hidden = first.flags & record::HIDDEN != 0;
        let file_id = rock
            .serial
            .filter(|&serial| serial != 0)
            .map_or_else(|| Node::record_id(at), Node::serial_id);
        let (id, len, body) = match kind {
            FileKind::Dir => {
                let data = first.data.ok_or_else(bad)?;
                if group.len() > 1 || self.past_end(data, u64::from(first.len)) {
                    return Err(bad());
                }
                let len = u64::from(first.len);
                (Node::dir_id(data), len, Body::Dir { at: data, len })
            },
            FileKind::File => {
                let data = self.data(group, &rock, op)?;
                let len = rock.zisofs.map_or(data.stored, |zf| u64::from(zf.size));
                (file_id, len, Body::File(data))
            },
            FileKind::Symlink => {
                let target = rock
                    .target
                    .filter(|target| !target.is_empty())
                    .ok_or_else(|| CdfsError::BadSymlink { at }.into_error(op))?;
                let len = u64::try_from(target.len()).unwrap_or(u64::MAX);
                (file_id, len, Body::Symlink(target))
            },
            _ => (file_id, 0, Body::Special),
        };
        Ok(Node {
            kind,
            id,
            len,
            times,
            hidden,
            body,
        })
    }

    /// Returns where the data of the file whose records are `group` is,
    /// and whether it can be read.
    fn data(&self, group: &[Owned], rock: &Rock, op: OpKind) -> Result<Data, Error> {
        let at = group.first().map_or(0, |first| first.at);
        let bad = || CdfsError::BadRecord { at }.into_error(op);
        let mut extents = Vec::with_capacity(group.len());
        let mut stored = 0_u64;
        for part in group {
            let len = u64::from(part.len);
            let position = part.data.ok_or_else(bad)?;
            if len > 0 && self.past_end(position, len) {
                return Err(bad());
            }
            extents.push((position, len));
            stored = stored.checked_add(len).ok_or_else(bad)?;
        }
        let mut zisofs = None;
        let refusal = if group.iter().any(|part| part.interleaved) {
            Some(CdfsError::Interleaved { at })
        } else if rock.sparse {
            Some(CdfsError::Sparse { at })
        } else if let Some(zf) = rock.zisofs {
            let readable =
                cfg!(feature = "zisofs") && zf.algorithm == *b"pz" && (15..=17).contains(&zf.log);
            zisofs = Some((zf.log, zf.header));
            (!readable).then_some(CdfsError::UnsupportedCompression { at })
        } else {
            None
        };
        Ok(Data {
            at,
            extents,
            stored,
            zisofs,
            refusal,
        })
    }

    /// Returns `true` if `len` bytes from `at` run past the image's end.
    fn past_end(&self, at: u64, len: u64) -> bool {
        at.checked_add(len).is_none_or(|end| end > self.size)
    }
}

/// Returns a record's times: Rock Ridge's, with the record's date as its
/// modification time if they lack one.
fn times(rock: &Rock, date: &[u8]) -> susp::Times {
    let mut times = rock.times;
    if times.modified.is_none() {
        times.modified = time::short(date);
    }
    times
}

/// Returns `identifier` without its version: what follows its last `;`,
/// if only digits do; and then the `.` before it, if it ends with one,
/// which a name without an extension keeps before its version.
fn unversioned<T: Copy + PartialEq>(identifier: &[T], byte: impl Fn(T) -> Option<u8>) -> &[T] {
    let Some(semicolon) = identifier.iter().rposition(|&c| byte(c) == Some(b';')) else {
        return identifier;
    };
    let (name, version) = identifier.split_at(semicolon);
    let digits = version
        .get(1..)
        .unwrap_or_default()
        .iter()
        .all(|&c| byte(c).is_some_and(|byte| byte.is_ascii_digit()));
    if !digits {
        return identifier;
    }
    match name.split_last() {
        Some((&last, rest)) if byte(last) == Some(b'.') && !rest.is_empty() => rest,
        _ => name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_versions() {
        let plain = |name: &[u8]| unversioned(name, Some).to_vec();
        assert_eq!(plain(b"README.TXT;1"), b"README.TXT");
        assert_eq!(plain(b"README.;1"), b"README");
        assert_eq!(plain(b"README;12"), b"README");
        assert_eq!(plain(b"DIR"), b"DIR");
        assert_eq!(plain(b"A;B"), b"A;B");
        assert_eq!(plain(b".;1"), b".");
        assert_eq!(plain(b"A.B."), b"A.B.");
    }
}
