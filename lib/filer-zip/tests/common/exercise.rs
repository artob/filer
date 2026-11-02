// This is free and unencumbered software released into the public domain.

//! What fuzzing does with each input: open the archive, walk all of it,
//! and check that every answer is consistent and that the zip bomb limits
//! hold. Shared by `tests/fuzz.rs` and the `cargo-fuzz` targets in `fuzz/`.

use filer_api::{Dir, Error, ErrorKind, FileKind, MetaMask};
use filer_spec::block_on;
use filer_zip::{Limits, Zip};
use std::collections::BTreeMap;

/// How deep the walk descends, and how many entries it visits at most.
const MAX_DEPTH: usize = 12;
const MAX_ENTRIES: usize = 4096;

/// How far one input got.
#[derive(Clone, Copy, Debug, Default)]
pub struct Outcome {
    pub opened: bool,
    pub entries: usize,
    pub files: u64,
    pub bytes: u64,
    pub errors: usize,
}

/// Opens and walks the archive `bytes`, panicking if any answer is wrong:
/// an error other than those a hostile archive may cause, a listed entry
/// whose id differs from its stat's, a read that does not return exactly
/// the file's length, a special file that opens, or more bytes read than
/// the limits allow.
pub fn exercise(bytes: Vec<u8>) -> Outcome {
    let len = bytes.len() as u64;
    let mut outcome = Outcome::default();
    match block_on(Zip::open(bytes)) {
        Ok(root) => {
            outcome.opened = true;
            walk(&root, "", 0, &mut outcome);
        },
        Err(error) => allowed(
            &error,
            "open",
            &[ErrorKind::InvalidName, ErrorKind::LimitExceeded],
        ),
    }
    // Members do not overlap, so their packed sizes sum to at most the
    // archive's length, and each decodes to at most the exempt size or
    // `max_ratio` times its packed size.
    let limits = Limits::default();
    let bound = outcome.files * Limits::RATIO_EXEMPT + len * limits.max_ratio;
    assert!(outcome.bytes <= bound, "{} bytes from {len}", outcome.bytes);
    outcome.errors += usize::from(!outcome.opened);
    outcome
}

/// Checks that `error` is one a hostile archive may cause when reading:
/// `Corrupt` or `Unsupported`, or one of `also`.
fn allowed(error: &Error, what: &str, also: &[ErrorKind]) {
    let kind = error.kind();
    assert!(
        matches!(kind, ErrorKind::Corrupt | ErrorKind::Unsupported) || also.contains(&kind),
        "{what}: unexpected {error:?}"
    );
}

fn walk(root: &Dir<Zip<Vec<u8>>>, dir: &str, depth: usize, outcome: &mut Outcome) {
    if depth > MAX_DEPTH {
        return;
    }
    let handle = block_on(root.open_dir(dir)).expect("listed directories open");
    let mut entries = handle.entries().expect("READ right");
    let mut listed = Vec::new();
    while let Some(entry) = block_on(entries.next()) {
        listed.push(entry.expect("listings of an index cannot fail"));
        if outcome.entries + listed.len() >= MAX_ENTRIES {
            break;
        }
    }
    outcome.entries += listed.len();
    let mut names: BTreeMap<String, usize> = BTreeMap::new();
    for entry in &listed {
        *names.entry(entry.name.to_string()).or_default() += 1;
    }
    assert!(names.values().all(|&n| n == 1), "{dir}: duplicate names");
    for entry in listed {
        // Foreign names cannot be paths; they are read by entry.
        let Some(path) = entry.name.as_name().map(|name| match dir {
            "" => name.as_str().to_owned(),
            dir => format!("{dir}/{}", name.as_str()),
        }) else {
            continue;
        };
        let metadata = block_on(root.symlink_metadata(path.as_str(), MetaMask::all()))
            .expect("listed entries stat");
        assert_eq!(metadata.id, entry.id, "{path}: id");
        assert_eq!(metadata.kind, entry.kind, "{path}: kind");
        match metadata.kind {
            Some(FileKind::Dir) => walk(root, &path, depth + 1, outcome),
            Some(FileKind::Symlink) => {
                block_on(root.read_link(path.as_str())).expect("symlinks read");
            },
            Some(FileKind::File) => {
                let file = block_on(root.open_file(path.as_str())).expect("files open");
                outcome.files += 1;
                match block_on(file.read_to_end()) {
                    Ok(contents) => {
                        assert_eq!(Some(contents.len() as u64), metadata.len, "{path}: len");
                        outcome.bytes += contents.len() as u64;
                    },
                    Err(error) => {
                        allowed(&error, &path, &[]);
                        outcome.errors += 1;
                    },
                }
            },
            Some(
                FileKind::Fifo | FileKind::Socket | FileKind::CharDevice | FileKind::BlockDevice,
            ) => match block_on(root.open_file(path.as_str())) {
                Ok(_) => panic!("{path}: opened a special file"),
                Err(error) => assert_eq!(error.kind(), ErrorKind::NotRegularFile, "{path}"),
            },
            kind => panic!("{path}: unexpected kind {kind:?}"),
        }
    }
}
