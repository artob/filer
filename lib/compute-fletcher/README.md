# compute-fletcher

Fletcher's checksums as file systems stamp them, written clean room from their
definitions and dedicated to the public domain: ZFS's Fletcher-4, its default
checksum, and Fletcher-2, of older pools, of either byte order; and APFS's
Fletcher-64, on every object; each a type and a feature of its own. Neither
ZFS's specification nor APFS's reference defines them beyond their names, so
each was established by observation, against what ZFS and macOS stamp. Each
computes over bytes fed in pieces, as they arrive, and Fletcher-4 in lanes that
may run side by side. It is pure, safe Rust without dependencies, and builds
with `#![no_std]`, without an allocator.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_fletcher::{Fletcher4, Fletcher64, fletcher4, fletcher64};

fn main() {
    // ZFS's default, at once or in pieces.
    let block = [1, 0, 0, 0, 2, 0, 0, 0];
    assert_eq!(fletcher4(&block), [3, 4, 5, 6]);
    let sum = Fletcher4::new().chain(&block[..3]).chain(&block[3..]);
    assert_eq!(sum.finish(), fletcher4(&block));
    // APFS's check values bring the sums to zero.
    let check = fletcher64(&block);
    let whole = Fletcher64::new().chain(&block).chain(&check.to_le_bytes());
    assert_eq!(whole.sums(), [0, 0]);
}
```

This is free and unencumbered software released into the public domain.
