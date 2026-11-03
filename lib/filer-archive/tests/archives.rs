// This is free and unencumbered software released into the public domain.

//! Archives of each format, opened by name and by format, and the errors
//! of those that cannot be. `tests/data` holds small archives that real
//! tools wrote, copied from each format's crate. Tests of a format this
//! build does not read check that it is refused instead, so they pass
//! with any features.

use filer_api::{ErrorKind, Rights};
use filer_archive::{Archive, Format, FormatError, Limits};
use filer_spec::block_on;

/// Each format's archive, and a file in it with its contents.
const ARCHIVES: [(Format, &str, &str, &[u8]); 4] = [
    (Format::Ar, "hello.a", "notes.txt", b"Hello, world!\n"),
    (
        Format::Cpio,
        "hello.cpio",
        "dir/hello.txt",
        b"Hello, world!\n",
    ),
    (
        Format::Tar,
        "hello.tar",
        "hello/world.txt",
        b"Hello, world!\n",
    ),
    (
        Format::Zip,
        "hello.zip",
        "hello/world.txt",
        b"Hello, world!\n",
    ),
];

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/tests/data/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn format_error(error: &filer_api::Error) -> Option<&FormatError> {
    core::error::Error::source(error).and_then(|source| source.downcast_ref())
}

#[test]
fn opens_each_format_by_name() {
    for (format, name, path, contents) in ARCHIVES {
        let opened = block_on(Archive::open(fixture(name), name));
        if !format.is_enabled() {
            let error = opened.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Unsupported);
            assert_eq!(format_error(&error), Some(&FormatError::Disabled(format)));
            continue;
        }
        let root = opened.unwrap();
        assert_eq!(root.driver().format(), format);
        assert_eq!(root.rights(), Rights::READ);
        let file = block_on(root.open_file(path)).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), contents, "{name}");
        // The name labels the root, so errors say which archive failed.
        let error = block_on(root.open_file("nope")).unwrap_err();
        assert_eq!(
            error.to_string(),
            format!("open \"nope\" in {name}: not found")
        );
    }
}

#[test]
fn opens_archives_whose_names_say_nothing() {
    for (format, name, path, contents) in ARCHIVES {
        if !format.is_enabled() {
            continue;
        }
        let root = block_on(Archive::open_as(fixture(name), format)).unwrap();
        assert_eq!(root.label(), None);
        let file = block_on(root.open_file(path)).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), contents, "{name}");
    }
    // Upper case, as some tools write.
    if Format::Zip.is_enabled() {
        let root = block_on(Archive::open(fixture("hello.zip"), "HELLO.ZIP")).unwrap();
        assert_eq!(root.driver().format(), Format::Zip);
    }
}

#[test]
fn refuses_names_of_no_format() {
    for name in ["hello.txt", "release.tar.gz", "initrd.img", "archive"] {
        let error = block_on(Archive::open(fixture("hello.tar"), name)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported, "{name}");
        assert_eq!(
            format_error(&error),
            Some(&FormatError::Unknown(name.into())),
            "{name}"
        );
    }
    let error = FormatError::Unknown("initrd.img".into());
    assert_eq!(
        error.to_string(),
        "no archive format is named by \"initrd.img\""
    );
    let error = FormatError::Disabled(Format::Cpio);
    assert_eq!(error.to_string(), "the cpio format is not enabled");
}

#[test]
fn passes_limits_and_errors_through() {
    for (format, name, ..) in ARCHIVES {
        if !format.is_enabled() {
            continue;
        }
        let limits = Limits::default().with_max_entries(1);
        let error = block_on(Archive::open_with(fixture(name), format, limits)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::LimitExceeded, "{name}");
        // A name that lies about the format opens nothing.
        let other = if format == Format::Zip {
            "hello.tar"
        } else {
            "hello.zip"
        };
        let error = block_on(Archive::open(fixture(other), name)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Corrupt, "{other} as {name}");
    }
}
