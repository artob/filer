// This is free and unencumbered software released into the public domain.

//! The boot sector, parsed and validated into a volume's geometry.

use crate::{FatError, FatType};
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// The size of the boot sector read, which holds every field parsed here
/// and the signature at bytes 510 and 511, whatever the sector size.
pub(crate) const BOOT_SECTOR: usize = 512;

/// The size of a directory entry.
pub(crate) const DIR_ENTRY: u64 = 32;

/// A data cluster: a number from 2 to the volume's last cluster. Only a
/// [`Geometry`] makes them, so holding one proves it is in range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Cluster(u32);

impl Cluster {
    /// Returns the cluster number.
    pub(crate) const fn get(self) -> u32 {
        self.0
    }
}

/// Where a volume's root directory lies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Root {
    /// FAT12 and FAT16: a fixed region of `entries` directory entries.
    Region { offset: u64, entries: u16 },

    /// FAT32: a cluster chain, like any other directory.
    Chain(Cluster),
}

/// A volume's layout, derived from a validated boot sector. Immutable, and
/// constructed only by [`Geometry::parse`], so every value in it has been
/// checked: the regions fit in the volume and the FAT covers every cluster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    fat_type: FatType,
    sector_size: u32,
    cluster_size: u32,
    clusters: u32,
    /// The offset of the FAT to read: the first, or the active one when a
    /// FAT32 volume disables mirroring.
    fat: u64,
    fat_len: u64,
    fat_count: u8,
    root: Root,
    /// The offset of cluster 2, the first data cluster.
    data: u64,
    volume_len: u64,
    /// The volume serial number, if the boot sector has an extended boot
    /// record: not layout, but read from the same sector, and as fixed.
    serial: Option<u32>,
}

/// Reads the byte at `at`.
fn u8_at(sector: &[u8], at: usize) -> Result<u8, FatError> {
    sector.get(at).copied().ok_or(FatError::ShortBootSector)
}

/// Reads the little-endian `u16` at `at`.
fn u16_at(sector: &[u8], at: usize) -> Result<u16, FatError> {
    sector
        .get(at..)
        .and_then(<[u8]>::first_chunk)
        .map(|bytes| u16::from_le_bytes(*bytes))
        .ok_or(FatError::ShortBootSector)
}

/// Reads the little-endian `u32` at `at`.
fn u32_at(sector: &[u8], at: usize) -> Result<u32, FatError> {
    sector
        .get(at..)
        .and_then(<[u8]>::first_chunk)
        .map(|bytes| u32::from_le_bytes(*bytes))
        .ok_or(FatError::ShortBootSector)
}

impl Geometry {
    /// Parses and validates a boot sector, of which the first
    /// [`BOOT_SECTOR`] bytes are needed.
    ///
    /// Field offsets and rules follow Microsoft's FAT specification.
    pub(crate) fn parse(sector: &[u8]) -> Result<Self, FatError> {
        if u8_at(sector, 510)? != 0x55 || u8_at(sector, 511)? != 0xAA {
            return Err(FatError::BadSignature);
        }
        let sector_size = u16_at(sector, 11)?;
        if !matches!(sector_size, 512 | 1024 | 2048 | 4096) {
            return Err(FatError::BadSectorSize(sector_size));
        }
        let cluster_sectors = u8_at(sector, 13)?;
        if !cluster_sectors.is_power_of_two() {
            return Err(FatError::BadClusterSize(cluster_sectors));
        }
        let reserved = u16_at(sector, 14)?;
        if reserved == 0 {
            return Err(FatError::NoReservedSectors);
        }
        let fat_count = u8_at(sector, 16)?;
        if fat_count == 0 {
            return Err(FatError::NoFats);
        }
        let root_entries = u16_at(sector, 17)?;
        let media = u8_at(sector, 21)?;
        if media != 0xF0 && media < 0xF8 {
            return Err(FatError::BadMedia(media));
        }
        let total = match u16_at(sector, 19)? {
            0 => u32_at(sector, 32)?,
            total => u32::from(total),
        };
        if total == 0 {
            return Err(FatError::NoTotalSectors);
        }
        let fat16_sectors = u16_at(sector, 22)?;
        // The 32-bit FAT size exists only where the 16-bit one is zero;
        // elsewhere those bytes belong to the FAT12/16 extended fields.
        let fat_sectors = match fat16_sectors {
            0 => u32_at(sector, 36)?,
            sectors => u32::from(sectors),
        };
        if fat_sectors == 0 {
            return Err(FatError::NoFatSize);
        }

        let sector_len = u64::from(sector_size);
        let root_sectors = u64::from(root_entries)
            .checked_mul(DIR_ENTRY)
            .ok_or(FatError::BadLayout)?
            .div_ceil(sector_len);
        let fat_len_sectors = u64::from(fat_sectors);
        let meta_sectors = u64::from(fat_count)
            .checked_mul(fat_len_sectors)
            .and_then(|fats| fats.checked_add(u64::from(reserved)))
            .and_then(|sectors| sectors.checked_add(root_sectors))
            .ok_or(FatError::BadLayout)?;
        let data_sectors = u64::from(total)
            .checked_sub(meta_sectors)
            .ok_or(FatError::BadLayout)?;
        let clusters = data_sectors
            .checked_div(u64::from(cluster_sectors))
            .and_then(|clusters| u32::try_from(clusters).ok())
            .ok_or(FatError::BadLayout)?;
        if clusters == 0 {
            return Err(FatError::NoClusters);
        }

        let fat_type = FatType::for_clusters(clusters);
        if fat_type == FatType::Fat32 {
            if root_entries != 0 || fat16_sectors != 0 {
                return Err(FatError::BadRootEntries);
            }
            if clusters > FatType::MAX_FAT32_CLUSTERS {
                return Err(FatError::BadLayout);
            }
            let version = u16_at(sector, 42)?;
            if version != 0 {
                return Err(FatError::UnsupportedVersion(version));
            }
        } else if root_entries == 0 {
            return Err(FatError::BadRootEntries);
        }

        let fat_len = fat_len_sectors
            .checked_mul(sector_len)
            .ok_or(FatError::BadLayout)?;
        let needed = u64::from(clusters)
            .checked_add(2)
            .and_then(|entries| fat_type.table_len(entries))
            .ok_or(FatError::BadLayout)?;
        if needed > fat_len {
            return Err(FatError::FatTooSmall {
                needed,
                actual: fat_len,
            });
        }

        // A FAT32 volume may disable mirroring and use one FAT only.
        let active = if fat_type == FatType::Fat32 {
            let flags = u16_at(sector, 40)?;
            if flags & 0x80 == 0 {
                0
            } else {
                let active = u8::try_from(flags & 0x0F).map_err(|_| FatError::BadLayout)?;
                if active >= fat_count {
                    return Err(FatError::BadActiveFat(active));
                }
                active
            }
        } else {
            0
        };

        let at = |sectors: u64| sectors.checked_mul(sector_len).ok_or(FatError::BadLayout);
        let first_fat = at(u64::from(reserved))?;
        let fat = u64::from(active)
            .checked_mul(fat_len)
            .and_then(|offset| offset.checked_add(first_fat))
            .ok_or(FatError::BadLayout)?;
        let root_region = u64::from(fat_count)
            .checked_mul(fat_len)
            .and_then(|fats| fats.checked_add(first_fat))
            .ok_or(FatError::BadLayout)?;
        let data = at(root_sectors)?
            .checked_add(root_region)
            .ok_or(FatError::BadLayout)?;
        let cluster_size = u32::from(sector_size)
            .checked_mul(u32::from(cluster_sectors))
            .ok_or(FatError::BadLayout)?;
        let volume_len = at(u64::from(total))?;

        let mut geometry = Self {
            fat_type,
            sector_size: u32::from(sector_size),
            cluster_size,
            clusters,
            fat,
            fat_len,
            fat_count,
            root: Root::Region {
                offset: root_region,
                entries: root_entries,
            },
            data,
            volume_len,
            serial: None,
        };
        // The extended boot record follows the BPB, which FAT32 extends;
        // its signature byte, 0x28 or 0x29, says the serial is present.
        let (signature, serial) = match fat_type {
            FatType::Fat12 | FatType::Fat16 => (38, 39),
            FatType::Fat32 => (66, 67),
        };
        if matches!(sector.get(signature), Some(0x28 | 0x29)) {
            geometry.serial = Some(u32_at(sector, serial)?);
        }
        if fat_type == FatType::Fat32 {
            let root = u32_at(sector, 44)?;
            let root = geometry
                .cluster(root)
                .ok_or(FatError::BadRootCluster(root))?;
            geometry.root = Root::Chain(root);
        }
        Ok(geometry)
    }

    /// Returns `n` as a [`Cluster`] if it is a data cluster of this volume.
    pub(crate) fn cluster(&self, n: u32) -> Option<Cluster> {
        let last = self.clusters.checked_add(1)?;
        (2..=last).contains(&n).then_some(Cluster(n))
    }

    /// Returns the offset of `cluster`'s first byte.
    pub(crate) fn cluster_offset(&self, cluster: Cluster) -> Option<u64> {
        u64::from(cluster.get().checked_sub(2)?)
            .checked_mul(u64::from(self.cluster_size))?
            .checked_add(self.data)
    }

    pub(crate) const fn fat_type(&self) -> FatType {
        self.fat_type
    }

    #[allow(dead_code, reason = "for writing, later")]
    pub(crate) const fn sector_size(&self) -> u32 {
        self.sector_size
    }

    pub(crate) const fn cluster_size(&self) -> u32 {
        self.cluster_size
    }

    /// Returns the number of data clusters.
    pub(crate) const fn clusters(&self) -> u32 {
        self.clusters
    }

    /// Returns the offset of the FAT to read.
    pub(crate) const fn fat(&self) -> u64 {
        self.fat
    }

    pub(crate) const fn fat_len(&self) -> u64 {
        self.fat_len
    }

    #[allow(dead_code, reason = "used by tests, and by writing later")]
    pub(crate) const fn fat_count(&self) -> u8 {
        self.fat_count
    }

    pub(crate) const fn root(&self) -> Root {
        self.root
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn volume_len(&self) -> u64 {
        self.volume_len
    }

    /// Returns the volume serial number, if the volume has one.
    pub(crate) const fn serial(&self) -> Option<u32> {
        self.serial
    }
}

/// Reads and validates the boot sector of the volume in `source`, which
/// must hold the whole volume.
pub(crate) async fn read<S: ReadAt + ?Sized>(source: &S) -> Result<Geometry, Error> {
    let op = OpKind::Open;
    let size = source.size().await?;
    let BufResult(result, sector) = source
        .read_exact_at(Vec::with_capacity(BOOT_SECTOR), 0)
        .await;
    match result {
        Ok(()) => {},
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            return Err(FatError::ShortBootSector.into_error(op));
        },
        Err(error) => return Err(error),
    }
    let geometry = Geometry::parse(&sector).map_err(|error| error.into_error(op))?;
    if geometry.volume_len > size {
        let volume = geometry.volume_len;
        return Err(FatError::Truncated {
            volume,
            source: size,
        }
        .into_error(op));
    }
    Ok(geometry)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;
    use filer_spec::block_on;

    /// The fields of a boot sector, for building test volumes.
    #[derive(Clone, Copy)]
    pub(crate) struct Bpb {
        pub(crate) sector_size: u16,
        pub(crate) cluster_sectors: u8,
        pub(crate) reserved: u16,
        pub(crate) fats: u8,
        pub(crate) root_entries: u16,
        pub(crate) total16: u16,
        pub(crate) media: u8,
        pub(crate) fat16: u16,
        pub(crate) total32: u32,
        pub(crate) fat32: u32,
        pub(crate) ext_flags: u16,
        pub(crate) version: u16,
        pub(crate) root_cluster: u32,
        pub(crate) signature: bool,
    }

    impl Bpb {
        /// A 1.44 MB floppy: FAT12, 2,847 clusters.
        pub(crate) fn floppy() -> Self {
            Self {
                sector_size: 512,
                cluster_sectors: 1,
                reserved: 1,
                fats: 2,
                root_entries: 224,
                total16: 2880,
                media: 0xF0,
                fat16: 9,
                total32: 0,
                fat32: 0,
                ext_flags: 0,
                version: 0,
                root_cluster: 0,
                signature: true,
            }
        }

        /// 64 MiB of FAT16: 32,695 clusters of 2 KiB.
        pub(crate) fn fat16() -> Self {
            Self {
                cluster_sectors: 4,
                root_entries: 512,
                total16: 0,
                media: 0xF8,
                fat16: 128,
                total32: 131_072,
                ..Self::floppy()
            }
        }

        /// 64 MiB of FAT32: 129,008 clusters of 512 bytes.
        pub(crate) fn fat32() -> Self {
            Self {
                cluster_sectors: 1,
                reserved: 32,
                root_entries: 0,
                fat16: 0,
                fat32: 1016,
                root_cluster: 2,
                ..Self::fat16()
            }
        }

        pub(crate) fn bytes(&self) -> [u8; BOOT_SECTOR] {
            let mut b = [0_u8; BOOT_SECTOR];
            b[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
            b[11..13].copy_from_slice(&self.sector_size.to_le_bytes());
            b[13] = self.cluster_sectors;
            b[14..16].copy_from_slice(&self.reserved.to_le_bytes());
            b[16] = self.fats;
            b[17..19].copy_from_slice(&self.root_entries.to_le_bytes());
            b[19..21].copy_from_slice(&self.total16.to_le_bytes());
            b[21] = self.media;
            b[22..24].copy_from_slice(&self.fat16.to_le_bytes());
            b[32..36].copy_from_slice(&self.total32.to_le_bytes());
            if self.fat16 == 0 {
                b[36..40].copy_from_slice(&self.fat32.to_le_bytes());
                b[40..42].copy_from_slice(&self.ext_flags.to_le_bytes());
                b[42..44].copy_from_slice(&self.version.to_le_bytes());
                b[44..48].copy_from_slice(&self.root_cluster.to_le_bytes());
            }
            if self.signature {
                b[510] = 0x55;
                b[511] = 0xAA;
            }
            b
        }

        fn parse(&self) -> Result<Geometry, FatError> {
            Geometry::parse(&self.bytes())
        }
    }

    #[test]
    fn reads_the_serial_number_if_present() {
        for (bpb, extended) in [(Bpb::floppy(), 36), (Bpb::fat32(), 64)] {
            let mut bytes = bpb.bytes();
            assert_eq!(Geometry::parse(&bytes).unwrap().serial(), None);
            bytes[extended + 3..extended + 7].copy_from_slice(&0x1234_ABCD_u32.to_le_bytes());
            assert_eq!(Geometry::parse(&bytes).unwrap().serial(), None);
            for signature in [0x28, 0x29] {
                bytes[extended + 2] = signature;
                let serial = Geometry::parse(&bytes).unwrap().serial();
                assert_eq!(serial, Some(0x1234_ABCD));
            }
        }
    }

    #[test]
    fn parses_a_fat12_floppy() {
        let g = Bpb::floppy().parse().unwrap();
        assert_eq!(g.fat_type(), FatType::Fat12);
        assert_eq!((g.clusters(), g.cluster_size()), (2847, 512));
        assert_eq!((g.fat(), g.fat_len(), g.fat_count()), (512, 9 * 512, 2));
        let root = Root::Region {
            offset: 19 * 512,
            entries: 224,
        };
        assert_eq!(g.root(), root);
        assert_eq!(g.cluster_offset(g.cluster(2).unwrap()), Some(33 * 512));
        assert_eq!(g.volume_len(), 2880 * 512);
    }

    #[test]
    fn parses_fat16() {
        let g = Bpb::fat16().parse().unwrap();
        assert_eq!(g.fat_type(), FatType::Fat16);
        assert_eq!((g.clusters(), g.cluster_size()), (32_695, 2048));
        assert_eq!(
            g.cluster_offset(g.cluster(3).unwrap()),
            Some(289 * 512 + 2048)
        );
    }

    #[test]
    fn parses_fat32() {
        let g = Bpb::fat32().parse().unwrap();
        assert_eq!(g.fat_type(), FatType::Fat32);
        assert_eq!(g.clusters(), 129_008);
        assert_eq!(g.root(), Root::Chain(g.cluster(2).unwrap()));
        assert_eq!(g.cluster_offset(g.cluster(2).unwrap()), Some(2064 * 512));
    }

    #[test]
    fn decides_the_type_by_cluster_count_alone() {
        // A FAT32-style boot sector whose clusters number too few for FAT32
        // is a FAT16 volume, and one without root entries is invalid.
        let small = Bpb {
            cluster_sectors: 4,
            ..Bpb::fat32()
        };
        assert_eq!(small.parse(), Err(FatError::BadRootEntries));
    }

    #[test]
    fn reads_the_active_fat_when_mirroring_is_off() {
        let g = Bpb {
            ext_flags: 0x81,
            ..Bpb::fat32()
        };
        assert_eq!(g.parse().unwrap().fat(), (32 + 1016) * 512);
        let bad = Bpb {
            ext_flags: 0x82,
            ..Bpb::fat32()
        };
        assert_eq!(bad.parse(), Err(FatError::BadActiveFat(2)));
        // With mirroring on, the active-FAT bits are ignored.
        let mirrored = Bpb {
            ext_flags: 0x05,
            ..Bpb::fat32()
        };
        assert_eq!(mirrored.parse().unwrap().fat(), 32 * 512);
    }

    #[test]
    fn bounds_clusters() {
        let g = Bpb::floppy().parse().unwrap();
        assert!(g.cluster(0).is_none() && g.cluster(1).is_none());
        assert!(g.cluster(2848).is_some());
        assert!(g.cluster(2849).is_none());
        assert!(g.cluster(u32::MAX).is_none());
    }

    #[test]
    fn rejects_malformed_boot_sectors() {
        let floppy = Bpb::floppy();
        let cases = [
            (
                Bpb {
                    signature: false,
                    ..floppy
                },
                FatError::BadSignature,
            ),
            (
                Bpb {
                    sector_size: 513,
                    ..floppy
                },
                FatError::BadSectorSize(513),
            ),
            (
                Bpb {
                    sector_size: 256,
                    ..floppy
                },
                FatError::BadSectorSize(256),
            ),
            (
                Bpb {
                    cluster_sectors: 0,
                    ..floppy
                },
                FatError::BadClusterSize(0),
            ),
            (
                Bpb {
                    cluster_sectors: 3,
                    ..floppy
                },
                FatError::BadClusterSize(3),
            ),
            (
                Bpb {
                    reserved: 0,
                    ..floppy
                },
                FatError::NoReservedSectors,
            ),
            (Bpb { fats: 0, ..floppy }, FatError::NoFats),
            (
                Bpb {
                    media: 0x12,
                    ..floppy
                },
                FatError::BadMedia(0x12),
            ),
            (
                Bpb {
                    total16: 0,
                    ..floppy
                },
                FatError::NoTotalSectors,
            ),
            (Bpb { fat16: 0, ..floppy }, FatError::NoFatSize),
            (
                Bpb {
                    reserved: 0xFFFF,
                    ..floppy
                },
                FatError::BadLayout,
            ),
            (
                Bpb {
                    total16: 33,
                    ..floppy
                },
                FatError::NoClusters,
            ),
            (
                Bpb {
                    root_entries: 0,
                    ..floppy
                },
                FatError::BadRootEntries,
            ),
            (
                Bpb {
                    fat16: 100,
                    ..Bpb::fat16()
                },
                FatError::FatTooSmall {
                    needed: 65_422,
                    actual: 51_200,
                },
            ),
            (
                Bpb {
                    root_entries: 512,
                    ..Bpb::fat32()
                },
                FatError::BadRootEntries,
            ),
            (
                Bpb {
                    root_cluster: 1,
                    ..Bpb::fat32()
                },
                FatError::BadRootCluster(1),
            ),
            (
                Bpb {
                    root_cluster: 200_000,
                    ..Bpb::fat32()
                },
                FatError::BadRootCluster(200_000),
            ),
            (
                Bpb {
                    version: 1,
                    ..Bpb::fat32()
                },
                FatError::UnsupportedVersion(1),
            ),
        ];
        for (bpb, error) in cases {
            assert_eq!(bpb.parse(), Err(error));
        }
        assert_eq!(Geometry::parse(&[0; 100]), Err(FatError::ShortBootSector));
    }

    #[test]
    fn reads_volumes_from_sources() {
        let mut volume = vec![0_u8; 2880 * 512];
        volume[..BOOT_SECTOR].copy_from_slice(&Bpb::floppy().bytes());
        let g = block_on(read(&volume)).unwrap();
        assert_eq!(g.fat_type(), FatType::Fat12);

        let error = block_on(read(&volume[..BOOT_SECTOR])).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let source = core::error::Error::source(&error).unwrap();
        assert_eq!(
            source.downcast_ref::<FatError>(),
            Some(&FatError::Truncated {
                volume: 2880 * 512,
                source: 512
            })
        );

        let error = block_on(read(&volume[..100])).unwrap_err();
        let source = core::error::Error::source(&error).unwrap();
        assert_eq!(source.downcast_ref(), Some(&FatError::ShortBootSector));
    }
}
