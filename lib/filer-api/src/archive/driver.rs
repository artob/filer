// This is free and unencumbered software released into the public domain.

use crate::{
    BlockingResolver, BufResult, Completion, DirCookie, Driver, DriverId, DriverKey, Entry,
    EntryName, Error, ErrorKind, FileId, Handle, HandleKind, IoBufMut, LinkTarget, Metadata, Op,
    OpKind, OpenHow, Output, ReadAt, Resolve, Target,
    archive::{Codec, Decoder, Member, Stored, lock::Lock},
    path::RelPath,
    tree::{Ino, Node, ROOT, Tree},
};
use alloc::{boxed::Box, sync::Arc, vec, vec::Vec};
use core::future::Future;

/// A read-only driver serving an indexed archive.
///
/// An archive format's crate parses the headers into an
/// [`Index`](crate::archive::Index), then serves the finished tree with an
/// `Archive` over the same [`ReadAt`] source: reading a stored member reads
/// its bytes directly from the source, never past the member's end. Format
/// crates usually wrap it in their own type (such as `filer_tar::Tar`) and
/// hand out its root as a [`Dir`](crate::Dir) restricted to
/// [`Rights::READ`](crate::Rights::READ).
///
/// Members encoded by the format's [`Codec`] are decoded as streams: a
/// read continues a stream left part way through the member at or before
/// its offset, or starts a new one, and decodes up to the end of the read,
/// never past the member's declared size. A few streams are kept between
/// reads, so reading a member sequentially costs one pass over it, while
/// seeking backwards costs decoding again from the start.
///
/// It answers the read operations: paths resolve through the tree's
/// [`BlockingResolver`], so symlinks in the archive stay confined to it;
/// metadata has kind, length, id, and, for members, the modification time;
/// listings are sorted by name bytes. Everything else is
/// [`Unsupported`](ErrorKind::Unsupported): archives are read-only,
/// permanently.
///
/// # Examples
///
/// ```
/// use filer_api::{Dir, Rights, archive::{Archive, Index, Member}, tree::Node};
///
/// let bytes: &'static [u8] = b"hello";
/// let mut index = Index::new("example");
/// index.insert(b"greeting", Node::File(Member::new(0, 5)), 0)?;
/// let archive = Archive::new(index.into_tree(), bytes);
/// let root = archive.root();
/// let dir = Dir::new(archive, root)?.restrict(Rights::READ);
/// # Ok::<(), filer_api::Error>(())
/// ```
pub struct Archive<S, C = Stored> {
    key: Arc<DriverKey>,
    tree: Arc<Tree<Member<C>>>,
    source: Arc<S>,
    streams: Arc<Lock<Vec<Stream>>>,
}

/// How many decoding streams an archive keeps between reads.
const STREAMS: usize = 4;

/// How much packed input a stream reads from the source at once.
const INPUT_CHUNK: usize = 64 * 1024;

/// How much a stream decodes at once.
const OUTPUT_CHUNK: usize = 32 * 1024;

/// A member's decoder, part way through the member.
struct Stream {
    ino: Ino,
    /// The decoded bytes produced so far.
    decoded: u64,
    /// The packed bytes read from the source so far, including `input`.
    fetched: u64,
    /// Packed bytes read but not yet all decoded, from `used` on.
    input: Vec<u8>,
    used: usize,
    decoder: Box<dyn Decoder>,
}

impl<S: ReadAt + 'static, C: Codec> Archive<S, C> {
    /// Returns a driver serving `tree`, whose members lie in `source`.
    #[must_use]
    pub fn new(tree: Tree<Member<C>>, source: S) -> Self {
        Self {
            key: Arc::new(DriverKey::new()),
            tree: Arc::new(tree),
            source: Arc::new(source),
            streams: Arc::new(Lock::new(Vec::new())),
        }
    }

    /// Returns a new handle to the archive's root directory.
    #[must_use]
    pub fn root(&self) -> Handle {
        self.key.handle(HandleKind::Dir, ROOT)
    }

    async fn perform(&self, op: Op) -> Completion {
        let kind = op.kind();
        let error = |error| Error::new(error, kind);
        match op {
            Op::Open { at, path, how } => Completion::new(
                self.open_entry(&at, &path, how)
                    .map_err(|error| error.with_path(&*path)),
            ),
            Op::Close { handle } => {
                Completion::new(self.ino(&handle).map(|_| Output::Done).map_err(error))
            },
            Op::Read { file, mut buf, off } => {
                let result = match self.member(&file) {
                    Ok((ino, member)) => self.read(ino, member, &mut buf, off).await,
                    Err(kind) => Err(error(kind)),
                };
                Completion::new(result.map(Output::Read)).with_buf(buf)
            },
            Op::Stat { target, .. } => {
                let ino = match &target {
                    Target::Handle(handle) => self.ino(handle).map_err(error),
                    Target::Entry {
                        at,
                        name,
                        follow,
                        resolve,
                    } => self
                        .entry(at, name, *follow, *resolve, kind)
                        .map_err(|error| with_name(error, name)),
                };
                Completion::new(ino.map(|ino| Output::Metadata(self.metadata(ino))))
            },
            Op::ReadDir { dir, cookie, max } => {
                Completion::new(self.read_dir(&dir, cookie, max).map_err(error))
            },
            Op::ReadLink { at, name } => {
                let result = self
                    .entry(&at, &name, false, Resolve::default(), kind)
                    .and_then(|ino| match self.tree.node(ino) {
                        Node::Symlink(target) => {
                            Ok(Output::LinkTarget(LinkTarget::from_bytes(target.to_vec())))
                        },
                        _ => Err(error(ErrorKind::NotASymlink)),
                    });
                Completion::new(result.map_err(|error| with_name(error, &name)))
            },
            op => op.unsupported(),
        }
    }

    /// Reads from `member`, the inode `ino`, at `off` into the spare
    /// capacity of `buf`, never past the member's end.
    async fn read(
        &self,
        ino: Ino,
        member: Member<C>,
        buf: &mut impl IoBufMut,
        off: u64,
    ) -> Result<usize, Error> {
        let remaining = member.size.saturating_sub(off);
        let wanted = usize::try_from(remaining).map_or(buf.spare(), |r| r.min(buf.spare()));
        if wanted == 0 {
            return Ok(0);
        }
        let data = if member.codec.is_stored() {
            // A bounded intermediate buffer keeps the source from reading
            // past the member into the next header.
            self.read_packed(&member, off, wanted).await?
        } else {
            self.decode(ino, &member, off, wanted).await?
        };
        buf.fill(&mut |spare| {
            spare[..data.len()].copy_from_slice(&data);
            Ok(data.len())
        })
    }

    /// Reads exactly `len` of `member`'s packed bytes, from `off` on.
    async fn read_packed(
        &self,
        member: &Member<C>,
        off: u64,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let corrupt = || Error::new(ErrorKind::Corrupt, OpKind::Read);
        let at = member.offset.checked_add(off).ok_or_else(corrupt)?;
        let BufResult(result, data) = self.source.read_exact_at(Vec::with_capacity(len), at).await;
        result.map_err(|error| match error.kind() {
            ErrorKind::UnexpectedEof => corrupt(),
            _ => error,
        })?;
        Ok(data)
    }

    /// Decodes `wanted` bytes of `member`, the inode `ino`, from `off` on,
    /// continuing a kept stream if one has not passed `off`.
    async fn decode(
        &self,
        ino: Ino,
        member: &Member<C>,
        off: u64,
        wanted: usize,
    ) -> Result<Vec<u8>, Error> {
        let corrupt = |_| Error::new(ErrorKind::Corrupt, OpKind::Read);
        let end = off.saturating_add(wanted as u64).min(member.size);
        let mut stream = match self.take_stream(ino, off) {
            Some(stream) => stream,
            None => Stream {
                ino,
                decoded: 0,
                fetched: 0,
                input: Vec::new(),
                used: 0,
                decoder: member.codec.decoder()?,
            },
        };
        let mut data = Vec::with_capacity(wanted);
        let mut output = vec![0; OUTPUT_CHUNK];
        while stream.decoded < end {
            if stream.used == stream.input.len() && stream.fetched < member.packed {
                let len = usize::try_from(member.packed - stream.fetched)
                    .map_or(INPUT_CHUNK, |n| n.min(INPUT_CHUNK));
                stream.input = self.read_packed(member, stream.fetched, len).await?;
                stream.used = 0;
                stream.fetched += len as u64;
            }
            // Stop at the end of the read, so that the kept stream can
            // continue from there, and so never pass the declared size.
            let room =
                usize::try_from(end - stream.decoded).map_or(OUTPUT_CHUNK, |n| n.min(OUTPUT_CHUNK));
            let input = &stream.input[stream.used..];
            let (consumed, produced) = stream.decoder.decode(input, &mut output[..room])?;
            if consumed > input.len() || produced > room {
                // The decoder broke its contract.
                return Err(corrupt(()));
            }
            if consumed == 0 && produced == 0 {
                // No progress: the member's data ended early.
                return Err(corrupt(()));
            }
            stream.used += consumed;
            let start = stream.decoded;
            stream.decoded += produced as u64;
            // Keep what falls within `off..end`.
            let from = off.saturating_sub(start).min(produced as u64) as usize;
            let to = end.saturating_sub(start).min(produced as u64) as usize;
            data.extend_from_slice(&output[from..to]);
            if stream.decoded == member.size {
                stream.decoder.finish()?;
            }
        }
        if stream.decoded < member.size {
            self.keep_stream(stream);
        }
        Ok(data)
    }

    /// Takes the kept stream of `ino` that is furthest along without
    /// having passed `off`.
    fn take_stream(&self, ino: Ino, off: u64) -> Option<Stream> {
        self.streams.with(|streams| {
            let best = streams
                .iter()
                .enumerate()
                .filter(|(_, s)| s.ino == ino && s.decoded <= off)
                .max_by_key(|(_, s)| s.decoded)
                .map(|(i, _)| i)?;
            Some(streams.remove(best))
        })
    }

    /// Keeps `stream` for later reads, dropping the least recently kept
    /// stream if there are too many.
    fn keep_stream(&self, stream: Stream) {
        self.streams.with(|streams| {
            if streams.len() >= STREAMS {
                streams.remove(0);
            }
            streams.push(stream);
        });
    }

    /// Returns the inode a handle refers to, if this archive minted it.
    fn ino(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        self.key
            .payload::<Ino>(handle)
            .copied()
            .ok_or(ErrorKind::CrossDriver)
    }

    fn dir(&self, handle: &Handle) -> Result<Ino, ErrorKind> {
        let ino = self.ino(handle)?;
        match self.tree.node(ino) {
            Node::Dir(_) => Ok(ino),
            _ => Err(ErrorKind::NotADirectory),
        }
    }

    fn member(&self, handle: &Handle) -> Result<(Ino, Member<C>), ErrorKind> {
        let ino = self.ino(handle)?;
        match self.tree.node(ino) {
            Node::File(member) => Ok((ino, *member)),
            Node::Dir(_) => Err(ErrorKind::IsADirectory),
            _ => Err(ErrorKind::NotRegularFile),
        }
    }

    fn open_entry(&self, at: &Handle, path: &RelPath, how: OpenHow) -> Result<Output, Error> {
        let op = OpKind::Open;
        let dir = self.dir(at).map_err(|error| Error::new(error, op))?;
        let ino = self
            .tree
            .resolve(&dir, path.as_str().as_bytes(), how.resolve, true, op)?;
        let mismatch = match (self.tree.node(ino), how.kind) {
            (Node::Dir(_), HandleKind::Dir) | (Node::File(_), HandleKind::File) => None,
            (Node::Dir(_), HandleKind::File) => Some(ErrorKind::IsADirectory),
            (_, HandleKind::Dir) => Some(ErrorKind::NotADirectory),
            (_, HandleKind::File) => Some(ErrorKind::NotRegularFile),
        };
        match mismatch {
            None => Ok(Output::Opened(self.key.handle(how.kind, ino))),
            Some(error) => Err(Error::new(error, op)),
        }
    }

    fn entry(
        &self,
        at: &Handle,
        name: &EntryName,
        follow: bool,
        resolve: Resolve,
        op: OpKind,
    ) -> Result<Ino, Error> {
        let dir = self.dir(at).map_err(|error| Error::new(error, op))?;
        let bytes = match name {
            EntryName::Utf8(name) => name.as_str().as_bytes(),
            EntryName::Foreign(name) if name.origin() == self.key.id() => name.as_bytes(),
            EntryName::Foreign(_) => return Err(Error::new(ErrorKind::ForeignNameMismatch, op)),
        };
        self.tree.resolve(&dir, bytes, resolve, follow, op)
    }

    fn metadata(&self, ino: Ino) -> Metadata {
        let mut metadata = Metadata::default();
        let node = self.tree.node(ino);
        metadata.kind = Some(node.kind());
        metadata.len = Some(match node {
            Node::Dir(entries) => entries.len() as u64,
            Node::File(member) => member.size,
            Node::Symlink(target) => target.len() as u64,
            Node::Special(_) => 0,
        });
        if let Node::File(member) = node {
            metadata.modified = member.modified;
        }
        metadata.id = Some(FileId::new(0, ino as u128));
        metadata
    }

    fn read_dir(
        &self,
        dir: &Handle,
        cookie: Option<DirCookie>,
        max: usize,
    ) -> Result<Output, ErrorKind> {
        let children = self
            .tree
            .entries(self.dir(dir)?)
            .expect("`dir` returns directories");
        let start = cookie.map_or(0, |cookie| {
            usize::try_from(cookie.get()).unwrap_or(usize::MAX)
        });
        let mut entries = Vec::new();
        for (name, &ino) in children.iter().skip(start).take(max) {
            let name = self
                .key
                .entry_name(name)
                .expect("archive names are checked when indexed");
            let mut entry = Entry::new(name);
            entry.kind = Some(self.tree.node(ino).kind());
            entry.id = Some(FileId::new(0, ino as u128));
            entries.push(entry);
        }
        let end = start.saturating_add(entries.len());
        let next = (end < children.len()).then(|| DirCookie::new(end as u64));
        Ok(Output::Entries { entries, next })
    }
}

impl<S: ReadAt + 'static, C: Codec> Driver for Archive<S, C> {
    fn id(&self) -> DriverId {
        self.key.id()
    }

    fn submit(&self, op: Op) -> impl Future<Output = Completion> + Send + use<S, C> {
        let archive = self.clone();
        async move { archive.perform(op).await }
    }
}

impl<S, C> Clone for Archive<S, C> {
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            tree: self.tree.clone(),
            source: self.source.clone(),
            streams: self.streams.clone(),
        }
    }
}

impl<S, C> core::fmt::Debug for Archive<S, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Archive")
            .field("id", &self.key.id())
            .field("nodes", &self.tree.node_count())
            .finish_non_exhaustive()
    }
}

/// Adds an entry's name to an error, if it is UTF-8.
fn with_name(error: Error, name: &EntryName) -> Error {
    match name.as_name() {
        Some(name) => error.with_path(name),
        None => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Dir, Rights, archive::Index, driver::tests::block_on};
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Decoders started, across every test in this module.
    static STARTED: AtomicUsize = AtomicUsize::new(0);

    /// A toy codec: each packed byte decodes to two copies of itself, and
    /// the decoded bytes must sum to `sum`.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct Doubled {
        sum: u64,
    }

    struct Doubler {
        pending: Option<u8>,
        sum: u64,
        expected: u64,
    }

    impl Decoder for Doubler {
        fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Error> {
            let (mut consumed, mut produced) = (0, 0);
            while produced < output.len() {
                let byte = match self.pending.take() {
                    Some(byte) => byte,
                    None => {
                        let Some(&byte) = input.get(consumed) else {
                            break;
                        };
                        consumed += 1;
                        self.pending = Some(byte);
                        byte
                    },
                };
                output[produced] = byte;
                produced += 1;
                self.sum += u64::from(byte);
            }
            Ok((consumed, produced))
        }

        fn finish(&mut self) -> Result<(), Error> {
            if self.sum == self.expected {
                Ok(())
            } else {
                Err(Error::new(ErrorKind::Corrupt, OpKind::Read))
            }
        }
    }

    impl Codec for Doubled {
        fn is_stored(&self) -> bool {
            false
        }

        fn decoder(&self) -> Result<Box<dyn Decoder>, Error> {
            STARTED.fetch_add(1, Ordering::Relaxed);
            Ok(Box::new(Doubler {
                pending: None,
                sum: 0,
                expected: self.sum,
            }))
        }
    }

    /// Returns an archive whose file `f` decodes `packed` as `size` bytes.
    fn archive(packed: &'static [u8], size: u64, sum: u64) -> Dir<Archive<&'static [u8], Doubled>> {
        let mut index = Index::new("toy");
        let member = Member::encoded(0, packed.len() as u64, size, Doubled { sum });
        index.insert(b"f", Node::File(member), 0).unwrap();
        let archive = Archive::new(index.into_tree(), packed);
        let root = archive.root();
        Dir::new(archive, root).unwrap().restrict(Rights::READ)
    }

    fn read_at(
        root: &Dir<Archive<&'static [u8], Doubled>>,
        off: u64,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let file = block_on(root.open_file("f")).unwrap();
        let BufResult(result, buf) = block_on(file.read_at(Vec::with_capacity(len), off));
        result.map(|_| buf)
    }

    #[test]
    fn decodes_members_sequentially_and_at_random() {
        let sum = (97 + 98 + 99) * 2;
        let root = archive(b"abc", 6, sum);
        let file = block_on(root.open_file("f")).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), b"aabbcc");
        assert_eq!(read_at(&root, 3, 2).unwrap(), b"bc");
        assert_eq!(read_at(&root, 1, 100).unwrap(), b"abbcc");
        assert_eq!(read_at(&root, 6, 1).unwrap(), b"");
        let metadata = block_on(root.metadata("f", crate::MetaMask::LEN)).unwrap();
        assert_eq!(metadata.len, Some(6));
    }

    #[test]
    fn continues_kept_streams() {
        // Large enough to take several output chunks.
        let packed: &'static [u8] = Box::leak(vec![1; 3 * OUTPUT_CHUNK].into_boxed_slice());
        let size = 2 * packed.len() as u64;
        let root = archive(packed, size, size);
        let before = STARTED.load(Ordering::Relaxed);
        let mut off = 0;
        while off < size {
            off += read_at(&root, off, 1000).unwrap().len() as u64;
        }
        // Other tests may start decoders meanwhile, but not this many.
        let started = STARTED.load(Ordering::Relaxed) - before;
        assert!(started < 10, "{started} decoders for one sequential read");
        // A read before every kept stream starts a new one.
        assert_eq!(read_at(&root, 0, 2).unwrap(), [1, 1]);
    }

    #[test]
    fn refuses_truncated_and_failing_members() {
        // Three packed bytes cannot make eight.
        let error = read_at(&archive(b"abc", 8, 0), 0, 8).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt);
        // The checksum fails, but only once the end is read.
        let root = archive(b"abc", 6, 1);
        assert_eq!(read_at(&root, 0, 5).unwrap(), b"aabbc");
        assert_eq!(read_at(&root, 0, 6).unwrap_err().kind(), ErrorKind::Corrupt);
    }

    #[test]
    fn stops_at_the_declared_size() {
        // The decoder would produce six bytes; only four are asked for.
        let root = archive(b"abc", 4, (97 + 98) * 2);
        assert_eq!(read_at(&root, 0, 100).unwrap(), b"aabb");
    }
}
