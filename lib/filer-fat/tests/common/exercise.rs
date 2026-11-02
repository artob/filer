// This is free and unencumbered software released into the public domain.

//! What fuzzing does with each input: open the volume, walk all of it, and
//! check that every answer is consistent. Shared by `tests/fuzz.rs` and
//! the `cargo-fuzz` targets in `fuzz/`.

use filer_api::{Dir, Error, ErrorKind, FileKind, MetaMask};
use filer_fat::Fat;
use filer_spec::block_on;
use std::collections::BTreeMap;

/// A volume image as a base image with bytes patched over it.
pub type Overlay = filer_spec::Overlay<Vec<u8>>;

/// How deep the walk descends. Corrupt volumes can make a directory point
/// back at an ancestor; the driver never loops, as each path is finite,
/// but a recursive walk would.
const MAX_DEPTH: usize = 12;

/// How many entries the walk visits at most, to bound each input's time.
const MAX_ENTRIES: usize = 4096;

/// How far one input got.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    /// Whether the volume opened.
    pub opened: bool,
    /// The entries listed.
    pub entries: usize,
    /// The bytes read from files.
    pub bytes: u64,
    /// The errors met, all of them allowed.
    pub errors: usize,
}

/// Opens and walks `source`, panicking if any answer is wrong: an error
/// other than `Corrupt` (or `Unsupported`, for an unknown FAT32 version),
/// a listed entry that does not stat as listed, or a read that does not
/// return exactly the file's length.
pub fn exercise(source: Overlay) -> Outcome {
    let mut outcome = Outcome::default();
    match block_on(Fat::open(source)) {
        Ok(root) => {
            outcome.opened = true;
            walk(&root, "", 0, &mut outcome);
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

fn walk(root: &Dir<Fat<Overlay>>, dir: &str, depth: usize, outcome: &mut Outcome) {
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
        let metadata = match block_on(root.metadata(path.as_str(), MetaMask::all())) {
            Ok(metadata) => metadata,
            Err(error) => {
                allowed(&error, &path, outcome);
                continue;
            },
        };
        // With duplicate names, a lookup finds the first of them.
        if names[&name] == 1 {
            assert_eq!(metadata.kind, entry.kind, "{path}: kind");
            assert_eq!(metadata.id, entry.id, "{path}: id");
        }
        match metadata.kind {
            Some(FileKind::Dir) => walk(root, &path, depth + 1, outcome),
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
