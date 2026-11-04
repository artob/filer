# compute-adler32

Adler-32, written clean room from RFC 1950 and dedicated to the public domain:
the checksum that zlib streams end with, and so PNG images and the compressed
files of btrfs, SquashFS, and APFS that are zlib's, computed over bytes fed in
pieces, as they arrive, in 16 lanes, which compilers vectorize. It is pure,
safe Rust without dependencies, and builds with `#![no_std]`, without an
allocator.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_adler32::{Adler32, adler32};

fn main() {
    // At once, or in pieces.
    assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
    let sum = Adler32::new().update(b"Wiki").update(b"pedia");
    assert_eq!(sum.finish(), 0x11E6_0398);
}
```

This is free and unencumbered software released into the public domain.
