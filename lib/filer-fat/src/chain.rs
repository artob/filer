// This is free and unencumbered software released into the public domain.

//! The file allocation table, and cluster chains read from it.

use crate::{
    FatError, FatType,
    geometry::{Cluster, Geometry},
};
use alloc::vec::Vec;
use core::num::NonZeroU32;
use filer_api::{BufResult, Error, OpKind, ReadAt};

/// How much of the FAT is read at a time while walking a chain.
const WINDOW: u64 = 4096;

/// Extra bytes read past each window, so that an entry starting at its end
/// (a FAT12 entry straddling two bytes, or a 32-bit entry) is whole.
const SLACK: u64 = 4;

/// A decoded FAT entry: what follows a cluster in its chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Entry {
    /// The cluster is free; it belongs to no chain.
    Free,

    /// The chain continues with this cluster, which is in range.
    Next(Cluster),

    /// The cluster is marked bad.
    Bad,

    /// The chain ends with this cluster.
    End,

    /// A value no chain may contain: cluster 1, a cluster past the end of
    /// the volume, or a reserved value.
    Invalid(u32),
}

impl Entry {
    /// Decodes a raw FAT entry value, already extracted for its width.
    pub(crate) fn decode(geometry: &Geometry, raw: u32) -> Self {
        let (bad, end) = match geometry.fat_type() {
            FatType::Fat12 => (0xFF7, 0xFF8),
            FatType::Fat16 => (0xFFF7, 0xFFF8),
            FatType::Fat32 => (0x0FFF_FFF7, 0x0FFF_FFF8),
        };
        match raw {
            0 => Self::Free,
            raw if raw == bad => Self::Bad,
            raw if raw >= end => Self::End,
            raw => geometry.cluster(raw).map_or(Self::Invalid(raw), Self::Next),
        }
    }
}

/// Reads FAT entries through a small window, so that walking a chain reads
/// the table in a few large reads rather than many tiny ones.
pub(crate) struct FatReader<'a, S: ?Sized> {
    source: &'a S,
    geometry: &'a Geometry,
    /// The table offset of the window's first byte, and its bytes.
    window: Option<(u64, Vec<u8>)>,
}

impl<'a, S: ReadAt + ?Sized> FatReader<'a, S> {
    pub(crate) fn new(source: &'a S, geometry: &'a Geometry) -> Self {
        Self {
            source,
            geometry,
            window: None,
        }
    }

    /// Returns the entry for `cluster`.
    pub(crate) async fn entry(&mut self, cluster: Cluster, op: OpKind) -> Result<Entry, Error> {
        let n = u64::from(cluster.get());
        let geometry = self.geometry;
        // FAT12 packs two entries into three bytes.
        let (at, len) = match geometry.fat_type() {
            FatType::Fat12 => (n.checked_mul(3).map(|nibbles| nibbles / 2), 2),
            FatType::Fat16 => (n.checked_mul(2), 2),
            FatType::Fat32 => (n.checked_mul(4), 4),
        };
        let at = at.ok_or_else(|| FatError::BadLayout.into_error(op))?;
        let bytes = self.bytes(at, len, op).await?;
        let raw = match (geometry.fat_type(), bytes) {
            (FatType::Fat12, &[low, high, ..]) => {
                let pair = u16::from_le_bytes([low, high]);
                // An odd cluster's entry is the high 12 bits of its pair.
                u32::from(if n % 2 == 0 { pair & 0x0FFF } else { pair >> 4 })
            },
            (FatType::Fat16, &[low, high, ..]) => u32::from(u16::from_le_bytes([low, high])),
            // The top four bits of a FAT32 entry are reserved.
            (FatType::Fat32, &[a, b, c, d, ..]) => u32::from_le_bytes([a, b, c, d]) & 0x0FFF_FFFF,
            _ => return Err(FatError::BadLayout.into_error(op)),
        };
        Ok(Entry::decode(geometry, raw))
    }

    /// Returns `len` bytes of the table from offset `at`, refilling the
    /// window if it does not hold them.
    async fn bytes(&mut self, at: u64, len: u64, op: OpKind) -> Result<&[u8], Error> {
        let end = at
            .checked_add(len)
            .filter(|&end| end <= self.geometry.fat_len())
            .ok_or_else(|| FatError::BadLayout.into_error(op))?;
        let held = self.window.as_ref().is_some_and(|(start, bytes)| {
            at >= *start
                && u64::try_from(bytes.len())
                    .ok()
                    .and_then(|len| start.checked_add(len))
                    .is_some_and(|stop| end <= stop)
        });
        if !held {
            let start = at.saturating_sub(at % WINDOW);
            let stop = start
                .saturating_add(WINDOW)
                .saturating_add(SLACK)
                .min(self.geometry.fat_len());
            let wanted = stop
                .checked_sub(start)
                .and_then(|wanted| usize::try_from(wanted).ok())
                .ok_or_else(|| FatError::BadLayout.into_error(op))?;
            let offset = self
                .geometry
                .fat()
                .checked_add(start)
                .ok_or_else(|| FatError::BadLayout.into_error(op))?;
            // Opening checked that the source holds the volume, so running
            // out now means the source shrank: that error stands as it is.
            let BufResult(result, bytes) = self
                .source
                .read_exact_at(Vec::with_capacity(wanted), offset)
                .await;
            result?;
            self.window = Some((start, bytes));
        }
        let (start, bytes) = self
            .window
            .as_ref()
            .ok_or_else(|| FatError::BadLayout.into_error(op))?;
        let index = |offset: u64| {
            offset
                .checked_sub(*start)
                .and_then(|index| usize::try_from(index).ok())
        };
        index(at)
            .zip(index(end))
            .and_then(|(from, to)| bytes.get(from..to))
            .ok_or_else(|| FatError::BadLayout.into_error(op))
    }
}

/// Clusters that follow each other on the volume: `len` clusters from
/// `first`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Run {
    first: Cluster,
    len: NonZeroU32,
    /// The index, within the chain, of this run's first cluster.
    start: u32,
}

/// How long a chain must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Length {
    /// Exactly this many clusters, as a file's size requires: the chain
    /// must end right there.
    Exactly(NonZeroU32),

    /// Any length up to this many clusters, as for a directory.
    AtMost(NonZeroU32),
}

/// A validated cluster chain, as runs of contiguous clusters. Immutable
/// once read, so a handle can keep it and read without walking the FAT
/// again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Chain {
    runs: Vec<Run>,
    clusters: u32,
    cluster_size: u32,
}

impl Chain {
    /// Walks the chain from `first`, which must have `length` clusters.
    ///
    /// Fails with a [`FatError`] if the chain meets a free, bad, or invalid
    /// entry, is shorter or longer than `length` allows, or loops; a loop
    /// is caught because no valid chain has more clusters than the volume.
    pub(crate) async fn read<S: ReadAt + ?Sized>(
        fat: &mut FatReader<'_, S>,
        first: Cluster,
        length: Length,
        op: OpKind,
    ) -> Result<Self, Error> {
        let start = first.get();
        let fail = |error: FatError| error.into_error(op);
        let limit = match length {
            Length::Exactly(n) | Length::AtMost(n) => n.get(),
        };
        let mut runs: Vec<Run> = Vec::new();
        let mut clusters: u32 = 0;
        let mut current = first;
        loop {
            clusters = clusters
                .checked_add(1)
                .filter(|&count| count <= fat.geometry.clusters())
                .ok_or_else(|| fail(FatError::ChainLoop { start }))?;
            if clusters > limit {
                return Err(fail(match length {
                    Length::Exactly(expected) => FatError::LongChain {
                        start,
                        expected: expected.get(),
                    },
                    Length::AtMost(_) => FatError::DirectoryTooLarge { start },
                }));
            }
            // Extend the last run if this cluster follows it on disk.
            let extends = runs.last_mut().and_then(|run| {
                let next = run.first.get().checked_add(run.len.get())?;
                (next == current.get()).then_some(run)
            });
            match extends {
                Some(run) => {
                    run.len = run
                        .len
                        .checked_add(1)
                        .ok_or_else(|| fail(FatError::ChainLoop { start }))?;
                },
                None => runs.push(Run {
                    first: current,
                    len: NonZeroU32::MIN,
                    start: clusters.saturating_sub(1),
                }),
            }
            let cluster = current.get();
            current = match fat.entry(current, op).await? {
                Entry::Next(next) => next,
                Entry::End => break,
                Entry::Free => return Err(fail(FatError::FreeInChain { start, cluster })),
                Entry::Bad => return Err(fail(FatError::BadInChain { start, cluster })),
                Entry::Invalid(next) => {
                    return Err(fail(FatError::BadNext {
                        start,
                        cluster,
                        next,
                    }));
                },
            };
        }
        if let Length::Exactly(expected) = length
            && clusters < expected.get()
        {
            return Err(fail(FatError::ShortChain {
                start,
                expected: expected.get(),
                found: clusters,
            }));
        }
        Ok(Self {
            runs,
            clusters,
            cluster_size: fat.geometry.cluster_size(),
        })
    }

    /// Returns the number of clusters in the chain.
    #[allow(dead_code, reason = "used by tests, and by writing later")]
    pub(crate) const fn clusters(&self) -> u32 {
        self.clusters
    }

    /// Returns the chain's length in bytes.
    pub(crate) fn len(&self) -> u64 {
        u64::from(self.clusters).saturating_mul(u64::from(self.cluster_size))
    }

    /// Maps `offset` within the chain to an offset on the volume and the
    /// number of bytes contiguous from there to the end of its run, or
    /// `None` if `offset` is past the chain's end.
    pub(crate) fn locate(&self, geometry: &Geometry, offset: u64) -> Option<(u64, u64)> {
        let cluster_size = u64::from(self.cluster_size);
        let index = u32::try_from(offset.checked_div(cluster_size)?).ok()?;
        // The last run starting at or before `index`.
        let run = self.runs.get(
            self.runs
                .partition_point(|run| run.start <= index)
                .checked_sub(1)?,
        )?;
        let within = index.checked_sub(run.start)?;
        if within >= run.len.get() {
            return None;
        }
        let into = offset.checked_rem(cluster_size)?;
        let at = geometry
            .cluster_offset(run.first)?
            .checked_add(u64::from(within).checked_mul(cluster_size)?)?
            .checked_add(into)?;
        let left = u64::from(run.len.get().checked_sub(within)?)
            .checked_mul(cluster_size)?
            .checked_sub(into)?;
        Some((at, left))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geometry::tests::Bpb;
    use alloc::vec;
    use filer_spec::block_on;

    /// A volume image built from a boot sector, with FAT entries set.
    pub(crate) struct Image {
        pub(crate) bytes: Vec<u8>,
        pub(crate) geometry: Geometry,
    }

    impl Image {
        pub(crate) fn new(bpb: Bpb) -> Self {
            let boot = bpb.bytes();
            let geometry = Geometry::parse(&boot).unwrap();
            let mut bytes = vec![0; usize::try_from(geometry.volume_len()).unwrap()];
            bytes[..boot.len()].copy_from_slice(&boot);
            Self { bytes, geometry }
        }

        /// Sets the raw entry for cluster `n` in every FAT, truncated to the
        /// entry width.
        pub(crate) fn set(&mut self, n: u32, raw: u32) -> &mut Self {
            let g = self.geometry;
            let first = usize::try_from(g.fat()).unwrap();
            let len = usize::try_from(g.fat_len()).unwrap();
            for copy in 0..usize::from(g.fat_count()) {
                let table = first + copy * len;
                let n = usize::try_from(n).unwrap();
                match g.fat_type() {
                    FatType::Fat12 => {
                        let at = table + n * 3 / 2;
                        let pair = u16::from_le_bytes([self.bytes[at], self.bytes[at + 1]]);
                        let raw = u16::try_from(raw & 0x0FFF).unwrap();
                        let pair = if n % 2 == 0 {
                            (pair & 0xF000) | raw
                        } else {
                            (pair & 0x000F) | (raw << 4)
                        };
                        self.bytes[at..at + 2].copy_from_slice(&pair.to_le_bytes());
                    },
                    FatType::Fat16 => {
                        let raw = u16::try_from(raw & 0xFFFF).unwrap();
                        self.bytes[table + n * 2..][..2].copy_from_slice(&raw.to_le_bytes());
                    },
                    FatType::Fat32 => {
                        self.bytes[table + n * 4..][..4].copy_from_slice(&raw.to_le_bytes());
                    },
                }
            }
            self
        }

        /// Links `clusters` into a chain, ending it with `end`.
        pub(crate) fn chain(&mut self, clusters: &[u32], end: u32) -> &mut Self {
            for pair in clusters.windows(2) {
                self.set(pair[0], pair[1]);
            }
            self.set(*clusters.last().unwrap(), end)
        }
    }

    fn read(image: &Image, first: u32, length: Length) -> Result<Chain, FatError> {
        let mut fat = FatReader::new(&image.bytes, &image.geometry);
        let first = image.geometry.cluster(first).unwrap();
        block_on(Chain::read(&mut fat, first, length, OpKind::Read)).map_err(|error| {
            *core::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<FatError>()
                .unwrap()
        })
    }

    fn exactly(n: u32) -> Length {
        Length::Exactly(NonZeroU32::new(n).unwrap())
    }

    fn at_most(n: u32) -> Length {
        Length::AtMost(NonZeroU32::new(n).unwrap())
    }

    #[test]
    fn decodes_entries() {
        let g = Image::new(Bpb::floppy()).geometry;
        assert_eq!(Entry::decode(&g, 0), Entry::Free);
        assert_eq!(Entry::decode(&g, 1), Entry::Invalid(1));
        assert_eq!(Entry::decode(&g, 5), Entry::Next(g.cluster(5).unwrap()));
        assert_eq!(Entry::decode(&g, 2849), Entry::Invalid(2849));
        assert_eq!(Entry::decode(&g, 0xFF6), Entry::Invalid(0xFF6));
        assert_eq!(Entry::decode(&g, 0xFF7), Entry::Bad);
        assert_eq!(Entry::decode(&g, 0xFF8), Entry::End);
        assert_eq!(Entry::decode(&g, 0xFFF), Entry::End);
    }

    #[test]
    fn reads_chains_of_every_width() {
        for bpb in [Bpb::floppy(), Bpb::fat16(), Bpb::fat32()] {
            let mut image = Image::new(bpb);
            image.chain(&[2, 3, 4, 10, 11, 7], 0x0FFF_FFFF);
            let chain = read(&image, 2, exactly(6)).unwrap();
            assert_eq!(chain.clusters(), 6);
            assert_eq!(chain.runs.len(), 3, "{:?}", image.geometry.fat_type());
        }
    }

    #[test]
    fn reads_fat12_entries_straddling_windows() {
        // Cluster 2730's entry spans bytes 4095 and 4096, across the end of
        // the first window.
        let mut image = Image::new(Bpb::floppy());
        image.chain(&[2729, 2730, 2731], 0xFFF);
        assert_eq!(read(&image, 2729, exactly(3)).unwrap().clusters(), 3);
    }

    #[test]
    fn reads_fat32_entries_ignoring_reserved_bits() {
        let mut image = Image::new(Bpb::fat32());
        image.set(2, 0xF000_0003).set(3, 0xFFFF_FFFF);
        assert_eq!(read(&image, 2, exactly(2)).unwrap().clusters(), 2);
    }

    #[test]
    fn rejects_broken_chains() {
        let mut image = Image::new(Bpb::floppy());
        image.chain(&[2, 3], 0xFFF);
        assert_eq!(
            read(&image, 2, exactly(3)),
            Err(FatError::ShortChain {
                start: 2,
                expected: 3,
                found: 2
            })
        );
        assert_eq!(
            read(&image, 2, exactly(1)),
            Err(FatError::LongChain {
                start: 2,
                expected: 1
            })
        );
        image.chain(&[5, 20], 0xFFF).set(20, 0);
        assert_eq!(
            read(&image, 5, at_most(10)),
            Err(FatError::FreeInChain {
                start: 5,
                cluster: 20
            })
        );
        image.set(20, 0xFF7);
        assert_eq!(
            read(&image, 5, at_most(10)),
            Err(FatError::BadInChain {
                start: 5,
                cluster: 20
            })
        );
        image.set(20, 4000);
        assert_eq!(
            read(&image, 5, at_most(10)),
            Err(FatError::BadNext {
                start: 5,
                cluster: 20,
                next: 4000
            })
        );
    }

    #[test]
    fn bounds_directories_and_catches_loops() {
        let mut image = Image::new(Bpb::floppy());
        image.chain(&[2, 3, 4], 0xFFF);
        assert_eq!(
            read(&image, 2, at_most(2)),
            Err(FatError::DirectoryTooLarge { start: 2 })
        );
        image.set(4, 2);
        let limit = image.geometry.clusters();
        assert_eq!(
            read(&image, 2, at_most(limit)),
            Err(FatError::ChainLoop { start: 2 })
        );
    }

    #[test]
    fn locates_offsets_across_runs() {
        let mut image = Image::new(Bpb::floppy());
        image.chain(&[2, 3, 10], 0xFFF);
        let g = image.geometry;
        let chain = read(&image, 2, exactly(3)).unwrap();
        let data = g.cluster_offset(g.cluster(2).unwrap()).unwrap();
        assert_eq!(chain.locate(&g, 0), Some((data, 1024)));
        assert_eq!(chain.locate(&g, 700), Some((data + 700, 324)));
        let ten = g.cluster_offset(g.cluster(10).unwrap()).unwrap();
        assert_eq!(chain.locate(&g, 1024 + 5), Some((ten + 5, 507)));
        assert_eq!(chain.locate(&g, 1536), None);
        assert_eq!(chain.len(), 1536);
    }
}
