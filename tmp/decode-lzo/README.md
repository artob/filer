# decode-lzo

`decode-lzo` is to be an LZO decoder, written clean room from a description of
the algorithm, with `lzokay` (MIT) read for facts only, and never `minilzo`'s
(GPL) code, and dedicated to the public domain: pure, safe Rust without
dependencies, for `#![no_std]` with an allocator, never panicking on any input,
of the same sans-I/O shape as `decode-zstd`. LZO1X compresses btrfs, SquashFS,
F2FS, and initramfs images. It is one of the `decode-*` crates, clean-room,
public-domain decoders of common codecs; this release reserves its name, and
exports nothing yet.
