// This is free and unencumbered software released into the public domain.

use crate::VolumeDriver;
use filer_fs::le;

/// A file system format, as precisely as a volume's first bytes tell it:
/// which of ext2, ext3, and ext4, or of FAT12, FAT16, and FAT32, it is.
///
/// Each format has one [`VolumeDriver`], which reads it. Every format is
/// listed whatever this build's features, so code naming one compiles
/// everywhere; [`is_enabled`](Self::is_enabled) says whether this build
/// reads it.
///
/// # Examples
///
/// ```
/// use filer_volume::{VolumeDriver, VolumeFormat};
///
/// let image = include_bytes!("../tests/data/hello.img");
/// let format = VolumeFormat::detect(image);
/// assert_eq!(format, Some(VolumeFormat::Fat12));
/// assert_eq!(format.map(VolumeFormat::driver), Some(VolumeDriver::Fat));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum VolumeFormat {
    /// ext2: no journal, and none of ext4's features.
    Ext2,
    /// ext3: ext2 with a journal.
    Ext3,
    /// ext4: with any feature ext3 lacks, such as extents, 64-bit block
    /// numbers, flexible block groups, or metadata checksums.
    Ext4,
    /// FAT12: fewer than 4,085 clusters.
    Fat12,
    /// FAT16: fewer than 65,525 clusters.
    Fat16,
    /// FAT32: 65,525 clusters or more.
    Fat32,
    /// ISO 9660, with any of its extensions.
    Iso9660,
    /// NTFS.
    Ntfs,
    /// SquashFS, version 4.0.
    Squashfs,
    /// UDF, of any revision.
    Udf,
    /// XFS, version 4 or 5.
    Xfs,
}

/// ext's feature flags: the compatible one for a journal, the
/// incompatible ones ext3 has, the one of an external journal's device,
/// and the read-only compatible ones ext3 has.
const COMPAT_HAS_JOURNAL: u32 = 0x4;
const INCOMPAT_EXT3: u32 = INCOMPAT_FILETYPE | INCOMPAT_RECOVER | INCOMPAT_META_BG;
const INCOMPAT_FILETYPE: u32 = 0x2;
const INCOMPAT_RECOVER: u32 = 0x4;
const INCOMPAT_JOURNAL_DEV: u32 = 0x8;
const INCOMPAT_META_BG: u32 = 0x10;
const RO_COMPAT_EXT3: u32 = 0x1 | 0x2 | 0x4;

/// The most clusters of a FAT12 volume, and of a FAT16 one.
const MAX_FAT12_CLUSTERS: u64 = 4084;
const MAX_FAT16_CLUSTERS: u64 = 65524;

impl VolumeFormat {
    /// Every format, in order.
    pub const ALL: [Self; 11] = [
        Self::Ext2,
        Self::Ext3,
        Self::Ext4,
        Self::Fat12,
        Self::Fat16,
        Self::Fat32,
        Self::Iso9660,
        Self::Ntfs,
        Self::Squashfs,
        Self::Udf,
        Self::Xfs,
    ];

    /// How many of a volume's first bytes [`detect`](Self::detect) reads:
    /// enough to reach the volume recognition sequence, from sector 16,
    /// ISO 9660's and UDF's, the last of the structures it looks at.
    pub const DETECT_LEN: usize = 48 * 2048;

    /// Returns the format whose signature `head`, a volume's first bytes,
    /// holds, if any: XFS's, SquashFS's, and NTFS's magic numbers, ext's
    /// in its superblock at offset 1024, whose feature flags tell ext2,
    /// ext3, and ext4 apart, UDF's `NSR02` or `NSR03` after `BEA01` in the
    /// volume recognition sequence, from sector 16, ISO 9660's volume
    /// descriptor at sector 16, identified by `CD001`, or a FAT boot sector
    /// as `filer-fat` accepts it, whose count of clusters tells FAT12,
    /// FAT16, and FAT32 apart. A disc's image may carry both UDF and
    /// ISO 9660, as a bridge: it is UDF's, whose names and sizes are
    /// whole, as for DVDs it is the file system that counts.
    /// A FAT boot sector may be left in a volume formatted since, so it is
    /// tried last; an ISO 9660 image's first sectors are free for any
    /// use, as hybrid images' partition tables. An ext journal's own
    /// device is no volume.
    ///
    /// [`DETECT_LEN`](Self::DETECT_LEN) bytes suffice; fewer, as of a
    /// smaller volume, may still name one.
    #[must_use]
    pub fn detect(head: &[u8]) -> Option<Self> {
        if head.starts_with(b"XFSB") {
            return Some(Self::Xfs);
        }
        if head.starts_with(b"hsqs") {
            return Some(Self::Squashfs);
        }
        if head.get(3..11) == Some(b"NTFS    ") {
            return Some(Self::Ntfs);
        }
        if let Some(superblock) = head.get(1024..).filter(|sb| is_ext(sb)) {
            return ext(superblock);
        }
        if is_udf(head) {
            return Some(Self::Udf);
        }
        if head.get(16 * 2048 + 1..16 * 2048 + 6) == Some(b"CD001") {
            return Some(Self::Iso9660);
        }
        fat(head)
    }

    /// Returns the driver that reads the format.
    #[must_use]
    pub const fn driver(self) -> VolumeDriver {
        match self {
            Self::Ext2 | Self::Ext3 | Self::Ext4 => VolumeDriver::Ext4,
            Self::Fat12 | Self::Fat16 | Self::Fat32 => VolumeDriver::Fat,
            Self::Iso9660 => VolumeDriver::Cdfs,
            Self::Ntfs => VolumeDriver::Ntfs,
            Self::Squashfs => VolumeDriver::Squashfs,
            Self::Udf => VolumeDriver::Udf,
            Self::Xfs => VolumeDriver::Xfs,
        }
    }

    /// Returns whether this build reads the format: whether its
    /// [`driver`](Self::driver) is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.driver().is_enabled()
    }

    /// Returns the format's name, in lowercase, as `mkfs` tools name it:
    /// `ext2`, `fat12`, `ntfs`, and so on.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ext2 => "ext2",
            Self::Ext3 => "ext3",
            Self::Ext4 => "ext4",
            Self::Fat12 => "fat12",
            Self::Fat16 => "fat16",
            Self::Fat32 => "fat32",
            Self::Iso9660 => "iso9660",
            Self::Ntfs => "ntfs",
            Self::Squashfs => "squashfs",
            Self::Udf => "udf",
            Self::Xfs => "xfs",
        }
    }
}

impl core::fmt::Display for VolumeFormat {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// Returns whether `head` holds UDF's volume recognition sequence: from
/// sector 16, descriptors of 2 KiB, or of whole sectors larger, each an
/// identifier at offset 1, where `BEA01` begins an extended area that
/// holds `NSR02` or `NSR03`.
fn is_udf(head: &[u8]) -> bool {
    let mut extended = false;
    for sector in 16..48_usize {
        let at = sector.saturating_mul(2048);
        match head.get(at.saturating_add(1)..at.saturating_add(6)) {
            Some(b"BEA01") => extended = true,
            Some(b"NSR02" | b"NSR03") if extended => return true,
            Some(b"TEA01") => extended = false,
            _ => {},
        }
    }
    false
}

/// Returns whether `superblock` is ext's: its magic number, a block size
/// of 1 to 64 KiB, and a known revision.
fn is_ext(superblock: &[u8]) -> bool {
    let at = |offset: usize| le::u32_at(superblock, offset);
    le::u16_at(superblock, 56) == Some(0xEF53)
        && at(24).is_some_and(|log_block| log_block <= 6)
        && at(76).is_some_and(|revision| revision <= 1)
}

/// Returns which ext an ext superblock's feature flags make it.
fn ext(superblock: &[u8]) -> Option<VolumeFormat> {
    let at = |offset: usize| le::u32_at(superblock, offset);
    let (compat, incompat, ro_compat) = (at(0x5C)?, at(0x60)?, at(0x64)?);
    if incompat & INCOMPAT_JOURNAL_DEV != 0 {
        None
    } else if incompat & !INCOMPAT_EXT3 != 0 || ro_compat & !RO_COMPAT_EXT3 != 0 {
        Some(VolumeFormat::Ext4)
    } else if compat & COMPAT_HAS_JOURNAL != 0 {
        Some(VolumeFormat::Ext3)
    } else {
        Some(VolumeFormat::Ext2)
    }
}

/// Returns which FAT `head` holds a boot sector of, by the checks
/// `filer-fat` makes of one before reading it, and its count of clusters.
fn fat(head: &[u8]) -> Option<VolumeFormat> {
    let u8_at = |at: usize| head.get(at).copied();
    let u16_at = |at: usize| le::u16_at(head, at);
    let u32_at = |at: usize| le::u32_at(head, at);
    if head.get(510..512) != Some(&[0x55, 0xAA]) {
        return None;
    }
    let sector = u16_at(11).filter(|size| matches!(size, 512 | 1024 | 2048 | 4096))?;
    let cluster = u8_at(13).filter(|sectors| sectors.is_power_of_two())?;
    let reserved = u16_at(14).filter(|&reserved| reserved > 0)?;
    let fats = u8_at(16).filter(|&fats| fats > 0)?;
    let root_entries = u16_at(17)?;
    u8_at(21).filter(|&media| media == 0xF0 || media >= 0xF8)?;
    let total = match u16_at(19)? {
        0 => u32_at(32)?,
        total => u32::from(total),
    };
    let fat_sectors = match u16_at(22)? {
        0 => u32_at(36)?,
        sectors => u32::from(sectors),
    };
    if total == 0 || fat_sectors == 0 {
        return None;
    }
    // The root directory's sectors, then the clusters after the metadata.
    let root_sectors = u64::from(root_entries)
        .checked_mul(32)?
        .div_ceil(u64::from(sector));
    let meta = u64::from(fats)
        .checked_mul(u64::from(fat_sectors))?
        .checked_add(u64::from(reserved))?
        .checked_add(root_sectors)?;
    let clusters = u64::from(total)
        .checked_sub(meta)?
        .checked_div(u64::from(cluster))?;
    if clusters == 0 {
        None
    } else if clusters <= MAX_FAT12_CLUSTERS {
        Some(VolumeFormat::Fat12)
    } else if clusters <= MAX_FAT16_CLUSTERS {
        Some(VolumeFormat::Fat16)
    } else {
        Some(VolumeFormat::Fat32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec, vec::Vec};

    /// A FAT boot sector of `total` sectors of 512 bytes, one per cluster,
    /// with a FAT of `fat_sectors`, and 512 root entries if not FAT32.
    fn fat(total: u32, fat_sectors: u32, fat32: bool) -> Vec<u8> {
        let mut head = vec![0; VolumeFormat::DETECT_LEN];
        head[..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        head[11..13].copy_from_slice(&512_u16.to_le_bytes());
        head[13] = 1;
        head[14..16].copy_from_slice(&1_u16.to_le_bytes());
        head[16] = 2;
        if !fat32 {
            head[17..19].copy_from_slice(&512_u16.to_le_bytes());
        }
        match u16::try_from(total) {
            Ok(total) => head[19..21].copy_from_slice(&total.to_le_bytes()),
            Err(_) => head[32..36].copy_from_slice(&total.to_le_bytes()),
        }
        head[21] = 0xF8;
        if fat32 {
            head[36..40].copy_from_slice(&fat_sectors.to_le_bytes());
        } else {
            let fat_sectors = u16::try_from(fat_sectors).unwrap();
            head[22..24].copy_from_slice(&fat_sectors.to_le_bytes());
        }
        head[510..512].copy_from_slice(&[0x55, 0xAA]);
        head
    }

    /// An ext superblock with the feature flags `compat`, `incompat`, and
    /// `ro_compat`.
    fn ext(compat: u32, incompat: u32, ro_compat: u32) -> Vec<u8> {
        let mut head = vec![0; VolumeFormat::DETECT_LEN];
        let sb = &mut head[1024..];
        sb[56..58].copy_from_slice(&0xEF53_u16.to_le_bytes());
        sb[76] = 1;
        sb[0x5C..0x60].copy_from_slice(&compat.to_le_bytes());
        sb[0x60..0x64].copy_from_slice(&incompat.to_le_bytes());
        sb[0x64..0x68].copy_from_slice(&ro_compat.to_le_bytes());
        head
    }

    #[test]
    fn tells_ext2_ext3_and_ext4_apart() {
        // As `mke2fs -t` makes each, and an ext3 volume needing recovery.
        let detect = |head: Vec<u8>| VolumeFormat::detect(&head);
        assert_eq!(detect(ext(0x38, 0x2, 0x3)), Some(VolumeFormat::Ext2));
        assert_eq!(detect(ext(0x3C, 0x2, 0x3)), Some(VolumeFormat::Ext3));
        assert_eq!(detect(ext(0x3C, 0x6, 0x3)), Some(VolumeFormat::Ext3));
        assert_eq!(detect(ext(0x3C, 0x2C2, 0x46B)), Some(VolumeFormat::Ext4));
        // One feature past ext3's makes ext4, as does a huge file.
        assert_eq!(detect(ext(0, 0x40, 0)), Some(VolumeFormat::Ext4));
        assert_eq!(detect(ext(0, 0, 0x8)), Some(VolumeFormat::Ext4));
        // An external journal's device.
        assert_eq!(detect(ext(0, 0x8, 0)), None);
    }

    #[test]
    fn tells_fat12_fat16_and_fat32_apart() {
        let detect = |head: Vec<u8>| VolumeFormat::detect(&head);
        // Clusters: what the total leaves after 1 reserved sector, two
        // FATs, and 32 root sectors.
        let clusters = |total: u32, fat_sectors: u32| total - 1 - 2 * fat_sectors - 32;
        assert_eq!(clusters(4084 + 33 + 24, 12), 4084);
        assert_eq!(
            detect(fat(4084 + 33 + 24, 12, false)),
            Some(VolumeFormat::Fat12)
        );
        assert_eq!(
            detect(fat(4085 + 33 + 24, 12, false)),
            Some(VolumeFormat::Fat16)
        );
        assert_eq!(
            detect(fat(65524 + 33 + 512, 256, false)),
            Some(VolumeFormat::Fat16)
        );
        assert_eq!(
            detect(fat(65525 + 1 + 1024, 512, true)),
            Some(VolumeFormat::Fat32)
        );
        // No clusters at all, or fewer sectors than the metadata takes.
        assert_eq!(detect(fat(33 + 24, 12, false)), None);
        assert_eq!(detect(fat(30, 12, false)), None);
    }

    #[test]
    fn detects_the_other_formats() {
        let mut xfs = vec![0; VolumeFormat::DETECT_LEN];
        xfs[..4].copy_from_slice(b"XFSB");
        assert_eq!(VolumeFormat::detect(b"hsqs"), Some(VolumeFormat::Squashfs));
        let mut iso = vec![0; 17 * 2048];
        iso[16 * 2048..16 * 2048 + 6].copy_from_slice(b"\x01CD001");
        assert_eq!(VolumeFormat::detect(&iso), Some(VolumeFormat::Iso9660));
        assert_eq!(VolumeFormat::detect(&iso[..16 * 2048 + 5]), None);
        // UDF, alone and as a bridge with ISO 9660; not without `BEA01`.
        let mut udf = vec![0; 20 * 2048];
        udf[16 * 2048..16 * 2048 + 6].copy_from_slice(b"\0BEA01");
        udf[17 * 2048..17 * 2048 + 6].copy_from_slice(b"\0NSR03");
        assert_eq!(VolumeFormat::detect(&udf), Some(VolumeFormat::Udf));
        let mut bridge = iso.clone();
        bridge.resize(20 * 2048, 0);
        bridge[18 * 2048..18 * 2048 + 6].copy_from_slice(b"\0BEA01");
        bridge[19 * 2048..19 * 2048 + 6].copy_from_slice(b"\0NSR02");
        assert_eq!(VolumeFormat::detect(&bridge), Some(VolumeFormat::Udf));
        udf[16 * 2048 + 1] = b'X';
        assert_eq!(VolumeFormat::detect(&udf), None);
        let mut ntfs = vec![0; VolumeFormat::DETECT_LEN];
        ntfs[3..11].copy_from_slice(b"NTFS    ");
        ntfs[510..512].copy_from_slice(&[0x55, 0xAA]);
        assert_eq!(VolumeFormat::detect(&xfs), Some(VolumeFormat::Xfs));
        assert_eq!(VolumeFormat::detect(&ntfs), Some(VolumeFormat::Ntfs));
        // A FAT boot sector left in a volume formatted as ext since.
        let mut both = ext(0x38, 0x2, 0x3);
        both[..512].copy_from_slice(&fat(2880, 9, false)[..512]);
        assert_eq!(VolumeFormat::detect(&both), Some(VolumeFormat::Ext2));
        assert_eq!(VolumeFormat::detect(&[]), None);
        assert_eq!(VolumeFormat::detect(&[0; VolumeFormat::DETECT_LEN]), None);
    }

    #[test]
    fn names_each_format_and_its_driver() {
        for format in VolumeFormat::ALL {
            assert_eq!(format.is_enabled(), format.driver().is_enabled());
            // ISO 9660's driver is named as Windows names it, CDFS.
            assert!(
                format == VolumeFormat::Iso9660
                    || format
                        .name()
                        .starts_with(format.driver().name().trim_end_matches('4'))
            );
        }
        assert_eq!(VolumeFormat::Fat32.to_string(), "fat32");
        assert_eq!(VolumeFormat::Iso9660.to_string(), "iso9660");
        assert_eq!(VolumeFormat::Iso9660.driver().name(), "cdfs");
        assert_eq!(VolumeFormat::Udf.to_string(), "udf");
    }
}
