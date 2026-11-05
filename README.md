# Filer

[![License](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://unlicense.org)
[![Compatibility](https://img.shields.io/badge/rust-1.97%2B-blue)](https://endoflife.date/rust)

**Filer provides a filesystem API with platform-specific backends for Rust.**

Every backend serves the same capability-based API: a `Dir` grants access
only beneath itself, so paths cannot escape it. The API is async-first,
pure Rust, and builds with `#![no_std]`. Besides the native file system,
drivers read disk images and archives from any source, even a file inside
another driver.

**Status:** experimental; every 0.0.x release may break the last. Reading
works natively on Unix and Windows, in memory, and in the file systems and
archives below; writing is not implemented yet.

## 🛠️ Prerequisites

- [Rust] 1.97+

## 📚 Reference

### Crates

Start with one of these:

| Crate | Summary |
| --- | --- |
| `filer-system` | The API, plus the native driver for the target |
| `filer-volume` | Read-only volumes of any supported file system |
| `filer-archive` | Read-only archives of any supported format |

Native backends:

| Crate | Platforms | Summary |
| --- | --- | --- |
| `filer-posix` | Unix | POSIX backend |
| `filer-windows` | Windows | Windows backend |
| `filer-wasi` | WASI | WASI backend (planned) |
| `filer-whatwg` | Browsers | WHATWG File System (OPFS) backend (planned) |

File systems and archives, all read-only and on every platform:

| Crate | Summary |
| --- | --- |
| `filer-ext4` | ext2, ext3, and ext4 |
| `filer-fat` | FAT12, FAT16, and FAT32 |
| `filer-ntfs` | NTFS |
| `filer-xfs` | XFS |
| `filer-apfs` | APFS (planned) |
| `filer-btrfs` | btrfs (planned) |
| `filer-cdfs` | ISO 9660 (planned) |
| `filer-exfat` | exFAT (planned) |
| `filer-minix` | Minix (planned) |
| `filer-refs` | ReFS (planned) |
| `filer-squashfs` | SquashFS (planned) |
| `filer-udf` | UDF (planned) |
| `filer-ufs` | UFS, BSD's FFS (planned) |
| `filer-zfs` | ZFS (planned) |
| `filer-tar` | TAR |
| `filer-zip` | ZIP |
| `filer-ar` | ar: Debian packages, static libraries |
| `filer-cpio` | cpio: initramfs images |
| `filer-rar` | RAR (planned) |
| `filer-7z` | 7z (planned) |

Foundations:

| Crate | Summary |
| --- | --- |
| `filer-api` | Platform-independent API |
| `filer-heap` | In-memory driver |
| `filer-fs` | Shared machinery of file system drivers |
| `filer-pack` | Shared machinery of archive drivers |
| `filer-msft` | Microsoft's on-disk conventions |
| `filer-spec` | Executable driver specification, for tests |
| `known-codepages` | DOS code pages, for FAT names |
| `filer-cli` | `filer` command-line interface (planned) |

Decoders, written clean room and dedicated to the public domain:

| Crate | Summary |
| --- | --- |
| `decode-bzip2` | bzip2 |
| `decode-zstd` | zstd |
| `decode-deflate` | Deflate |
| `decode-gzip` | gzip |
| `decode-zlib` | zlib |
| `decode-lzma` | LZMA and LZMA2 |
| `decode-xz` | xz |
| `decode-lz4` | LZ4 |
| `decode-lzo` | LZO1X and lzop |

Checksums and hashes, written clean room and dedicated to the public domain:

| Crate | Summary |
| --- | --- |
| `compute-crc` | CRC-32, CRC-32C, CRC-64/XZ, and more |
| `compute-sha256` | SHA-256 |
| `compute-xxhash` | XXH32, XXH64 |
| `compute-adler32` | Adler-32 |
| `compute-fletcher` | Fletcher-2, Fletcher-4, Fletcher-64 (planned) |
| `compute-sha512` | SHA-512, SHA-512/256 (planned) |
| `compute-blake2` | BLAKE2b, BLAKE2s, BLAKE2sp (planned) |
| `compute-blake3` | BLAKE3 (planned) |

Only `filer-windows` contains `unsafe`: the system calls the standard
library lacks, each wrapped safely in its private `sys` module.

## 👨‍💻 Development

```bash
git clone https://github.com/artob/filer.git
```

[Rust]: https://rust-lang.org
