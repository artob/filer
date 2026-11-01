// This is free and unencumbered software released into the public domain.

use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use filer_api::{Error, ErrorKind, FileKind, LinkTarget, OpKind, Resolver};

/// An inode number: an index into [`Tree::nodes`].
pub(crate) type Ino = usize;

/// The inode number of the root directory.
pub(crate) const ROOT: Ino = 0;

/// One file, directory, or symlink.
#[derive(Debug)]
pub(crate) enum Node {
    /// A directory, mapping raw entry names to inode numbers. Keys are
    /// bytes so that foreign (non-UTF-8) names fit too; the map keeps them
    /// sorted, which gives listings a stable order.
    Dir(BTreeMap<Box<[u8]>, Ino>),

    /// A regular file's contents.
    File(Vec<u8>),

    /// A symlink's target.
    Symlink(String),
}

/// All inodes of one heap; the root directory is [`ROOT`].
#[derive(Debug)]
pub(crate) struct Tree {
    pub(crate) nodes: Vec<Node>,
}

impl Tree {
    /// Returns a tree holding only an empty root directory.
    pub(crate) fn new() -> Self {
        Self {
            nodes: alloc::vec![Node::Dir(BTreeMap::new())],
        }
    }

    /// Returns the child named `name` of directory `dir`, if both exist.
    pub(crate) fn child(&self, dir: Ino, name: &[u8]) -> Option<Ino> {
        match &self.nodes[dir] {
            Node::Dir(entries) => entries.get(name).copied(),
            _ => None,
        }
    }
}

/// The [`Resolver`] primitives, on the private tree so that callers cannot
/// resolve from inode numbers they were never handed.
impl Resolver for Tree {
    type Node = Ino;

    fn lookup(&self, dir: &Ino, name: &[u8], op: OpKind) -> Result<Ino, Error> {
        match &self.nodes[*dir] {
            Node::Dir(entries) => entries
                .get(name)
                .copied()
                .ok_or_else(|| Error::new(ErrorKind::NotFound, op)),
            _ => Err(Error::new(ErrorKind::NotADirectory, op)),
        }
    }

    fn kind(&self, node: &Ino) -> FileKind {
        match self.nodes[*node] {
            Node::Dir(_) => FileKind::Dir,
            Node::File(_) => FileKind::File,
            Node::Symlink(_) => FileKind::Symlink,
        }
    }

    fn read_link(&self, dir: &Ino, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
        match &self.nodes[self.lookup(dir, name, op)?] {
            Node::Symlink(target) => Ok(LinkTarget::Utf8(target.clone())),
            _ => Err(Error::new(ErrorKind::NotASymlink, op)),
        }
    }
}
