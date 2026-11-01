# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to
[Semantic Versioning]. Releases are 0.0.x while the API is experimental,
so any release may break compatibility.

## Unreleased
### Added
- `filer-posix`: Kernel-confined opens with `openat2` on Linux, falling
  back to the userspace walk; `Posix::with_userspace_resolution`.

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
