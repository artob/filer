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
