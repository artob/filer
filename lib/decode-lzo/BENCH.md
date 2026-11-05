# decode-lzo benchmarks

The figures of `decode-lzo`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

**No results yet**: the harness that measures it is still to be built.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| miniLZO, the reference | `minilzo-rs` | C |
| rust-lzo | `rust-lzo` | Rust, translated from miniLZO |
| lzokay-native | `lzokay-native` | Rust |
| lzo1x | `lzo1x` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by lzop at levels 1
(LZO1X-1) and 9 (LZO1X-999).

## Machines

64-bit x86 on a current AMD core and a current Intel core, both bare
metal, and on a cloud VM; and 64-bit ARM on Apple M4 and on a Neoverse
server.

## Whole streams

MB/s of decoded output, per machine: a row per input and level, a column
per decoder, and ours relative to the reference.

## Streaming

MB/s with input and output in pieces of 4 KiB and of 64 KiB.

## Small streams

The time per stream of about 64 B, 1 KiB, and 16 KiB packed, the
decoder's construction included.

## Memory

Peak heap and allocations per stream, and the size of the decoder
itself.

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` binary.
