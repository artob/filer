# compute-crc

Cyclic redundancy checks, written clean room from their parameters and
dedicated to the public domain: CRC-32, which gzip, ZIP, xz, and GPT stamp;
CRC-32C, which ext4, XFS, and Btrfs stamp; CRC-32/BZIP2; CRC-64/XZ; and
CRC-16/MODBUS, each a type and a feature of its own. Each computes over bytes
fed in pieces, as they arrive, sixteen bytes at a time; exposes its register,
which formats that chain checksums store; and is a `Copy` value, so that a
copy taken anywhere goes on from there. It is pure, safe Rust without
dependencies, and builds with `#![no_std]`, without an allocator. Its
`intrinsics` feature takes the CPU's CRC instructions where it has them,
SSE4.2's and ARM's, two or more times as fast.

It is one of the `compute-*` crates: clean-room, public-domain checksums and
hashes, each of the same shape.

```rust
use compute_crc::{Crc32, Crc32c, crc32};

fn main() {
    // At once, or in pieces, in place or chained.
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    let mut crc = Crc32::new();
    crc.update(b"1234");
    crc.update(b"56789");
    assert_eq!(crc.finish(), 0xCBF4_3926);
    assert_eq!(Crc32::new().chain(b"1234").chain(b"56789"), crc);
    // ext4 chains CRC-32C registers from a seed.
    let seed = Crc32c::new().chain(b"volume UUID").state();
    let checksum = Crc32c::from_state(seed).chain(b"inode").state();
    assert_ne!(checksum, seed);
}
```

This is free and unencumbered software released into the public domain.
