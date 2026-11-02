// This is free and unencumbered software released into the public domain.

//! Interix special files: how Windows Services for UNIX, and ntfs-3g
//! after it, keep symlinks, FIFOs, sockets, and devices on NTFS, as
//! system files whose contents say what they are.
//!
//! The forms, as ntfs-3g writes and reads them: an empty system file is a
//! FIFO, and one of one byte a socket; `IntxLNK\1` and a UTF-16 target
//! make a symlink; `IntxCHR\0` or `IntxBLK\0` and the major and minor
//! numbers (each a little-endian `u64`) make a character or block device.
//! Any other system file is a regular file. The Linux kernel's `ntfs3`
//! reads them all as regular files.

use crate::wtf8;
use alloc::boxed::Box;
use filer_api::FileKind;

/// The longest file that may be a special file, a symlink of a long
/// target; longer ones are regular files, and never read to find out.
pub(crate) const MAX_LEN: u64 = 16 * 1024;

const LINK: &[u8; 8] = b"IntxLNK\x01";
const CHAR_DEVICE: &[u8; 8] = b"IntxCHR\x00";
const BLOCK_DEVICE: &[u8; 8] = b"IntxBLK\x00";

/// What a system file is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Interix {
    /// A symlink, with its target as WTF-8.
    Link(Box<[u8]>),
    Fifo,
    Socket,
    CharDevice,
    BlockDevice,
}

impl Interix {
    /// Classifies the contents of a system file: `None` for a regular
    /// file.
    pub(crate) fn parse(data: &[u8]) -> Option<Self> {
        match data {
            [] => return Some(Self::Fifo),
            [_] => return Some(Self::Socket),
            _ => {},
        }
        let (magic, rest) = data.split_first_chunk::<8>()?;
        match magic {
            LINK => {
                let (units, []) = rest.as_chunks::<2>() else {
                    return None;
                };
                let units: alloc::vec::Vec<u16> =
                    units.iter().map(|unit| u16::from_le_bytes(*unit)).collect();
                let target = wtf8::encode(&units);
                // No POSIX symlink has an empty target, or one with NUL.
                if target.is_empty() || target.contains(&0) {
                    return None;
                }
                Some(Self::Link(target.into_boxed_slice()))
            },
            CHAR_DEVICE if rest.len() == 16 => Some(Self::CharDevice),
            BLOCK_DEVICE if rest.len() == 16 => Some(Self::BlockDevice),
            _ => None,
        }
    }

    pub(crate) const fn kind(&self) -> FileKind {
        match self {
            Self::Link(_) => FileKind::Symlink,
            Self::Fifo => FileKind::Fifo,
            Self::Socket => FileKind::Socket,
            Self::CharDevice => FileKind::CharDevice,
            Self::BlockDevice => FileKind::BlockDevice,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    fn link(target: &str) -> Vec<u8> {
        let mut data = LINK.to_vec();
        data.extend(target.encode_utf16().flat_map(u16::to_le_bytes));
        data
    }

    fn device(magic: &[u8; 8], major: u64, minor: u64) -> Vec<u8> {
        let mut data = magic.to_vec();
        data.extend(major.to_le_bytes());
        data.extend(minor.to_le_bytes());
        data
    }

    #[test]
    fn classifies_special_files() {
        assert_eq!(Interix::parse(b""), Some(Interix::Fifo));
        assert_eq!(Interix::parse(b"\0"), Some(Interix::Socket));
        assert_eq!(
            Interix::parse(&link("../dir/ü.txt")),
            Some(Interix::Link(Box::from("../dir/ü.txt".as_bytes())))
        );
        assert_eq!(
            Interix::parse(&device(CHAR_DEVICE, 1, 3)),
            Some(Interix::CharDevice)
        );
        assert_eq!(
            Interix::parse(&device(BLOCK_DEVICE, 8, 1)),
            Some(Interix::BlockDevice)
        );
    }

    #[test]
    fn keeps_unpaired_surrogates_in_targets() {
        let mut data = LINK.to_vec();
        data.extend([0x00, 0xD8, b'a', 0]);
        assert_eq!(
            Interix::parse(&data),
            Some(Interix::Link(Box::from(&b"\xED\xA0\x80a"[..])))
        );
    }

    #[test]
    fn leaves_other_system_files_regular() {
        let mut odd = link("target");
        odd.push(0);
        let mut short = device(CHAR_DEVICE, 1, 3);
        short.pop();
        for data in [
            &b"desktop.ini"[..],
            b"IntxLNK",
            &LINK[..],
            &link("a\0b"),
            &odd,
            &short,
            &device(b"IntxFOO\0", 1, 3),
        ] {
            assert_eq!(Interix::parse(data), None, "{data:?}");
        }
    }
}
