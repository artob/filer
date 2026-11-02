// This is free and unencumbered software released into the public domain.

//! A mounted volume: its superblock and source, the nodes paths resolve
//! to, and what the driver shell serves of them.

use crate::{
    Ext4Error, Options,
    block_map::BlockMap,
    dir::Directory,
    inode::{self, Inode},
    journal,
    superblock::{self, ROOT_INODE, Superblock},
    symlink, xattr,
};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    DirCookie, DriverKey, Entry, Error, ErrorKind, FileId, FileKind, LinkTarget, MetaMask,
    Metadata, OpKind, ReadAt, XattrEntry, XattrNs,
};
use filer_fs::{
    FileSystem, Opened,
    recover::{Overlay, Recovered},
};

/// A volume: what every operation reads from, through what replaying its
/// journal wrote, if it needed recovery.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) sb: Superblock,
    pub(crate) source: Recovered<S>,
}

/// A file, directory, or symlink that a path resolved to: its inode.
#[derive(Clone, Debug)]
pub(crate) struct Node(Arc<Inode>);

impl Node {
    pub(crate) fn inode(&self) -> &Inode {
        &self.0
    }

    pub(crate) fn kind(&self) -> FileKind {
        self.0.kind()
    }

    pub(crate) fn number(&self) -> u32 {
        self.0.number()
    }
}

impl<S: ReadAt> Volume<S> {
    /// Opens the volume in `source`, reading its superblock, and its
    /// journal if it needs recovery, replaying it, unless `options` allow
    /// reading the volume as it is. The superblock is read again if the
    /// journal changes it.
    pub(crate) async fn open(source: S, options: Options) -> Result<Self, Error> {
        let mut sb = superblock::read(&source).await?;
        let mut overlay = Overlay::new();
        if sb.needs_recovery() && !options.allows_dirty() {
            overlay = journal::replay(&source, &sb).await?;
        }
        let source = Recovered::new(source, overlay);
        if !source.overlay().is_empty() {
            sb = superblock::read(&source).await?;
        }
        Ok(Self {
            key: DriverKey::new(),
            sb,
            source,
        })
    }

    /// Reads inode `number`.
    pub(crate) async fn node(&self, number: u32, op: OpKind) -> Result<Node, Error> {
        let inode = inode::read(&self.source, &self.sb, number, op).await?;
        Ok(Node(Arc::new(inode)))
    }

    /// Reads the root directory.
    pub(crate) async fn root(&self, op: OpKind) -> Result<Node, Error> {
        let node = self.node(ROOT_INODE, op).await?;
        if node.kind() != FileKind::Dir {
            return Err(Ext4Error::BadInode { inode: ROOT_INODE }.into_error(op));
        }
        Ok(node)
    }

    /// Returns the block map of `node`.
    pub(crate) fn map(&self, node: &Node, op: OpKind) -> Result<BlockMap, Error> {
        BlockMap::new(node.inode(), &self.sb).map_err(|error| error.into_error(op))
    }

    /// Returns the contents of directory `node`.
    pub(crate) fn directory(&self, node: &Node, op: OpKind) -> Result<Directory, Error> {
        if node.kind() != FileKind::Dir {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        }
        let map = self.map(node, op)?;
        let indexed = node.inode().is_indexed() && self.sb.has_dir_index();
        Directory::new(node.number(), map, node.inode().size(), indexed, &self.sb)
            .map_err(|error| error.into_error(op))
    }

    /// Returns the target of symlink `node`.
    pub(crate) async fn link(&self, node: &Node, op: OpKind) -> Result<LinkTarget, Error> {
        if node.kind() != FileKind::Symlink {
            return Err(Error::new(ErrorKind::NotASymlink, op));
        }
        let target = symlink::read(&self.source, &self.sb, node.inode(), op).await?;
        Ok(LinkTarget::from_bytes(target))
    }

    /// Finds the entry `name` in directory `dir`, matching bytes exactly.
    pub(crate) async fn lookup(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<Node, Error> {
        let directory = self.directory(dir, op)?;
        match directory.lookup(&self.source, &self.sb, name, op).await? {
            Some(entry) => self.node(entry.inode, op).await,
            None => Err(Error::new(ErrorKind::NotFound, op)),
        }
    }
}

impl<S: ReadAt> Volume<S> {
    /// Returns the id of inode `number`: the volume's, and the number.
    fn id(&self, number: u32) -> FileId {
        FileId::new(self.sb.volume_id(), u128::from(number))
    }
}

impl<S: ReadAt + 'static> FileSystem for Volume<S> {
    type Node = Node;
    type Dir = Directory;
    type File = BlockMap;

    fn key(&self) -> &DriverKey {
        &self.key
    }

    fn root(&self, op: OpKind) -> impl Future<Output = Result<Node, Error>> + Send {
        Volume::root(self, op)
    }

    fn kind(&self, node: &Node) -> FileKind {
        node.kind()
    }

    fn lookup(
        &self,
        dir: &Node,
        name: &[u8],
        op: OpKind,
    ) -> impl Future<Output = Result<Node, Error>> + Send {
        Volume::lookup(self, dir, name, op)
    }

    fn read_link(
        &self,
        link: &Node,
        op: OpKind,
    ) -> impl Future<Output = Result<LinkTarget, Error>> + Send {
        self.link(link, op)
    }

    async fn open_dir(&self, dir: &Node, op: OpKind) -> Result<Directory, Error> {
        self.directory(dir, op)
    }

    async fn open_file(&self, file: &Node, op: OpKind) -> Result<BlockMap, Error> {
        self.map(file, op)
    }

    async fn read(
        &self,
        node: &Node,
        map: &BlockMap,
        off: u64,
        max: usize,
        op: OpKind,
    ) -> Result<Vec<u8>, Error> {
        let size = node.inode().size();
        map.read(&self.source, &self.sb, size, off, max, op).await
    }

    async fn metadata(
        &self,
        node: &Node,
        _opened: Option<Opened<'_, Self>>,
        _mask: MetaMask,
        _op: OpKind,
    ) -> Result<Metadata, Error> {
        let inode = node.inode();
        let times = inode.times();
        let mut metadata = Metadata::default();
        metadata.kind = Some(inode.kind());
        metadata.id = Some(self.id(inode.number()));
        metadata.len = Some(inode.size());
        metadata.modified = Some(times.modified);
        metadata.changed = Some(times.changed);
        metadata.accessed = Some(times.accessed);
        metadata.created = times.created;
        metadata.attrs = Some(inode.attrs());
        Ok(metadata)
    }

    async fn read_dir(
        &self,
        _node: &Node,
        dir: &Directory,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> Result<(Vec<Entry>, Option<DirCookie>), Error> {
        // The cookie is the offset of the next entry.
        let mut position = cookie.map_or(0, DirCookie::get);
        let first = position
            .checked_div(u64::from(self.sb.block_size()))
            .unwrap_or(0);
        let mut entries = Vec::new();
        for index in first..dir.blocks(&self.sb) {
            for found in dir.block(&self.source, &self.sb, index, op).await? {
                if found.offset < position {
                    continue;
                }
                if entries.len() >= max {
                    return Ok((entries, Some(DirCookie::new(position))));
                }
                position = found.next;
                if found.is_dot() {
                    continue;
                }
                let name = self.key.entry_name(&found.name).map_err(|_| {
                    Ext4Error::BadDirEntry {
                        dir: dir.inode(),
                        offset: found.offset,
                    }
                    .into_error(op)
                })?;
                let mut listed = Entry::new(name);
                listed.kind = found.kind;
                listed.id = Some(self.id(found.inode));
                entries.push(listed);
            }
        }
        Ok((entries, None))
    }

    async fn xattr_names(&self, node: &Node, op: OpKind) -> Result<Vec<XattrEntry>, Error> {
        let bad = || {
            Ext4Error::BadXattr {
                inode: node.number(),
            }
            .into_error(op)
        };
        xattr::read(&self.source, &self.sb, node.inode(), op)
            .await?
            .iter()
            .map(|xattr| {
                self.key
                    .xattr_entry(xattr.ns, &xattr.name)
                    .map_err(|_| bad())
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
        let xattrs = xattr::read(&self.source, &self.sb, node.inode(), op).await?;
        Ok(xattrs
            .into_iter()
            .find(|xattr| xattr.ns == ns && xattr.name == name)
            .map(|found| {
                let len = u64::try_from(found.value.len()).unwrap_or(u64::MAX);
                (found.value, len)
            }))
    }
}
