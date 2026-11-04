# compute-fletcher benchmarks

The figures of `compute-fletcher`'s Fletcher checksums, alone and
against the main alternatives, measured by the protocol of this
repository's [`bench/METHOD.md`](../../bench/METHOD.md): the same
inputs, machines, and builds for every implementation, each figure a
median with its 95% confidence interval, and every result checked
against the others'.

**No results yet**: it is still to be written.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| OpenZFS, the reference | (its own) | C, with SIMD |
| fletcher | `fletcher` | Rust |

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
