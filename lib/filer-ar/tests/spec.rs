// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against archives written from its tree in
//! each dialect of names.

mod common;

use common::{DIALECTS, Writer};
use filer_ar::Ar;
use filer_spec::{Profile, TreeEntry};

#[test]
fn passes_the_specification() {
    // ar holds only files: no symlinks, FIFOs, or attributes, and no
    // directories but those its files' names imply.
    let profile = Profile::default()
        .with_symlinks(false)
        .with_fifos(false)
        .with_empty_dirs(false)
        .with_xattrs(false);
    for dialect in DIALECTS {
        let report = filer_spec::run_blocking(profile, |tree| {
            let mut writer = Writer::new(dialect);
            for entry in tree.entries() {
                match entry {
                    TreeEntry::Dir(_) => &mut writer,
                    TreeEntry::File(path, contents) => {
                        writer.file(path.as_str().as_bytes(), contents)
                    },
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
            filer_spec::block_on(Ar::open(writer.finish())).unwrap()
        });
        println!("{dialect:?}: {report}");
        report.assert_passed();
    }
}
