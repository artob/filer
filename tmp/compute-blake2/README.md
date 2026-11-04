# compute-blake2

`compute-blake2` is to be BLAKE2b, BLAKE2s, and BLAKE2sp, written clean room
from RFC 7693 and the BLAKE2 specification, and dedicated to the public domain:
pure, safe Rust without dependencies, for `#![no_std]` without an allocator, of
the same shape as `compute-sha256`. BLAKE2b-256 is one of the checksums btrfs
may stamp on its blocks, and BLAKE2sp the hash that RAR 5 may record of its
files. It is one of the `compute-*` crates, clean-room, public-domain checksums
and hashes; this release reserves its name, and exports nothing yet.
