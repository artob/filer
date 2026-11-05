// This is free and unencumbered software released into the public domain.

//! A mounted image: its superblock and source, the nodes paths resolve
//! to, and what the driver shell serves of them.

use crate::{
    SquashfsError,
    dir::{self, Listing},
    file::File,
    inode::{self, Body, Inode},
    meta::{Cursor, Metadata},
    superblock::{self, Superblock},
    xattr,
};
use alloc::{sync::Arc, vec::Vec};
use core::{future::Future, time::Duration};
use filer_api::{
    DirCookie, DriverKey, Entry, Error, ErrorKind, FileAttrs, FileId, FileKind, LinkTarget,
    MetaMask, Metadata as Meta, OpKind, ReadAt, Timestamp, XattrEntry, XattrNs,
};
use filer_fs::{FileSystem, Lock, Opened};

/// How many bytes of the directory table the listings cached may span in
/// all, those read most recently first.
const LISTED: u32 = 1 << 20;

/// How many listings the cache keeps.
const LISTINGS: usize = 16;

/// An image: what every operation reads from.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) sb: Superblock,
    pub(crate) meta: Metadata<S>,
    /// The fragment blocks most recently decoded, by index.
    pub(crate) fragments: Lock<Vec<(u32, Arc<[u8]>)>>,
    /// The listings most recently read, by where they are and their
    /// lengths: a lookup searches its directory's listing, so paths through
    /// a directory of many entries would decode it again and again.
    listings: Lock<Vec<(Cursor, u32, Listing)>>,
    pub(crate) xattr_table: Option<xattr::Table>,
    /// The names of extended attributes most recently read, by their
    /// index: each is looked up among them.
    pub(crate) named: Lock<Option<(u32, Arc<[xattr::Xattr]>)>>,
}

/// An inode that a path resolved to.
pub(crate) type Node = Arc<Inode>;

impl<S: ReadAt> Volume<S> {
    /// Opens the image in `source`, reading its superblock, and its
    /// extended attributes' id table, if it has one.
    pub(crate) async fn open(source: S) -> Result<Self, Error> {
        let sb = superblock::read(&source).await?;
        let xattr_table = match sb.xattrs() {
            Some(at) => Some(xattr::Table::read(&source, at, sb.bytes_used, OpKind::Open).await?),
            None => None,
        };
        Ok(Self {
            key: DriverKey::new(),
            meta: Metadata::new(source, sb.bytes_used, sb.compressor),
            sb,
            fragments: Lock::new(Vec::new()),
            listings: Lock::new(Vec::new()),
            xattr_table,
            named: Lock::new(None),
        })
    }

    /// Reads the inode that `reference` names.
    pub(crate) async fn node(&self, reference: u64, op: OpKind) -> Result<Node, Error> {
        Ok(Arc::new(
            inode::read(&self.meta, &self.sb, reference, op).await?,
        ))
    }

    /// Reads the root directory.
    pub(crate) async fn root(&self, op: OpKind) -> Result<Node, Error> {
        let bad = || SquashfsError::BadRoot.into_error(op);
        let node = self.node(self.sb.root, op).await.map_err(|error| {
            // An inode that does not read is a bad root; reading the
            // image may fail otherwise too.
            if error.kind() == ErrorKind::Corrupt {
                bad()
            } else {
                error
            }
        })?;
        if node.kind != FileKind::Dir {
            return Err(bad());
        }
        Ok(node)
    }

    /// Returns the entries of directory `dir`, from the cache if it holds
    /// them.
    async fn listing(&self, dir: &Node, op: OpKind) -> Result<Listing, Error> {
        let Body::Dir { listing, len } = dir.body else {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        };
        let hit = self.listings.with(|cache| {
            let found = cache
                .iter()
                .position(|cached| (cached.0, cached.1) == (listing, len))?;
            // The most recent first.
            let cached = cache.remove(found);
            cache.insert(0, cached.clone());
            Some(cached.2)
        });
        if let Some(entries) = hit {
            return Ok(entries);
        }
        let entries = dir::entries(&self.meta, dir, op).await?;
        if len <= LISTED {
            self.listings.with(|cache| {
                // Within the bounds, those read most recently.
                let mut spanned = len;
                cache.truncate(LISTINGS.saturating_sub(1));
                cache.retain(|cached| {
                    spanned = spanned.saturating_add(cached.1);
                    spanned <= LISTED
                });
                cache.insert(0, (listing, len, entries.clone()));
            });
        }
        Ok(entries)
    }

    /// Finds the entry `name` of directory `dir`, matching bytes exactly.
    /// The entry repeats its inode's number and kind, and must agree, so
    /// that listings and lookups do.
    async fn find(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<Node, Error> {
        let entries = self.listing(dir, op).await?;
        let Some(listed) = entries.find(name) else {
            return Err(Error::new(ErrorKind::NotFound, op));
        };
        let node = self.node(listed.reference, op).await?;
        if (node.number, node.kind) != (listed.number, listed.kind) {
            return Err(SquashfsError::BadDirectory { inode: dir.number }.into_error(op));
        }
        Ok(node)
    }

    /// Returns the id of inode `number`: the image's, and the number.
    fn id(&self, number: u32) -> FileId {
        FileId::new(self.sb.volume_id(), u128::from(number))
    }
}

impl<S: ReadAt + 'static> FileSystem for Volume<S> {
    type Node = Node;
    type Dir = Listing;
    type File = File;

    fn key(&self) -> &DriverKey {
        &self.key
    }

    fn root(&self, op: OpKind) -> impl Future<Output = Result<Node, Error>> + Send {
        Volume::root(self, op)
    }

    fn kind(&self, node: &Node) -> FileKind {
        node.kind
    }

    fn lookup(
        &self,
        dir: &Node,
        name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<Node, Error>> + Send {
        self.find(dir, name, op)
    }

    async fn read_link(&self, link: &Node, op: OpKind) -> Result<LinkTarget, Error> {
        match &link.body {
            Body::Symlink(target) => Ok(LinkTarget::from_bytes(target.clone())),
            _ => Err(Error::new(ErrorKind::NotASymlink, op)),
        }
    }

    fn open_dir(
        &self,
        dir: &Node,
        op: OpKind,
    ) -> impl Future<Output = Result<Listing, Error>> + Send {
        self.listing(dir, op)
    }

    async fn open_file(&self, file: &Node, op: OpKind) -> Result<File, Error> {
        match &file.body {
            Body::File(layout) => Ok(File::new(layout.clone())),
            _ => Err(Error::new(ErrorKind::IsADirectory, op)),
        }
    }

    async fn read(
        &self,
        node: &Node,
        file: &File,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        self.read_file(node, file, off, max, op).await
    }

    async fn metadata(
        &self,
        node: &Node,
        _opened: Option<Opened<'_, Self>>,
        _mask: MetaMask,
        _op: OpKind,
    ) -> Result<Meta, Error> {
        // Only the modification time is recorded, in seconds.
        let modified =
            Timestamp::new(i64::from(node.mtime), 0).with_granularity(Duration::from_secs(1));
        let mut metadata = Meta::default();
        metadata.kind = Some(node.kind);
        metadata.id = Some(self.id(node.number));
        metadata.len = Some(node.size());
        metadata.modified = Some(modified);
        metadata.attrs = Some(FileAttrs::empty());
        Ok(metadata)
    }

    async fn read_dir(
        &self,
        node: &Node,
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
            let name = self
                .key
                .entry_name(&listed.name)
                .map_err(|_| SquashfsError::BadDirectory { inode: node.number }.into_error(op))?;
            let mut entry = Entry::new(name);
            entry.kind = Some(listed.kind);
            entry.id = Some(self.id(listed.number));
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

    async fn xattr_names(&self, node: &Node, op: OpKind) -> Result<Vec<XattrEntry>, Error> {
        let xattrs = self.xattrs(node.number, node.xattr, op).await?;
        xattrs
            .iter()
            .map(|xattr| {
                self.key
                    .xattr_entry(xattr.ns, &xattr.name)
                    .map_err(|_| SquashfsError::BadXattr { inode: node.number }.into_error(op))
            })
            .collect()
    }

    async fn xattr(
        &self,
        node: &Node,
        ns: XattrNs,
        name: &[u8],
        _max: usize,
        op: OpKind,
    ) -> Result<Option<(Vec<u8>, u64)>, Error> {
        let xattrs = self.xattrs(node.number, node.xattr, op).await?;
        let Some(found) = xattrs
            .iter()
            .find(|xattr| xattr.ns == ns && xattr.name == name)
        else {
            return Ok(None);
        };
        let value = self.xattr_value(node.number, found, op).await?;
        let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
        Ok(Some((value, len)))
    }
}
