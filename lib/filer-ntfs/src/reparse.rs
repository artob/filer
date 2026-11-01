// This is free and unencumbered software released into the public domain.

//! Reparse points: `$REPARSE_POINT` values, per [MS-FSCC] §2.1.2.
//!
//! A value starts with a 32-bit tag, a 16-bit data length, and two
//! reserved bytes. Symbolic links and junctions (mount points) hold a
//! substitute name and a print name in UTF-16, located by offsets into a
//! path buffer; symbolic links also hold flags, of which one marks a
//! relative target. WSL symlinks hold a version and their target in UTF-8.

use crate::wtf8;
use alloc::{boxed::Box, vec::Vec};

/// A symbolic link.
pub(crate) const SYMLINK: u32 = 0xA000_000C;

/// A junction (mount point).
pub(crate) const MOUNT_POINT: u32 = 0xA000_0003;

/// A WSL symlink.
pub(crate) const LX_SYMLINK: u32 = 0xA000_001D;

/// A symbolic link's flag marking its target relative.
const SYMLINK_FLAG_RELATIVE: u32 = 0x0000_0001;

/// What a reparse point makes of its file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Reparse {
    /// A symlink to this target, as Filer sees it: `/`-separated, and
    /// absolute (from the volume's root) for an absolute Windows path on a
    /// drive.
    Link(Box<[u8]>),

    /// A reparse point of another tag, whose data Filer cannot read.
    Other(u32),
}

/// Returns `true` if `tag` makes a symlink.
pub(crate) const fn is_link(tag: u32) -> bool {
    matches!(tag, SYMLINK | MOUNT_POINT | LX_SYMLINK)
}

impl Reparse {
    /// Parses a `$REPARSE_POINT` value, or returns `None` if it does not
    /// fit, or a link's target is empty or holds NUL.
    pub(crate) fn parse(value: &[u8]) -> Option<Self> {
        let tag = u32::from_le_bytes(*value.first_chunk()?);
        let len = usize::from(u16::from_le_bytes(*value.get(4..)?.first_chunk()?));
        let data = value.get(8..)?.get(..len)?;
        let target = match tag {
            SYMLINK => {
                let flags = u32::from_le_bytes(*data.get(8..)?.first_chunk()?);
                windows_target(data, 12, flags & SYMLINK_FLAG_RELATIVE != 0)?
            },
            MOUNT_POINT => windows_target(data, 8, false)?,
            LX_SYMLINK => {
                let version = u32::from_le_bytes(*data.first_chunk()?);
                (version == 2).then_some(())?;
                data.get(4..)?.to_vec()
            },
            _ => return Some(Self::Other(tag)),
        };
        if target.is_empty() || target.contains(&0) {
            return None;
        }
        Some(Self::Link(target.into_boxed_slice()))
    }
}

/// Reads the substitute name of a symbolic link or junction, whose path
/// buffer starts at `buffer` in `data`, and makes it a Filer target.
fn windows_target(data: &[u8], buffer: usize, relative: bool) -> Option<Vec<u8>> {
    let field = |at: usize| {
        Some(usize::from(u16::from_le_bytes(
            *data.get(at..)?.first_chunk()?,
        )))
    };
    let (offset, len) = (field(0)?, field(2)?);
    let bytes = data.get(buffer..)?.get(offset..)?.get(..len)?;
    let (units, rest) = bytes.as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    let units: Vec<u16> = units.iter().map(|&unit| u16::from_le_bytes(unit)).collect();
    let mut target: Vec<u8> = wtf8::encode(&units)
        .into_iter()
        .map(|byte| if byte == b'\\' { b'/' } else { byte })
        .collect();
    // `\??\X:\path` is a path on drive X: take it as the volume's root.
    let on_drive = match target.strip_prefix(b"/??/") {
        Some([letter, b':', rest @ ..]) if !relative && letter.is_ascii_alphabetic() => {
            let mut absolute = Vec::with_capacity(rest.len().saturating_add(1));
            if !rest.starts_with(b"/") {
                absolute.push(b'/');
            }
            absolute.extend_from_slice(rest);
            Some(absolute)
        },
        _ => None,
    };
    if let Some(absolute) = on_drive {
        target = absolute;
    }
    Some(target)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;

    fn utf16(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    /// Encodes a symbolic link (`flags` given) or junction (`None`).
    pub(crate) fn windows_link(substitute: &str, print: &str, flags: Option<u32>) -> Vec<u8> {
        let (substitute, print) = (utf16(substitute), utf16(print));
        let mut data = Vec::new();
        let field = |n: usize| u16::try_from(n).unwrap().to_le_bytes();
        data.extend(field(0));
        data.extend(field(substitute.len()));
        data.extend(field(substitute.len()));
        data.extend(field(print.len()));
        if let Some(flags) = flags {
            data.extend(flags.to_le_bytes());
        }
        data.extend(&substitute);
        data.extend(&print);
        let tag = if flags.is_some() {
            SYMLINK
        } else {
            MOUNT_POINT
        };
        let mut value = tag.to_le_bytes().to_vec();
        value.extend(field(data.len()));
        value.extend([0, 0]);
        value.extend(data);
        value
    }

    /// Encodes a WSL symlink.
    pub(crate) fn wsl_link(target: &str) -> Vec<u8> {
        let mut value = LX_SYMLINK.to_le_bytes().to_vec();
        value.extend(u16::try_from(4 + target.len()).unwrap().to_le_bytes());
        value.extend([0, 0]);
        value.extend(2u32.to_le_bytes());
        value.extend(target.as_bytes());
        value
    }

    fn link(value: &[u8]) -> Vec<u8> {
        match Reparse::parse(value) {
            Some(Reparse::Link(target)) => target.to_vec(),
            other => panic!("not a link: {other:?}"),
        }
    }

    #[test]
    fn maps_link_targets() {
        assert_eq!(
            link(&windows_link("Sub\\nested.txt", "Sub\\nested.txt", Some(1))),
            b"Sub/nested.txt"
        );
        assert_eq!(link(&windows_link("..\\up", "..\\up", Some(1))), b"../up");
        assert_eq!(
            link(&windows_link("\\??\\C:\\Users\\x", "C:\\Users\\x", Some(0))),
            b"/Users/x"
        );
        assert_eq!(link(&windows_link("\\??\\D:\\", "D:\\", None)), b"/");
        assert_eq!(
            link(&windows_link("\\??\\UNC\\server\\share", "x", Some(0))),
            b"/??/UNC/server/share"
        );
        // A drive-relative path, from the root.
        assert_eq!(
            link(&windows_link("\\Users", "\\Users", Some(1))),
            b"/Users"
        );
        assert_eq!(link(&wsl_link("/usr/bin/env")), b"/usr/bin/env");
        assert_eq!(link(&wsl_link("é ✓")), "é ✓".as_bytes());
    }

    #[test]
    fn keeps_other_tags_and_rejects_malformed_links() {
        let mut cloud = 0x9000_001Au32.to_le_bytes().to_vec();
        cloud.extend([0, 0, 0, 0]);
        assert_eq!(Reparse::parse(&cloud), Some(Reparse::Other(0x9000_001A)));
        let empty = windows_link("", "", Some(1));
        let mut short = wsl_link("x");
        short.truncate(9);
        let mut version = wsl_link("x");
        version[8] = 1;
        let mut odd = windows_link("ab", "ab", Some(1));
        odd[10] = 3;
        for value in [empty, short, version, odd, vec![1, 2, 3], wsl_link("a\0b")] {
            assert_eq!(Reparse::parse(&value), None, "{value:02X?}");
        }
    }
}
