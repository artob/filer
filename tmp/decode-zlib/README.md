# decode-zlib

`decode-zlib` is to be a zlib decoder, written clean room from RFC 1950, and
dedicated to the public domain: pure, safe Rust without dependencies, for
`#![no_std]` with an allocator, never panicking on any input, of the same
sans-I/O shape as `decode-zstd`. zlib (RFC 1950) frames Deflate in btrfs,
SquashFS (whose "gzip" is zlib), ZFS, zisofs on ISO 9660, and Apple's compressed
files, over `decode-deflate`. It is one of the `decode-*` crates, clean-room,
public-domain decoders of common codecs; this release reserves its name, and
exports nothing yet.
