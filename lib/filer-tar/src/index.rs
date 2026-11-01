// This is free and unencumbered software released into the public domain.

//! Building the index of an archive: one pass over its headers.

use crate::{
    Limits,
    header::{BLOCK, Header, decimal, pax_records, pax_time, text},
};
use alloc::{boxed::Box, vec, vec::Vec};
use core::time::Duration;
use filer_api::{
    BufResult, Error, ErrorKind, FileKind, OpKind, ReadAt, Timestamp,
    tree::{Ino, Node, ROOT, Tree},
};

/// Where a regular member's data lies in the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Member {
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) modified: Option<Timestamp>,
}

/// Why an archive cannot be opened, with the offset of the header at
/// fault; the source of the [`Error`] returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TarError {
    offset: u64,
    reason: &'static str,
}

impl core::fmt::Display for TarError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "tar header at offset {}: {}", self.offset, self.reason)
    }
}

impl core::error::Error for TarError {}

fn fail(kind: ErrorKind, offset: u64, reason: &'static str) -> Error {
    Error::new(kind, OpKind::Open).with_source(TarError { offset, reason })
}

/// Overrides from a PAX extended header or GNU long-name records, for the
/// next member only.
#[derive(Default)]
struct Pending {
    path: Option<Vec<u8>>,
    linkpath: Option<Vec<u8>>,
    size: Option<u64>,
    mtime: Option<(i64, u32)>,
}

/// Reads exactly `len` bytes at `offset`.
async fn read<S: ReadAt>(source: &S, offset: u64, len: usize) -> Result<Vec<u8>, Error> {
    let BufResult(result, buf) = source.read_exact_at(Vec::with_capacity(len), offset).await;
    match result {
        Ok(()) => Ok(buf),
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            Err(fail(ErrorKind::Corrupt, offset, "truncated"))
        },
        Err(error) => Err(error),
    }
}

/// Builds the index of the archive in `source`.
pub(crate) async fn index<S: ReadAt>(source: &S, limits: &Limits) -> Result<Tree<Member>, Error> {
    let end = source.size().await?;
    let mut tree = Tree::new();
    let mut pending = Pending::default();
    let mut entries = 0_usize;
    let mut offset = 0_u64;
    loop {
        // The archive should end with zero blocks, but ending at a block
        // boundary is accepted too.
        if offset == end {
            break;
        }
        if end - offset < BLOCK as u64 {
            return Err(fail(ErrorKind::Corrupt, offset, "truncated header"));
        }
        let block = read(source, offset, BLOCK).await?;
        let block: &[u8; BLOCK] = block.as_slice().try_into().expect("read one block");
        let header = Header(block);
        if header.is_zero() {
            break;
        }
        if !header.checksum_ok() {
            return Err(fail(ErrorKind::Corrupt, offset, "bad checksum"));
        }
        let typeflag = header.typeflag();
        let data = offset + BLOCK as u64;
        let header_size = header
            .size()
            .ok_or_else(|| fail(ErrorKind::Corrupt, offset, "bad size"))?;
        // A PAX size overrides the header's, for regular members.
        let size = match typeflag {
            b'x' | b'g' | b'L' | b'K' => header_size,
            _ => pending.size.take().unwrap_or(header_size),
        };
        if size > end - data {
            return Err(fail(
                ErrorKind::Corrupt,
                offset,
                "member extends past the end",
            ));
        }
        let next = data + size.div_ceil(BLOCK as u64) * BLOCK as u64;
        match typeflag {
            b'x' | b'L' | b'K' => {
                if size > limits.max_extension_len {
                    return Err(fail(
                        ErrorKind::LimitExceeded,
                        offset,
                        "extended header too large",
                    ));
                }
                let record = read(source, data, size as usize).await?;
                match typeflag {
                    b'x' => apply_pax(&mut pending, &record, offset)?,
                    b'L' => pending.path = Some(text(&record).to_vec()),
                    _ => pending.linkpath = Some(text(&record).to_vec()),
                }
                offset = next;
                continue;
            },
            // Global PAX headers are not applied.
            b'g' => {
                offset = next;
                continue;
            },
            b'S' => {
                return Err(fail(ErrorKind::Unsupported, offset, "GNU sparse members"));
            },
            _ => {},
        }

        entries += 1;
        if entries > limits.max_entries {
            return Err(fail(ErrorKind::LimitExceeded, offset, "too many members"));
        }
        let name = pending.path.take().unwrap_or_else(|| header.name());
        let link = pending
            .linkpath
            .take()
            .unwrap_or_else(|| header.linkname().to_vec());
        if name.len() > limits.max_path_len || link.len() > limits.max_path_len {
            return Err(fail(ErrorKind::LimitExceeded, offset, "name too long"));
        }
        let modified = match pending.mtime.take() {
            Some((secs, nanos)) => Some(Timestamp::new(secs, nanos)),
            None => header
                .mtime()
                .and_then(|secs| i64::try_from(secs).ok())
                .map(|secs| Timestamp::new(secs, 0).with_granularity(Duration::from_secs(1))),
        };
        let components =
            components(&name).ok_or_else(|| fail(ErrorKind::InvalidName, offset, "unsafe name"))?;

        if typeflag == b'5' {
            make_dirs(&mut tree, &components);
        } else {
            let Some((&last, parents)) = components.split_last() else {
                return Err(fail(
                    ErrorKind::InvalidName,
                    offset,
                    "member names the root",
                ));
            };
            let node = match typeflag {
                b'1' => {
                    let target = components_of_link(&link)
                        .and_then(|target| lookup(&tree, &target))
                        .filter(|&ino| tree.node(ino).kind() != FileKind::Dir)
                        .ok_or_else(|| fail(ErrorKind::Corrupt, offset, "bad hardlink"))?;
                    let parent = make_dirs(&mut tree, parents);
                    tree.link(parent, last, target).expect("a directory");
                    offset = next;
                    continue;
                },
                b'2' => Node::Symlink(Box::from(link.as_slice())),
                b'3' => Node::Special(FileKind::CharDevice),
                b'4' => Node::Special(FileKind::BlockDevice),
                b'6' => Node::Special(FileKind::Fifo),
                // Regular files, contiguous files, and, as POSIX requires,
                // unknown types.
                _ => Node::File(Member {
                    offset: data,
                    size,
                    modified,
                }),
            };
            let parent = make_dirs(&mut tree, parents);
            let ino = tree.add(node);
            // The later of duplicate members wins.
            tree.link(parent, last, ino).expect("a directory");
        }
        offset = next;
    }
    Ok(tree)
}

fn apply_pax(pending: &mut Pending, record: &[u8], offset: u64) -> Result<(), Error> {
    let corrupt = || fail(ErrorKind::Corrupt, offset, "bad PAX header");
    for (key, value) in pax_records(record).ok_or_else(corrupt)? {
        match key {
            b"path" => pending.path = Some(value.to_vec()),
            b"linkpath" => pending.linkpath = Some(value.to_vec()),
            b"size" => pending.size = Some(decimal(value).ok_or_else(corrupt)?),
            b"mtime" => pending.mtime = Some(pax_time(value).ok_or_else(corrupt)?),
            _ => {},
        }
    }
    Ok(())
}

/// Splits a member name into its components, dropping empty and `.` ones,
/// or returns `None` for names that could escape the archive root:
/// absolute paths, `..` components, and names containing NUL.
fn components(name: &[u8]) -> Option<Vec<&[u8]>> {
    if name.starts_with(b"/") || name.contains(&0) {
        return None;
    }
    let mut components = vec![];
    for component in name.split(|&b| b == b'/') {
        match component {
            b"" | b"." => {},
            b".." => return None,
            _ => components.push(component),
        }
    }
    Some(components)
}

/// Splits a hardlink target, a member name, into components.
fn components_of_link(link: &[u8]) -> Option<Vec<&[u8]>> {
    components(link).filter(|components| !components.is_empty())
}

/// Returns the directory at `components`, creating or replacing (the
/// later member wins) whatever is in the way.
fn make_dirs(tree: &mut Tree<Member>, components: &[&[u8]]) -> Ino {
    let mut dir = ROOT;
    for &name in components {
        dir = match tree.make_dir(dir, name) {
            Ok(child) => child,
            Err(_) => {
                let child = tree.add(Node::Dir(Default::default()));
                tree.link(dir, name, child).expect("a directory");
                child
            },
        };
    }
    dir
}

/// Looks up a member by its literal path, without following symlinks.
fn lookup(tree: &Tree<Member>, components: &[&[u8]]) -> Option<Ino> {
    components
        .iter()
        .try_fold(ROOT, |dir, name| tree.child(dir, name))
}
