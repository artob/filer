// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against archives written from its tree in
//! each format.

mod common;

use common::{FORMATS, Writer};
use filer_cpio::Cpio;
use filer_spec::{Profile, TreeEntry};

#[test]
fn passes_the_specification() {
    // cpio records everything the specification's tree describes but
    // extended attributes.
    for format in FORMATS {
        let report = filer_spec::run_blocking(Profile::default().with_xattrs(false), |tree| {
            let mut writer = Writer::new(format);
            for entry in tree.entries() {
                match entry {
                    TreeEntry::Dir(path) => writer.dir(path.as_str().as_bytes()),
                    TreeEntry::File(path, contents) => {
                        writer.file(path.as_str().as_bytes(), contents)
                    },
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
            filer_spec::block_on(Cpio::open(writer.finish())).unwrap()
        });
        println!("{format:?}: {report}");
        report.assert_passed();
    }
}
