// This is free and unencumbered software released into the public domain.

use crate::{
    Heap,
    node::{Ino, Node, ROOT, Tree},
};
use alloc::{boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use core::fmt::Display;
use filer_api::path::{RelPath, RelPathBuf};

/// Builds a [`Heap`] with fixed initial contents.
///
/// Each method adds one entry, creating missing parent directories as
/// needed, like `mkdir -p`. The builder is meant for tests and fixtures,
/// so it favors convenience: paths may be plain strings, validated when
/// the method runs, or [`rel!`](filer_api::rel) literals, validated at
/// compile time; contents may be anything convertible to bytes. Mistakes
/// panic, pointing at the caller, rather than return errors.
///
/// # Examples
///
/// ```
/// use filer_api::rel;
/// use filer_heap::Heap;
///
/// let heap = Heap::builder()
///     .dir("empty")
///     .file("config/app.toml", "debug = true")
///     .file(rel!("data/blob.bin"), [0xFF, 0x00])
///     .symlink("current", "config")
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
    /// Panics if `path` is not a valid [`RelPath`] or a non-directory is in
    /// the way.
    #[track_caller]
    pub fn dir<P>(mut self, path: P) -> Self
    where
        P: TryInto<RelPathBuf>,
        P::Error: Display,
    {
        let path = parse(path);
        let mut dir = ROOT;
        for name in path.components() {
            dir = self.subdir(dir, name.as_str(), &path);
        }
        self
    }

    /// Adds a regular file at `path` with `contents`, such as a `&str`, a
    /// byte string, a byte array, or a `Vec<u8>`.
    ///
    /// # Panics
    ///
    /// Panics if `path` is not a valid [`RelPath`], is empty, or already
    /// exists, or if a non-directory is in the way.
    #[track_caller]
    pub fn file<P>(self, path: P, contents: impl Into<Vec<u8>>) -> Self
    where
        P: TryInto<RelPathBuf>,
        P::Error: Display,
    {
        self.add(&parse(path), Node::File(contents.into()))
    }

    /// Adds a symlink at `path` pointing to `target`, which is stored as
    /// given: it may be absolute, contain `..`, or dangle.
    ///
    /// # Panics
    ///
    /// Panics if `path` is not a valid [`RelPath`], is empty, or already
    /// exists, or if a non-directory is in the way.
    #[track_caller]
    pub fn symlink<P>(self, path: P, target: impl Into<String>) -> Self
    where
        P: TryInto<RelPathBuf>,
        P::Error: Display,
    {
        self.add(&parse(path), Node::Symlink(target.into()))
    }

    /// Returns the heap.
    #[must_use]
    pub fn build(self) -> Heap {
        Heap::new(self.tree)
    }

    #[track_caller]
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
    #[track_caller]
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

/// Converts a builder argument to a path, panicking if it is invalid.
#[track_caller]
fn parse<P>(path: P) -> RelPathBuf
where
    P: TryInto<RelPathBuf>,
    P::Error: Display,
{
    match path.try_into() {
        Ok(path) => path,
        Err(error) => panic!("{error}"),
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
    fn accepts_strings_and_paths() {
        let builder = HeapBuilder::new()
            .file("a", "x")
            .file(String::from("b"), b"y")
            .file(rel!("c"), [0_u8])
            .file(RelPathBuf::try_from("d").unwrap(), alloc::vec![0_u8])
            .symlink("e", String::from("a"));
        assert_eq!(builder.tree.nodes.len(), 6);
    }

    #[test]
    #[should_panic(expected = "invalid path component 1: name is '.' or '..'")]
    fn rejects_invalid_strings() {
        let _ = HeapBuilder::new().file("a/../b", "");
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
