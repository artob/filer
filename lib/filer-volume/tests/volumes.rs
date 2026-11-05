// This is free and unencumbered software released into the public domain.

//! Volumes of each format, detected and opened as they are, as partitions
//! of a disk, and the errors of those that cannot be. Each format's
//! volume is a fixture of its own crate, in the workspace, so it is not
//! copied here. Tests of a format this build does not read check that it
//! is refused instead, so they pass with any features.

use filer_api::{Dir, Driver, ErrorKind, Rights};
use filer_spec::{Sparse, block_on};
use filer_volume::{Volume, VolumeDriver, VolumeError, VolumeFormat, VolumeOptions};

/// A volume of each format, as the path of a fixture in the workspace.
const VOLUMES: [(VolumeFormat, &str); 11] = [
    (
        VolumeFormat::Ext2,
        "filer-ext4/tests/fixtures/ext2-attrs.sparse",
    ),
    (
        VolumeFormat::Ext3,
        "filer-ext4/tests/fixtures/ext3-4k.sparse",
    ),
    (
        VolumeFormat::Ext4,
        "filer-ext4/tests/fixtures/ext4-4k.sparse",
    ),
    (VolumeFormat::Fat12, "filer-fat/tests/fixtures/fat12.sparse"),
    (VolumeFormat::Fat16, "filer-fat/tests/fixtures/fat16.sparse"),
    (VolumeFormat::Fat32, "filer-fat/tests/fixtures/fat32.sparse"),
    (VolumeFormat::Iso9660, "filer-cdfs/tests/fixtures/spec.iso"),
    (
        VolumeFormat::Ntfs,
        "filer-ntfs/tests/fixtures/ntfs-links.sparse",
    ),
    (
        VolumeFormat::Squashfs,
        "filer-squashfs/tests/fixtures/spec.sqfs",
    ),
    (VolumeFormat::Udf, "filer-udf/tests/fixtures/spec.sparse"),
    (VolumeFormat::Xfs, "filer-xfs/tests/fixtures/spec.sparse"),
];

fn fixture(path: &str) -> Vec<u8> {
    let bytes = std::fs::read(format!("{}/../{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    // SquashFS and ISO 9660 images are small, and stored as they are.
    if path.ends_with(".sqfs") || path.ends_with(".iso") {
        return bytes;
    }
    Sparse::new(&bytes).to_vec()
}

fn hello() -> Vec<u8> {
    std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/hello.img")).unwrap()
}

fn volume_error(error: &filer_api::Error) -> Option<VolumeError> {
    core::error::Error::source(error)
        .and_then(|source| source.downcast_ref())
        .copied()
}

/// Lists `root`, sorted, as text: a foreign name differs between drivers.
fn names<D: Driver>(root: &Dir<D>) -> Vec<String> {
    let names = block_on(root.names()).unwrap();
    let mut names: Vec<String> = names.iter().map(ToString::to_string).collect();
    names.sort();
    names
}

/// Returns what `driver` itself lists at the root of `image`.
fn listed_by_its_driver(driver: VolumeDriver, image: Vec<u8>) -> Vec<String> {
    match driver {
        #[cfg(feature = "cdfs")]
        VolumeDriver::Cdfs => names(&block_on(filer_volume::cdfs::Cdfs::open(image)).unwrap()),
        #[cfg(feature = "ext4")]
        VolumeDriver::Ext4 => names(&block_on(filer_volume::ext4::Ext4::open(image)).unwrap()),
        #[cfg(feature = "fat")]
        VolumeDriver::Fat => names(&block_on(filer_volume::fat::Fat::open(image)).unwrap()),
        #[cfg(feature = "ntfs")]
        VolumeDriver::Ntfs => names(&block_on(filer_volume::ntfs::Ntfs::open(image)).unwrap()),
        #[cfg(feature = "squashfs")]
        VolumeDriver::Squashfs => {
            names(&block_on(filer_volume::squashfs::Squashfs::open(image)).unwrap())
        },
        #[cfg(feature = "udf")]
        VolumeDriver::Udf => names(&block_on(filer_volume::udf::Udf::open(image)).unwrap()),
        #[cfg(feature = "xfs")]
        VolumeDriver::Xfs => names(&block_on(filer_volume::xfs::Xfs::open(image)).unwrap()),
        _ => unreachable!("{driver} is disabled, so {} bytes went unread", image.len()),
    }
}

#[test]
fn detects_and_opens_each_format() {
    for (format, path) in VOLUMES {
        let image = fixture(path);
        let driver = format.driver();
        assert_eq!(VolumeFormat::detect(&image), Some(format), "{path}");
        assert_eq!(VolumeDriver::detect(&image), Some(driver), "{path}");
        let opened = block_on(Volume::open(image.clone()));
        if !driver.is_enabled() {
            let error = opened.unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Unsupported, "{path}");
            assert_eq!(volume_error(&error), Some(VolumeError::Disabled(driver)));
            continue;
        }
        // The SquashFS image is compressed by gzip.
        if format == VolumeFormat::Squashfs && !cfg!(feature = "gzip") {
            assert_eq!(opened.unwrap_err().kind(), ErrorKind::Unsupported);
            continue;
        }
        let root = opened.unwrap();
        assert_eq!(root.driver().volume_driver(), driver);
        assert_eq!(root.driver().format(), Some(format), "{path}");
        assert_eq!(root.rights(), Rights::READ);
        assert_eq!(
            names(&root),
            listed_by_its_driver(driver, image.clone()),
            "{path}"
        );
        // Opened with its driver, it is still known for its format.
        let root = block_on(Volume::open_as(image, driver)).unwrap();
        assert_eq!(root.driver().format(), Some(format), "{path}");
    }
}

#[test]
fn opens_partitions_of_disks() {
    // An MBR naming one FAT partition, `hello.img`, from sector 2048.
    let image = hello();
    let mut disk = vec![0_u8; 1 << 20];
    disk[446 + 4] = 0x01;
    disk[446 + 8..446 + 12].copy_from_slice(&2048_u32.to_le_bytes());
    let sectors = u32::try_from(image.len() / 512).unwrap();
    disk[446 + 12..446 + 16].copy_from_slice(&sectors.to_le_bytes());
    disk[510..512].copy_from_slice(&[0x55, 0xAA]);
    disk.extend_from_slice(&image);

    let error = block_on(Volume::open(disk.clone())).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Unsupported);
    assert_eq!(volume_error(&error), Some(VolumeError::Partitioned));
    let opened = block_on(Volume::open_partition(
        disk.clone(),
        1,
        VolumeOptions::default(),
    ));
    if VolumeDriver::Fat.is_enabled() {
        let root = opened.unwrap();
        let file = block_on(root.open_file("hello/world.txt")).unwrap();
        assert_eq!(block_on(file.read_to_end()).unwrap(), b"Hello, world!\n");
    } else {
        assert_eq!(opened.unwrap_err().kind(), ErrorKind::Unsupported);
    }
    let error = block_on(Volume::open_partition(disk, 2, VolumeOptions::default())).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert_eq!(volume_error(&error), Some(VolumeError::NoPartition(2)));
    // A bare volume has no partition table.
    let error = block_on(Volume::open_partition(image, 1, VolumeOptions::default())).unwrap_err();
    assert_eq!(volume_error(&error), Some(VolumeError::NoPartition(1)));
}

#[test]
fn refuses_what_is_no_volume() {
    for bytes in [Vec::new(), vec![0; 4096], b"not a volume".to_vec()] {
        let error = block_on(Volume::open(bytes)).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert_eq!(volume_error(&error), Some(VolumeError::Unknown));
    }
    assert_eq!(VolumeError::Unknown.to_string(), "no known file system");
    assert_eq!(
        VolumeError::Disabled(VolumeDriver::Xfs).to_string(),
        "the xfs driver is not enabled"
    );
    // A volume opened as another format is that format's to refuse.
    for driver in [VolumeDriver::Ext4, VolumeDriver::Ntfs, VolumeDriver::Xfs] {
        if driver.is_enabled() {
            let error = block_on(Volume::open_as(hello(), driver)).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::Corrupt, "{driver}");
        }
    }
}

#[test]
fn passes_options_through() {
    // A volume whose journal needs recovery: replayed by default, read as
    // it is on disk with `allow_dirty`.
    if !VolumeDriver::Ext4.is_enabled() {
        return;
    }
    let image = fixture("filer-ext4/tests/fixtures/ext3-dirty.sparse");
    let replayed = block_on(Volume::open(image.clone())).unwrap();
    let options = VolumeOptions::default().allow_dirty(true);
    let as_is = block_on(Volume::open_with(image, options)).unwrap();
    assert_ne!(names(&replayed), names(&as_is));
}
