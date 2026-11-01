// This is free and unencumbered software released into the public domain.

//! Immutable trees of directories, files, and links, for virtual drivers.
//!
//! An in-memory filesystem and an archive index have the same shape: nodes
//! numbered by inode, directories mapping names to inodes, and leaves.
//! [`Tree`] is that shape, generic over what a file leaf holds (its bytes
//! for an in-memory driver; an offset, size, and codec for an archive).
//! Drivers build a tree, then serve it, resolving paths through its
//! [`Resolver`] implementation, which confines them beneath the starting
//! directory.
//!
//! The tree only provides primitives; policies belong to the driver: what
//! a duplicate name means, how hostile names are quarantined, what limits
//! apply. Names must be valid entry names (non-empty, without `/` or NUL,
//! and neither `.` nor `..`), as bytes so that foreign names fit.
//!
//! A driver keeps its tree private: inode numbers are raw references, and
//! code that could reach the tree could resolve from inodes it was never
//! handed as capabilities.
//!
//! # Examples
//!
//! ```
//! use filer_api::tree::{Node, ROOT, Tree};
//!
//! let mut tree = Tree::new();
//! let docs = tree.make_dir(ROOT, b"docs").unwrap();
//! let readme = tree.add(Node::File(b"hello".to_vec()));
//! tree.link(docs, b"README", readme).unwrap();
//! // A hardlink: a second name for the same node.
//! tree.link(ROOT, b"README", readme).unwrap();
//! assert_eq!(tree.child(ROOT, b"README"), tree.child(docs, b"README"));
//! ```

use crate::{Error, ErrorKind, FileKind, LinkTarget, OpKind, Resolver};
use alloc::{boxed::Box, collections::BTreeMap, vec::Vec};

/// An inode number: a node's index in its [`Tree`].
pub type Ino = usize;

/// The inode number of every tree's root directory.
pub const ROOT: Ino = 0;

/// One node of a [`Tree`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Node<F> {
    /// A directory, mapping entry names to inodes. Keys are bytes, so that
    /// foreign names fit, and sorted, so listings have a stable order.
    Dir(BTreeMap<Box<[u8]>, Ino>),

    /// A regular file, with the driver's payload for it.
    File(F),

    /// A symlink, with its target as stored (possibly not UTF-8).
    Symlink(Box<[u8]>),

    /// Another kind of entry, such as a FIFO or a device, which has no
    /// contents to read.
    Special(FileKind),
}

impl<F> Node<F> {
    /// Returns the node's [`FileKind`].
    pub fn kind(&self) -> FileKind {
        match self {
            Self::Dir(_) => FileKind::Dir,
            Self::File(_) => FileKind::File,
            Self::Symlink(_) => FileKind::Symlink,
            Self::Special(kind) => *kind,
        }
    }
}

/// Directories, files, and links, numbered by inode, with a root directory
/// at [`ROOT`]. See the [module docs](self).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree<F> {
    nodes: Vec<Node<F>>,
}

impl<F> Default for Tree<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F> Tree<F> {
    /// Returns a tree holding only an empty root directory.
    #[must_use]
    pub fn new() -> Self {
        Self {
            nodes: alloc::vec![Node::Dir(BTreeMap::new())],
        }
    }

    /// Returns the number of nodes, including unlinked ones.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Returns the node `ino`.
    ///
    /// # Panics
    ///
    /// Panics if `ino` is not a node of this tree.
    #[must_use]
    pub fn node(&self, ino: Ino) -> &Node<F> {
        &self.nodes[ino]
    }

    /// Returns the entries of directory `dir`, or `None` if it is not one.
    #[must_use]
    pub fn entries(&self, dir: Ino) -> Option<&BTreeMap<Box<[u8]>, Ino>> {
        match self.nodes.get(dir) {
            Some(Node::Dir(entries)) => Some(entries),
            _ => None,
        }
    }

    /// Returns the entry `name` of directory `dir`, if both exist.
    #[must_use]
    pub fn child(&self, dir: Ino, name: &[u8]) -> Option<Ino> {
        self.entries(dir)?.get(name).copied()
    }

    /// Adds `node`, not yet linked into any directory, and returns its
    /// inode.
    pub fn add(&mut self, node: Node<F>) -> Ino {
        self.nodes.push(node);
        self.nodes.len() - 1
    }

    /// Links `ino` into directory `dir` as `name`, returning the inode the
    /// name previously referred to, if any. Linking a node under a second
    /// name makes a hardlink.
    ///
    /// # Errors
    ///
    /// [`NotADirectory`](ErrorKind::NotADirectory) if `dir` is not a
    /// directory.
    ///
    /// # Panics
    ///
    /// Panics if `ino` is not a node of this tree.
    pub fn link(&mut self, dir: Ino, name: &[u8], ino: Ino) -> Result<Option<Ino>, ErrorKind> {
        assert!(ino < self.nodes.len(), "no inode {ino} in this tree");
        debug_assert!(is_entry_name(name), "invalid entry name {name:?}");
        match self.nodes.get_mut(dir) {
            Some(Node::Dir(entries)) => Ok(entries.insert(Box::from(name), ino)),
            _ => Err(ErrorKind::NotADirectory),
        }
    }

    /// Returns the subdirectory `name` of `dir`, creating it if it does not
    /// exist, as one step of `mkdir -p`.
    ///
    /// # Errors
    ///
    /// [`NotADirectory`](ErrorKind::NotADirectory) if `dir` is not a
    /// directory or `name` exists and is not one.
    pub fn make_dir(&mut self, dir: Ino, name: &[u8]) -> Result<Ino, ErrorKind> {
        match self.child(dir, name) {
            Some(child) => match self.nodes[child] {
                Node::Dir(_) => Ok(child),
                _ => Err(ErrorKind::NotADirectory),
            },
            None => {
                if self.entries(dir).is_none() {
                    return Err(ErrorKind::NotADirectory);
                }
                let child = self.add(Node::Dir(BTreeMap::new()));
                self.link(dir, name, child)?;
                Ok(child)
            },
        }
    }
}

/// Returns `true` for valid entry names.
fn is_entry_name(name: &[u8]) -> bool {
    !matches!(name, [] | b"." | b"..") && !name.iter().any(|&b| b == b'/' || b == 0)
}

impl<F> Resolver for Tree<F> {
    type Node = Ino;

    fn lookup(&self, dir: &Ino, name: &[u8], op: OpKind) -> Result<Ino, Error> {
        match self.entries(*dir) {
            Some(entries) => entries
                .get(name)
                .copied()
                .ok_or_else(|| Error::new(ErrorKind::NotFound, op)),
            None => Err(Error::new(ErrorKind::NotADirectory, op)),
        }
    }

    fn kind(&self, node: &Ino) -> FileKind {
        self.nodes[*node].kind()
    }

    fn read_link(&self, dir: &Ino, name: &[u8], op: OpKind) -> Result<LinkTarget, Error> {
        match &self.nodes[self.lookup(dir, name, op)?] {
            Node::Symlink(target) => Ok(LinkTarget::from_bytes(target.to_vec())),
            _ => Err(Error::new(ErrorKind::NotASymlink, op)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Resolve, Symlinks};

    #[test]
    fn replaces_and_reports_previous_entries() {
        let mut tree: Tree<u8> = Tree::new();
        let first = tree.add(Node::File(1));
        let second = tree.add(Node::File(2));
        assert_eq!(tree.link(ROOT, b"a", first), Ok(None));
        assert_eq!(tree.link(ROOT, b"a", second), Ok(Some(first)));
        assert_eq!(tree.node(tree.child(ROOT, b"a").unwrap()), &Node::File(2));
    }

    #[test]
    fn makes_directories_once() {
        let mut tree: Tree<()> = Tree::new();
        let a = tree.make_dir(ROOT, b"a").unwrap();
        assert_eq!(tree.make_dir(ROOT, b"a"), Ok(a));
        let file = tree.add(Node::File(()));
        tree.link(a, b"f", file).unwrap();
        assert_eq!(tree.make_dir(a, b"f"), Err(ErrorKind::NotADirectory));
        assert_eq!(tree.make_dir(file, b"x"), Err(ErrorKind::NotADirectory));
        assert_eq!(tree.link(file, b"x", a), Err(ErrorKind::NotADirectory));
        assert_eq!(tree.node_count(), 3);
    }

    #[test]
    fn resolves_symlinks_with_any_target_bytes() {
        let mut tree: Tree<()> = Tree::new();
        let dir = tree.make_dir(ROOT, b"d").unwrap();
        let file = tree.add(Node::File(()));
        tree.link(dir, b"\xFF", file).unwrap();
        let link = tree.add(Node::Symlink(Box::from(&b"d/\xFF"[..])));
        tree.link(ROOT, b"l", link).unwrap();
        let resolved = tree.resolve(&ROOT, b"l", Resolve::default(), true, OpKind::Open);
        assert_eq!(resolved.unwrap(), file);
        let denied = Resolve::default().with_symlinks(Symlinks::Deny);
        let error = tree
            .resolve(&ROOT, b"l", denied, true, OpKind::Open)
            .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::SymlinkLoop);
    }

    #[test]
    fn reports_kinds() {
        let mut tree: Tree<()> = Tree::new();
        let fifo = tree.add(Node::Special(FileKind::Fifo));
        assert_eq!(tree.kind(&fifo), FileKind::Fifo);
        assert_eq!(tree.kind(&ROOT), FileKind::Dir);
    }
}
