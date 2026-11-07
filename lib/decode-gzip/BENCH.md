# decode-gzip benchmarks

The figures of `decode-gzip`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

**Only its code size so far**: the harness measures nothing else of it
yet.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| zlib, the reference | `libz-sys` | C |
| zlib-ng | `libz-ng-sys` | C |
| libdeflate | `libdeflater` | C, whole buffers only |
| zlib-rs | `zlib-rs` | Rust |
| miniz_oxide, by flate2 | `flate2` | Rust |
| zune-inflate | `zune-inflate` | Rust, whole buffers only |

## Inputs

The protocol's corpus and synthetic cases, encoded by GNU gzip at levels
1, 6, and 9, pigz with Zopfli (`-11`), and members concatenated.

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

## Checkpoints

The time to take a checkpoint and to resume from one, and the bytes each
holds.

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Target | `opt-level` 3 | `opt-level` "s" |
| --- | ---: | ---: |
| aarch64-apple-darwin | 12960 | 8980 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`decode-gzip` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
