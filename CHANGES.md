# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased
### Added
- `filer-api`: New crate for the platform-independent API, with no items yet.
- `filer-posix`: New crate for the Unix backend, with no items yet.
- `filer-wasi`: New crate for the WASI backend, with no items yet.
- `filer-windows`: New crate for the Windows backend, with no items yet.
- `filer-cli`: New crate for the `filer` command, which does nothing yet.
- `filer-api`: `path::Name` and `path::NameBuf`, validated path components.
- `filer-api`: `path::RelPath` and `path::RelPathBuf`, confined relative paths.
- `filer-api`: `name!` and `rel!` macros for compile-time-checked literals.
- `filer-api`: `Error` and its `ErrorKind`, `OpKind`, and context types.
- `filer-api`: `EntryName`, `ForeignName`, `LinkTarget`, and `DriverKey`.
- `filer-api`: `Metadata`, `MetaMask`, `FileKind`, `FileId`, and `Timestamp`.
- `filer-api`: `IoBuf`, `IoBufMut`, and `BufResult` for owned I/O buffers.
- `filer-api`: `Handle` and `HandleKind`, minted through a `DriverKey`.
- `filer-api`: `Op`, `Output`, and `Completion` for read operations.
- `filer-api`: `Driver`, the trait every driver implements.
- `filer-api`: `DynDriver`, a type-erased `Driver`.
- `filer-api`: `ErrorKind::NotASymlink`.
- `filer-api`: `Resolver`, confined userspace path resolution.
- `filer-heap`: New crate for the in-memory driver, read-only so far.
- `filer-api`: `AmbientAuthority` and `ambient_authority()`.
- `filer-posix`: `Posix::blocking()`, a read-only driver over `rustix`.
- `filer-api`: `Rights`, the four coarse handle rights.
- `filer-api`: `Dir` and `File`, the read-only frontend, with `Mode` and
  `ReadOnly`; `ErrorKind::UnexpectedEof`; `Resolve::with_symlinks` and
  `From<Symlinks>`. **Breaking:** `Target::Entry` gains `resolve`.
- `filer-api`: `ReadDir` and `Dir::entries`, batched directory listings.
- `filer-heap`: `Heap::root_dir`.
- `filer-posix`: `Posix::open_ambient`, returning a labeled `Dir`.
- `filer-system`: New crate re-exporting `filer-api` with `System`, the
  target's native driver.
- `filer-cli`: Depends on `filer-system` instead of each backend.
- `filer-whatwg`: New crate for the browser backend (WHATWG File System,
  OPFS), with no items yet.
- `filer-spec`: New crate, the executable specification of the driver
  contract, run by `filer-heap` and `filer-posix`.
- `filer-api`: `ReadAt`, positional byte sources for archive drivers.
- `filer-tar`, `filer-zip`: New crates for the archive drivers, with no
  items yet.
- `filer-api`: `tree`, the immutable trees of virtual drivers, now used by
  `filer-heap`.
- `filer-tar`: `Tar`, a read-only driver over uncompressed TAR archives,
  with strict `Limits`.
- `filer-api`: `ErrorKind::LimitExceeded`.
