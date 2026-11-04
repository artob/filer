# decode-gzip

`decode-gzip` is to be a gzip decoder, written clean room from RFC 1952, and
dedicated to the public domain: pure, safe Rust without dependencies, for
`#![no_std]` with an allocator, never panicking on any input, of the same
sans-I/O shape as `decode-zstd`. gzip (RFC 1952) frames Deflate in `.gz` files,
compressed tarballs, and initramfs images, which this crate is to decode over
`decode-deflate`. It is one of the `decode-*` crates, clean-room, public-domain
decoders of common codecs; this release reserves its name, and exports nothing
yet.
