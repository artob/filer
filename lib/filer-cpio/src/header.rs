// This is free and unencumbered software released into the public domain.

//! cpio member headers, in the three formats still written:
//!
//! - **new ASCII** (`newc`, magic `070701`; `070702` with checksums), as
//!   Linux initramfs images and RPM payloads use: 13 fields of 8
//!   hexadecimal digits, 110 bytes in all, with the name and the data each
//!   padded to a multiple of 4 bytes;
//! - **portable ASCII** (`odc`, magic `070707`), as POSIX specifies: 11
//!   octal fields, 76 bytes, without padding;
//! - **old binary** (magic `070707` as a 16-bit word, in either byte
//!   order): 13 words, 26 bytes, with the name and the data each padded to
//!   an even length; times and sizes take two words, the high one first.
//!
//! In every format the name follows the header and ends with NUL, which
//! its length counts, and the data follows the name; a symlink's data is
//! its target. The member `TRAILER!!!` ends an archive.

/// The longest header, `newc`'s.
pub(crate) const MAX_HEADER: usize = 110;

/// The name of the member that ends an archive.
pub(crate) const TRAILER: &[u8] = b"TRAILER!!!";

/// A header's format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Format {
    /// `newc`, with checksums if `crc`.
    Newc { crc: bool },
    /// `odc`.
    Odc,
    /// Old binary, big-endian if `big`.
    Binary { big: bool },
}

impl Format {
    /// Returns the format whose magic number begins `bytes`, if any.
    pub(crate) fn detect(bytes: &[u8]) -> Option<Self> {
        match bytes {
            [b'0', b'7', b'0', b'7', b'0', b'1', ..] => Some(Self::Newc { crc: false }),
            [b'0', b'7', b'0', b'7', b'0', b'2', ..] => Some(Self::Newc { crc: true }),
            [b'0', b'7', b'0', b'7', b'0', b'7', ..] => Some(Self::Odc),
            [0xC7, 0x71, ..] => Some(Self::Binary { big: false }),
            [0x71, 0xC7, ..] => Some(Self::Binary { big: true }),
            _ => None,
        }
    }

    /// Returns the length of the format's header.
    pub(crate) const fn len(self) -> usize {
        match self {
            Self::Newc { .. } => MAX_HEADER,
            Self::Odc => 76,
            Self::Binary { .. } => 26,
        }
    }

    /// Returns the alignment of the format's names and data, from the
    /// start of the archive.
    pub(crate) const fn alignment(self) -> u64 {
        match self {
            Self::Newc { .. } => 4,
            Self::Odc => 1,
            Self::Binary { .. } => 2,
        }
    }
}

/// What a header says of its member.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Header {
    /// The device and inode numbers, which identify hardlinks; `newc`
    /// records the device's major and minor numbers, combined here.
    pub(crate) dev: u64,
    pub(crate) ino: u64,
    pub(crate) mode: u64,
    pub(crate) nlink: u64,
    pub(crate) mtime: u64,
    /// The name's length, with its NUL.
    pub(crate) name_len: u64,
    pub(crate) size: u64,
    /// The sum of the data's bytes, for `newc` with checksums.
    pub(crate) check: u32,
}

/// Parses a header of `format` from `bytes`, which hold at least its
/// length, or returns `None` if a field is malformed.
pub(crate) fn parse(format: Format, bytes: &[u8]) -> Option<Header> {
    match format {
        Format::Newc { .. } => {
            let field = |n: usize| -> Option<u64> {
                let at = n.checked_mul(8)?.checked_add(6)?;
                number(bytes.get(at..)?.get(..8)?, 16)
            };
            let (major, minor) = (field(7)?, field(8)?);
            Some(Header {
                dev: major.checked_shl(32)? | minor,
                ino: field(0)?,
                mode: field(1)?,
                nlink: field(4)?,
                mtime: field(5)?,
                size: field(6)?,
                name_len: field(11)?,
                check: u32::try_from(field(12)?).ok()?,
            })
        },
        Format::Odc => {
            // Each field's offset and width.
            let field = |at: usize, len: usize| number(bytes.get(at..)?.get(..len)?, 8);
            Some(Header {
                dev: field(6, 6)?,
                ino: field(12, 6)?,
                mode: field(18, 6)?,
                nlink: field(36, 6)?,
                mtime: field(48, 11)?,
                name_len: field(59, 6)?,
                size: field(65, 11)?,
                check: 0,
            })
        },
        Format::Binary { big } => {
            let word = |n: usize| -> Option<u64> {
                let at = n.checked_mul(2)?;
                let pair: [u8; 2] = bytes.get(at..)?.get(..2)?.try_into().ok()?;
                Some(u64::from(if big {
                    u16::from_be_bytes(pair)
                } else {
                    u16::from_le_bytes(pair)
                }))
            };
            let long = |n: usize| -> Option<u64> {
                Some(word(n)?.checked_shl(16)? | word(n.checked_add(1)?)?)
            };
            Some(Header {
                dev: word(1)?,
                ino: word(2)?,
                mode: word(3)?,
                nlink: word(6)?,
                mtime: long(8)?,
                name_len: word(10)?,
                size: long(11)?,
                check: 0,
            })
        },
    }
}

/// Parses a field of digits in `radix`, every byte a digit.
fn number(field: &[u8], radix: u32) -> Option<u64> {
    if field.is_empty() {
        return None;
    }
    field.iter().try_fold(0_u64, |n, &b| {
        let digit = char::from(b).to_digit(radix)?;
        n.checked_mul(u64::from(radix))?
            .checked_add(u64::from(digit))
    })
}

/// Returns `at` rounded up to the next multiple of `alignment` from
/// `base`, or `None` on overflow.
pub(crate) fn align(at: u64, base: u64, alignment: u64) -> Option<u64> {
    let from_base = at.checked_sub(base)?;
    base.checked_add(from_base.checked_next_multiple_of(alignment)?)
}

/// Returns whether `bytes` begin as a compressed stream that Linux accepts
/// in an initramfs: gzip, bzip2, LZMA, xz, LZO, LZ4, or zstd.
pub(crate) fn is_compressed(bytes: &[u8]) -> bool {
    [
        &[0x1F, 0x8B][..],
        b"BZh",
        &[0x5D, 0x00, 0x00],
        &[0xFD, b'7', b'z', b'X', b'Z', 0x00],
        &[0x89, b'L', b'Z', b'O'],
        &[0x02, 0x21, 0x4C, 0x18],
        &[0x28, 0xB5, 0x2F, 0xFD],
    ]
    .iter()
    .any(|magic| bytes.starts_with(magic))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        assert_eq!(
            Format::detect(b"070701..."),
            Some(Format::Newc { crc: false })
        );
        assert_eq!(Format::detect(b"070702"), Some(Format::Newc { crc: true }));
        assert_eq!(Format::detect(b"070707"), Some(Format::Odc));
        assert_eq!(
            Format::detect(&[0xC7, 0x71]),
            Some(Format::Binary { big: false })
        );
        assert_eq!(
            Format::detect(&[0x71, 0xC7]),
            Some(Format::Binary { big: true })
        );
        assert_eq!(Format::detect(b"07070"), None);
        assert!(is_compressed(&[0x28, 0xB5, 0x2F, 0xFD, 0x84]));
        assert!(!is_compressed(b"070701"));
    }

    #[test]
    fn parses_each_format() {
        // ino, mode, uid, gid, nlink, mtime, size, dev major and minor,
        // rdev major and minor, name length, check.
        let newc = [
            "070701", "00000002", "0000A1FF", "000003E8", "000003E8", "00000001", "0000000A",
            "0000000D", "00000008", "00000003", "00000000", "00000000", "00000005", "00000000",
        ]
        .concat();
        let header = parse(Format::Newc { crc: false }, newc.as_bytes()).unwrap();
        assert_eq!(header.ino, 2);
        assert_eq!(header.mode, 0o120_777);
        assert_eq!((header.mtime, header.size, header.name_len), (10, 13, 5));
        assert_eq!(header.dev, 8 << 32 | 3);

        // dev, ino, mode, uid, gid, nlink, rdev, mtime, name length, size.
        let odc = [
            "070707",
            "000010",
            "000002",
            "100644",
            "000000",
            "000000",
            "000001",
            "000000",
            "00000000010",
            "000002",
            "00000000007",
        ]
        .concat();
        assert_eq!(odc.len(), Format::Odc.len());
        let header = parse(Format::Odc, odc.as_bytes()).unwrap();
        assert_eq!((header.dev, header.ino, header.mode), (8, 2, 0o100_644));
        assert_eq!((header.mtime, header.name_len, header.size), (8, 2, 7));

        let mut binary = [0_u8; 26];
        for (n, word) in [0x71C7_u16, 8, 2, 0o100_644, 0, 0, 1, 0, 1, 2, 4, 0, 7]
            .into_iter()
            .enumerate()
        {
            binary[2 * n..2 * n + 2].copy_from_slice(&word.to_be_bytes());
        }
        let header = parse(Format::Binary { big: true }, &binary).unwrap();
        assert_eq!(
            (header.mtime, header.name_len, header.size),
            (0x1_0002, 4, 7)
        );
        assert_eq!(number(b"0000000G", 16), None);
        assert_eq!(number(b"000008", 8), None);
    }

    #[test]
    fn aligns_from_the_archive_start() {
        assert_eq!(align(113, 0, 4), Some(116));
        assert_eq!(align(113, 1, 4), Some(113));
        assert_eq!(align(7, 0, 1), Some(7));
        assert_eq!(align(u64::MAX, 0, 4), None);
    }
}
