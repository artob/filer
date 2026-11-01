// This is free and unencumbered software released into the public domain.

use crate::{
    Heap,
    node::{Ino, Node, ROOT, Tree},
};
use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use filer_api::path::RelPath;

/// Builds a [`Heap`] with fixed initial contents.
///
/// Each method adds one entry, creating missing parent directories as
/// needed, like `mkdir -p`. The builder is meant for tests and fixtures,
/// so mistakes panic rather than return errors.
///
/// # Examples
///
/// ```
/// use filer_api::rel;
/// use filer_heap::Heap;
///
/// let heap = Heap::builder()
///     .dir(rel!("empty"))
///     .file(rel!("config/app.toml"), "debug = true")
///     .symlink(rel!("current"), "config")
///     .build();
/// ```
#[derive(Debug)]
#[must_use]
pub struct HeapBuilder {
    tree: Tree,
}

impl HeapBuilder {
    pub(crate) fn new() -> Self {
        Self { tree: Tree::new() }
    }

    /// Adds a directory at `path`, if it does not exist yet.
    ///
    /// # Panics
    ///
    /// Panics if a non-directory is in the way.
    pub fn dir(mut self, path: &RelPath) -> Self {
        let mut dir = ROOT;
        for name in path.components() {
            dir = self.subdir(dir, name.as_str(), path);
        }
        self
    }

    /// Adds a regular file at `path` with `contents`.
    ///
    /// # Panics
    ///
    /// Panics if `path` is empty or already exists, or if a non-directory
    /// is in the way.
    pub fn file(self, path: &RelPath, contents: impl Into<Vec<u8>>) -> Self {
        self.add(path, Node::File(contents.into()))
    }

    /// Adds a symlink at `path` pointing to `target`, which is stored as
    /// given: it may be absolute, contain `..`, or dangle.
    ///
    /// # Panics
    ///
    /// Panics if `path` is empty or already exists, or if a non-directory
    /// is in the way.
    pub fn symlink(self, path: &RelPath, target: &str) -> Self {
        self.add(path, Node::Symlink(String::from(target)))
    }

    /// Returns the heap.
    #[must_use]
    pub fn build(self) -> Heap {
        Heap::new(self.tree)
    }

    fn add(mut self, path: &RelPath, node: Node) -> Self {
        let Some((parent, name)) = path.split_last() else {
            panic!("cannot replace the root directory");
        };
        let mut dir = ROOT;
        for component in parent.components() {
            dir = self.subdir(dir, component.as_str(), path);
        }
        let ino = self.tree.nodes.len();
        self.tree.nodes.push(node);
        let Node::Dir(entries) = &mut self.tree.nodes[dir] else {
            unreachable!("`subdir` returns directories");
        };
        let key: Box<[u8]> = Box::from(name.as_str().as_bytes());
        assert!(
            entries.insert(key, ino).is_none(),
            "{path:?} already exists"
        );
        self
    }

    /// Returns the subdirectory `name` of `dir`, creating it if missing.
    fn subdir(&mut self, dir: Ino, name: &str, path: &RelPath) -> Ino {
        if let Some(child) = self.tree.child(dir, name.as_bytes()) {
            assert!(
                matches!(self.tree.nodes[child], Node::Dir(_)),
                "{name:?} in {path:?} is not a directory",
            );
            return child;
        }
        let ino = self.tree.nodes.len();
        self.tree.nodes.push(Node::Dir(BTreeMap::new()));
        let Node::Dir(entries) = &mut self.tree.nodes[dir] else {
            unreachable!("callers pass directories");
        };
        entries.insert(Box::from(name.as_bytes()), ino);
        ino
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filer_api::rel;

    #[test]
    fn creates_parents_once() {
        let builder = HeapBuilder::new()
            .file(rel!("a/b/c"), "x")
            .file(rel!("a/b/d"), "y")
            .dir(rel!("a/b"));
        // The root, `a`, `b`, `c`, and `d`.
        assert_eq!(builder.tree.nodes.len(), 5);
    }

    #[test]
    #[should_panic(expected = "already exists")]
    fn rejects_duplicates() {
        let _ = HeapBuilder::new()
            .file(rel!("a"), "")
            .symlink(rel!("a"), "b");
    }

    #[test]
    #[should_panic(expected = "is not a directory")]
    fn rejects_files_as_parents() {
        let _ = HeapBuilder::new().file(rel!("a"), "").file(rel!("a/b"), "");
    }
}
