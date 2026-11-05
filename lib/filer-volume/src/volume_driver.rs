// This is free and unencumbered software released into the public domain.

use crate::VolumeFormat;

/// A file system driver: the crate that reads volumes of some formats,
/// each a [`VolumeFormat`].
///
/// Every driver is listed whatever this build's features, so code naming
/// one compiles everywhere; [`is_enabled`](Self::is_enabled) says whether
/// this build has it.
///
/// # Examples
///
/// ```
/// use filer_volume::VolumeDriver;
///
/// let mut head = vec![0; VolumeDriver::DETECT_LEN];
/// head[..4].copy_from_slice(b"XFSB");
/// assert_eq!(VolumeDriver::detect(&head), Some(VolumeDriver::Xfs));
/// assert_eq!(VolumeDriver::detect(&[0; 512]), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum VolumeDriver {
    /// ext2, ext3, and ext4, read by `filer-ext4`.
    Ext4,
    /// FAT12, FAT16, and FAT32, read by `filer-fat`.
    Fat,
    /// NTFS, read by `filer-ntfs`.
    Ntfs,
    /// SquashFS, read by `filer-squashfs`.
    Squashfs,
    /// XFS, read by `filer-xfs`.
    Xfs,
}

impl VolumeDriver {
    /// Every driver, in order.
    pub const ALL: [Self; 5] = [Self::Ext4, Self::Fat, Self::Ntfs, Self::Squashfs, Self::Xfs];

    /// How many of a volume's first bytes [`detect`](Self::detect) reads,
    /// as [`VolumeFormat::DETECT_LEN`].
    pub const DETECT_LEN: usize = VolumeFormat::DETECT_LEN;

    /// Returns the driver for the format whose signature `head`, a
    /// volume's first bytes, holds, if any: the driver of the
    /// [`VolumeFormat`] that [`VolumeFormat::detect`] finds.
    #[must_use]
    pub fn detect(head: &[u8]) -> Option<Self> {
        VolumeFormat::detect(head).map(VolumeFormat::driver)
    }

    /// Returns whether this build has the driver: whether its feature,
    /// named as the driver is (`ext4`, `fat`, `ntfs`, `squashfs`, or
    /// `xfs`), is
    /// enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        match self {
            Self::Ext4 => cfg!(feature = "ext4"),
            Self::Fat => cfg!(feature = "fat"),
            Self::Ntfs => cfg!(feature = "ntfs"),
            Self::Squashfs => cfg!(feature = "squashfs"),
            Self::Xfs => cfg!(feature = "xfs"),
        }
    }

    /// Returns the driver's name, in lowercase, as its feature has it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ext4 => "ext4",
            Self::Fat => "fat",
            Self::Ntfs => "ntfs",
            Self::Squashfs => "squashfs",
            Self::Xfs => "xfs",
        }
    }
}

impl core::fmt::Display for VolumeDriver {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};

    #[test]
    fn detects_the_driver_of_each_format() {
        let mut head = vec![0; VolumeDriver::DETECT_LEN];
        head[..4].copy_from_slice(b"XFSB");
        assert_eq!(VolumeDriver::detect(&head), Some(VolumeDriver::Xfs));
        assert_eq!(VolumeDriver::detect(&[0; 512]), None);
        for format in VolumeFormat::ALL {
            assert!(VolumeDriver::ALL.contains(&format.driver()));
        }
        assert_eq!(VolumeDriver::Ntfs.to_string(), "ntfs");
    }
}
