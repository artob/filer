# compute-adler32

`compute-adler32` is to be Adler-32, written clean room from RFC 1950, and
dedicated to the public domain: pure, safe Rust without dependencies, for
`#![no_std]` without an allocator, of the same shape as `compute-crc`. Adler-32
is the checksum that zlib streams end with, and so PNG images and `decode-zlib`
check them with. It is one of the `compute-*` crates, clean-room, public-domain
checksums and hashes; this release reserves its name, and exports nothing yet.
