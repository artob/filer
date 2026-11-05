// This is free and unencumbered software released into the public domain.

//! Directories: their data is a stream of file identifier descriptors,
//! each a tag, flags, its name's length, the address of its file's
//! entry, implementation use, and the name, padded to four bytes; the
//! parent's first, and deleted ones among them.

use crate::{
    UdfError,
    entry::{self, Entry},
    tag, text,
    volume::Volume,
};
use alloc::{collections::BTreeSet, sync::Arc, vec::Vec};
use filer_api::{Error, OpKind, ReadAt};
use filer_fs::le;

/// The flags of a file identifier: hidden, a directory, deleted, and the
/// parent.
const HIDDEN: u8 = 0x01;
const DELETED: u8 = 0x04;
const PARENT: u8 = 0x08;

/// A directory's entry: its name, as shown, and its file's entry.
#[derive(Clone, Debug)]
pub(crate) struct Listed {
    pub(crate) name: Vec<u8>,
    pub(crate) entry: Arc<Entry>,
}

/// A directory's entries, in the order of their identifiers.
#[derive(Clone, Debug)]
pub(crate) struct Listing {
    pub(crate) entries: Arc<[Listed]>,
    /// Whether their names ascend, so that a lookup may search them by
    /// halves.
    sorted: bool,
}

impl Listing {
    /// Finds the entry `name`, matching bytes exactly.
    pub(crate) fn find(&self, name: &[u8]) -> Option<&Listed> {
        if self.sorted {
            let found = self
                .entries
                .binary_search_by(|listed| listed.name.as_slice().cmp(name));
            found.ok().and_then(|at| self.entries.get(at))
        } else {
            self.entries.iter().find(|listed| listed.name == name)
        }
    }
}

impl<S: ReadAt> Volume<S> {
    /// Reads the entries of directory `dir`.
    pub(crate) async fn list(&self, dir: &Entry, op: OpKind) -> Result<Listing, Error> {
        let bad = || UdfError::BadDirectory { at: dir.at }.into_error(op);
        if dir.len > self.size {
            return Err(bad());
        }
        let data = self.contents(&self.partitions, dir, dir.len, op).await?;
        let (mut entries, mut seen) = (Vec::new(), BTreeSet::new());
        let mut at = 0_usize;
        while let Some(identifier) = data.get(at..).filter(|rest| rest.len() >= 38) {
            // Zeros after the last identifier end the stream.
            if identifier
                .get(..16)
                .is_some_and(|tag| tag.iter().all(|&byte| byte == 0))
            {
                break;
            }
            if !tag::is(identifier, tag::FILE_IDENTIFIER) {
                return Err(bad());
            }
            let flags = *identifier.get(18).ok_or_else(bad)?;
            let name_len = usize::from(*identifier.get(19).ok_or_else(bad)?);
            let address =
                entry::long_ad(identifier.get(20..36).ok_or_else(bad)?).ok_or_else(bad)?;
            let use_len = usize::from(le::u16_at(identifier, 36).ok_or_else(bad)?);
            let name_at = use_len.checked_add(38).ok_or_else(bad)?;
            let name_end = name_at.checked_add(name_len).ok_or_else(bad)?;
            let name = identifier.get(name_at..name_end).ok_or_else(bad)?;
            at = at.saturating_add(name_end.checked_next_multiple_of(4).ok_or_else(bad)?);
            if flags & (DELETED | PARENT) != 0 {
                continue;
            }
            let name = text::name(name).ok_or_else(bad)?;
            let invalid =
                name.contains(&b'/') || name.contains(&0) || name == b"." || name == b"..";
            if invalid {
                return Err(bad());
            }
            let entry = self.entry(address, flags & HIDDEN != 0, op).await?;
            // A corrupt directory's names may repeat: only the first is
            // found, so only it is listed.
            if seen.insert(name.clone()) {
                entries.push(Listed {
                    name,
                    entry: Arc::new(entry),
                });
            }
        }
        Ok(Listing {
            sorted: entries.is_sorted_by(|a, b| a.name < b.name),
            entries: entries.into(),
        })
    }
}
