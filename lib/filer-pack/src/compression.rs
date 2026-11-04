// This is free and unencumbered software released into the public domain.

use crate::{Codec, Decoder};
use alloc::boxed::Box;
use filer_api::{Error, ErrorKind, OpKind};

/// What an archive is compressed with, as a whole: a tarball, or the
/// later part of an initramfs image. As a [`Codec`], it decodes the
/// stream, with the decoder of the feature named as it is.
///
/// # Examples
///
/// ```
/// use filer_pack::Compression;
///
/// assert_eq!(Compression::detect(&[0x1F, 0x8B, 0x08]), Some(Compression::Gzip));
/// // bzip2 only if a block, or the stream's end, follows its header.
/// assert_eq!(Compression::detect(b"BZh9 not a stream"), None);
/// assert_eq!(Compression::detect(b"ustar"), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Compression {
    /// bzip2, decoded with the `bzip2` feature.
    Bzip2,
    /// gzip, decoded with the `gzip` feature.
    Gzip,
    /// xz, decoded with the `xz` feature.
    Xz,
    /// zstd, decoded with the `zstd` feature.
    Zstd,
}

impl Compression {
    /// Returns what `head`, a stream's first bytes, says it is compressed
    /// with, by its magic number, if anything.
    ///
    /// bzip2's own is two letters and a digit, so its first block's, or
    /// its end's, must follow, as a name in an archive might begin `BZh9`.
    #[must_use]
    pub fn detect(head: &[u8]) -> Option<Self> {
        if head.starts_with(&[0x1F, 0x8B]) {
            Some(Self::Gzip)
        } else if head.starts_with(&[0x28, 0xB5, 0x2F, 0xFD]) {
            Some(Self::Zstd)
        } else if head.starts_with(&[0xFD, b'7', b'z', b'X', b'Z', 0]) {
            Some(Self::Xz)
        } else {
            let [b'B', b'Z', b'h', b'1'..=b'9', rest @ ..] = head else {
                return None;
            };
            let block = rest.starts_with(&[0x31, 0x41, 0x59, 0x26, 0x53, 0x59]);
            let end = rest.starts_with(&[0x17, 0x72, 0x45, 0x38, 0x50, 0x90]);
            (block || end).then_some(Self::Bzip2)
        }
    }

    /// Returns whether this build decodes it: whether its feature, named
    /// as it is, is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        match self {
            Self::Bzip2 => cfg!(feature = "bzip2"),
            Self::Gzip => cfg!(feature = "gzip"),
            Self::Xz => cfg!(feature = "xz"),
            Self::Zstd => cfg!(feature = "zstd"),
        }
    }
}

impl Codec for Compression {
    fn is_stored(&self) -> bool {
        false
    }

    fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
        match self {
            #[cfg(feature = "bzip2")]
            Self::Bzip2 => Ok(Box::new(crate::Bzip2::new())),
            #[cfg(feature = "gzip")]
            Self::Gzip => Ok(Box::new(crate::Gzip::new())),
            #[cfg(feature = "xz")]
            Self::Xz => Ok(Box::new(crate::Xz::new())),
            #[cfg(feature = "zstd")]
            Self::Zstd => Ok(Box::new(crate::Zstd::new())),
            #[allow(unreachable_patterns, reason = "with every compression enabled")]
            _ => Err(Error::new(ErrorKind::Unsupported, OpKind::Read)),
        }
    }
}
