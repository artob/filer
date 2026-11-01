// This is free and unencumbered software released into the public domain.

//! Data runs (mapping pairs), which map an attribute's virtual clusters
//! (VCNs) to the volume's logical clusters (LCNs).
//!
//! Each run starts with a header byte: its low four bits give the size of
//! the run's length, its high four bits the size of its offset, both in
//! bytes, little-endian. The offset is signed and relative to the previous
//! run's LCN; a run without one is sparse. A zero header byte ends them.

use alloc::vec::Vec;
use core::num::NonZeroU64;

/// Clusters that follow each other in an attribute: `len` virtual clusters
/// from `vcn`, at `lcn` on the volume, or sparse (all zeros) if `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Run {
    pub(crate) vcn: u64,
    pub(crate) len: NonZeroU64,
    pub(crate) lcn: Option<u64>,
}

impl Run {
    /// Returns the first VCN past the run.
    pub(crate) fn end(&self) -> u64 {
        self.vcn.saturating_add(self.len.get())
    }
}

/// Reads a little-endian integer of `bytes.len()` (at most 8) bytes,
/// sign-extended if `signed`.
fn integer(bytes: &[u8], signed: bool) -> Option<i128> {
    let negative = signed && bytes.last().is_some_and(|&top| top & 0x80 != 0);
    let mut value = [if negative { 0xFF } else { 0 }; 16];
    value.get_mut(..bytes.len())?.copy_from_slice(bytes);
    Some(i128::from_le_bytes(value))
}

/// Decodes the data runs of an extent mapping VCNs `first..=last` on a
/// volume of `clusters` clusters. Returns `None` if a run is malformed,
/// lies outside the volume, or the runs do not cover exactly that range.
pub(crate) fn decode(bytes: &[u8], first: u64, last: u64, clusters: u64) -> Option<Vec<Run>> {
    // The empty extent: no runs, and a last VCN of -1.
    let end = last.wrapping_add(1);
    let mut runs = Vec::new();
    let mut vcn = first;
    let mut lcn: i128 = 0;
    let mut at = 0usize;
    loop {
        let header = *bytes.get(at)?;
        if header == 0 {
            break;
        }
        let len_size = usize::from(header & 0x0F);
        let offset_size = usize::from(header >> 4);
        if !(1..=8).contains(&len_size) || offset_size > 8 {
            return None;
        }
        let len_at = at.checked_add(1)?;
        let offset_at = len_at.checked_add(len_size)?;
        let next = offset_at.checked_add(offset_size)?;
        let len = integer(bytes.get(len_at..offset_at)?, false)?;
        let len = NonZeroU64::new(u64::try_from(len).ok()?)?;
        let run_lcn = if offset_size == 0 {
            None
        } else {
            lcn = lcn.checked_add(integer(bytes.get(offset_at..next)?, true)?)?;
            let start = u64::try_from(lcn).ok()?;
            // The whole run must lie in the volume.
            start
                .checked_add(len.get())
                .filter(|&end| end <= clusters)?;
            Some(start)
        };
        runs.push(Run {
            vcn,
            len,
            lcn: run_lcn,
        });
        vcn = vcn.checked_add(len.get())?;
        if vcn > end {
            return None;
        }
        at = next;
    }
    (vcn == end).then_some(runs)
}

/// Returns the run of `runs` (sorted, contiguous) holding `vcn`.
pub(crate) fn find(runs: &[Run], vcn: u64) -> Option<&Run> {
    let index = runs.partition_point(|run| run.end() <= vcn);
    runs.get(index).filter(|run| run.vcn <= vcn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn run(vcn: u64, len: u64, lcn: Option<u64>) -> Run {
        Run {
            vcn,
            len: NonZeroU64::new(len).unwrap(),
            lcn,
        }
    }

    #[test]
    fn decodes_runs() {
        // 0x21: a 1-byte length (0x18) and a 2-byte offset (0x5634); then
        // a sparse run of 16; then -0x10 clusters back for 8.
        let bytes = [0x21, 0x18, 0x34, 0x56, 0x01, 0x10, 0x11, 0x08, 0xF0, 0x00];
        let runs = decode(&bytes, 0, 47, 100_000).unwrap();
        assert_eq!(
            runs,
            vec![
                run(0, 0x18, Some(0x5634)),
                run(0x18, 0x10, None),
                run(0x28, 8, Some(0x5634 - 0x10)),
            ]
        );
        assert_eq!(find(&runs, 0x17), Some(&runs[0]));
        assert_eq!(find(&runs, 0x18), Some(&runs[1]));
        assert_eq!(find(&runs, 47), Some(&runs[2]));
        assert_eq!(find(&runs, 48), None);
    }

    #[test]
    fn decodes_extents_and_empty_attributes() {
        let runs = decode(&[0x11, 0x04, 0x20, 0x00], 10, 13, 100).unwrap();
        assert_eq!(runs, vec![run(10, 4, Some(0x20))]);
        assert_eq!(decode(&[0x00], 0, u64::MAX, 100), Some(vec![]));
    }

    #[test]
    fn rejects_malformed_runs() {
        for (bytes, last) in [
            // No terminator, a zero-size or oversize length, a zero length.
            (&[0x11, 0x04, 0x20][..], 3),
            (&[0x10, 0x20, 0x00], 3),
            (&[0x19, 0, 0, 0, 0, 0, 0, 0, 0, 4, 0x00], 3),
            (&[0x11, 0x00, 0x20, 0x00], 3),
            // Shorter or longer than the extent.
            (&[0x11, 0x04, 0x20, 0x00], 4),
            (&[0x11, 0x04, 0x20, 0x00], 2),
            // A negative LCN, and a run past the volume's end.
            (&[0x11, 0x04, 0xF0, 0x00], 3),
            (&[0x11, 0x04, 0x62, 0x00], 3),
            // A length cut short.
            (&[0x12, 0x04], 3),
        ] {
            assert_eq!(decode(bytes, 0, last, 100), None, "{bytes:02X?}");
        }
    }
}
