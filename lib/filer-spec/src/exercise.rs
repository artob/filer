// This is free and unencumbered software released into the public domain.

//! Walking what a driver makes of a corrupt volume, checking that every
//! answer is one a corrupt volume may give: what fuzzers and corruption
//! tests do with each input.

use crate::block_on;
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use filer_api::{
    BufResult, Dir, Driver, Error, ErrorKind, File, FileKind, MetaMask, ReadOnly, XattrEntry,
};

/// How deep the walk descends. Corrupt volumes can make a directory point
/// back at an ancestor; a driver never loops, as each path is finite, but
/// a recursive walk would.
const MAX_DEPTH: usize = 12;

/// How many entries the walk visits at most, to bound each input's time.
const MAX_ENTRIES: usize = 4096;

/// How much of each extended attribute the walk reads.
const XATTR_BUF: usize = 4096;

/// How much of each file the walk reads by default, as a corrupt size may
/// make a sparse file of terabytes, and in what pieces.
const READ_CAP: u64 = 256 * 1024;
const READ_PIECE: usize = 64 * 1024;

/// What [`exercise`] checks beyond what holds for every driver, as a
/// format allows.
///
/// # Examples
///
/// ```
/// use filer_spec::Checks;
///
/// // XFS: listed kinds come from directory entries and stats from inodes,
/// // and lookups follow hash indexes a corrupt block may no longer match.
/// let xfs = Checks::new().lookups(false);
/// // FAT: both come from the same entry, and there are no attributes.
/// let fat = Checks::new().kinds(true).xattrs(false);
/// // ZIP: an archive may be refused, and its limits bound what it decodes.
/// let zip = Checks::new().kinds(true).xattrs(false).refusals(true).read_cap(u64::MAX);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checks {
    kinds: bool,
    lookups: bool,
    xattrs: bool,
    refusals: bool,
    read_cap: u64,
}

impl Checks {
    /// Returns the default checks: what is listed must be found, listed
    /// kinds are not compared with stats, extended attributes are read, an
    /// open may fail only as for a corrupt volume, and up to 256 KiB of
    /// each file is read.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            kinds: false,
            lookups: true,
            xattrs: true,
            refusals: false,
            read_cap: READ_CAP,
        }
    }

    /// Checks, if `check`, that each listed entry's kind is its stat's:
    /// for a format that records both in the same place, as FAT does in
    /// directory entries, so that no corruption can make them disagree.
    #[must_use]
    pub const fn kinds(mut self, check: bool) -> Self {
        self.kinds = check;
        self
    }

    /// Checks, if `check`, that each listed entry and extended attribute
    /// is found: not for a format whose lookups follow indexes, as XFS's
    /// follow hashes, that a corrupt block may no longer match.
    #[must_use]
    pub const fn lookups(mut self, check: bool) -> Self {
        self.lookups = check;
        self
    }

    /// Lists and reads extended attributes, if `read`: not for a driver
    /// that has none.
    #[must_use]
    pub const fn xattrs(mut self, read: bool) -> Self {
        self.xattrs = read;
        self
    }

    /// Allows, if `allow`, an open to fail with
    /// [`InvalidName`](ErrorKind::InvalidName) or
    /// [`LimitExceeded`](ErrorKind::LimitExceeded) too: for an archive
    /// driver, which refuses archives with hostile names or past its
    /// limits.
    #[must_use]
    pub const fn refusals(mut self, allow: bool) -> Self {
        self.refusals = allow;
        self
    }

    /// Reads at most `cap` bytes of each file: 256 KiB by default, as a
    /// corrupt size may make a sparse file of terabytes. An archive driver
    /// whose limits bound what its members decode to may be read whole, so
    /// that a test can check that bound.
    #[must_use]
    pub const fn read_cap(mut self, cap: u64) -> Self {
        self.read_cap = cap;
        self
    }
}

impl Default for Checks {
    fn default() -> Self {
        Self::new()
    }
}

/// How far one input got.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Whether the volume opened.
    pub opened: bool,
    /// The entries listed.
    pub entries: usize,
    /// The regular files opened to read.
    pub files: u64,
    /// The bytes read from files and extended attributes.
    pub bytes: u64,
    /// The errors met, all of them allowed.
    pub errors: usize,
}

/// Walks the volume whose opening returned `opened`, panicking if any
/// answer is wrong: an error other than
/// [`Corrupt`](ErrorKind::Corrupt) or
/// [`Unsupported`](ErrorKind::Unsupported) (or a refusal `checks`
/// allows), a listed entry whose id differs from its stat's, a read that
/// does not return exactly the file's length (up to a cap), a special
/// file that opens, or what `checks` adds.
///
/// The walk is bounded in depth, entries, and bytes read, so that each
/// input takes little time whatever its sizes say.
///
/// # Examples
///
/// ```
/// use filer_spec::{Checks, block_on, exercise};
/// # use filer_api::{Dir, Driver};
/// # fn example<D: Driver>(open: impl Future<Output = filer_api::Result<Dir<D>>>) {
/// let outcome = exercise(block_on(open), Checks::new());
/// assert!(outcome.opened || outcome.errors > 0);
/// # }
/// ```
pub fn exercise<D: Driver>(opened: Result<Dir<D>, Error>, checks: Checks) -> Outcome {
    let mut outcome = Outcome::default();
    match opened {
        Ok(root) => {
            outcome.opened = true;
            walk(&root, "", 0, checks, &mut outcome);
        },
        Err(error)
            if checks.refusals
                && matches!(
                    error.kind(),
                    ErrorKind::InvalidName | ErrorKind::LimitExceeded
                ) =>
        {
            outcome.errors += 1;
        },
        Err(error) => allowed(&error, "open", &mut outcome),
    }
    outcome
}

/// Panics unless `error` is one a corrupt volume may cause.
fn allowed(error: &Error, what: &str, outcome: &mut Outcome) {
    assert!(
        matches!(error.kind(), ErrorKind::Corrupt | ErrorKind::Unsupported),
        "{what}: unexpected {error:?}"
    );
    outcome.errors += 1;
}

fn walk<D: Driver>(root: &Dir<D>, dir: &str, depth: usize, checks: Checks, outcome: &mut Outcome) {
    if depth > MAX_DEPTH {
        return;
    }
    let handle = match block_on(root.open_dir(dir)) {
        Ok(handle) => handle,
        Err(error) => return allowed(&error, dir, outcome),
    };
    if checks.xattrs {
        xattrs(
            block_on(handle.xattr_names()),
            |name| block_on(handle.xattr_into(name, Vec::with_capacity(XATTR_BUF))),
            dir,
            checks,
            outcome,
        );
    }
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
            "" => name.as_str().to_string(),
            dir => format!("{dir}/{}", name.as_str()),
        }) else {
            continue;
        };
        let metadata = match block_on(root.symlink_metadata(path.as_str(), MetaMask::all())) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound && !checks.lookups => {
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
        if checks.kinds {
            assert_eq!(metadata.kind, entry.kind, "{path}: kind");
        }
        match metadata.kind {
            Some(FileKind::Dir) => walk(root, &path, depth + 1, checks, outcome),
            Some(FileKind::Symlink) => {
                if let Err(error) = block_on(root.read_link(path.as_str())) {
                    allowed(&error, &path, outcome);
                }
            },
            Some(FileKind::File) => match block_on(root.open_file(path.as_str())) {
                Ok(file) => {
                    if checks.xattrs {
                        xattrs(
                            block_on(file.xattr_names()),
                            |name| block_on(file.xattr_into(name, Vec::with_capacity(XATTR_BUF))),
                            &path,
                            checks,
                            outcome,
                        );
                    }
                    outcome.files += 1;
                    let len = metadata.len.unwrap_or(0);
                    match read(&file, len, checks.read_cap) {
                        Ok(read) => {
                            assert_eq!(read, len.min(checks.read_cap), "{path}: len");
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

/// Reads `file`, of `len` bytes by its stat, up to `cap`, returning how
/// many bytes it read. Pieces are no larger than what is left, but for a
/// byte past the end, which must not be read.
fn read<D: Driver>(file: &File<ReadOnly, D>, len: u64, cap: u64) -> filer_api::Result<u64> {
    let mut at = 0;
    while at < cap {
        let piece = len.saturating_sub(at).clamp(1, READ_PIECE as u64) as usize;
        let BufResult(result, _) = block_on(file.read_at(Vec::with_capacity(piece), at));
        match result? {
            0 => break,
            n => at += n as u64,
        }
    }
    Ok(at.min(cap))
}

/// Reads the start of each extended attribute `names` lists with `get`,
/// checking that each found is read as far as the buffer holds, and that
/// each is found if `checks` say so.
fn xattrs(
    names: filer_api::Result<Vec<XattrEntry>>,
    get: impl Fn(XattrEntry) -> BufResult<Option<u64>, Vec<u8>>,
    path: &str,
    checks: Checks,
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
            Ok(None) if checks.lookups => panic!("{path}: listed {name} not found"),
            Ok(None) => outcome.errors += 1,
            Err(error) => allowed(&error, path, outcome),
        }
    }
}
