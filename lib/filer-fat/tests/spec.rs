// This is free and unencumbered software released into the public domain.

//! The Filer specification, run against volumes written from its tree.

mod common;

use common::{Image, KINDS, Node, dir};
use filer_fat::Fat;
use filer_spec::{Profile, TreeEntry};

#[test]
fn passes_the_specification() {
    // FAT has no symlinks or FIFOs, and its only names that are not UTF-8
    // have unpaired surrogates, unlike the specification's foreign name.
    let profile = Profile::default()
        .with_symlinks(false)
        .with_fifos(false)
        .with_foreign_names(false);
    for kind in KINDS {
        for fragmented in [false, true] {
            let report = filer_spec::run_blocking(profile, |tree| {
                let mut root = dir("", Vec::new());
                for entry in tree.entries() {
                    match entry {
                        TreeEntry::Dir(path) => root.add(path.as_str(), None),
                        TreeEntry::File(path, contents) => {
                            root.add(path.as_str(), Some(contents.clone()));
                        },
                        entry => panic!("unexpected entry: {entry:?}"),
                    }
                }
                let Node::Dir(_, children) = root else {
                    unreachable!()
                };
                let image = if fragmented {
                    Image::fragmented(kind, children)
                } else {
                    Image::new(kind, children)
                };
                filer_spec::block_on(Fat::open(image.bytes)).unwrap()
            });
            println!("{kind:?}, fragmented: {fragmented}\n{report}");
            report.assert_passed();
        }
    }
}
