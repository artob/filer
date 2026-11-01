// This is free and unencumbered software released into the public domain.

//! A mounted volume: its geometry, MFT, and `$UpCase`, the nodes paths
//! resolve to, and lookup.

use crate::{
    NtfsError, Options,
    file_name::FileName,
    geometry::{self, Geometry},
    index::Index,
    mft::{File, Mft},
    record::{AttrType, Body, FileRef},
    stream::Stream,
    upcase::UpCase,
    wtf8,
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{DriverKey, Error, ErrorKind, FileKind, LinkTarget, OpKind, ReadAt, Resolver};

/// The record of the root directory.
pub(crate) const ROOT: u64 = 5;

/// Records below this are system files, hidden from listings and lookups.
pub(crate) const FIRST_USER_RECORD: u64 = 16;

const VOLUME: u64 = 3;
const UPCASE: u64 = 10;
const VOLUME_INFORMATION: AttrType = AttrType(0x70);
const VOLUME_IS_DIRTY: u16 = 0x0001;

/// An index's name for directories.
pub(crate) const I30: &str = "$I30";

/// A volume: what every operation reads from.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) geometry: Geometry,
    pub(crate) source: S,
    pub(crate) mft: Mft,
    pub(crate) upcase: UpCase,
}

/// A file or directory that a path resolved to.
#[derive(Clone, Debug)]
pub(crate) struct Node(pub(crate) Arc<File>);

impl Node {
    pub(crate) fn kind(&self) -> FileKind {
        if self.0.base().is_directory() {
            FileKind::Dir
        } else {
            FileKind::File
        }
    }

    pub(crate) fn number(&self) -> u64 {
        self.0.number()
    }
}

impl<S: ReadAt> Volume<S> {
    /// Opens the volume in `source`: its geometry, its MFT, and its
    /// `$UpCase`; fails if it is of an unknown version, or dirty without
    /// `options` allowing it.
    pub(crate) async fn open(source: S, options: Options) -> Result<Self, Error> {
        let op = OpKind::Open;
        let fail = |error: NtfsError| error.into_error(op);
        let geometry = geometry::read(&source).await?;
        let mft = Mft::load(&source, &geometry).await?;
        let volume = system_file(&mft, &source, &geometry, VOLUME).await?;
        let information = volume
            .stream(&geometry, VOLUME_INFORMATION, "")
            .map_err(fail)?
            .ok_or_else(|| {
                fail(NtfsError::MissingAttribute {
                    record: VOLUME,
                    kind: VOLUME_INFORMATION.0,
                })
            })?;
        let Stream::Resident(information) = information else {
            return Err(fail(NtfsError::BadDataSize { record: VOLUME }));
        };
        let (major, minor, flags) = match information.get(8..12) {
            Some(&[major, minor, low, high]) => (major, minor, u16::from_le_bytes([low, high])),
            _ => return Err(fail(NtfsError::BadDataSize { record: VOLUME })),
        };
        if !matches!(major, 1 | 3) {
            return Err(fail(NtfsError::UnsupportedVersion { major, minor }));
        }
        if flags & VOLUME_IS_DIRTY != 0 && !options.allows_dirty() {
            return Err(fail(NtfsError::Dirty));
        }
        let upcase = system_file(&mft, &source, &geometry, UPCASE).await?;
        let table = upcase
            .stream(&geometry, AttrType::DATA, "")
            .map_err(fail)?
            .ok_or_else(|| fail(NtfsError::BadUpCase))?;
        let len = usize::try_from(table.len()).map_err(|_| fail(NtfsError::BadUpCase))?;
        if len != 0x2_0000 {
            return Err(fail(NtfsError::BadUpCase));
        }
        let bytes = table.read_exact(&source, &geometry, 0, len, op).await?;
        let upcase = UpCase::parse(&bytes).ok_or_else(|| fail(NtfsError::BadUpCase))?;
        Ok(Self {
            key: DriverKey::new(),
            geometry,
            source,
            mft,
            upcase,
        })
    }

    /// Reads the file `reference` names.
    pub(crate) async fn node(&self, reference: FileRef, op: OpKind) -> Result<Node, Error> {
        let file = self
            .mft
            .file(&self.source, &self.geometry, reference, op)
            .await?;
        Ok(Node(Arc::new(file)))
    }

    /// Reads the root directory.
    pub(crate) async fn root(&self, op: OpKind) -> Result<Node, Error> {
        let file = system_file(&self.mft, &self.source, &self.geometry, ROOT).await?;
        if !file.base().is_directory() {
            return Err(NtfsError::BadReference { record: ROOT }.into_error(op));
        }
        Ok(Node(Arc::new(file)))
    }

    /// Returns the index of directory `node`.
    pub(crate) fn index(&self, node: &Node, op: OpKind) -> Result<Index, Error> {
        let fail = |error: NtfsError| error.into_error(op);
        let file = &node.0;
        if !file.base().is_directory() {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        }
        let record = file.number();
        let root = file
            .stream(&self.geometry, AttrType::INDEX_ROOT, I30)
            .map_err(fail)?
            .ok_or_else(|| {
                fail(NtfsError::MissingAttribute {
                    record,
                    kind: AttrType::INDEX_ROOT.0,
                })
            })?;
        let Stream::Resident(root) = root else {
            return Err(fail(NtfsError::BadIndex { record }));
        };
        let allocation = file
            .stream(&self.geometry, AttrType::INDEX_ALLOCATION, I30)
            .map_err(fail)?;
        Index::new(&root, allocation, &self.geometry, record).map_err(fail)
    }

    /// Returns the unnamed `$DATA` of file `node`, empty if it has none.
    pub(crate) fn data(&self, node: &Node, op: OpKind) -> Result<Stream, Error> {
        let fail = |error: NtfsError| error.into_error(op);
        let extents = node.0.extents(AttrType::DATA, "").map_err(fail)?;
        if extents.iter().any(|extent| extent.is_encrypted()) {
            return Err(Error::new(ErrorKind::Unsupported, op));
        }
        if extents.is_empty() {
            return Ok(Stream::Resident(Vec::new()));
        }
        Stream::new(&extents, &self.geometry, node.number()).map_err(fail)
    }

    /// Finds the entry `name` in directory `dir`.
    ///
    /// Names match exactly, or else ignoring case by `$UpCase`, as on
    /// Windows; an exact match wins, and otherwise the first in the index.
    /// System files (records below 16) are never found, as listings hide
    /// them.
    pub(crate) async fn lookup(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<Node, Error> {
        let not_found = || Error::new(ErrorKind::NotFound, op);
        let sought = wtf8::decode(name).ok_or_else(not_found)?;
        let index = self.index(dir, op)?;
        let mut walk = index.walk();
        let mut candidate = None;
        while let Some((file, entry)) = walk.next(&index, &self.source, &self.geometry, op).await? {
            if file.record < FIRST_USER_RECORD {
                continue;
            }
            if *entry.name == *sought {
                return self.node(file, op).await;
            }
            if candidate.is_none() && self.upcase.eq_ignore_case(&entry.name, &sought) {
                candidate = Some(file);
            }
        }
        match candidate {
            Some(file) => self.node(file, op).await,
            None => Err(not_found()),
        }
    }
}

/// Reads system file `number`, whatever its sequence number.
async fn system_file<S: ReadAt + ?Sized>(
    mft: &Mft,
    source: &S,
    geometry: &Geometry,
    number: u64,
) -> Result<File, Error> {
    let op = OpKind::Open;
    let record = mft.read(source, geometry, number, op).await?;
    let reference = FileRef {
        record: number,
        sequence: record.sequence(),
    };
    mft.file(source, geometry, reference, op).await
}

/// Returns the name an index entry lists, as WTF-8, if it is one a
/// listing shows: not a DOS name another entry shadows.
pub(crate) fn listed_name(entry: &FileName) -> Option<Vec<u8>> {
    (!entry.is_dos_only()).then(|| wtf8::encode(&entry.name))
}

/// Returns `true` if `name` can name an entry: it is not empty, has no
/// `/` or NUL, and is not `.` or `..`.
pub(crate) fn is_valid_name(name: &[u8]) -> bool {
    !matches!(name, b"" | b"." | b"..") && !name.iter().any(|&b| b == b'/' || b == 0)
}

/// The `$STANDARD_INFORMATION` of a file: its times and attributes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StandardInformation {
    pub(crate) created: u64,
    pub(crate) modified: u64,
    pub(crate) changed: u64,
    pub(crate) accessed: u64,
    pub(crate) attributes: u32,
}

impl StandardInformation {
    /// Reads the `$STANDARD_INFORMATION` of `file`.
    pub(crate) fn of(file: &File) -> Result<Self, NtfsError> {
        let record = file.number();
        let missing = NtfsError::MissingAttribute {
            record,
            kind: AttrType::STANDARD_INFORMATION.0,
        };
        let extents = file.extents(AttrType::STANDARD_INFORMATION, "")?;
        let Some(Body::Resident(value)) = extents.first().map(|extent| extent.body()) else {
            return Err(missing);
        };
        let u64_at = |at: usize| {
            value
                .get(at..)?
                .first_chunk()
                .map(|b| u64::from_le_bytes(*b))
        };
        let parsed = (|| {
            Some(Self {
                created: u64_at(0)?,
                modified: u64_at(8)?,
                changed: u64_at(0x10)?,
                accessed: u64_at(0x18)?,
                attributes: u32::from_le_bytes(*value.get(0x20..)?.first_chunk()?),
            })
        })();
        parsed.ok_or(NtfsError::BadDataSize { record })
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

    /// Reparse points are not followed yet (NTFS-08), so the walk never
    /// asks.
    fn read_link(
        &self,
        _dir: &Node,
        _name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<LinkTarget, Error>> + Send {
        core::future::ready(Err(Error::new(ErrorKind::NotASymlink, op)))
    }
}
