// This is free and unencumbered software released into the public domain.

//! A mounted volume: its superblock and source, the nodes paths resolve
//! to, and lookup.

use crate::{
    Ext4Error,
    block_map::BlockMap,
    dir::Directory,
    inode::{self, Inode},
    superblock::{self, ROOT_INODE, Superblock},
    symlink,
};
use alloc::sync::Arc;
use core::future::Future;
use filer_api::{DriverKey, Error, ErrorKind, FileKind, LinkTarget, OpKind, ReadAt, Resolver};

/// A volume: what every operation reads from.
pub(crate) struct Volume<S> {
    pub(crate) key: DriverKey,
    pub(crate) sb: Superblock,
    pub(crate) source: S,
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
    /// Opens the volume in `source`, reading its superblock.
    pub(crate) async fn open(source: S) -> Result<Self, Error> {
        let sb = superblock::read(&source).await?;
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
        Directory::new(node.number(), map, node.inode().size(), &self.sb)
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

    async fn read_link(&self, dir: &Node, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
        let node = self.0.lookup(dir, name, op).await?;
        self.0.link(&node, op).await
    }
}
