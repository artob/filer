// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against a volume holding its tree,
//! written through the Linux kernel by `tests/fixtures/make.sh`.

mod common;

use filer_spec::{Profile, TreeEntry};
use filer_xfs::Xfs;

#[test]
fn passes_the_specification() {
    check(include_bytes!("fixtures/spec.sparse"));
}

/// On a version 5 volume.
#[test]
fn passes_the_specification_v5() {
    check(include_bytes!("fixtures/spec5.sparse"));
}

/// On volumes shut down before their logs were written back, which are
/// read as their logs replay.
#[test]
fn passes_the_specification_after_recovery() {
    check(include_bytes!("fixtures/xfs-dirty.sparse"));
    check(include_bytes!("fixtures/xfs5d-dirty.sparse"));
}

/// Runs the specification against the volume in `sparse`.
fn check(sparse: &'static [u8]) {
    let profile = Profile::default();
    let report = filer_spec::run_blocking(profile, |tree| {
        // The fixture holds exactly this tree; if the specification's
        // changes, `make.sh`'s `spec` must follow.
        let paths: Vec<String> = tree
            .entries()
            .iter()
            .map(|entry| match entry {
                TreeEntry::Dir(path) => format!("{path}/"),
                TreeEntry::File(path, contents) => format!("{path}={}", contents.len()),
                TreeEntry::Symlink(path, target) => format!("{path}->{target}"),
                TreeEntry::Xattr(path, name, value) => format!("{path}:{name}={}", value.len()),
                TreeEntry::Fifo(path) => format!("{path}|"),
                TreeEntry::ForeignFile {
                    dir,
                    name,
                    contents,
                } => format!("{dir}/{}={}", name.escape_ascii(), contents.len()),
                entry => panic!("unexpected entry: {entry:?}"),
            })
            .collect();
        let mut expected = vec![
            "a/",
            "a/b/",
            "a/b/c.txt=8",
            "a/z.txt=0",
            "a/empty/",
            "a/b/c.txt:user.comment=9",
            "a/b/c.txt:user.empty=0",
            "a:user.tag=3",
            "a/link->b/c.txt",
            "a/up->../a/b/c.txt",
            "a/abs->/a/b/c.txt",
            "a/escape->../../c.txt",
            "a/loop->loop",
            "a/dangling->nope",
            "many/",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
        expected.extend((0..600).map(|i| format!("many/f{i:03}=0")));
        expected.extend(
            [
                "special/",
                "special/fifo|",
                "foreign/",
                "foreign/caf\\xe9=7",
            ]
            .map(String::from),
        );
        assert_eq!(paths, expected, "the specification's tree changed");
        let image = common::Sparse::new(sparse);
        filer_spec::block_on(Xfs::open(image)).unwrap()
    });
    println!("{report}");
    report.assert_passed();
}
