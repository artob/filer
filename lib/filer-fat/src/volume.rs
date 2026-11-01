// This is free and unencumbered software released into the public domain.

//! A mounted volume: its geometry and source, the nodes paths resolve to,
//! and directory reading and lookup.

use crate::{
    CodePage, FatError, Options,
    chain::{Chain, FatReader, Length},
    dir_entry::{Assembler, Body, DirEntry, Item, MAX_LONG_NAME, SLOT, Step},
    geometry::{Cluster, Geometry, Root},
};
use alloc::{sync::Arc, vec::Vec};
use core::{future::Future, num::NonZeroU32};
use filer_api::{
    BufResult, DriverKey, Error, ErrorKind, FileKind, LinkTarget, OpKind, ReadAt, Resolver,
};

/// The most slots a directory may have, per the specification.
const MAX_DIR_SLOTS: u64 = 65_536;

/// How much of a directory is read at a time.
const DIR_READ: u64 = 16 * 1024;

/// A volume: what every operation reads from.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) geometry: Geometry,
    pub(crate) source: S,
    pub(crate) options: Options,
}

/// A file or directory that a path resolved to.
#[derive(Clone, Debug)]
pub(crate) enum Node {
    /// The root directory, which has no entry.
    Root,

    /// A file or directory with an entry in a directory.
    Entry(Arc<Found>),
}

/// An entry found in a directory.
#[derive(Debug)]
pub(crate) struct Found {
    /// The first cluster of the directory holding the entry, or 0 for a
    /// FAT12/16 root directory.
    pub(crate) dir: u32,
    pub(crate) entry: DirEntry,
}

impl Node {
    pub(crate) fn kind(&self) -> FileKind {
        match self {
            Self::Root => FileKind::Dir,
            Self::Entry(found) => match found.entry.body() {
                Body::Dir(_) => FileKind::Dir,
                Body::EmptyFile | Body::File { .. } => FileKind::File,
            },
        }
    }

    /// Returns a number identifying the node within its volume: 0 for the
    /// root, else one more than its directory's first cluster and its
    /// entry's slot, as files without clusters have nothing else.
    pub(crate) fn id(&self) -> u128 {
        match self {
            Self::Root => 0,
            Self::Entry(found) => {
                let location = (u128::from(found.dir) << 32) | u128::from(found.entry.slot());
                location.saturating_add(1)
            },
        }
    }
}

/// Where a directory's slots are.
#[derive(Clone, Debug)]
pub(crate) enum Extent {
    /// The fixed FAT12/16 root directory: `len` bytes at `offset`.
    Region { offset: u64, len: u64 },

    /// A cluster chain, starting at `first`.
    Chain { first: Cluster, chain: Chain },
}

impl Extent {
    /// Returns the directory's first cluster, or 0 for a fixed root, as
    /// errors and ids name directories.
    pub(crate) fn id(&self) -> u32 {
        match self {
            Self::Region { .. } => 0,
            Self::Chain { first, .. } => first.get(),
        }
    }

    /// Returns the directory's length in bytes.
    pub(crate) fn len(&self) -> u64 {
        match self {
            Self::Region { len, .. } => *len,
            Self::Chain { chain, .. } => chain.len(),
        }
    }

    /// Maps `offset` within the directory to an offset on the volume and
    /// the bytes contiguous from there, or `None` past the end.
    fn locate(&self, geometry: &Geometry, offset: u64) -> Option<(u64, u64)> {
        match self {
            Self::Region { offset: at, len } => {
                Some((at.checked_add(offset)?, len.checked_sub(offset)?))
                    .filter(|&(_, left)| left > 0)
            },
            Self::Chain { chain, .. } => chain.locate(geometry, offset),
        }
    }
}

impl<S: ReadAt> Volume<S> {
    /// Reads where the directory `node` keeps its slots.
    pub(crate) async fn extent(&self, node: &Node, op: OpKind) -> Result<Extent, Error> {
        let first = match node {
            Node::Root => match self.geometry.root() {
                Root::Region { offset, entries } => {
                    let len = u64::from(entries).saturating_mul(SLOT_LEN);
                    return Ok(Extent::Region { offset, len });
                },
                Root::Chain(first) => first,
            },
            Node::Entry(found) => match found.entry.body() {
                Body::Dir(first) => first,
                Body::EmptyFile | Body::File { .. } => {
                    return Err(Error::new(ErrorKind::NotADirectory, op));
                },
            },
        };
        let cluster_size = u64::from(self.geometry.cluster_size());
        let most = MAX_DIR_SLOTS
            .saturating_mul(SLOT_LEN)
            .div_ceil(cluster_size.max(1));
        let most =
            NonZeroU32::new(u32::try_from(most).unwrap_or(u32::MAX)).unwrap_or(NonZeroU32::MIN);
        let mut fat = FatReader::new(&self.source, &self.geometry);
        let chain = Chain::read(&mut fat, first, Length::AtMost(most), op).await?;
        Ok(Extent::Chain { first, chain })
    }

    /// Reads the cluster chain of a file, which must cover its size.
    pub(crate) async fn file_chain(&self, found: &Found, op: OpKind) -> Result<FileData, Error> {
        match found.entry.body() {
            Body::EmptyFile => Ok(FileData::Empty),
            Body::File { first, size } => {
                let cluster_size = self.geometry.cluster_size().max(1);
                let clusters = size.get().div_ceil(cluster_size);
                let clusters = NonZeroU32::new(clusters).unwrap_or(NonZeroU32::MIN);
                let mut fat = FatReader::new(&self.source, &self.geometry);
                let chain = Chain::read(&mut fat, first, Length::Exactly(clusters), op).await?;
                Ok(FileData::Chain {
                    chain,
                    size: u64::from(size.get()),
                })
            },
            Body::Dir(_) => Err(Error::new(ErrorKind::IsADirectory, op)),
        }
    }

    /// Reads up to `max` bytes of `data` at `off`, never past its size.
    pub(crate) async fn read_file(
        &self,
        data: &FileData,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let FileData::Chain { chain, size } = data else {
            return Ok(Vec::new());
        };
        let left = size.saturating_sub(off);
        let Some((at, contiguous)) = chain.locate(&self.geometry, off).filter(|_| left > 0) else {
            return Ok(Vec::new());
        };
        let wanted = left.min(contiguous);
        let wanted = usize::try_from(wanted).map_or(max, |wanted| wanted.min(max));
        self.read_exact(at, wanted, op).await
    }

    /// Reads exactly `len` bytes at `at`. The volume lies within the
    /// source, as opening checked, so running short means the source
    /// shrank, and that error stands as it is.
    async fn read_exact(&self, at: u64, len: usize, op: OpKind) -> Result<Vec<u8>, Error> {
        let BufResult(result, bytes) = self.source.read_exact_at(Vec::with_capacity(len), at).await;
        result.map_err(|error| match error.op() {
            OpKind::Read if op != OpKind::Read => Error::new(error.kind(), op),
            _ => error,
        })?;
        Ok(bytes)
    }

    /// Finds the entry `name` in the directory `node`.
    ///
    /// Names match as the listing shows them, byte for byte, or else
    /// case-insensitively against the long or short name, as FAT preserves
    /// case but does not distinguish it. An exact match wins over a
    /// case-insensitive one; otherwise the first in the directory does.
    pub(crate) async fn lookup(&self, node: &Node, name: &[u8], op: OpKind) -> Result<Node, Error> {
        let extent = self.extent(node, op).await?;
        let mut stream = DirStream::new(self, &extent, 0);
        let folded = core::str::from_utf8(name).ok();
        let mut candidate = None;
        let page = self.options.code_page();
        while let Some((entry, listed)) = stream.next(op).await? {
            match matches(&entry, &listed, page, name, folded) {
                Match::Exact => {
                    return Ok(Node::Entry(Arc::new(Found {
                        dir: extent.id(),
                        entry,
                    })));
                },
                Match::Folded if candidate.is_none() => candidate = Some(entry),
                Match::Folded | Match::No => {},
            }
        }
        candidate
            .map(|entry| {
                Node::Entry(Arc::new(Found {
                    dir: extent.id(),
                    entry,
                }))
            })
            .ok_or_else(|| Error::new(ErrorKind::NotFound, op))
    }
}

const SLOT_LEN: u64 = 32;

/// A file's data.
#[derive(Clone, Debug)]
pub(crate) enum FileData {
    Empty,
    Chain { chain: Chain, size: u64 },
}

impl FileData {
    pub(crate) const fn size(&self) -> u64 {
        match self {
            Self::Empty => 0,
            Self::Chain { size, .. } => *size,
        }
    }
}

/// Reads a directory's files and subdirectories in order, from a slot on,
/// a block at a time.
pub(crate) struct DirStream<'a, S> {
    volume: &'a Volume<S>,
    extent: &'a Extent,
    assembler: Assembler<'a>,
    /// The offset of the next slot in the directory.
    pos: u64,
    /// Bytes read ahead, starting at `pos`.
    block: Vec<u8>,
    used: usize,
}

impl<'a, S: ReadAt> DirStream<'a, S> {
    pub(crate) fn new(volume: &'a Volume<S>, extent: &'a Extent, slot: u32) -> Self {
        Self {
            volume,
            extent,
            assembler: Assembler::new(&volume.geometry, extent.id(), slot),
            pos: u64::from(slot).saturating_mul(SLOT_LEN),
            block: Vec::new(),
            used: 0,
        }
    }

    /// Returns the index of the slot read next.
    pub(crate) fn slot(&self) -> u32 {
        u32::try_from(self.pos / SLOT_LEN).unwrap_or(u32::MAX)
    }

    /// Returns the next file or subdirectory, with the name to list it by,
    /// skipping `.`, `..`, and the volume label, or `None` at the end of
    /// the directory.
    ///
    /// Fails if the directory is inconsistent, or holds a name that does
    /// not decode in the volume's code page, is empty, contains `/` or
    /// NUL, or is `.` or `..`.
    pub(crate) async fn next(&mut self, op: OpKind) -> Result<Option<(DirEntry, Vec<u8>)>, Error> {
        let fail = |error: FatError| error.into_error(op);
        loop {
            if self.pos >= self.extent.len() {
                return Ok(None);
            }
            let slot = self.slot_bytes(op).await?;
            match self.assembler.push(&slot).map_err(fail)? {
                Step::End => {
                    self.pos = self.extent.len();
                    return Ok(None);
                },
                Step::More | Step::Item(Item::Dot | Item::DotDot | Item::Label(_)) => {},
                Step::Item(Item::Entry(entry)) => {
                    let (dir, slot) = (self.extent.id(), entry.slot());
                    let page = self.volume.options.code_page();
                    let name = entry.name(page).map_err(|byte| {
                        fail(FatError::Undecodable {
                            dir,
                            slot,
                            byte,
                            code_page: page.number(),
                        })
                    })?;
                    if !is_valid_name(&name) {
                        return Err(fail(FatError::BadName { dir, slot }));
                    }
                    return Ok(Some((entry, name)));
                },
            }
        }
    }

    /// Returns the slot at `pos` and moves past it, reading a block if
    /// needed.
    async fn slot_bytes(&mut self, op: OpKind) -> Result<[u8; SLOT], Error> {
        let fail = || FatError::BadLayout.into_error(op);
        if self.block.get(self.used..).is_none_or(<[u8]>::is_empty) {
            let geometry = &self.volume.geometry;
            let (at, contiguous) = self.extent.locate(geometry, self.pos).ok_or_else(fail)?;
            let len = contiguous.min(DIR_READ);
            let len = usize::try_from(len).map_err(|_| fail())?;
            self.block = self.volume.read_exact(at, len, op).await?;
            self.used = 0;
        }
        let slot = self
            .block
            .get(self.used..)
            .and_then(<[u8]>::first_chunk::<SLOT>)
            .copied()
            .ok_or_else(fail)?;
        self.used = self.used.saturating_add(SLOT);
        self.pos = self.pos.saturating_add(SLOT_LEN);
        Ok(slot)
    }
}

/// Returns `true` if `name` can name an entry: it is not empty, has no
/// `/` or NUL, and is not `.` or `..`.
fn is_valid_name(name: &[u8]) -> bool {
    !matches!(name, b"" | b"." | b"..") && !name.iter().any(|&b| b == b'/' || b == 0)
}

enum Match {
    Exact,
    Folded,
    No,
}

/// Compares an entry, listed as `listed`, with a name sought, given as
/// bytes and, if UTF-8, as a string. A short alias that does not decode in
/// `page` matches nothing.
fn matches(
    entry: &DirEntry,
    listed: &[u8],
    page: CodePage,
    name: &[u8],
    folded: Option<&str>,
) -> Match {
    if listed == name {
        return Match::Exact;
    }
    let Some(sought) = folded else {
        return Match::No;
    };
    if sought.encode_utf16().nth(MAX_LONG_NAME).is_some() {
        return Match::No;
    }
    let long = entry
        .long_name()
        .map(|long| char::decode_utf16(long.units().iter().copied()));
    let long_matches = long.is_some_and(|long| {
        let mut sought = sought.chars();
        long.map(|unit| unit.ok()).all(|c| {
            c.zip(sought.next())
                .is_some_and(|(a, b)| fold(a) == fold(b))
        }) && sought.next().is_none()
    });
    let short = entry.short_name().display(Default::default(), page);
    let short_matches =
        short.is_ok_and(|short| short.chars().map(fold).eq(sought.chars().map(fold)));
    if long_matches || short_matches {
        Match::Folded
    } else {
        Match::No
    }
}

/// Folds case for comparison: a character's uppercase if that is one
/// character, as FAT's per-character upcase table has it, else itself.
fn fold(c: char) -> char {
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(upper), None) => upper,
        _ => c,
    }
}

/// The [`Resolver`] primitives over a volume. A private type, so that
/// nodes cannot be resolved from without a handle.
pub(crate) struct Walker<'a, S>(pub(crate) &'a Volume<S>);

impl<S: ReadAt> Resolver for Walker<'_, S> {
    type Node = Node;

    fn lookup(
        &self,
        dir: &Node,
        name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<Node, Error>> + Send {
        self.0.lookup(dir, name, op)
    }

    fn kind(&self, node: &Node) -> FileKind {
        node.kind()
    }

    /// FAT has no symlinks, so the walk never asks.
    fn read_link(
        &self,
        _dir: &Node,
        _name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<LinkTarget, Error>> + Send {
        core::future::ready(Err(Error::new(ErrorKind::NotASymlink, op)))
    }
}
