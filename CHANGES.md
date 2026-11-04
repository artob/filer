# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to
[Semantic Versioning]. Releases are 0.0.x while the API is experimental,
so any release may break compatibility.

## Unreleased
### Added
- `filer-apfs`, `filer-btrfs`, `filer-cdfs` (ISO 9660), `filer-exfat`,
  `filer-minix`, `filer-refs`, `filer-squashfs`, `filer-ufs`, and
  `filer-zfs`: New crates, with no items yet, published at 0.0.2 to
  reserve their names.
- `filer-msft`: New crate: Microsoft's on-disk conventions, shared by
  the drivers reading them: `nt`, `wtf8`, and `reparse`, moved from
  `filer-fs`, and `dos`, FAT's local dates and times from `filer-fat`,
  with `UtcOffset`, which `filer-fat` re-exports.
- `filer-pack`: New crate: the archive drivers' shared machinery,
  `filer_api::archive` moved whole to its root, with `spin`. `Index`
  now applies `Limits` itself, taking them in `Index::new` with a
  `Parser`, which reads a format's headers and names it in errors.
  `Verified` checks a `Digest`, such as `Crc32`, of what a `Decoder`
  produced, over another decoder or `Passthrough`. `Streams` keeps
  decoding `Stream`s part way, by `StreamId`, and `Unpacked` reads a
  packed range of a source decoded, as a `ReadAt` source itself, of a
  size declared or, with `Unpacked::measured`, learned by decoding it;
  `Compression` tells by its first bytes what a stream is compressed
  with, and decodes it. `Decoder::checkpoint` offers where decoding may
  go on from, as `Zstd` does between frames, `Bzip2` between blocks, and
  `Deflate` and `Gzip` anywhere, which `Streams` and `Unpacked` record
  and resume from, rather than from the start, given again what
  `Decoder::buffered` says the decoder held; `Verified` passes them on
  with its `Digest`, which is now `Clone`.
  `Deflate`, with the `deflate` feature, decodes what was `filer-zip`'s
  own, over `decode-deflate`, and `Deflate::deflate64`, with
  `deflate64`, Deflate64; `Gzip`, with `gzip`, decodes gzip streams over
  `decode-gzip`; `Lzma` and `Lzma2`, with `lzma`, raw LZMA and LZMA2
  streams, over `decode-lzma`; `Xz`, with `xz`, xz streams, over
  `decode-xz`, with checkpoints between blocks; `Zstd`, with `zstd`,
  zstd streams, over `decode-zstd`; and `Bzip2`, with `bzip2`, bzip2
  streams, over `decode-bzip2`. A decoder now reads the packed bytes after the
  declared size, such as gzip's trailer, before it finishes. `Reader`
  reads headers ahead, adaptively, so that the tar, ar, cpio, and ZIP
  parsers open archives of many small members in a few reads rather than
  one or two per member.
- `decode-lz4`, `decode-lzo`, and `decode-zlib`: New crates, with no
  items yet, reserving the names of decoders to come (D62).
- `decode-xz`: New crate: an xz decoder, written clean room from the
  `.xz` file format specification and dedicated to the public domain,
  over `decode-lzma`, for `no_std` with an allocator: streams, padding,
  every filter xz writes, CRC-32, CRC-64, and SHA-256 checks, and the
  index; it offers checkpoints between blocks.
- `decode-lzma`: New crate: an LZMA and LZMA2 decoder, ported from the
  LZMA SDK and dedicated to the public domain, without dependencies, for
  `no_std` with an allocator. `Decoder::lzma` decodes LZMA with the
  properties its container gives, of a known size or to its end marker,
  and `Decoder::lzma2` LZMA2; each refuses what the LZMA SDK refuses,
  and offers checkpoints anywhere.
- `decode-gzip`: New crate: a gzip decoder, written clean room from RFC
  1952 and dedicated to the public domain, over `decode-deflate`, for
  `no_std` with an allocator. It decodes members in turn, checking each
  one's CRC-32 and length, leaves what follows the last unread, and
  offers checkpoints anywhere.
- `decode-deflate`: New crate: a Deflate decoder, written clean room from
  RFC 1951 and dedicated to the public domain, without dependencies, for
  `no_std` with an allocator. It refuses what zlib refuses, consumes a
  stream to its end and no further, and offers checkpoints anywhere;
  `Decoder::deflate64`, with `deflate64`, decodes Deflate64.
- `decode-bzip2`: New crate: a bzip2 decoder, written clean room from Joe
  Tsai's description of the format and dedicated to the public domain,
  without dependencies, for `no_std` with an allocator. Its `Decoder` is
  sans-I/O, as `decode-zstd`'s is, and offers checkpoints between blocks.
- `decode-zstd`: New crate: a zstd decoder, written clean room from RFC
  8878 and dedicated to the public domain, without dependencies, for
  `no_std` with an allocator. `Decoder` is sans-I/O: `decode` takes what
  input there is and fills what room there is, `finish` checks that the
  stream ended where it may, and `checkpoint` and `buffered` let a reader
  resume between frames. `Error` is `Corrupt` or `Unsupported`.
- `filer-api`: `FileKind::from_unix_mode`, and `Tree::node_mut`.
- `filer-archive`: Formats told by content (`Format::detect`,
  `Archive::open_by_content`, and `FormatError::Unrecognized`), which
  `Archive::open` now goes by before the name. Compressed tarballs, by
  their extensions (`.tar.gz`, `.tgz`, `.tar.zst`, `.tar.xz`, `.tar.bz2`,
  and the like); the `bzip2`, `gzip`, `lzma`, `xz`, and `zstd` features,
  which `filer-cpio`, `filer-tar`, and `filer-zip` decode with.
- `filer-cpio`: Initramfs images whose later archive is compressed with
  gzip, zstd, xz, or bzip2, as distributions build them, behind features
  named as they are; `Compression`, and `Cpio::compression`.
- `filer-spec`: `Counted`, a source counting its reads; `Outcome::files`;
  `Checks::refusals` and
  `Checks::read_cap`, for archive drivers; `Mutations::aligned` and
  `Mutations::most`.
- `filer-tar`: Archives compressed with gzip, zstd, xz, or bzip2, told by
  their first bytes, behind features named as they are; `Compression`,
  and `Tar::compression`.
- `filer-zip`: Members compressed with Deflate64, bzip2, LZMA, zstd, or
  xz, behind features named as they are, which `filer-archive` forwards. Names that
  DOS and Windows hosts wrote without the UTF-8 flag, in a `CodePage`,
  437 by default, and Info-ZIP's Unicode Path extra field; `Options`,
  which `Zip::open_with` now takes, or `Limits` alone; the `code-pages`
  feature.
### Changed
- `filer-ar`, `filer-cpio`: `Limits::max_entries` counts only the
  members an archive shows, no longer ar's symbol and name tables or
  cpio's trailers.
### Removed
- `filer-api`: `archive`, now `filer-pack`'s, and the `spin` dependency.
- `filer-fs`: `nt`, `reparse`, and `wtf8`, now `filer-msft`'s, and
  `unix::mode_kind`, now `FileKind::from_unix_mode`.

## 0.0.2 - 2026-11-03
Read-only file systems (ext2/3/4, FAT, NTFS, XFS) and more archive
formats (ar, cpio, ZIP), each also opened by `filer-volume` or
`filer-archive` without naming its format; the Windows driver; and
thread-pool executors.
### Added
- `filer-api`:
  - Executors: `Spawner` and `Job`; with `std`, `DefaultPool` and
    `run_on`.
  - Resolution: `BlockingResolver`, the synchronous walk.
  - Listing: `Dir::names`, and comparisons of `EntryName` with strings.
  - Extended attributes: `Op::Xattr`, `XattrName`, `XattrEntry`, and
    `File::xattr` and `Dir::xattr`, with their kin.
  - Metadata: `FileAttrs`, in `Metadata::attrs`.
  - `archive`: what archive drivers share: `Archive`, `Index`, `Member`,
    `Limits`, `ArchiveError`, and `Codec` for compressed members.
  - `checksum`: CRC-32C, CRC-32, and CRC-16/MODBUS.
- `filer-ar`: New crate. `Ar`: ar archives, such as Debian packages and
  static libraries.
- `filer-archive`: New crate. `Archive`, opening an archive of any
  supported format by its file name, with `Format`; a feature per format.
- `filer-cpio`: New crate. `Cpio`: cpio archives, such as initramfs
  images.
- `filer-ext4`: New crate. `Ext4`: ext2, ext3, and ext4.
- `filer-fat`: New crate. `Fat`: FAT12, FAT16, and FAT32; the
  `code-pages` feature.
- `filer-fs`: New crate. What file system drivers share: `FileSystem`
  and `Mounted`, the driver shell; `partition`, for MBR and GPT; and
  `recover`, `map`, `nt`, `reparse`, `wtf8`, and the rest.
- `filer-heap`: Extended attributes.
- `filer-ntfs`: New crate. `Ntfs`: NTFS, with named streams as extended
  attributes.
- `filer-posix`: `Posix::thread_pool`; opens confined by the Linux kernel
  with `openat2`.
- `filer-spec`: Cases for extended attributes; `Sparse`, `Overlay`, and
  `exercise`, for drivers of volume images; with `std`, `fuzz`.
- `filer-system`: `System::thread_pool`; on Windows, `filer-windows`.
- `filer-volume`: New crate. `Volume`, opening a volume of any supported
  file system as its first bytes say, or a partition of a disk, with
  `VolumeFormat` and `VolumeDriver`; a feature per driver.
- `filer-windows`: `Windows`, the Windows driver.
- `filer-xfs`: New crate. `Xfs`: XFS, versions 4 and 5.
- `filer-zip`: `Zip`: ZIP archives, with ZIP64; the `deflate` feature.
- `known-codepages`: New crate. `CodePage`: DOS and Windows code pages.
- `filer-7z`, `filer-rar`, and `filer-udf`: New crates, with no items yet.
### Changed
- `filer-api`: `Resolver` is asynchronous; `tree::Tree::node` returns an
  `Option`.
- `filer-tar`: `Limits` is `filer_api::archive::Limits`.

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
