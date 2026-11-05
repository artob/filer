// This is free and unencumbered software released into the public domain.

//! What a directory record, and the System Use entries after it, say of a
//! file: everything Filer reports of it, and where its data is.

use crate::{CdfsError, susp::Times};
use alloc::vec::Vec;
use filer_api::FileKind;

/// A file, directory, symlink, or special file, as its record has it.
#[derive(Clone, Debug)]
pub(crate) struct Node {
    pub(crate) kind: FileKind,
    /// Unique in the image: see [`Node::id`].
    pub(crate) id: u128,
    /// Its size, as Linux reports it: a file's, uncompressed; a
    /// directory's extent's; a symlink target's.
    pub(crate) len: u64,
    pub(crate) times: Times,
    pub(crate) hidden: bool,
    pub(crate) body: Body,
}

/// What a node adds, by its kind.
#[derive(Clone, Debug)]
pub(crate) enum Body {
    /// A directory: where its extent is, and its length.
    Dir {
        at: u64,
        len: u64,
    },
    File(Data),
    Symlink(Vec<u8>),
    /// A device, FIFO, or socket.
    Special,
}

/// Where a file's data is.
#[derive(Clone, Debug)]
pub(crate) struct Data {
    /// The position of its record, which errors name.
    pub(crate) at: u64,
    /// Its extents: each's position and length, in order.
    pub(crate) extents: Vec<(u64, u64)>,
    /// Their lengths' sum: the data as stored.
    pub(crate) stored: u64,
    /// How zisofs compresses it, if it does: the base-2 logarithm of its
    /// block size, and the header's size.
    pub(crate) zisofs: Option<(u8, u8)>,
    /// Why it cannot be read, if it cannot.
    pub(crate) refusal: Option<CdfsError>,
}

impl Node {
    /// The id of a directory: the position of its extent, which its
    /// record in its parent and a relocation's `CL` record share.
    pub(crate) fn dir_id(at: u64) -> u128 {
        DIR | u128::from(at)
    }

    /// The id of a file Rock Ridge numbers, which hard links share.
    pub(crate) fn serial_id(serial: u32) -> u128 {
        SERIAL | u128::from(serial)
    }

    /// The id of any other file: its record's position.
    pub(crate) fn record_id(at: u64) -> u128 {
        RECORD | u128::from(at)
    }
}

/// What tells the three kinds of ids apart, above their 64 bits.
const DIR: u128 = 1 << 64;
const SERIAL: u128 = 2 << 64;
const RECORD: u128 = 3 << 64;
