// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against archives written from its tree.

mod common;

use common::Writer;
use filer_spec::{Profile, TreeEntry};
use filer_tar::Tar;

#[test]
fn passes_the_specification() {
    // TAR records everything the specification's tree describes.
    let report = filer_spec::run_blocking(Profile::default().with_xattrs(false), |tree| {
        let mut writer = Writer::default();
        for entry in tree.entries() {
            match entry {
                TreeEntry::Dir(path) => writer.dir(format!("{path}/").as_bytes()),
                TreeEntry::File(path, contents) => writer.file(path.as_str().as_bytes(), contents),
                TreeEntry::Symlink(path, target) => {
                    writer.symlink(path.as_str().as_bytes(), target.as_bytes())
                },
                TreeEntry::Fifo(path) => writer.member(path.as_str().as_bytes(), b'6', b"", b""),
                TreeEntry::ForeignFile {
                    dir,
                    name,
                    contents,
                } => {
                    let mut path = format!("{dir}/").into_bytes();
                    path.extend_from_slice(name);
                    writer.file(&path, contents)
                },
                entry => panic!("unknown entry: {entry:?}"),
            };
        }
        filer_spec::block_on(Tar::open(writer.finish())).unwrap()
    });
    println!("{report}");
    report.assert_passed();
}
