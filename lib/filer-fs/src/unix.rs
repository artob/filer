// This is free and unencumbered software released into the public domain.

//! What Unix file systems record of a file's kind: the type bits of its
//! mode, and the type byte a directory entry may carry, as ext2's
//! `filetype` feature numbers them and XFS's `FTYPE` does after it.
//!
//! # Examples
//!
//! ```
//! use filer_api::FileKind;
//! use filer_fs::unix;
//!
//! assert_eq!(unix::mode_kind(0o100644), Some(FileKind::File));
//! assert_eq!(unix::entry_kind(2), Ok(Some(FileKind::Dir)));
//! assert_eq!(unix::entry_kind(0), Ok(None));
//! ```

use filer_api::FileKind;

/// Returns the kind of a file of `mode`, by its type bits (`S_IFMT`), or
/// `None` if they name no kind.
#[must_use]
pub const fn mode_kind(mode: u32) -> Option<FileKind> {
    Some(match mode & 0o170_000 {
        0o010_000 => FileKind::Fifo,
        0o020_000 => FileKind::CharDevice,
        0o040_000 => FileKind::Dir,
        0o060_000 => FileKind::BlockDevice,
        0o100_000 => FileKind::File,
        0o120_000 => FileKind::Symlink,
        0o140_000 => FileKind::Socket,
        _ => return None,
    })
}

/// Returns the kind a directory entry's type byte records: `None` for 0,
/// which records none, as entries made without the feature do.
///
/// # Errors
///
/// Returns `byte` if it names no kind, which no valid entry has.
pub const fn entry_kind(byte: u8) -> Result<Option<FileKind>, u8> {
    Ok(Some(match byte {
        0 => return Ok(None),
        1 => FileKind::File,
        2 => FileKind::Dir,
        3 => FileKind::CharDevice,
        4 => FileKind::BlockDevice,
        5 => FileKind::Fifo,
        6 => FileKind::Socket,
        7 => FileKind::Symlink,
        _ => return Err(byte),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_modes_by_their_type_bits() {
        assert_eq!(mode_kind(0o040_755), Some(FileKind::Dir));
        assert_eq!(mode_kind(0o120_777), Some(FileKind::Symlink));
        assert_eq!(mode_kind(0o140_000), Some(FileKind::Socket));
        assert_eq!(mode_kind(0o000_644), None);
        assert_eq!(mode_kind(0o170_000), None);
    }

    #[test]
    fn decodes_entry_types() {
        assert_eq!(entry_kind(7), Ok(Some(FileKind::Symlink)));
        assert_eq!(entry_kind(8), Err(8));
    }
}
