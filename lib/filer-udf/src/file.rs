// This is free and unencumbered software released into the public domain.

//! Open files: their data, embedded in their entries, or in extents.

use crate::{
    entry::{Entry, Extent},
    volume::Volume,
};
use alloc::vec::Vec;
use filer_api::{Error, OpKind, ReadAt};

/// An open file: its embedded data, or its extents.
pub(crate) enum File {
    Embedded(Vec<u8>),
    Extents(Vec<Extent>),
}

impl<S: ReadAt> Volume<S> {
    /// Opens the file whose entry is `entry`.
    pub(crate) async fn open_entry(&self, entry: &Entry, op: OpKind) -> Result<File, Error> {
        if entry.is_embedded() {
            let data = self
                .contents(&self.partitions, entry, entry.len, op)
                .await?;
            return Ok(File::Embedded(data));
        }
        Ok(File::Extents(
            self.extents_in(&self.partitions, entry, op).await?,
        ))
    }

    /// Reads up to `max` bytes of `file`, whose entry is `entry`, at `off`.
    pub(crate) async fn read_file(
        &self,
        entry: &Entry,
        file: &File,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let end = off
            .saturating_add(u64::try_from(max).unwrap_or(u64::MAX))
            .min(entry.len);
        let len = usize::try_from(end.saturating_sub(off)).unwrap_or(usize::MAX);
        match file {
            File::Embedded(data) => {
                let start = usize::try_from(off).unwrap_or(usize::MAX).min(data.len());
                let data = data.get(start..).unwrap_or_default();
                let mut bytes = data.get(..len.min(data.len())).unwrap_or_default().to_vec();
                // Embedded data shorter than the file reads as zeros.
                bytes.resize(len, 0);
                Ok(bytes)
            },
            File::Extents(extents) => {
                self.read_extents(&self.partitions, entry, extents, off, len, op)
                    .await
            },
        }
    }
}
