// This is free and unencumbered software released into the public domain.

//! The boot sector, and the volume layout it describes.

use crate::NtfsError;
use alloc::vec::Vec;
use filer_api::{BufResult, Error, ErrorKind, OpKind, ReadAt};

/// The size of the boot sector the parser reads.
pub(crate) const BOOT_SECTOR: usize = 512;

/// The largest cluster NTFS allows: 2 MiB.
const MAX_CLUSTER: u32 = 2 * 1024 * 1024;

/// The smallest and largest MFT and index records accepted: whole fixup
/// strides of 512 bytes, up to 64 KiB.
const MIN_RECORD: u32 = 512;
const MAX_RECORD: u32 = 64 * 1024;

/// A volume's layout, from a validated boot sector. Immutable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Geometry {
    sector_size: u32,
    cluster_size: u32,
    /// The number of clusters in the volume.
    clusters: u64,
    /// The first cluster of `$MFT` and of `$MFTMirr`.
    mft: u64,
    mft_mirror: u64,
    record_size: u32,
    index_record_size: u32,
    serial: u64,
    volume_len: u64,
}

fn u8_at(sector: &[u8], at: usize) -> Result<u8, NtfsError> {
    sector.get(at).copied().ok_or(NtfsError::ShortBootSector)
}

fn u16_at(sector: &[u8], at: usize) -> Result<u16, NtfsError> {
    sector
        .get(at..)
        .and_then(<[u8]>::first_chunk)
        .map(|bytes| u16::from_le_bytes(*bytes))
        .ok_or(NtfsError::ShortBootSector)
}

fn u64_at(sector: &[u8], at: usize) -> Result<u64, NtfsError> {
    sector
        .get(at..)
        .and_then(<[u8]>::first_chunk)
        .map(|bytes| u64::from_le_bytes(*bytes))
        .ok_or(NtfsError::ShortBootSector)
}

/// Decodes a record size: a positive count of clusters, or, if negative
/// (n), 2^-n bytes. Returns `None` unless the size is a power of two in
/// the accepted range.
fn decode_record_size(code: i8, cluster_size: u32) -> Option<u32> {
    let size = if code > 0 {
        cluster_size.checked_mul(u32::from(code.unsigned_abs()))?
    } else {
        1u32.checked_shl(u32::from(code.unsigned_abs()))?
    };
    (size.is_power_of_two() && (MIN_RECORD..=MAX_RECORD).contains(&size)).then_some(size)
}

impl Geometry {
    /// Parses and validates a boot sector, of which the first 512 bytes
    /// are read.
    pub(crate) fn parse(sector: &[u8]) -> Result<Self, NtfsError> {
        if u8_at(sector, 510)? != 0x55 || u8_at(sector, 511)? != 0xAA {
            return Err(NtfsError::BadSignature);
        }
        if sector.get(3..11) != Some(b"NTFS    ".as_slice()) {
            return Err(NtfsError::NotNtfs);
        }
        let sector_size = u16_at(sector, 11)?;
        if !matches!(sector_size, 512 | 1024 | 2048 | 4096) {
            return Err(NtfsError::BadSectorSize(sector_size));
        }
        let sector_size = u32::from(sector_size);
        // A code above 0x80 is a negative exponent: 2^(256 - code) sectors.
        let code = u8_at(sector, 13)?;
        let cluster_sectors = match code {
            0 => None,
            1..=0x80 => Some(u32::from(code)),
            _ => 1u32.checked_shl(256u32.saturating_sub(u32::from(code))),
        };
        let cluster_size = cluster_sectors
            .and_then(|sectors| sectors.checked_mul(sector_size))
            .filter(|size| size.is_power_of_two() && *size <= MAX_CLUSTER)
            .ok_or(NtfsError::BadClusterSize(code))?;
        let sectors = u64_at(sector, 0x28)?;
        let volume_len = sectors
            .checked_mul(u64::from(sector_size))
            .ok_or(NtfsError::NoClusters)?;
        let clusters = volume_len.checked_div(u64::from(cluster_size)).unwrap_or(0);
        if clusters == 0 {
            return Err(NtfsError::NoClusters);
        }
        let mft = u64_at(sector, 0x30)?;
        if mft >= clusters {
            return Err(NtfsError::BadMftCluster(mft));
        }
        let mft_mirror = u64_at(sector, 0x38)?;
        if mft_mirror >= clusters {
            return Err(NtfsError::BadMftMirrorCluster(mft_mirror));
        }
        let code = i8::from_le_bytes([u8_at(sector, 0x40)?]);
        let record_size =
            decode_record_size(code, cluster_size).ok_or(NtfsError::BadRecordSize(code))?;
        let code = i8::from_le_bytes([u8_at(sector, 0x44)?]);
        let index_record_size =
            decode_record_size(code, cluster_size).ok_or(NtfsError::BadIndexRecordSize(code))?;
        Ok(Self {
            sector_size,
            cluster_size,
            clusters,
            mft,
            mft_mirror,
            record_size,
            index_record_size,
            serial: u64_at(sector, 0x48)?,
            volume_len,
        })
    }

    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn sector_size(&self) -> u32 {
        self.sector_size
    }

    pub(crate) const fn cluster_size(&self) -> u32 {
        self.cluster_size
    }

    /// Returns the number of clusters in the volume.
    pub(crate) const fn clusters(&self) -> u64 {
        self.clusters
    }

    /// Returns the offset of cluster `lcn`, if it is in the volume.
    pub(crate) fn cluster_offset(&self, lcn: u64) -> Option<u64> {
        (lcn < self.clusters).then(|| lcn.checked_mul(u64::from(self.cluster_size)))?
    }

    /// Returns the first cluster of `$MFT`.
    pub(crate) const fn mft(&self) -> u64 {
        self.mft
    }

    /// Returns the first cluster of `$MFTMirr`.
    #[allow(dead_code, reason = "for reading $MFTMirr when $MFT is damaged, later")]
    pub(crate) const fn mft_mirror(&self) -> u64 {
        self.mft_mirror
    }

    /// Returns the size of an MFT record in bytes.
    pub(crate) const fn record_size(&self) -> u32 {
        self.record_size
    }

    /// Returns the size of an index record in bytes.
    #[allow(dead_code, reason = "used by tests; indexes state their own size")]
    pub(crate) const fn index_record_size(&self) -> u32 {
        self.index_record_size
    }

    /// Returns the volume serial number.
    pub(crate) const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns the volume's size in bytes, per its boot sector.
    #[allow(dead_code, reason = "used by tests")]
    pub(crate) const fn volume_len(&self) -> u64 {
        self.volume_len
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
            return Err(NtfsError::ShortBootSector.into_error(op));
        },
        Err(error) => return Err(error),
    }
    let geometry = Geometry::parse(&sector).map_err(|error| error.into_error(op))?;
    if geometry.volume_len > size {
        let volume = geometry.volume_len;
        return Err(NtfsError::Truncated {
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
    pub(crate) struct Boot {
        pub(crate) sector_size: u16,
        pub(crate) cluster_code: u8,
        pub(crate) sectors: u64,
        pub(crate) mft: u64,
        pub(crate) mft_mirror: u64,
        pub(crate) record_code: i8,
        pub(crate) index_code: i8,
        pub(crate) serial: u64,
        pub(crate) ntfs: bool,
        pub(crate) signature: bool,
    }

    impl Boot {
        /// A 10 MiB volume as `mkntfs` makes it: 4 KiB clusters, 1 KiB MFT
        /// records, 4 KiB index records.
        pub(crate) fn new() -> Self {
            Self {
                sector_size: 512,
                cluster_code: 8,
                sectors: 20479,
                mft: 4,
                mft_mirror: 2,
                record_code: -10,
                index_code: 1,
                serial: 0x0123_4567_89AB_CDEF,
                ntfs: true,
                signature: true,
            }
        }

        pub(crate) fn bytes(&self) -> [u8; BOOT_SECTOR] {
            let mut b = [0_u8; BOOT_SECTOR];
            b[0..3].copy_from_slice(&[0xEB, 0x52, 0x90]);
            if self.ntfs {
                b[3..11].copy_from_slice(b"NTFS    ");
            }
            b[11..13].copy_from_slice(&self.sector_size.to_le_bytes());
            b[13] = self.cluster_code;
            b[21] = 0xF8;
            b[0x28..0x30].copy_from_slice(&self.sectors.to_le_bytes());
            b[0x30..0x38].copy_from_slice(&self.mft.to_le_bytes());
            b[0x38..0x40].copy_from_slice(&self.mft_mirror.to_le_bytes());
            b[0x40] = self.record_code.to_le_bytes()[0];
            b[0x44] = self.index_code.to_le_bytes()[0];
            b[0x48..0x50].copy_from_slice(&self.serial.to_le_bytes());
            if self.signature {
                b[510] = 0x55;
                b[511] = 0xAA;
            }
            b
        }

        fn parse(&self) -> Result<Geometry, NtfsError> {
            Geometry::parse(&self.bytes())
        }
    }

    #[test]
    fn parses_a_volume() {
        let g = Boot::new().parse().unwrap();
        assert_eq!((g.sector_size(), g.cluster_size()), (512, 4096));
        assert_eq!(g.clusters(), 20479 * 512 / 4096);
        assert_eq!((g.mft(), g.mft_mirror()), (4, 2));
        assert_eq!((g.record_size(), g.index_record_size()), (1024, 4096));
        assert_eq!(g.serial(), 0x0123_4567_89AB_CDEF);
        assert_eq!(g.volume_len(), 20479 * 512);
        assert_eq!(g.cluster_offset(4), Some(4 * 4096));
        assert_eq!(g.cluster_offset(g.clusters()), None);
    }

    #[test]
    fn decodes_large_clusters_and_record_sizes() {
        // 0xF4 is 2^(256 - 0xF4) = 4096 sectors: 2 MiB clusters.
        // Index records stay 4 KiB, encoded as 2^12 bytes.
        let boot = Boot {
            cluster_code: 0xF4,
            sectors: 1 << 20,
            index_code: -12,
            ..Boot::new()
        };
        assert_eq!(boot.parse().unwrap().cluster_size(), 2 * 1024 * 1024);
        let boot = Boot {
            cluster_code: 0xF3,
            ..boot
        };
        assert_eq!(boot.parse(), Err(NtfsError::BadClusterSize(0xF3)));

        for (code, size) in [(-9, Some(512)), (-12, Some(4096)), (-16, Some(65536))] {
            let boot = Boot {
                record_code: code,
                ..Boot::new()
            };
            assert_eq!(boot.parse().ok().map(|g| g.record_size()), size);
        }
        for code in [-8, -17, 0, 32, 127, -128] {
            let boot = Boot {
                record_code: code,
                ..Boot::new()
            };
            assert_eq!(boot.parse(), Err(NtfsError::BadRecordSize(code)), "{code}");
            let boot = Boot {
                index_code: code,
                ..Boot::new()
            };
            assert_eq!(
                boot.parse(),
                Err(NtfsError::BadIndexRecordSize(code)),
                "{code}"
            );
        }
        let boot = Boot {
            index_code: 16,
            ..Boot::new()
        };
        assert_eq!(boot.parse().unwrap().index_record_size(), 65536);
    }

    #[test]
    fn rejects_malformed_boot_sectors() {
        let new = Boot::new;
        let cases = [
            (
                Boot {
                    signature: false,
                    ..new()
                },
                NtfsError::BadSignature,
            ),
            (
                Boot {
                    ntfs: false,
                    ..new()
                },
                NtfsError::NotNtfs,
            ),
            (
                Boot {
                    sector_size: 256,
                    ..new()
                },
                NtfsError::BadSectorSize(256),
            ),
            (
                Boot {
                    cluster_code: 0,
                    ..new()
                },
                NtfsError::BadClusterSize(0),
            ),
            (
                Boot {
                    cluster_code: 3,
                    ..new()
                },
                NtfsError::BadClusterSize(3),
            ),
            (
                Boot {
                    sectors: 7,
                    ..new()
                },
                NtfsError::NoClusters,
            ),
            (Boot { mft: 2559, ..new() }, NtfsError::BadMftCluster(2559)),
            (
                Boot {
                    mft_mirror: u64::MAX,
                    ..new()
                },
                NtfsError::BadMftMirrorCluster(u64::MAX),
            ),
            (
                Boot {
                    sectors: u64::MAX,
                    ..new()
                },
                NtfsError::NoClusters,
            ),
        ];
        for (boot, error) in cases {
            assert_eq!(boot.parse(), Err(error));
        }
        assert_eq!(Geometry::parse(&[0; 100]), Err(NtfsError::ShortBootSector));
    }

    #[test]
    fn reads_volumes_from_sources() {
        let boot = Boot::new().bytes();
        let mut volume = vec![0; 20479 * 512];
        volume[..BOOT_SECTOR].copy_from_slice(&boot);
        assert!(block_on(read(volume.as_slice())).is_ok());
        let error = block_on(read(&volume[..4096])).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        let source = core::error::Error::source(&error).unwrap();
        assert_eq!(
            source.downcast_ref(),
            Some(&NtfsError::Truncated {
                volume: 20479 * 512,
                source: 4096
            })
        );
        let error = block_on(read(&boot[..100])).unwrap_err();
        let source = core::error::Error::source(&error).unwrap();
        assert_eq!(source.downcast_ref(), Some(&NtfsError::ShortBootSector));
    }
}
