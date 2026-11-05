// This is free and unencumbered software released into the public domain.

//! Volumes made here, descriptor by descriptor, for what `mkudffs` and
//! the Linux kernel do not write: metadata partitions, packets a sparing
//! table moved, allocation descriptors continued elsewhere, hidden files,
//! and descriptors that are malformed.

mod common;

use common::{
    LEN, START, file_entry, file_set, identifier, long_ad, named_map, physical_map, put, short_ad,
    tagged, volume,
};
use filer_api::{Dir, ErrorKind, FileAttrs, MetaMask};
use filer_spec::block_on;
use filer_udf::{Udf, UdfError};

type Root = Dir<Udf<Vec<u8>>>;

/// File types: a directory, a file, a symlink, and a metadata file.
const DIR: u8 = 4;
const FILE: u8 = 5;
const SYMLINK: u8 = 12;
const METADATA: u8 = 250;

/// How data is allocated.
const SHORT: u16 = 0;
const LONG: u16 = 1;
const EMBEDDED: u16 = 3;

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

fn source(error: &filer_api::Error) -> UdfError {
    *core::error::Error::source(error)
        .unwrap()
        .downcast_ref::<UdfError>()
        .unwrap()
}

/// Writes, at partition block `block` of a physical partition, `bytes`.
fn at(image: &mut [u8], block: u32, bytes: &[u8]) {
    put(image, (START + block) as usize, bytes);
}

/// A root directory whose identifiers are `identifiers`, embedded, after
/// its parent's.
fn root_entry(location: u32, reference: u16, identifiers: &[Vec<u8>]) -> Vec<u8> {
    let mut data = identifier(&[], 0x0A, location, reference);
    for identifier in identifiers {
        data.extend_from_slice(identifier);
    }
    file_entry(location, DIR, data.len() as u64, EMBEDDED, &data)
}

/// A metadata partition: its file, in the physical partition, maps the
/// partition's blocks, where the file set, the root, and a file are.
#[test]
fn reads_a_metadata_partition() {
    let mut location = 10_u32.to_le_bytes().to_vec();
    location.extend(10_u32.to_le_bytes());
    let maps = [
        physical_map(),
        named_map(b"*UDF Metadata Partition", &location),
    ];
    let mut image = volume(600, &maps, 0, 1);
    // The metadata file: four blocks, from block 20 of the physical
    // partition.
    at(
        &mut image,
        10,
        &file_entry(10, METADATA, 2048, SHORT, &short_ad(2048, 20, 0)),
    );
    at(&mut image, 20, &file_set(0, 1, 1));
    let hello = identifier(b"\x08hello", 0, 2, 1);
    at(&mut image, 21, &root_entry(1, 1, &[hello]));
    at(&mut image, 22, &file_entry(2, FILE, 2, EMBEDDED, b"hi"));
    let root = block_on(Udf::open(image)).unwrap();
    assert_eq!(names(&root, ""), ["hello"]);
    assert_eq!(read(&root, "hello").unwrap(), b"hi");
}

/// A sparable partition, whose packet of four blocks from block 4 a
/// sparing table moved to sector 500: the root is read there.
#[test]
fn reads_moved_packets() {
    let mut rest = 4_u16.to_le_bytes().to_vec();
    rest.extend([1, 0]);
    rest.extend(512_u32.to_le_bytes());
    rest.extend(450_u32.to_le_bytes());
    let maps = [named_map(b"*UDF Sparable Partition", &rest)];
    let mut image = volume(600, &maps, 0, 0);
    let mut entries = 1_u16.to_le_bytes().to_vec();
    entries.extend([0; 6]);
    entries.extend(4_u32.to_le_bytes());
    entries.extend(500_u32.to_le_bytes());
    let table = tagged(0, 450, &[(17, b"*UDF Sparing Table"), (48, &entries)]);
    put(&mut image, 450, &table);
    at(&mut image, 0, &file_set(0, 5, 0));
    let hello = identifier(b"\x08hello", 0, 6, 0);
    // Where the root and the file would be, but for the table; and where
    // they are.
    at(&mut image, 5, &[0xFF; 512]);
    put(&mut image, 501, &root_entry(5, 0, &[hello]));
    put(&mut image, 502, &file_entry(6, FILE, 2, EMBEDDED, b"hi"));
    let root = block_on(Udf::open(image)).unwrap();
    assert_eq!(read(&root, "hello").unwrap(), b"hi");
}

/// A file of long allocation descriptors, continued in an allocation
/// extent descriptor; with an extent not recorded, which reads as zeros.
#[test]
fn follows_allocation_extents() {
    let maps = [physical_map()];
    let mut image = volume(600, &maps, 0, 0);
    at(&mut image, 0, &file_set(0, 1, 0));
    let file = identifier(b"\x08file", 0, 2, 0);
    at(&mut image, 1, &root_entry(1, 0, &[file]));
    let first = long_ad(512, 10, 0);
    let unrecorded = long_ad(512 | 1 << 30, 0, 0);
    let next = long_ad(512 | 3 << 30, 3, 0);
    let area = [first, unrecorded, next].concat();
    at(&mut image, 2, &file_entry(2, FILE, 1536, LONG, &area));
    let mut allocated = (16_u32).to_le_bytes().to_vec();
    allocated.extend(long_ad(512, 11, 0));
    at(&mut image, 3, &tagged(258, 3, &[(20, &allocated)]));
    at(&mut image, 10, &[b'a'; 512]);
    at(&mut image, 11, &[b'b'; 512]);
    let root = block_on(Udf::open(image)).unwrap();
    let mut expected = vec![b'a'; 512];
    expected.extend([0; 512]);
    expected.extend([b'b'; 512]);
    assert_eq!(read(&root, "file").unwrap(), expected);
}

/// Allocation extent descriptors that continue in themselves are
/// malformed, not a hang.
#[test]
fn refuses_endless_allocation_extents() {
    let maps = [physical_map()];
    let mut image = volume(600, &maps, 0, 0);
    at(&mut image, 0, &file_set(0, 1, 0));
    let file = identifier(b"\x08loop", 0, 2, 0);
    at(&mut image, 1, &root_entry(1, 0, &[file]));
    at(
        &mut image,
        2,
        &file_entry(2, FILE, 512, SHORT, &short_ad(512, 3, 3)),
    );
    let mut allocated = 8_u32.to_le_bytes().to_vec();
    allocated.extend(short_ad(512, 3, 3));
    at(&mut image, 3, &tagged(258, 3, &[(20, &allocated)]));
    let root = block_on(Udf::open(image)).unwrap();
    let error = read(&root, "loop").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    assert!(matches!(source(&error), UdfError::BadExtents { .. }));
}

/// Hidden files are shown, marked; deleted ones are not; names of 16-bit
/// characters, unpaired surrogates too; and a symlink's components.
#[test]
fn reads_identifiers() {
    let maps = [physical_map()];
    let mut image = volume(600, &maps, 0, 0);
    at(&mut image, 0, &file_set(0, 1, 0));
    let identifiers = [
        identifier(b"\x08secret", 0x01, 2, 0),
        identifier(b"\x08gone", 0x04, 2, 0),
        identifier(b"\x10\0w\xD8\x3D\xDE\x00", 0, 2, 0),
        identifier(b"\x10\xD8\x00", 0, 2, 0),
        identifier(b"\x08link", 0, 3, 0),
    ];
    at(&mut image, 1, &root_entry(1, 0, &identifiers));
    at(&mut image, 2, &file_entry(2, FILE, 2, EMBEDDED, b"hi"));
    let components = [&[2_u8, 0, 0, 0][..], &[5, 4, 0, 0, 8, b'e', b't', b'c']].concat();
    at(
        &mut image,
        3,
        &file_entry(3, SYMLINK, components.len() as u64, EMBEDDED, &components),
    );
    let root = block_on(Udf::open(image)).unwrap();
    let listed = names(&root, "");
    assert_eq!(listed.len(), 4, "{listed:?}");
    assert!(listed.contains(&"secret".to_owned()) && listed.contains(&"w😀".to_owned()));
    let metadata = block_on(root.symlink_metadata("secret", MetaMask::all())).unwrap();
    assert!(metadata.attrs.unwrap().contains(FileAttrs::HIDDEN));
    assert_eq!(read(&root, "w😀").unwrap(), b"hi");
    assert_eq!(read(&root, "gone").unwrap_err().kind(), ErrorKind::NotFound);
    let target = block_on(root.read_link("link")).unwrap();
    assert_eq!(target.as_bytes(), b"/etc");
    let metadata = block_on(root.symlink_metadata("link", MetaMask::all())).unwrap();
    assert_eq!(metadata.len, Some(4));
}

/// A root directory whose identifier's tag is invalid: opening lists it.
#[test]
fn refuses_malformed_identifiers() {
    let maps = [physical_map()];
    let mut image = volume(600, &maps, 0, 0);
    at(&mut image, 0, &file_set(0, 1, 0));
    let mut bad = identifier(b"\x08bad", 0, 2, 0);
    bad[4] = bad[4].wrapping_add(1);
    at(&mut image, 1, &root_entry(1, 0, &[bad]));
    let error = block_on(Udf::open(image)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    assert!(matches!(source(&error), UdfError::BadDirectory { .. }));
}

/// Partition maps of a kind this driver does not read, and blocks
/// outside a partition.
#[test]
fn refuses_unknown_partitions_and_blocks() {
    let maps = [named_map(b"*UDF Unknown Partition", &[])];
    let error = block_on(Udf::open(volume(600, &maps, 0, 0))).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert_eq!(source(&error), UdfError::UnsupportedPartition);
    let maps = [physical_map()];
    let error = block_on(Udf::open(volume(600, &maps, LEN, 0))).unwrap_err();
    assert_eq!(source(&error), UdfError::BadFileSet);
}
