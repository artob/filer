// This is free and unencumbered software released into the public domain.

//! Read-only volumes of any supported file system for the Filer API.
//!
//! [`Volume`] opens a file system volume, such as a disk image or one of
//! its partitions, in whichever format it is in, detected from its first
//! bytes, and serves it as a [`Dir`](filer_api::Dir), read-only, through
//! that format's own driver. It is the one dependency a program needs to
//! read volumes whatever their file system, as `filer-archive` is for
//! archives:
//!
//! | Format | Detected by | Feature | Driver |
//! | --- | --- | --- | --- |
//! | ext2, ext3, ext4 | superblock magic number, at offset 1080 | `ext4` | `filer-ext4` |
//! | FAT12, FAT16, FAT32 | boot sector, as `filer-fat` checks it | `fat`, `code-pages` | `filer-fat` |
//! | NTFS | `NTFS` at offset 3 | `ntfs` | `filer-ntfs` |
//! | XFS | `XFSB` at offset 0 | `xfs` | `filer-xfs` |
//!
//! Each format's feature enables its driver, which is then also
//! re-exported here as a module (as [`ext4`] for `filer-ext4`), for what
//! is particular to it, such as its [`Options`]' parts; `code-pages`
//! forwards `filer-fat`'s, making every DOS code page available for FAT's
//! short names. `udf` enables `filer-udf`, which reads nothing yet, so
//! [`Format`] has no UDF until it does. `all`, enabled by default,
//! enables every format, but not `code-pages`, as `filer-fat`'s does not.
//! Without a format's feature, [`Format`] still names it, and opening it
//! fails with [`Unsupported`](filer_api::ErrorKind::Unsupported).
//!
//! A disk image with a partition table (MBR or GPT) is not a volume, but
//! [`Volume::open_partition`] opens one of its partitions.
//!
//! It needs no OS and builds with `#![no_std]`; the `std` feature, enabled
//! by default, forwards to `filer-api` and the drivers.
//!
//! # Examples
//!
//! A FAT volume, as `mkfs.fat` makes one:
//!
//! ```
//! use filer_volume::Volume;
//!
//! # #[cfg(feature = "fat")]
//! # filer_spec::block_on(async {
//! let image: &'static [u8] = include_bytes!("../tests/data/hello.img");
//! let root = Volume::open(image).await?;
//! assert_eq!(root.names().await?, ["hello"]);
//! let file = root.open_file("hello/world.txt").await?;
//! assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
//! # Ok::<(), filer_api::Error>(())
//! # }).unwrap();
//! ```
//!
//! The same volume as the first partition of a disk image, whose MBR
//! places it at sector 2048:
//!
//! ```
//! use filer_api::ErrorKind;
//! use filer_volume::{FormatError, Options, Volume};
//!
//! # #[cfg(feature = "fat")]
//! # filer_spec::block_on(async {
//! let volume = include_bytes!("../tests/data/hello.img");
//! // An MBR naming one FAT12 partition (type 0x01) of 128 sectors, from
//! // sector 2048, where the volume follows.
//! let mut disk = vec![0; 2048 * 512];
//! disk[446 + 4] = 0x01;
//! disk[446 + 8..446 + 12].copy_from_slice(&2048_u32.to_le_bytes());
//! disk[446 + 12..446 + 16].copy_from_slice(&128_u32.to_le_bytes());
//! disk[510..512].copy_from_slice(&[0x55, 0xAA]);
//! disk.extend_from_slice(volume);
//!
//! // A disk is not a volume, but each of its partitions is.
//! let error = Volume::open(disk.clone()).await.unwrap_err();
//! assert_eq!(error.kind(), ErrorKind::Unsupported);
//! let source = core::error::Error::source(&error);
//! let reason = source.and_then(|source| source.downcast_ref());
//! assert_eq!(reason, Some(&FormatError::Partitioned));
//! let root = Volume::open_partition(disk, 1, Options::default()).await?;
//! let file = root.open_file("hello/world.txt").await?;
//! assert_eq!(file.read_to_end().await?, b"Hello, world!\n");
//! # Ok::<(), filer_api::Error>(())
//! # }).unwrap();
//! ```

#![no_std]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// No input may make this crate panic (DESIGN.md D47).
#![deny(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::let_underscore_must_use,
    clippy::panic,
    clippy::string_slice,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::unwrap_used,
    unused_must_use
)]
// Tests may panic: that is how they fail.
#![cfg_attr(
    test,
    allow(
        clippy::arithmetic_side_effects,
        clippy::as_conversions,
        clippy::expect_used,
        clippy::indexing_slicing,
        clippy::let_underscore_must_use,
        clippy::panic,
        clippy::unreachable,
        clippy::unwrap_used
    )
)]
// The crate docs link to items that exist only with some features.
#![cfg_attr(not(feature = "ext4"), allow(rustdoc::broken_intra_doc_links))]

extern crate alloc;

mod format;
pub use format::*;

mod volume;
pub use volume::*;

#[cfg(feature = "ext4")]
#[doc(no_inline)]
pub use filer_ext4 as ext4;

#[cfg(feature = "fat")]
#[doc(no_inline)]
pub use filer_fat as fat;

#[cfg(feature = "ntfs")]
#[doc(no_inline)]
pub use filer_ntfs as ntfs;

#[cfg(feature = "udf")]
#[doc(no_inline)]
pub use filer_udf as udf;

#[cfg(feature = "xfs")]
#[doc(no_inline)]
pub use filer_xfs as xfs;
