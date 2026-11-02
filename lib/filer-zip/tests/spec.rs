// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against archives written from its tree.

mod common;

use common::Writer;
use filer_spec::{Profile, TreeEntry};
use filer_zip::Zip;

/// Writes the specification's tree as an archive.
fn write(tree: &filer_spec::Tree, zip64: bool, deflate: bool) -> Vec<u8> {
    let mut writer = Writer {
        zip64,
        deflate,
        ..Writer::default()
    };
    for entry in tree.entries() {
        match entry {
            TreeEntry::Dir(path) => writer.dir(format!("{path}/").as_bytes()),
            TreeEntry::File(path, contents) => writer.file(path.as_str().as_bytes(), contents),
            TreeEntry::Symlink(path, target) => {
                writer.symlink(path.as_str().as_bytes(), target.as_bytes())
            },
            TreeEntry::Fifo(path) => writer.fifo(path.as_str().as_bytes()),
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
    writer.finish()
}

#[test]
fn passes_the_specification() {
    // With Unix modes, ZIP records everything the specification's tree
    // describes but extended attributes.
    for (zip64, deflate) in [(false, false), (true, false), (false, true)] {
        let report = filer_spec::run_blocking(Profile::default().with_xattrs(false), |tree| {
            filer_spec::block_on(Zip::open(write(tree, zip64, deflate))).unwrap()
        });
        println!("{report}");
        report.assert_passed();
    }
}
