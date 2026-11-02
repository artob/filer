// This is free and unencumbered software released into the public domain.

//! Reading a file through a map of its blocks onto the volume's.
//!
//! However a format records where a file's data is (ext2's indirect
//! blocks, ext4's and XFS's extents, FAT's cluster chains, NTFS's data
//! runs), reading comes down to the same loop: find the run of blocks
//! holding the next byte, read as much of it as the request needs, or
//! zeros for a hole, and go on to the next run. A format implements
//! [`Mapping`]; [`read`] is the loop.
//!
//! Blocks here are a format's unit of allocation, as one map counts them
//! both in the file and on the volume: blocks, or clusters.

use crate::read_exact;
use alloc::vec::Vec;
use core::{future::Future, num::NonZeroU64};
use filer_api::{Error, OpKind, ReadAt};

/// A run of a file's blocks: `len` of them, stored contiguously from
/// volume block `physical`, or all reading as zeros if it is `None`, as a
/// hole or an unwritten extent does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    /// The number of blocks, at least 1.
    pub len: u64,
    /// The volume block of the run's first block, if it is stored.
    pub physical: Option<u64>,
}

/// A map of a file's blocks onto the volume's.
pub trait Mapping: Sync {
    /// Returns the size of a block, in bytes.
    fn block_size(&self) -> NonZeroU64;

    /// Returns the offset of volume block `block`, if it is in the volume.
    fn offset(&self, block: u64) -> Option<u64>;

    /// Returns the run of at most `max` blocks (at least 1) from file block
    /// `logical`, reading what the map needs from `source`.
    ///
    /// # Errors
    ///
    /// The format's error for a map that does not reach `logical` or is
    /// inconsistent; any error reading `source`.
    fn map<S: ReadAt + ?Sized>(
        &self,
        source: &S,
        logical: u64,
        max: u64,
        op: OpKind,
    ) -> impl Future<Output = Result<Run, Error>> + Send;

    /// Returns the format's error for a map that is inconsistent with
    /// itself: one whose runs are empty, or lie past the end of the volume.
    fn corrupt(&self, op: OpKind) -> Error;
}

/// Reads up to `len` bytes of a file of `size` bytes from `off`, through
/// `map`: fewer only at its end, and none at or past it. Holes read as
/// zeros. The bytes are read in a piece per run, and returned together.
///
/// # Errors
///
/// [`Mapping::corrupt`] for a run that is empty or not on the volume; any
/// error from [`Mapping::map`] or reading `source`.
pub async fn read<S: ReadAt + ?Sized, M: Mapping + ?Sized>(
    map: &M,
    source: &S,
    size: u64,
    off: u64,
    len: usize,
    op: OpKind,
) -> Result<Vec<u8>, Error> {
    let bad = || map.corrupt(op);
    let block = map.block_size().get();
    let end = u64::try_from(len)
        .ok()
        .and_then(|len| off.checked_add(len))
        .map_or(size, |end| end.min(size));
    let mut out = Vec::new();
    let mut at = off;
    while at < end {
        let logical = at.checked_div(block).ok_or_else(bad)?;
        let within = at.checked_rem(block).ok_or_else(bad)?;
        let max = end
            .saturating_sub(at.saturating_sub(within))
            .div_ceil(block);
        let run = map.map(source, logical, max, op).await?;
        let run_end = logical
            .checked_add(run.len)
            .and_then(|blocks| blocks.checked_mul(block))
            .ok_or_else(bad)?
            .min(end);
        let n = run_end.saturating_sub(at);
        let n = usize::try_from(n).map_err(|_| bad())?;
        if n == 0 {
            // An empty run would never get anywhere.
            return Err(bad());
        }
        match run.physical {
            None => out.resize(out.len().saturating_add(n), 0),
            Some(physical) => {
                let from = map
                    .offset(physical)
                    .and_then(|from| from.checked_add(within))
                    .ok_or_else(bad)?;
                out.extend(read_exact(source, from, n, op).await?);
            },
        }
        at = run_end;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use filer_api::ErrorKind;
    use filer_spec::block_on;

    /// A file of blocks of 4 bytes: two at volume block 2, a hole of one,
    /// then one at volume block 0; or, if `broken`, runs that are empty.
    struct Fixed {
        broken: bool,
    }

    impl Mapping for Fixed {
        fn block_size(&self) -> NonZeroU64 {
            NonZeroU64::new(4).unwrap()
        }

        fn offset(&self, block: u64) -> Option<u64> {
            (block < 4).then(|| block * 4)
        }

        async fn map<S: ReadAt + ?Sized>(
            &self,
            _source: &S,
            logical: u64,
            max: u64,
            _op: OpKind,
        ) -> Result<Run, Error> {
            let (len, physical) = match logical {
                _ if self.broken => (0, None),
                0 | 1 => (2 - logical, Some(2 + logical)),
                2 => (1, None),
                _ => (1, Some(0)),
            };
            Ok(Run {
                len: len.min(max),
                physical,
            })
        }

        fn corrupt(&self, op: OpKind) -> Error {
            Error::new(ErrorKind::Corrupt, op)
        }
    }

    #[test]
    fn reads_across_runs_and_holes() {
        let volume: Vec<u8> = (0..16).collect();
        let map = Fixed { broken: false };
        let read = |off, len| block_on(read(&map, &volume, 15, off, len, OpKind::Read)).unwrap();
        assert_eq!(
            read(0, 100),
            [8, 9, 10, 11, 12, 13, 14, 15, 0, 0, 0, 0, 0, 1, 2]
        );
        assert_eq!(read(6, 4), [14, 15, 0, 0]);
        assert_eq!(read(13, 1), [1]);
        assert_eq!(read(15, 1), vec![]);
        assert_eq!(read(0, 0), vec![]);
    }

    #[test]
    fn refuses_empty_runs() {
        let map = Fixed { broken: true };
        let error = block_on(read(&map, &[0; 16], 8, 0, 8, OpKind::Read)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
    }
}
