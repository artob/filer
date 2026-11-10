# decode-xz benchmarks

The figures of `decode-xz`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

**Only its code size so far**: the harness measures nothing else of it
yet.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| liblzma, the reference | `liblzma` | C |
| lzma-rust2 | `lzma-rust2` | Rust |
| lzma-rs | `lzma-rs` | Rust |
| xz4rust | `xz4rust` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by xz 5 at presets 0,
6, and 9e, in one block and in several (as `-T0` writes), with CRC-64
and with SHA-256 checks, and with the x86 BCJ filter on executables.

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
| aarch64-apple-darwin | 30512 | 25132 |
| x86_64-unknown-linux-gnu | 39062 | 29553 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`, `decode-xz`
0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`decode-xz` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
