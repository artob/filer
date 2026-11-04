# decode-xz

`decode-xz` is to be an xz decoder, written clean room from the `.xz` file
format specification, in the public domain, and FIPS 180-4 for SHA-256, and
dedicated to the public domain: pure, safe Rust without dependencies, for
`#![no_std]` with an allocator, never panicking on any input, of the same
sans-I/O shape as `decode-zstd`. xz is the container of `.xz` files, compressed
tarballs, Debian packages, and initramfs and SquashFS images: streams of blocks,
filters, CRC-32, CRC-64, and SHA-256 checks, and an index, over `decode-lzma`'s
LZMA2, its blocks resumable alone. It is one of the `decode-*` crates,
clean-room, public-domain decoders of common codecs; this release reserves its
name, and exports nothing yet.
