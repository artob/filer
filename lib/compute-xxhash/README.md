# compute-xxhash

The xxHash hashes, written clean room from the xxHash specification and
dedicated to the public domain: XXH64, which zstd checks frames with, computed
over bytes fed in pieces, as they arrive, from any seed. It is pure, safe Rust
without dependencies, and builds with `#![no_std]`, without an allocator.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_xxhash::{Xxh64, xxh64};

fn main() {
    // At once, or in pieces, from a seed.
    assert_eq!(xxh64(b"abc", 0), 0x44BC_2CF5_AD77_0999);
    let mut hash = Xxh64::new(0);
    hash.update(b"a");
    hash.update(b"bc");
    assert_eq!(hash.finish(), xxh64(b"abc", 0));
}
```

This is free and unencumbered software released into the public domain.
