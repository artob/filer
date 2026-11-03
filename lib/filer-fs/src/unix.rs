// This is free and unencumbered software released into the public domain.

//! What Unix file systems record of a file's kind in a directory entry: a
//! type byte, as ext2's `filetype` feature numbers them and XFS's `FTYPE`
//! does after it. The type bits of an inode's mode are
//! [`FileKind::from_unix_mode`]'s, which archives read too.
//!
//! # Examples
//!
//! ```
//! use filer_api::FileKind;
//! use filer_fs::unix;
//!
//! assert_eq!(unix::entry_kind(2), Ok(Some(FileKind::Dir)));
//! assert_eq!(unix::entry_kind(0), Ok(None));
//! ```

use filer_api::FileKind;

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
    fn decodes_entry_types() {
        assert_eq!(entry_kind(7), Ok(Some(FileKind::Symlink)));
        assert_eq!(entry_kind(8), Err(8));
    }
}
