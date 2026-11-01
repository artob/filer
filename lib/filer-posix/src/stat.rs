// This is free and unencumbered software released into the public domain.

use filer_api::{FileId, FileKind, Metadata, Timestamp};
use rustix::fs::{FileType, Stat};

/// Converts a [`FileType`] into a [`FileKind`].
pub(crate) fn file_kind(file_type: FileType) -> FileKind {
    match file_type {
        FileType::RegularFile => FileKind::File,
        FileType::Directory => FileKind::Dir,
        FileType::Symlink => FileKind::Symlink,
        FileType::Fifo => FileKind::Fifo,
        FileType::Socket => FileKind::Socket,
        FileType::CharacterDevice => FileKind::CharDevice,
        FileType::BlockDevice => FileKind::BlockDevice,
        _ => FileKind::Other,
    }
}

/// Returns the kind of the file `stat` describes.
pub(crate) fn kind(stat: &Stat) -> FileKind {
    file_kind(FileType::from_raw_mode(stat.st_mode))
}

/// Returns the device `stat`'s file is on.
// The field's type differs by platform and backend.
#[allow(clippy::unnecessary_cast)]
pub(crate) fn device(stat: &Stat) -> u64 {
    stat.st_dev as u64
}

/// Converts `stat` into [`Metadata`]: kind, length, the modification,
/// change, and access times, and the id. Birth times need `statx` or
/// platform-specific fields and are not reported yet.
// The `stat` field types differ by platform and backend, so the casts are
// necessary on some and not on others.
#[allow(clippy::unnecessary_cast)]
pub(crate) fn metadata(stat: &Stat) -> Metadata {
    let time = |secs, nanos| Timestamp::new(secs as i64, nanos as u32);
    let mut metadata = Metadata::default();
    metadata.kind = Some(kind(stat));
    metadata.len = u64::try_from(stat.st_size).ok();
    metadata.modified = Some(time(stat.st_mtime, stat.st_mtime_nsec));
    metadata.changed = Some(time(stat.st_ctime, stat.st_ctime_nsec));
    metadata.accessed = Some(time(stat.st_atime, stat.st_atime_nsec));
    metadata.id = Some(FileId::new(device(stat), stat.st_ino as u128));
    metadata
}
