# compute-sha512

SHA-512 and its truncation SHA-512/256, written clean room from FIPS 180-4 and
dedicated to the public domain: the hashes that ZFS may stamp on its blocks,
among others, computed over bytes fed in pieces, as they arrive. It checks what
is stored, as a file system's checks do, and makes no claim against attackers,
as a cryptographic library would. It is pure, safe Rust without dependencies,
and builds with `#![no_std]`, without an allocator.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_sha512::{Sha512, Sha512_256, sha512, sha512_256};

fn main() {
    // At once, or in pieces.
    let digest = sha512(b"abc");
    assert_eq!(digest[..4], [0xDD, 0xAF, 0x35, 0xA1]);
    let mut sha = Sha512::new();
    sha.update(b"a");
    sha.update(b"bc");
    assert_eq!(sha.finish(), digest);
    // ZFS's truncation, from its own initial hash.
    let sha = Sha512_256::new().chain(b"abc");
    assert_eq!(sha.finish(), sha512_256(b"abc"));
}
```

This is free and unencumbered software released into the public domain.
