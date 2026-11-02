// This is free and unencumbered software released into the public domain.

use filer_api::FileKind;
use filer_fs::reparse;
use std::{fs::File, sync::Arc};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
};

/// The tag of an `AF_UNIX` socket's reparse point.
const AF_UNIX: u32 = 0x8000_0023;

/// The bit of a reparse tag that marks a name surrogate: a reparse point
/// that stands for another file, as links do.
const NAME_SURROGATE: u32 = 0x2000_0000;

/// The payload of a directory handle.
#[derive(Debug)]
pub(crate) struct DirPayload {
    /// The directory, opened to list and traverse it and to read its
    /// attributes.
    pub(crate) file: Arc<File>,
}

impl DirPayload {
    pub(crate) const fn new(file: Arc<File>) -> Self {
        Self { file }
    }
}

/// The payload of a file handle.
#[derive(Debug)]
pub(crate) struct FilePayload {
    /// The file, opened to read its data and attributes.
    pub(crate) file: File,
}

/// A step of path resolution: an open directory, or another entry, open
/// only to read its attributes and, if it is a reparse point, opened as
/// one rather than followed. Opening a file reopens the entry's handle,
/// so a rename between lookup and open cannot swap the file.
#[derive(Clone, Debug)]
pub(crate) enum Node {
    Dir(Arc<File>),
    Entry { file: Arc<File>, kind: FileKind },
}

impl Node {
    /// Returns the handle the node holds.
    pub(crate) fn file(&self) -> &File {
        match self {
            Self::Dir(file) | Self::Entry { file, .. } => file,
        }
    }
}

/// Returns the kind of a file with the attributes `attributes` and, if it
/// is a reparse point, the tag `tag`.
///
/// Symbolic links, junctions, and WSL symlinks are symlinks; an `AF_UNIX`
/// socket is a socket; other name surrogates stand for files Filer does
/// not follow, so they are [`FileKind::Other`]. Any other reparse point
/// (a cloud placeholder, a compressed or deduplicated file) is the file
/// or directory it is, read through the filter that owns its tag.
pub(crate) const fn kind(attributes: u32, tag: u32) -> FileKind {
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        if reparse::is_link(tag) {
            return FileKind::Symlink;
        }
        if tag == AF_UNIX {
            return FileKind::Socket;
        }
        if tag & NAME_SURROGATE != 0 {
            return FileKind::Other;
        }
    }
    if attributes & FILE_ATTRIBUTE_DIRECTORY != 0 {
        FileKind::Dir
    } else {
        FileKind::File
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_reparse_points_by_tag() {
        let reparse = FILE_ATTRIBUTE_REPARSE_POINT;
        let dir = FILE_ATTRIBUTE_DIRECTORY;
        assert_eq!(kind(0, 0), FileKind::File);
        assert_eq!(kind(dir, 0), FileKind::Dir);
        assert_eq!(kind(reparse, reparse::SYMLINK), FileKind::Symlink);
        assert_eq!(kind(reparse | dir, reparse::MOUNT_POINT), FileKind::Symlink);
        assert_eq!(kind(reparse, AF_UNIX), FileKind::Socket);
        // A name surrogate Filer does not follow.
        assert_eq!(kind(reparse | dir, 0xA000_0010), FileKind::Other);
        // A cloud placeholder directory, and a compressed file.
        assert_eq!(kind(reparse | dir, 0x9000_601A), FileKind::Dir);
        assert_eq!(kind(reparse, 0x8000_0017), FileKind::File);
        // A tag without the attribute means nothing.
        assert_eq!(kind(0, reparse::SYMLINK), FileKind::File);
    }
}
