// This is free and unencumbered software released into the public domain.

use filer_api::FileKind;
use rustix::fd::OwnedFd;
use std::{
    sync::{Arc, Mutex},
    vec::Vec,
};

/// The payload of a directory handle.
#[derive(Debug)]
pub(crate) struct DirPayload {
    /// The directory, opened with `O_RDONLY | O_DIRECTORY`.
    pub(crate) fd: Arc<OwnedFd>,

    /// The listing in progress, kept between `ReadDir` batches so that a
    /// sequential listing reads the directory once.
    pub(crate) listing: Mutex<Option<Listing>>,
}

impl DirPayload {
    pub(crate) fn new(fd: Arc<OwnedFd>) -> Self {
        Self {
            fd,
            listing: Mutex::new(None),
        }
    }
}

/// A directory stream and how many entries it has yielded, which is the
/// cookie that continues it.
#[derive(Debug)]
pub(crate) struct Listing {
    pub(crate) next: u64,
    pub(crate) dir: rustix::fs::Dir,
}

/// The payload of a file handle.
#[derive(Debug)]
pub(crate) struct FilePayload {
    /// The file, opened with `O_RDONLY`.
    pub(crate) fd: OwnedFd,
}

/// A step of path resolution: an open directory, or another entry named
/// within an open directory. Only directories are opened during the walk;
/// files are opened once resolution ends, and symlinks are read by name.
#[derive(Clone, Debug)]
pub(crate) enum Node {
    Dir(Arc<OwnedFd>),
    Entry {
        dir: Arc<OwnedFd>,
        name: Vec<u8>,
        kind: FileKind,
    },
}
