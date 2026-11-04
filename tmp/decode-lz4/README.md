# decode-lz4

`decode-lz4` is to be an LZ4 decoder, written clean room from LZ4's block and
frame format descriptions, and dedicated to the public domain: pure, safe Rust
without dependencies, for `#![no_std]` with an allocator, never panicking on any
input, of the same sans-I/O shape as `decode-zstd`. LZ4, in blocks and frames,
compresses SquashFS, EROFS, ZFS, F2FS, and ReFS, and initramfs images. It is one
of the `decode-*` crates, clean-room, public-domain decoders of common codecs;
this release reserves its name, and exports nothing yet.
