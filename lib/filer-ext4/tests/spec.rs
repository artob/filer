// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against a volume holding its tree,
//! written through the Linux kernel by `tests/fixtures/make.sh`.

mod common;

use filer_ext4::Ext4;
use filer_spec::{Profile, TreeEntry};

#[test]
fn passes_the_specification() {
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
        let image = common::unsparse(include_bytes!("fixtures/spec.sparse"));
        filer_spec::block_on(Ext4::open(image)).unwrap()
    });
    println!("{report}");
    report.assert_passed();
}
