# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to
[Semantic Versioning]. Releases are 0.0.x while the API is experimental,
so any release may break compatibility.

## Unreleased
### Added
- `filer-posix`: Kernel-confined opens with `openat2` on Linux, falling
  back to the userspace walk; `Posix::with_userspace_resolution`.
- `filer-api`: `Spawner` and `Job`; under `std`, `run_on` and
  `DefaultPool`, the built-in thread pool.
- `filer-posix`, `filer-system`: `thread_pool(spawner)` constructors, which
  run each operation on a `Spawner` instead of inside `poll`.
- `filer-ar`, `filer-cpio`, and `filer-rar`: New crates, with no items yet.
- `filer-ext4`, `filer-ntfs`, `filer-udf`, and `filer-xfs`: New crates for
  read-only file system drivers, with no items yet but `filer-ntfs`'s
  `Ntfs` driver, `Options`, and `NtfsError`.
- `filer-fat`: New crate. `Fat`, a read-only driver for FAT12, FAT16, and
  FAT32 volumes, with `Options`, `UtcOffset`, `FatType`, and `FatError`;
  the `code-pages` feature decodes short names in any `CodePage`.
- `known-codepages`: New crate. `CodePage`, the single-byte DOS and
  Windows code pages, decoded by table.
- `filer-api`: `archive`, the shared machinery of archive drivers:
  `Archive`, `Index`, `Member`, `Limits`, and `ArchiveError`.
- `filer-api`: Reading extended attributes: `Op::Xattr` with `XattrOp`,
  `XattrName`, `XattrNs`, `XattrEntry`, and `XattrNameError`;
  `File::xattr`, `xattr_into`, and `xattr_names`, likewise on `Dir`;
  `DriverKey::xattr_entry`; `Output::Xattr` and `Output::XattrNames`.
- `filer-api`: `FileAttrs` (file flags) in `Metadata::attrs`, requested
  with `MetaMask::ATTRS`.
- `filer-api`: `Op::unsupported` and `Op::into_buf`, so a driver's answer
  to an operation it does not implement returns the operation's buffer.
- `filer-heap`: Extended attributes, set by `HeapBuilder::xattr`.
- `filer-spec`: `Profile::xattrs`, `TreeEntry::Xattr`, and the
  `reads_xattrs` case.
### Changed
- `filer-api`: `Resolver` is now asynchronous, as async-first; the
  synchronous walk is `BlockingResolver`, and both share one
  implementation.
- `filer-tar`: `Tar` is a thin wrapper around `filer_api::archive::Archive`;
  `filer_tar::Limits` is now a re-export of `filer_api::archive::Limits`.

## 0.0.1 - 2026-11-01
The first usable version: read-only access through a capability-based
frontend, over in-memory, POSIX, and TAR archive drivers.
### Added
- `filer-api`:
  - Paths: `path::Name`, `path::NameBuf`, `path::RelPath`, and
    `path::RelPathBuf`, with `NameError`, `RelPathError`, `Components`,
    and the compile-time-checked `name!` and `rel!` macros.
  - Frontend: `Dir`, `File` (with `Mode`, `Readable`, and `ReadOnly`),
    `ReadDir`, and `Rights`; `AmbientAuthority` and `ambient_authority()`.
  - Names on disk: `EntryName`, `ForeignName`, and `LinkTarget`.
  - Metadata: `Metadata`, `MetaMask`, `FileKind`, `FileId`, and
    `Timestamp`.
  - Errors: `Error`, `ErrorKind`, `OpKind`, `RawOsError`, `Fallback`, and
    `Result`.
  - Resolution: `Resolve`, `Symlinks`, and the userspace `Resolver`, with
    `MAX_SYMLINK_HOPS`.
  - Buffers and sources: `IoBuf`, `IoBufMut`, `BufResult`, `ErasedBufMut`,
    and `ReadAt`.
  - Drivers: `Driver`, `DynDriver`, `DriverKey`, `DriverId`, `Handle`, and
    `HandleKind`; the operations `Op`, `OpenHow`, `Target`, `Entry`,
    `DirCookie`, `Output`, and `Completion`.
  - Virtual drivers: `tree::Tree`, `tree::Node`, `tree::Ino`, and
    `tree::ROOT`.
- `filer-heap`: New crate. `Heap`, an in-memory driver, and `HeapBuilder`.
- `filer-posix`: `Posix`, a driver over `rustix`, with a blocking executor.
- `filer-system`: New crate. `System`, the target's native driver, and
  `open_ambient`; re-exports `filer-api`.
- `filer-spec`: New crate. The executable specification of the driver
  contract: `run`, `run_blocking`, `Profile`, `Tree`, `TreeEntry`,
  `Report`, `Verdict`, and `block_on`.
- `filer-tar`: New crate. `Tar`, a driver over uncompressed TAR archives,
  with `Limits`.
- `filer-whatwg` and `filer-zip`: New crates, with no items yet.

## 0.0.0 - 2026-10-31
### Added
- `filer-api`, `filer-posix`, `filer-wasi`, `filer-windows`, and
  `filer-cli`: New crates, with no items yet.

[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
