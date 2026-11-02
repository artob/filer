// This is free and unencumbered software released into the public domain.

//! A mounted volume: its superblock and source, the nodes paths resolve
//! to, and what the driver shell serves of them.

use crate::{
    Options, XfsError, attr,
    dir::Directory,
    file_map::FileMap,
    inode::{self, Inode},
    log, recover,
    superblock::{self, Superblock},
    symlink,
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

/// A volume: what every operation reads from, through what its log
/// replays.
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
}

impl<S: ReadAt> Volume<S> {
    /// Opens the volume in `source`, reading its superblock, and its log,
    /// replaying what it holds, unless dirty volumes are allowed as they
    /// are. The superblock is read again if the log changes it.
    pub(crate) async fn open(source: S, options: Options) -> Result<Self, Error> {
        let mut sb = superblock::read(&source).await?;
        let mut overlay = Overlay::default();
        if !options.allows_dirty()
            && let Some(dirty) = log::check(&source, &sb).await?
        {
            let log = log::of(&source, &sb)?;
            overlay = recover::replay(&source, &sb, &log, dirty).await?;
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
    pub(crate) async fn node(&self, number: u64, op: OpKind) -> Result<Node, Error> {
        let inode = inode::read(&self.source, &self.sb, number, op).await?;
        Ok(Node(Arc::new(inode)))
    }

    /// Reads the root directory.
    pub(crate) async fn root(&self, op: OpKind) -> Result<Node, Error> {
        let number = self.sb.root_inode();
        let node = self.node(number, op).await?;
        if node.kind() != FileKind::Dir {
            return Err(XfsError::BadInode { inode: number }.into_error(op));
        }
        Ok(node)
    }

    /// Returns the map of file `node`'s data.
    pub(crate) fn map(&self, node: &Node, op: OpKind) -> Result<FileMap, Error> {
        FileMap::data(node.inode(), &self.sb).map_err(|error| error.into_error(op))
    }

    /// Returns the contents of directory `node`.
    pub(crate) fn directory(&self, node: &Node, op: OpKind) -> Result<Directory, Error> {
        if node.kind() != FileKind::Dir {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        }
        Directory::new(node.inode(), &self.sb).map_err(|error| error.into_error(op))
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
    fn id(&self, number: u64) -> FileId {
        FileId::new(self.sb.volume_id(), u128::from(number))
    }
}

impl<S: ReadAt + 'static> FileSystem for Volume<S> {
    type Node = Node;
    type Dir = Directory;
    type File = FileMap;

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

    async fn open_file(&self, file: &Node, op: OpKind) -> Result<FileMap, Error> {
        self.map(file, op)
    }

    async fn read(
        &self,
        node: &Node,
        map: &FileMap,
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
        // The cookie is the offset of the next entry, or below it.
        let mut position = cookie.map_or(0, DirCookie::get);
        let first = dir.block_of(&self.sb, position);
        let mut entries = Vec::new();
        for index in first..dir.blocks(&self.sb) {
            for found in dir.block(&self.source, &self.sb, index, op).await? {
                if found.offset < position {
                    continue;
                }
                if entries.len() >= max {
                    return Ok((entries, Some(DirCookie::new(position))));
                }
                // Offsets ascend, so the next entry is past this one.
                position = found.offset.saturating_add(1);
                if found.is_dot() {
                    continue;
                }
                let name = self.key.entry_name(&found.name).map_err(|_| {
                    XfsError::BadDirEntry {
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
            XfsError::BadXattr {
                inode: node.inode().number(),
            }
            .into_error(op)
        };
        attr::read(&self.source, &self.sb, node.inode(), op)
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
        let found = attr::find(&self.source, &self.sb, node.inode(), ns, name, op).await?;
        Ok(found.map(|value| {
            let len = u64::try_from(value.len()).unwrap_or(u64::MAX);
            (value, len)
        }))
    }
}
