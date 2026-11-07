# decode-bzip2 benchmarks

The figures of `decode-bzip2`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

The harness's `tables` command generates the figures below from its raw
data, which stays on the machines that measured it; small streams,
checkpoints, and code size are not measured yet.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| libbzip2, the reference | `bzip2-sys` | C |
| libbz2-rs-sys | `libbz2-rs-sys` | Rust, ported from libbzip2 |
| bzip2-rs | `bzip2-rs` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by bzip2 1.0.8 at
levels 1 and 9.

## Machines

64-bit x86 on a current AMD core and a current Intel core, both bare
metal, and on a cloud VM; and 64-bit ARM on Apple M4 and on a Neoverse
server.

<!-- filer-bench: machines -->

Measured so far: none.

<!-- filer-bench: end -->

## Whole streams

MB/s of decoded output, per machine: a row per input and level, a column
per decoder, and ours relative to the reference.

<!-- filer-bench: whole -->

Not measured yet.

<!-- filer-bench: end -->

## Streaming

MB/s with input and output in pieces of 4 KiB and of 64 KiB.

<!-- filer-bench: streaming -->

Not measured yet.

<!-- filer-bench: end -->

## Small streams

The time per stream of about 64 B, 1 KiB, and 16 KiB packed, the
decoder's construction included.

## Memory

Peak heap and allocations per stream, and the size of the decoder
itself.

<!-- filer-bench: memory -->

Not measured yet.

<!-- filer-bench: end -->

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
| aarch64-apple-darwin | 11412 | 6980 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`decode-bzip2` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
