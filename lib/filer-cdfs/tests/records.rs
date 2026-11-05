// This is free and unencumbered software released into the public domain.

//! Images made here, record by record, for what the tools do not make:
//! files of several extents, versions, hidden and associated files,
//! interleaved and sparse files, entries continued elsewhere, and records
//! that are malformed.

mod common;

use common::{
    ASSOCIATED, DATA, HIDDEN, MULTI_EXTENT, ROOT, entry, image, pattern, record, sharing,
};
use filer_api::{Dir, ErrorKind, FileAttrs, MetaMask};
use filer_cdfs::{Cdfs, CdfsError};
use filer_spec::block_on;

type Root = Dir<Cdfs<Vec<u8>>>;

fn open(image: Vec<u8>) -> Root {
    block_on(Cdfs::open(image)).unwrap()
}

fn names(root: &Root, dir: &str) -> Vec<String> {
    let handle = block_on(root.open_dir(dir)).unwrap();
    block_on(handle.names())
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn read(root: &Root, path: &str) -> filer_api::Result<Vec<u8>> {
    let file = block_on(root.open_file(path))?;
    block_on(file.read_to_end())
}

/// Returns the `CdfsError` an error carries.
fn source(error: &filer_api::Error) -> CdfsError {
    *core::error::Error::source(error)
        .unwrap()
        .downcast_ref::<CdfsError>()
        .unwrap()
}

/// A file of two extents, the first ending a block short of the second,
/// reads as one, across them.
#[test]
fn reads_files_of_several_extents() {
    let (first, second) = (pattern(2048, 1), pattern(2048, 2));
    let image = image(
        &[],
        &[
            record(b"BIG.BIN;1", DATA, 2048, MULTI_EXTENT, &[]),
            record(b"BIG.BIN;1", DATA + 4, 100, 0, &[]),
        ],
        &[(DATA, &first), (DATA + 4, &second)],
    );
    let root = open(image);
    assert_eq!(names(&root, ""), ["BIG.BIN"]);
    let mut expected = first.clone();
    expected.extend_from_slice(&second[..100]);
    assert_eq!(read(&root, "BIG.BIN").unwrap(), expected);
    let metadata = block_on(root.symlink_metadata("BIG.BIN", MetaMask::all())).unwrap();
    assert_eq!(metadata.len, Some(2148));
}

/// Several versions of a name show as one, the first, which is the
/// latest; the version and the dot of a name without an extension go.
#[test]
fn shows_one_version_of_a_name() {
    let image = image(
        &[],
        &[
            record(b"A.TXT;2", DATA, 2, 0, &[]),
            record(b"A.TXT;1", DATA + 1, 1, 0, &[]),
            record(b"README.;1", DATA + 2, 3, 0, &[]),
        ],
        &[(DATA, b"v2"), (DATA + 1, b"1"), (DATA + 2, b"abc")],
    );
    let root = open(image);
    assert_eq!(names(&root, ""), ["A.TXT", "README"]);
    assert_eq!(read(&root, "A.TXT").unwrap(), b"v2");
    assert_eq!(read(&root, "README").unwrap(), b"abc");
}

/// Hidden files are shown, marked; associated ones are not.
#[test]
fn shows_hidden_files_but_not_associated_ones() {
    let image = image(
        &[],
        &[
            record(b"FILE.TXT;1", DATA, 2, ASSOCIATED, &[]),
            record(b"FILE.TXT;1", DATA + 1, 4, 0, &[]),
            record(b"SECRET.TXT;1", DATA + 2, 6, HIDDEN, &[]),
        ],
        &[(DATA, b"rs"), (DATA + 1, b"data"), (DATA + 2, b"secret")],
    );
    let root = open(image);
    assert_eq!(names(&root, ""), ["FILE.TXT", "SECRET.TXT"]);
    assert_eq!(read(&root, "FILE.TXT").unwrap(), b"data");
    let attrs = |path| {
        block_on(root.symlink_metadata(path, MetaMask::all()))
            .unwrap()
            .attrs
            .unwrap()
    };
    assert!(attrs("SECRET.TXT").contains(FileAttrs::HIDDEN));
    assert!(!attrs("FILE.TXT").contains(FileAttrs::HIDDEN));
    assert_eq!(read(&root, "SECRET.TXT").unwrap(), b"secret");
}

/// Interleaved and sparse files list, but do not open.
#[test]
fn refuses_files_it_does_not_read() {
    let mut interleaved = record(b"I.BIN;1", DATA, 4, 0, &[]);
    interleaved[26] = 1;
    let image = image(
        &sharing(),
        &[
            interleaved,
            record(b"S.BIN;1", DATA, 4, 0, &entry(b"SF", &[0; 17])),
        ],
        &[(DATA, b"data")],
    );
    let root = open(image);
    assert_eq!(names(&root, ""), ["I.BIN", "S.BIN"]);
    for (path, expected) in [
        ("I.BIN", CdfsError::Interleaved { at: 0 }),
        ("S.BIN", CdfsError::Sparse { at: 0 }),
    ] {
        let error = read(&root, path).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported, "{path}");
        assert_eq!(
            core::mem::discriminant(&source(&error)),
            core::mem::discriminant(&expected),
            "{path}"
        );
    }
}

/// Rock Ridge's entries, continued in an area a `CE` entry points to: a
/// name, and a symlink's target.
#[test]
fn follows_continuation_areas() {
    let continued = [
        entry(b"NM", b"\0continued name"),
        entry(b"SL", b"\0\0\x06target"),
    ]
    .concat();
    let ce = |len: usize| {
        let mut data = Vec::new();
        for value in [DATA, 0, u32::try_from(len).unwrap()] {
            data.extend(value.to_le_bytes());
            data.extend(value.to_be_bytes());
        }
        entry(b"CE", &data)
    };
    let mut px = 0o120_777_u32.to_le_bytes().to_vec();
    px.extend(0o120_777_u32.to_be_bytes());
    px.extend([0; 24]);
    let use_ = [entry(b"PX", &px), ce(continued.len())].concat();
    let image = image(
        &sharing(),
        &[record(b"LINK.;1", 0, 0, 0, &use_)],
        &[(DATA, &continued)],
    );
    let root = open(image);
    assert_eq!(names(&root, ""), ["continued name"]);
    let target = block_on(root.read_link("continued name")).unwrap();
    assert_eq!(target.as_bytes(), b"target");
}

/// A continuation area that continues in itself is malformed, not a
/// hang: in the root, whose listing opening reads.
#[test]
fn refuses_endless_continuations() {
    let mut data = Vec::new();
    for value in [DATA, 0, 28] {
        data.extend(value.to_le_bytes());
        data.extend(value.to_be_bytes());
    }
    let ce = entry(b"CE", &data);
    let image = image(
        &sharing(),
        &[record(b"LOOP.;1", 0, 0, 0, &ce)],
        &[(DATA, &ce)],
    );
    let error = block_on(Cdfs::open(image)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    assert!(matches!(source(&error), CdfsError::BadSystemUse { .. }));
}

/// Directories whose records are malformed: the root's, which opening
/// lists.
#[test]
fn refuses_malformed_directories() {
    let mut long = record(b"LONG.TXT;1", DATA, 1, 0, &[]);
    long[32] = 200;
    let cases = [
        // A name longer than its record.
        vec![long],
        // A file of several extents whose last is missing.
        vec![record(b"PART.BIN;1", DATA, 2048, MULTI_EXTENT, &[])],
        // Parts of different names.
        vec![
            record(b"A.BIN;1", DATA, 2048, MULTI_EXTENT, &[]),
            record(b"B.BIN;1", DATA, 1, 0, &[]),
        ],
        // An extent past the image's end.
        vec![record(b"FAR.BIN;1", 1 << 20, 1, 0, &[])],
    ];
    for records in cases {
        let error = block_on(Cdfs::open(image(&[], &records, &[(DATA, b"x")]))).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt, "{records:?}");
    }
}

/// The root's `.` record must name a directory where the descriptor
/// says.
#[test]
fn refuses_a_bad_root() {
    let mut image = image(&[], &[], &[]);
    image[ROOT as usize * 2048 + 25] = 0;
    let error = block_on(Cdfs::open(image)).unwrap_err();
    assert_eq!(source(&error), CdfsError::BadRoot);
}
