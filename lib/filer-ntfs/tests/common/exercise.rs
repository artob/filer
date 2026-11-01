// This is free and unencumbered software released into the public domain.

//! What fuzzing does with each input: open the volume, walk all of it, and
//! check that every answer is consistent. Shared by `tests/fuzz.rs` and
//! the `cargo-fuzz` targets in `fuzz/`.

use filer_api::{BufResult, Dir, Error, ErrorKind, FileKind, IoBufMut, MetaMask, ReadAt};
use filer_ntfs::{Ntfs, Options};
use filer_spec::block_on;
use std::{collections::BTreeMap, future::Future, sync::Arc};

/// A volume image as a base image with bytes patched over it, and possibly
/// cut short, so that mutating a large image costs nothing.
#[derive(Clone)]
pub struct Overlay {
    pub base: Arc<Vec<u8>>,
    pub patches: BTreeMap<u64, u8>,
    pub len: u64,
}

impl Overlay {
    pub fn new(base: Arc<Vec<u8>>) -> Self {
        let len = base.len() as u64;
        Self {
            base,
            patches: BTreeMap::new(),
            len,
        }
    }

    /// Patches `bytes` over the image at `at`, within its length.
    pub fn patch(&mut self, at: u64, bytes: &[u8]) {
        for (at, &byte) in (at..self.len).zip(bytes) {
            self.patches.insert(at, byte);
        }
    }

    /// Returns the byte at `at`, patched or not.
    pub fn byte(&self, at: u64) -> u8 {
        self.patches
            .get(&at)
            .copied()
            .unwrap_or_else(|| self.base.get(at as usize).copied().unwrap_or(0))
    }
}

impl ReadAt for Overlay {
    fn read_at<B: IoBufMut>(
        &self,
        mut buf: B,
        off: u64,
    ) -> impl Future<Output = BufResult<usize, B>> + Send {
        let result = buf.fill(&mut |spare| {
            let n = (spare.len() as u64).min(self.len.saturating_sub(off)) as usize;
            let base = self.base.get(off as usize..).unwrap_or_default();
            let from_base = n.min(base.len());
            spare[..from_base].copy_from_slice(&base[..from_base]);
            for (&at, &byte) in self.patches.range(off..off + n as u64) {
                spare[(at - off) as usize] = byte;
            }
            Ok(n)
        });
        core::future::ready(BufResult(result, buf))
    }

    fn size(&self) -> impl Future<Output = filer_api::Result<u64>> + Send {
        core::future::ready(Ok(self.len))
    }
}

/// How deep the walk descends, and how many entries it visits at most.
const MAX_DEPTH: usize = 12;
const MAX_ENTRIES: usize = 4096;

/// How far one input got.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    pub opened: bool,
    pub entries: usize,
    pub bytes: u64,
    pub errors: usize,
}

/// Opens (allowing dirty volumes) and walks `source`, panicking if any
/// answer is wrong: an error other than `Corrupt` or `Unsupported`, a
/// listed entry whose id differs from its stat's, or a read that does not
/// return exactly the file's length.
///
/// Listed kinds come from the index's copy of a file's attributes and
/// stats from its record; a corrupt volume may make them disagree, which
/// is not checked.
pub fn exercise(source: Overlay) -> Outcome {
    let mut outcome = Outcome::default();
    match block_on(Ntfs::open_with(source, Options::new().allow_dirty(true))) {
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

fn walk(root: &Dir<Ntfs<Overlay>>, dir: &str, depth: usize, outcome: &mut Outcome) {
    if depth > MAX_DEPTH {
        return;
    }
    let handle = match block_on(root.open_dir(dir)) {
        Ok(handle) => handle,
        Err(error) => return allowed(&error, dir, outcome),
    };
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
                Ok(file) => match block_on(file.read_to_end()) {
                    Ok(contents) => {
                        assert_eq!(Some(contents.len() as u64), metadata.len, "{path}: len");
                        outcome.bytes += contents.len() as u64;
                    },
                    Err(error) => allowed(&error, &path, outcome),
                },
                Err(error) => allowed(&error, &path, outcome),
            },
            kind => panic!("{path}: unexpected kind {kind:?}"),
        }
    }
}
