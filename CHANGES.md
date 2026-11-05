# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog], and this project adheres to
[Semantic Versioning]. Releases are 0.0.x while the API is experimental,
so any release may break compatibility.

## Unreleased
### Added
- `filer-fs`: `Lock`, for what a driver caches.
- `filer-squashfs`: `Squashfs`, the read-only SquashFS 4.0 driver, with
  `SquashfsError`: images of every compressor `mksquashfs` offers, each
  a feature (`gzip`, `lz4`, `lzma`, `lzo`, `xz`, and `zstd`), over the
  `decode-*` crates; fragments, sparse files, hard links, and extended
  attributes.
- `filer-volume`: SquashFS images, detected by `hsqs`, with the
  `squashfs` feature, `VolumeDriver::Squashfs`, `VolumeFormat::Squashfs`,
  and the compressor features, forwarded.

## 0.0.3 - 2026-11-04
Our own decoders and checksums, written clean room and dedicated to the
public domain, each a crate: bzip2, Deflate and Deflate64, gzip, LZ4,
LZMA and LZMA2, LZO, xz, zlib, and zstd, each a sans-I/O `Decoder` that
offers checkpoints to resume from, over Adler-32, CRCs, SHA-256, and
xxHash; compressed tarballs, ZIP members, and initramfs images, read
through them; and archives told by their content.
### Added
- `compute-adler32`: New crate. `Adler32`: Adler-32.
- `compute-crc`: New crate. CRC-16/MODBUS, CRC-32, CRC-32C,
  CRC-32/BZIP2, and CRC-64/XZ, a type and a feature each.
- `compute-sha256`: New crate. `Sha256`: SHA-256.
- `compute-xxhash`: New crate. `Xxh32` and `Xxh64`: XXH32 and XXH64,
  from any seed.
- `decode-bzip2`: New crate. bzip2.
- `decode-deflate`: New crate. Deflate, and with `deflate64`, Deflate64.
- `decode-gzip`: New crate. gzip, member by member.
- `decode-lz4`: New crate. LZ4 frames, legacy frames too, and with
  `decode_block`, raw blocks.
- `decode-lzma`: New crate. LZMA and LZMA2.
- `decode-lzo`: New crate. lzop files, and with `decode_block`, raw
  LZO1X blocks.
- `decode-xz`: New crate. xz, with every filter xz writes.
- `decode-zlib`: New crate. zlib.
- `decode-zstd`: New crate. zstd.
- `filer-api`: `FileKind::from_unix_mode` and `Tree::node_mut`; every
  CRC in `checksum`, now `compute-crc`'s, has `from_state`, `state`, and
  `CHECK`.
- `filer-archive`: Formats told by content, with `Format::detect` and
  `Archive::open_by_content`; compressed tarballs; the `bzip2`, `gzip`,
  `lzma`, `xz`, and `zstd` features.
- `filer-cpio`: Initramfs images whose later archive is compressed with
  gzip, zstd, xz, or bzip2; `Cpio::compression`.
- `filer-msft`: New crate. Microsoft's on-disk conventions: `nt`,
  `reparse`, and `wtf8`, and `dos`, FAT's dates and times.
- `filer-pack`: New crate. What archive drivers share, moved from
  `filer_api::archive`: `Index`, now applying `Limits`; `Verified`,
  `Streams`, and `Unpacked`, decoding packed ranges and resuming from
  checkpoints; `Compression`, telling a stream's compression by its
  first bytes; and `Bzip2`, `Deflate`, `Gzip`, `Lzma`, `Lzma2`, `Xz`,
  and `Zstd`, over the `decode-*` crates, each behind a feature.
- `filer-spec`: `Counted`; `Outcome::files`; `Checks::refusals` and
  `Checks::read_cap`; `Mutations::aligned` and `Mutations::most`.
- `filer-tar`: Archives compressed with gzip, zstd, xz, or bzip2;
  `Tar::compression`.
- `filer-zip`: Members compressed with Deflate64, bzip2, LZMA, zstd, or
  xz; names in a DOS `CodePage`, and Info-ZIP's Unicode paths;
  `Options`; the `code-pages` feature.
- `compute-blake2`, `compute-blake3`, `compute-fletcher`,
  `compute-sha512`, `filer-apfs`, `filer-btrfs`, `filer-cdfs`,
  `filer-exfat`, `filer-minix`, `filer-refs`, `filer-squashfs`,
  `filer-ufs`, and `filer-zfs`: New crates, with no items yet.
### Changed
- `filer-ar`, `filer-cpio`: `Limits::max_entries` counts only the
  members an archive shows.
- `filer-fat`: `UtcOffset` is `filer-msft`'s, re-exported.
- `filer-zip`: `Zip::open_with` takes `Options`, or `Limits` alone.
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
