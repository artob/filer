# compute-sha256

SHA-256, written clean room from FIPS 180-4 and dedicated to the public domain:
the hash that xz and 7z may check their data with, computed over bytes fed in
pieces, as they arrive. It checks what is stored, as an archive's checks do,
and makes no claim against attackers, as a cryptographic library would. It is
pure, safe Rust without dependencies, and builds with `#![no_std]`, without an
allocator.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_sha256::{Sha256, sha256};

fn main() {
    // At once, or in pieces.
    let digest = sha256(b"abc");
    assert_eq!(digest[..4], [0xBA, 0x78, 0x16, 0xBF]);
    let mut sha = Sha256::new();
    sha.update(b"a");
    sha.update(b"bc");
    assert_eq!(sha.finish(), digest);
}
```

This is free and unencumbered software released into the public domain.
