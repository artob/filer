# compute-sha256 benchmarks

The figures of `compute-sha256`'s SHA-256, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

**No results yet**: the harness that measures it is still to be built.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| OpenSSL | `openssl-sys` | C, with SHA instructions |
| RustCrypto's sha2 | `sha2` | Rust, with SHA instructions |
| ring | `ring` | C and assembly |

## Inputs

Buffers of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, whole and fed in
pieces of 7 bytes.

## Machines

64-bit x86 on a current AMD core and a current Intel core, both bare
metal, and on a cloud VM; and 64-bit ARM on Apple M4 and on a Neoverse
server.

## Throughput

GB/s per machine: a row per input, a column per implementation, and
ours relative to the fastest portable one.

## Code size

The bytes of `.text` it adds to a minimal `#![no_std]` binary.
