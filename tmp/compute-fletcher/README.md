# compute-fletcher

`compute-fletcher` is to be Fletcher's checksums as file systems stamp them,
written clean room from their definitions, and dedicated to the public domain:
pure, safe Rust without dependencies, for `#![no_std]` without an allocator, of
the same shape as `compute-crc`, each variant a type and a feature: ZFS's
Fletcher-4, its default checksum, and Fletcher-2, of older pools; and APFS's
Fletcher-64, on every object. It is one of the `compute-*` crates, clean-room,
public-domain checksums and hashes; this release reserves its name, and exports
nothing yet.
