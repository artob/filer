# decode-deflate

`decode-deflate` is to be a Deflate decoder, written clean room from RFC 1951,
and dedicated to the public domain: pure, safe Rust without dependencies, for
`#![no_std]` with an allocator, never panicking on any input, of the same
sans-I/O shape as `decode-zstd`. Deflate (RFC 1951) is the LZ77 and Huffman
coding of ZIP members, gzip and zlib streams, 7z archives, and PNG images.
Deflate64, PKWARE's variant for ZIP's method 9 and 7z, with a 64 KiB window, a
longer last length, and two more distances, is to come behind an additive
`deflate64` feature, chosen per stream. It is one of the `decode-*` crates,
clean-room, public-domain decoders of common codecs; this release reserves its
name, and exports nothing yet.
