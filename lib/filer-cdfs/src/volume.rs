// This is free and unencumbered software released into the public domain.

//! A mounted image: its source, the tree read, and what the driver shell
//! serves of it.

use crate::{
    CdfsError,
    descriptor::{self, Names, SECTOR, Tree},
    dir::Listing,
    file::File,
    node::{Body, Node},
    record::Record,
    susp,
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    DirCookie, DriverKey, Entry, Error, ErrorKind, FileAttrs, FileId, FileKind, LinkTarget,
    MetaMask, Metadata, OpKind, ReadAt,
};
use filer_fs::{FileSystem, Lock, Opened};

/// How many bytes of directories the listings cached may span in all,
/// those read most recently first.
const LISTED: u64 = 1 << 20;

/// How many listings the cache keeps.
const LISTINGS: usize = 16;

/// An image: what every operation reads from.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) source: S,
    /// The source's size in bytes.
    pub(crate) size: u64,
    /// The logical block size.
    pub(crate) block: u32,
    /// How the tree read records names.
    pub(crate) names: Names,
    /// How many bytes to skip in each record's System Use field, if the
    /// tree has Rock Ridge entries.
    pub(crate) sharing: Option<u8>,
    /// The image's id, which every file's id is in.
    pub(crate) id: u64,
    pub(crate) root: Arc<Node>,
    /// The listings most recently read, by where they are: a lookup
    /// searches its directory's listing, so paths through a directory of
    /// many entries would read it again and again.
    listings: Lock<Vec<(u64, u64, Listing)>>,
}

/// A file, directory, symlink, or special file a path resolved to.
pub(crate) type NodeRef = Arc<Node>;

impl<S: ReadAt> Volume<S> {
    /// Opens the image in `source`: reads its volume descriptors, and the
    /// root of the tree it reads, Rock Ridge's if the primary tree has
    /// its entries, else Joliet's, else ISO 9660:1999's, else the
    /// primary.
    pub(crate) async fn open(source: S) -> Result<Self, Error> {
        let op = OpKind::Open;
        let size = source.size().await?;
        let descriptors = descriptor::read(&source, op).await?;
        let primary = descriptors.primary;
        let root = root(&primary, op)?;
        // The root's `.` record begins with an `SP` entry if the tree has
        // Rock Ridge's.
        let len = usize::try_from(size.saturating_sub(root))
            .unwrap_or(SECTOR)
            .min(SECTOR);
        let bad = || CdfsError::BadRoot.into_error(op);
        let bytes = crate::read::read(&source, root, len, op, bad).await?;
        let sharing = Record::parse(&bytes)
            .filter(|dot| dot.name == [0])
            .and_then(|dot| susp::sharing(dot.system_use));
        let tree = match (sharing, descriptors.joliet, descriptors.enhanced) {
            (Some(_), _, _) => primary,
            (None, Some(joliet), _) => joliet,
            (None, None, Some(enhanced)) => enhanced,
            (None, None, None) => primary,
        };
        let mut volume = Self {
            key: DriverKey::new(),
            source,
            size,
            block: tree.block,
            names: tree.names,
            sharing,
            id: descriptors.id,
            root: Arc::new(placeholder()),
            listings: Lock::new(Vec::new()),
        };
        let root = self::root(&tree, op)?;
        // The root's own `.` record holds its Rock Ridge entries from its
        // start, the `SP` entry first.
        let node = volume
            .directory_at(root, sharing.map(|_| 0), false, op)
            .await
            .map_err(|error| {
                if error.kind() == ErrorKind::Corrupt {
                    CdfsError::BadRoot.into_error(op)
                } else {
                    error
                }
            })?;
        volume.root = Arc::new(node);
        Ok(volume)
    }

    /// Returns the entries of directory `dir`, from the cache if it holds
    /// them.
    async fn listing(&self, dir: &Node, op: OpKind) -> Result<Listing, Error> {
        let Body::Dir { at, len } = dir.body else {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        };
        let hit = self.listings.with(|cache| {
            let found = cache
                .iter()
                .position(|cached| (cached.0, cached.1) == (at, len))?;
            // The most recent first.
            let cached = cache.remove(found);
            cache.insert(0, cached.clone());
            Some(cached.2)
        });
        if let Some(listing) = hit {
            return Ok(listing);
        }
        let listing = self.list(at, len, op).await?;
        if len <= LISTED {
            self.listings.with(|cache| {
                // Within the bounds, those read most recently.
                let mut spanned = len;
                cache.truncate(LISTINGS.saturating_sub(1));
                cache.retain(|cached| {
                    spanned = spanned.saturating_add(cached.1);
                    spanned <= LISTED
                });
                cache.insert(0, (at, len, listing.clone()));
            });
        }
        Ok(listing)
    }

    /// Finds the entry `name` of directory `dir`, matching bytes exactly.
    async fn find(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<NodeRef, Error> {
        let listing = self.listing(dir, op).await?;
        let entry = listing
            .find(name)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, op))?;
        Ok(entry.node.clone())
    }
}

/// Returns where the root directory's extent is in `tree`.
fn root(tree: &Tree, op: OpKind) -> Result<u64, Error> {
    let bad = || CdfsError::BadRoot.into_error(op);
    let record = Record::parse(&tree.root).ok_or_else(bad)?;
    if record.flags & crate::record::DIRECTORY == 0 {
        return Err(bad());
    }
    record.data(tree.block).ok_or_else(bad)
}

/// What stands for the root while it is read.
fn placeholder() -> Node {
    Node {
        kind: FileKind::Dir,
        id: 0,
        len: 0,
        times: susp::Times::default(),
        hidden: false,
        body: Body::Special,
    }
}

impl<S: ReadAt + 'static> FileSystem for Volume<S> {
    type Node = NodeRef;
    type Dir = Listing;
    type File = File;

    fn key(&self) -> &DriverKey {
        &self.key
    }

    async fn root(&self, _op: OpKind) -> Result<NodeRef, Error> {
        Ok(self.root.clone())
    }

    fn kind(&self, node: &NodeRef) -> FileKind {
        node.kind
    }

    fn lookup(
        &self,
        dir: &NodeRef,
        name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<NodeRef, Error>> + Send {
        self.find(dir, name, op)
    }

    async fn read_link(&self, link: &NodeRef, op: OpKind) -> Result<LinkTarget, Error> {
        match &link.body {
            Body::Symlink(target) => Ok(LinkTarget::from_bytes(target.clone())),
            _ => Err(Error::new(ErrorKind::NotASymlink, op)),
        }
    }

    fn open_dir(
        &self,
        dir: &NodeRef,
        op: OpKind,
    ) -> impl Future<Output = Result<Listing, Error>> + Send {
        self.listing(dir, op)
    }

    async fn open_file(&self, file: &NodeRef, op: OpKind) -> Result<File, Error> {
        match &file.body {
            Body::File(data) => self.open_data(data, file.len, op).await,
            _ => Err(Error::new(ErrorKind::IsADirectory, op)),
        }
    }

    async fn read(
        &self,
        node: &NodeRef,
        file: &File,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        match &node.body {
            Body::File(data) => self.read_file(data, node.len, file, off, max, op).await,
            _ => Err(Error::new(ErrorKind::IsADirectory, op)),
        }
    }

    async fn metadata(
        &self,
        node: &NodeRef,
        _opened: Option<Opened<'_, Self>>,
        _mask: MetaMask,
        _op: OpKind,
    ) -> Result<Metadata, Error> {
        let mut attrs = FileAttrs::empty();
        attrs.set(FileAttrs::HIDDEN, node.hidden);
        let compressed = matches!(&node.body, Body::File(data) if data.zisofs.is_some());
        attrs.set(FileAttrs::COMPRESSED, compressed);
        let mut metadata = Metadata::default();
        metadata.kind = Some(node.kind);
        metadata.id = Some(FileId::new(self.id, node.id));
        metadata.len = Some(node.len);
        metadata.modified = node.times.modified;
        metadata.accessed = node.times.accessed;
        metadata.created = node.times.created;
        metadata.changed = node.times.changed;
        metadata.attrs = Some(attrs);
        Ok(metadata)
    }

    async fn read_dir(
        &self,
        node: &NodeRef,
        dir: &Listing,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> Result<(Vec<Entry>, Option<DirCookie>), Error> {
        // The cookie is the index of the next entry.
        let first = cookie.map_or(0, DirCookie::get);
        let first = usize::try_from(first).unwrap_or(usize::MAX);
        let mut entries = Vec::new();
        for listed in dir.entries.iter().skip(first).take(max) {
            let at = match &node.body {
                Body::Dir { at, .. } => *at,
                _ => 0,
            };
            let name = self
                .key
                .entry_name(&listed.name)
                .map_err(|_| CdfsError::BadDirectory { at }.into_error(op))?;
            let mut entry = Entry::new(name);
            entry.kind = Some(listed.node.kind);
            entry.id = Some(FileId::new(self.id, listed.node.id));
            entries.push(entry);
        }
        let next = first.saturating_add(entries.len());
        let more = next < dir.entries.len();
        let cookie = u64::try_from(next)
            .ok()
            .filter(|_| more)
            .map(DirCookie::new);
        Ok((entries, cookie))
    }
}
