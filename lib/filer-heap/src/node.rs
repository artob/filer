// This is free and unencumbered software released into the public domain.

use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};

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
