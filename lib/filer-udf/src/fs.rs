// This is free and unencumbered software released into the public domain.

//! What the driver shell serves of a volume.

use crate::{UdfError, dir::Listing, entry::Entry, file::File, volume::Volume};
use alloc::{sync::Arc, vec::Vec};
use core::future::Future;
use filer_api::{
    DirCookie, DriverKey, Entry as Listed, Error, ErrorKind, FileAttrs, FileId, FileKind,
    LinkTarget, MetaMask, Metadata, OpKind, ReadAt,
};
use filer_fs::{FileSystem, Opened};

/// A file, directory, symlink, or special file a path resolved to.
pub(crate) type Node = Arc<Entry>;

impl<S: ReadAt> Volume<S> {
    /// Finds the entry `name` of directory `dir`, matching bytes exactly.
    async fn find(&self, dir: &Entry, name: &[u8], op: OpKind) -> Result<Node, Error> {
        if dir.kind != FileKind::Dir {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        }
        let listing = self.listing(dir, op).await?;
        let listed = listing
            .find(name)
            .ok_or_else(|| Error::new(ErrorKind::NotFound, op))?;
        Ok(listed.entry.clone())
    }

    /// Opens directory `dir`.
    async fn open_listing(&self, dir: &Entry, op: OpKind) -> Result<Listing, Error> {
        if dir.kind != FileKind::Dir {
            return Err(Error::new(ErrorKind::NotADirectory, op));
        }
        self.listing(dir, op).await
    }
}

impl<S: ReadAt + 'static> FileSystem for Volume<S> {
    type Node = Node;
    type Dir = Listing;
    type File = File;

    fn key(&self) -> &DriverKey {
        &self.key
    }

    async fn root(&self, _op: OpKind) -> Result<Node, Error> {
        Ok(self.root.clone())
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
        match &link.target {
            Some(target) => Ok(LinkTarget::from_bytes(target.clone())),
            None => Err(Error::new(ErrorKind::NotASymlink, op)),
        }
    }

    fn open_dir(
        &self,
        dir: &Node,
        op: OpKind,
    ) -> impl Future<Output = Result<Listing, Error>> + Send {
        self.open_listing(dir, op)
    }

    async fn open_file(&self, file: &Node, op: OpKind) -> Result<File, Error> {
        if file.kind != FileKind::File {
            return Err(Error::new(ErrorKind::IsADirectory, op));
        }
        self.open_entry(file, op).await
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
    ) -> Result<Metadata, Error> {
        let mut attrs = FileAttrs::empty();
        attrs.set(FileAttrs::HIDDEN, node.hidden);
        let mut metadata = Metadata::default();
        metadata.kind = Some(node.kind);
        metadata.id = Some(FileId::new(self.id, node.id()));
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
        node: &Node,
        dir: &Listing,
        cookie: Option<DirCookie>,
        max: usize,
        op: OpKind,
    ) -> Result<(Vec<Listed>, Option<DirCookie>), Error> {
        // The cookie is the index of the next entry.
        let first = cookie.map_or(0, DirCookie::get);
        let first = usize::try_from(first).unwrap_or(usize::MAX);
        let mut entries = Vec::new();
        for listed in dir.entries.iter().skip(first).take(max) {
            let name = self
                .key
                .entry_name(&listed.name)
                .map_err(|_| UdfError::BadDirectory { at: node.at }.into_error(op))?;
            let mut entry = Listed::new(name);
            entry.kind = Some(listed.entry.kind);
            entry.id = Some(FileId::new(self.id, listed.entry.id()));
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
