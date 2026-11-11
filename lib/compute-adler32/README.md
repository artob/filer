# compute-adler32

Adler-32, written clean room from RFC 1950 and dedicated to the public domain:
the checksum that zlib streams end with, and so PNG images and the compressed
files of btrfs, SquashFS, and APFS that are zlib's, computed over bytes fed in
pieces, as they arrive, in 16 lanes, which compilers vectorize. It is pure,
safe Rust, and builds with `#![no_std]`, without an allocator. Its
`intrinsics` feature sums in the CPU's vectors where it has them, AVX2's,
AVX-512's, and NEON's: up to seven times as fast over long inputs.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_adler32::{Adler32, adler32};

fn main() {
    // At once, or in pieces, in place or chained.
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    let mut sum = Adler32::new();
    sum.update(b"Wiki");
    sum.update(b"pedia");
    assert_eq!(sum.finish(), 0x11E6_0398);
    assert_eq!(Adler32::new().chain(b"Wiki").chain(b"pedia"), sum);
}
```

This is free and unencumbered software released into the public domain.
