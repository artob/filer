// This is free and unencumbered software released into the public domain.

use filer_fs::le;

/// A file system format.
///
/// Every format is listed whatever this build's features, so code naming
/// one compiles everywhere; [`is_enabled`](Self::is_enabled) says whether
/// this build reads it.
///
/// # Examples
///
/// ```
/// use filer_volume::Format;
///
/// let mut head = vec![0; Format::DETECT_LEN];
/// head[..4].copy_from_slice(b"XFSB");
/// assert_eq!(Format::detect(&head), Some(Format::Xfs));
/// assert_eq!(Format::detect(&[0; 512]), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Format {
    /// ext2, ext3, and ext4, read by `filer-ext4`.
    Ext4,
    /// FAT12, FAT16, and FAT32, read by `filer-fat`.
    Fat,
    /// NTFS, read by `filer-ntfs`.
    Ntfs,
    /// XFS, read by `filer-xfs`.
    Xfs,
}

impl Format {
    /// Every format, in order.
    pub const ALL: [Self; 4] = [Self::Ext4, Self::Fat, Self::Ntfs, Self::Xfs];

    /// How many of a volume's first bytes [`detect`](Self::detect) reads:
    /// enough to reach ext's superblock, the last of the structures it
    /// looks at.
    pub const DETECT_LEN: usize = 2048;

    /// Returns the format whose signature `head`, a volume's first bytes,
    /// holds, if any: XFS's and NTFS's magic numbers, ext's in its
    /// superblock at offset 1024, or a FAT boot sector as `filer-fat`
    /// accepts it. A FAT boot sector may be left in a volume formatted
    /// since, so it is tried last.
    ///
    /// [`DETECT_LEN`](Self::DETECT_LEN) bytes suffice; fewer, as of a
    /// smaller volume, may still name one.
    #[must_use]
    pub fn detect(head: &[u8]) -> Option<Self> {
        if head.starts_with(b"XFSB") {
            return Some(Self::Xfs);
        }
        if head.get(3..11) == Some(b"NTFS    ") {
            return Some(Self::Ntfs);
        }
        if is_ext(head) {
            return Some(Self::Ext4);
        }
        is_fat(head).then_some(Self::Fat)
    }

    /// Returns whether this build reads the format: whether its feature,
    /// named as the format is (`ext4`, `fat`, `ntfs`, or `xfs`), is
    /// enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        match self {
            Self::Ext4 => cfg!(feature = "ext4"),
            Self::Fat => cfg!(feature = "fat"),
            Self::Ntfs => cfg!(feature = "ntfs"),
            Self::Xfs => cfg!(feature = "xfs"),
        }
    }

    /// Returns the format's name, in lowercase, as its feature has it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ext4 => "ext4",
            Self::Fat => "fat",
            Self::Ntfs => "ntfs",
            Self::Xfs => "xfs",
        }
    }
}

impl core::fmt::Display for Format {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

/// Returns whether `head` holds an ext superblock: its magic number, a
/// block size of 1 to 64 KiB, and a known revision.
fn is_ext(head: &[u8]) -> bool {
    let at = |offset: usize| head.get(1024..).and_then(|sb| le::u32_at(sb, offset));
    let magic = head.get(1024..).and_then(|sb| le::u16_at(sb, 56));
    magic == Some(0xEF53)
        && at(24).is_some_and(|log_block| log_block <= 6)
        && at(76).is_some_and(|revision| revision <= 1)
}

/// Returns whether `head` holds a FAT boot sector, by the checks
/// `filer-fat` makes of one before reading it.
fn is_fat(head: &[u8]) -> bool {
    let u8_at = |at: usize| head.get(at).copied();
    let u16_at = |at: usize| le::u16_at(head, at);
    let u32_at = |at: usize| le::u32_at(head, at);
    let total = match u16_at(19) {
        Some(0) => u32_at(32),
        total => total.map(u32::from),
    };
    let fat_sectors = match u16_at(22) {
        Some(0) => u32_at(36),
        sectors => sectors.map(u32::from),
    };
    head.get(510..512) == Some(&[0x55, 0xAA])
        && matches!(u16_at(11), Some(512 | 1024 | 2048 | 4096))
        && u8_at(13).is_some_and(u8::is_power_of_two)
        && u16_at(14).is_some_and(|reserved| reserved > 0)
        && u8_at(16).is_some_and(|fats| fats > 0)
        && u8_at(21).is_some_and(|media| media == 0xF0 || media >= 0xF8)
        && total.is_some_and(|total| total > 0)
        && fat_sectors.is_some_and(|sectors| sectors > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};

    /// A FAT12 boot sector, as `mkfs.fat` writes one for a floppy.
    fn fat() -> alloc::vec::Vec<u8> {
        let mut head = vec![0; Format::DETECT_LEN];
        head[..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        head[11..13].copy_from_slice(&512_u16.to_le_bytes());
        head[13] = 1;
        head[14..16].copy_from_slice(&1_u16.to_le_bytes());
        head[16] = 2;
        head[19..21].copy_from_slice(&2880_u16.to_le_bytes());
        head[21] = 0xF0;
        head[22..24].copy_from_slice(&9_u16.to_le_bytes());
        head[510..512].copy_from_slice(&[0x55, 0xAA]);
        head
    }

    fn ext() -> alloc::vec::Vec<u8> {
        let mut head = vec![0; Format::DETECT_LEN];
        head[1024 + 56..1024 + 58].copy_from_slice(&0xEF53_u16.to_le_bytes());
        head[1024 + 76] = 1;
        head
    }

    #[test]
    fn detects_each_format() {
        let mut xfs = vec![0; Format::DETECT_LEN];
        xfs[..4].copy_from_slice(b"XFSB");
        let mut ntfs = vec![0; Format::DETECT_LEN];
        ntfs[3..11].copy_from_slice(b"NTFS    ");
        ntfs[510..512].copy_from_slice(&[0x55, 0xAA]);
        assert_eq!(Format::detect(&xfs), Some(Format::Xfs));
        assert_eq!(Format::detect(&ntfs), Some(Format::Ntfs));
        assert_eq!(Format::detect(&ext()), Some(Format::Ext4));
        assert_eq!(Format::detect(&fat()), Some(Format::Fat));
        // A FAT boot sector left in a volume formatted as ext since.
        let mut both = ext();
        both[..512].copy_from_slice(&fat()[..512]);
        assert_eq!(Format::detect(&both), Some(Format::Ext4));
        assert_eq!(Format::detect(&ext()[..1100]), None);
        assert_eq!(Format::Ntfs.to_string(), "ntfs");
    }

    #[test]
    fn refuses_what_is_not_a_volume() {
        assert_eq!(Format::detect(&[]), None);
        assert_eq!(Format::detect(&[0; Format::DETECT_LEN]), None);
        // An MBR has the boot signature but no BPB.
        let mut mbr = vec![0; 512];
        mbr[446 + 4] = 0x0C;
        mbr[510..512].copy_from_slice(&[0x55, 0xAA]);
        assert_eq!(Format::detect(&mbr), None);
        for (at, value) in [(11, 0), (13, 3), (14, 0), (16, 0), (21, 0x12), (22, 0)] {
            let mut bad = fat();
            bad[at] = value;
            if at == 11 || at == 14 || at == 22 {
                bad[at + 1] = 0;
            }
            assert_eq!(Format::detect(&bad), None, "byte {at}");
        }
        let mut bad = ext();
        bad[1024 + 24] = 7;
        assert_eq!(Format::detect(&bad), None);
    }
}
