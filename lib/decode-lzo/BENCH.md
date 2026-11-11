# decode-lzo benchmarks

The figures of `decode-lzo`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

**Only its code size so far**: the harness measures nothing else of it
yet.

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

64-bit x86 on a current AMD core and a current Intel core, each a
cloud instance's own, and on an older AMD core, a VM's; and 64-bit ARM
on Apple M4 and on a Neoverse server.

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

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Target | `opt-level` 3 | `opt-level` "s" |
| --- | ---: | ---: |
| aarch64-apple-darwin | 5512 | 4868 |
| aarch64-unknown-linux-gnu | 6108 | 5452 |
| x86_64-unknown-linux-gnu | 6918 | 5692 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`, `decode-lzo`
0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-10, at `b674208`,
`decode-lzo` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`decode-lzo` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
