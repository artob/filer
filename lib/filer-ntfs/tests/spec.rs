// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against a volume holding its tree,
//! written through ntfs-3g by `tests/fixtures/make.sh`.

mod common;

use filer_ntfs::Ntfs;
use filer_spec::{Profile, TreeEntry};

#[test]
fn passes_the_specification() {
    // No symlinks or FIFOs yet (NTFS-08), and the specification's foreign
    // name is not WTF-8 (SPEC-06).
    let profile = Profile::default()
        .with_symlinks(false)
        .with_fifos(false)
        .with_foreign_names(false);
    let report = filer_spec::run_blocking(profile, |tree| {
        // The fixture holds exactly this tree; if the specification's
        // changes, `make.sh`'s `spec` must follow.
        let paths: Vec<String> = tree
            .entries()
            .iter()
            .map(|entry| match entry {
                TreeEntry::Dir(path) => format!("{path}/"),
                TreeEntry::File(path, contents) => format!("{path}={}", contents.len()),
                entry => panic!("unexpected entry: {entry:?}"),
            })
            .collect();
        let mut expected = vec![
            "a/",
            "a/b/",
            "a/b/c.txt=8",
            "a/z.txt=0",
            "a/empty/",
            "many/",
        ]
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
        expected.extend((0..600).map(|i| format!("many/f{i:03}=0")));
        assert_eq!(paths, expected, "the specification's tree changed");
        let image = common::unsparse(include_bytes!("fixtures/spec.sparse"));
        filer_spec::block_on(Ntfs::open(image)).unwrap()
    });
    println!("{report}");
    report.assert_passed();
}
