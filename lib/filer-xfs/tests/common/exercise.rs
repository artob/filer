// This is free and unencumbered software released into the public domain.

//! What fuzzing does with each input: open the volume, walk all of it, and
//! check that every answer is consistent. Shared by `tests/fuzz.rs` and
//! the `cargo-fuzz` targets in `fuzz/`.

use filer_api::{BufResult, Dir, Error, ErrorKind, FileKind, MetaMask, ReadAt, XattrEntry};
use filer_spec::Sparse;
use filer_spec::block_on;
use filer_xfs::Xfs;
use std::collections::BTreeMap;

/// A volume image as a base image with bytes patched over it.
pub type Overlay = filer_spec::Overlay<Sparse>;

/// How deep the walk descends, and how many entries it visits at most.
const MAX_DEPTH: usize = 12;
const MAX_ENTRIES: usize = 4096;

/// How much of each extended attribute the walk reads.
const XATTR_BUF: usize = 4096;

/// How much of each file the walk reads, as a corrupt size may make a
/// sparse file of terabytes, and in what pieces.
const READ_CAP: u64 = 256 * 1024;
const READ_PIECE: usize = 64 * 1024;

/// How far one input got.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    pub opened: bool,
    pub entries: usize,
    pub bytes: u64,
    pub errors: usize,
}

/// Opens and walks `source`, panicking if any answer is wrong: an error
/// other than `Corrupt` or `Unsupported`, a listed entry whose id differs
/// from its stat's, a read that does not return exactly the file's length
/// (up to a cap), or a special file that opens.
///
/// Listed kinds come from directory entries and stats from inodes; a
/// corrupt volume may make them disagree, which is not checked. Lookups
/// of names and attributes follow the hash indexes, as Linux's do, which
/// a corrupt block may no longer match: what is listed may then not be
/// found.
pub fn exercise<S: ReadAt + 'static>(source: S) -> Outcome {
    let mut outcome = Outcome::default();
    match block_on(Xfs::open(source)) {
        Ok(root) => {
            outcome.opened = true;
            walk(&root, "", 0, &mut outcome);
        },
        Err(error) => allowed(&error, "open", &mut outcome),
    }
    outcome
}

fn allowed(error: &Error, what: &str, outcome: &mut Outcome) {
    assert!(
        matches!(error.kind(), ErrorKind::Corrupt | ErrorKind::Unsupported),
        "{what}: unexpected {error:?}"
    );
    outcome.errors += 1;
}

fn walk<S: ReadAt + 'static>(root: &Dir<Xfs<S>>, dir: &str, depth: usize, outcome: &mut Outcome) {
    if depth > MAX_DEPTH {
        return;
    }
    let handle = match block_on(root.open_dir(dir)) {
        Ok(handle) => handle,
        Err(error) => return allowed(&error, dir, outcome),
    };
    xattrs(
        block_on(handle.xattr_names()),
        |name| block_on(handle.xattr_into(name, Vec::with_capacity(XATTR_BUF))),
        dir,
        outcome,
    );
    let mut entries = handle.entries().expect("READ right");
    let mut listed = Vec::new();
    while let Some(entry) = block_on(entries.next()) {
        match entry {
            Ok(entry) => listed.push(entry),
            Err(error) => {
                allowed(&error, dir, outcome);
                break;
            },
        }
        if outcome.entries + listed.len() >= MAX_ENTRIES {
            break;
        }
    }
    outcome.entries += listed.len();
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &listed {
        *names.entry(entry.name.to_string()).or_default() += 1;
    }
    for entry in listed {
        let name = entry.name.to_string();
        let Some(path) = entry.name.as_name().map(|name| match dir {
            "" => name.as_str().to_owned(),
            dir => format!("{dir}/{}", name.as_str()),
        }) else {
            continue;
        };
        let metadata = match block_on(root.symlink_metadata(path.as_str(), MetaMask::all())) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                outcome.errors += 1;
                continue;
            },
            Err(error) => {
                allowed(&error, &path, outcome);
                continue;
            },
        };
        // With duplicate names, a lookup finds the first of them.
        if names[&name] == 1 {
            assert_eq!(metadata.id, entry.id, "{path}: id");
        }
        match metadata.kind {
            Some(FileKind::Dir) => walk(root, &path, depth + 1, outcome),
            Some(FileKind::Symlink) => {
                if let Err(error) = block_on(root.read_link(path.as_str())) {
                    allowed(&error, &path, outcome);
                }
            },
            Some(FileKind::File) => match block_on(root.open_file(path.as_str())) {
                Ok(file) => {
                    xattrs(
                        block_on(file.xattr_names()),
                        |name| block_on(file.xattr_into(name, Vec::with_capacity(XATTR_BUF))),
                        &path,
                        outcome,
                    );
                    let len = metadata.len.unwrap_or(0);
                    match read(&file, len) {
                        Ok(read) => {
                            assert_eq!(read, len.min(READ_CAP), "{path}: len");
                            outcome.bytes += read;
                        },
                        Err(error) => allowed(&error, &path, outcome),
                    }
                },
                Err(error) => allowed(&error, &path, outcome),
            },
            Some(
                FileKind::Fifo | FileKind::Socket | FileKind::CharDevice | FileKind::BlockDevice,
            ) => match block_on(root.open_file(path.as_str())) {
                Ok(_) => panic!("{path}: opened a special file"),
                Err(error) if error.kind() == ErrorKind::NotRegularFile => {},
                Err(error) => allowed(&error, &path, outcome),
            },
            kind => panic!("{path}: unexpected kind {kind:?}"),
        }
    }
}

/// Reads `file`, of `len` bytes by its stat, up to the cap, returning how
/// many bytes it read. Pieces are no larger than what is left, but for a
/// byte past the end, which must not be read.
fn read<S: ReadAt + 'static>(
    file: &filer_api::File<filer_api::ReadOnly, Xfs<S>>,
    len: u64,
) -> filer_api::Result<u64> {
    let mut at = 0;
    while at < READ_CAP {
        let piece = len.saturating_sub(at).clamp(1, READ_PIECE as u64) as usize;
        let BufResult(result, _) = block_on(file.read_at(Vec::with_capacity(piece), at));
        match result? {
            0 => break,
            n => at += n as u64,
        }
    }
    Ok(at.min(READ_CAP))
}

/// Reads the start of each extended attribute `names` lists with `get`,
/// checking that each found is read as far as the buffer holds.
fn xattrs(
    names: filer_api::Result<Vec<XattrEntry>>,
    get: impl Fn(XattrEntry) -> BufResult<Option<u64>, Vec<u8>>,
    path: &str,
    outcome: &mut Outcome,
) {
    let names = match names {
        Ok(names) => names,
        Err(error) => return allowed(&error, path, outcome),
    };
    for name in names {
        let BufResult(result, buf) = get(name.clone());
        match result {
            Ok(Some(len)) => {
                assert_eq!(
                    buf.len() as u64,
                    len.min(XATTR_BUF as u64),
                    "{path}: {name}"
                );
                outcome.bytes += buf.len() as u64;
            },
            Ok(None) => outcome.errors += 1,
            Err(error) => allowed(&error, path, outcome),
        }
    }
}
