# decode-xz benchmarks

The figures of `decode-xz`'s decoder, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every decoder, each figure a median with its 95%
confidence interval, and every output checked against the original.

The harness's `tables` command generates the figures below from its raw
data, which stays on the machines that measured it; SHA-256 checks and
the x86 BCJ filter are not measured yet.

## Counterparts

| Decoder | Crate | Language |
| --- | --- | --- |
| liblzma, the reference | `liblzma` | C |
| lzma-rust2 | `lzma-rust2` | Rust, with its `unsafe` optimizations |
| lzma-rs | `lzma-rs` | Rust |
| xz4rust | `xz4rust` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by liblzma 5.8.4 at
presets 0, 6, and 9e, as xz writes them by default since 5.6: in its
multithreaded mode, in blocks of three times the dictionary and at least
1 MiB, the same on any number of threads, each checked by CRC-64.

## Machines

64-bit x86 on a current AMD core and a current Intel core, each a
cloud instance's own, and on an older AMD core, a VM's; and 64-bit ARM
on Apple M4 and on a Neoverse server.

<!-- filer-bench: machines -->

<!-- filer-bench: end -->

## Whole streams

MB/s of decoded output, per machine: a row per input and level, a column
per decoder, and ours relative to the reference.

<!-- filer-bench: whole -->

<!-- filer-bench: end -->

## Streaming

MB/s with input and output in pieces of 4 KiB and of 64 KiB.

<!-- filer-bench: streaming -->

<!-- filer-bench: end -->

## Small streams

The time per stream of about 64 B, 1 KiB, and 16 KiB packed, the
decoder's construction included, in µs, per machine: a row per size and
level, a column per decoder, and the reference's time relative to ours.

<!-- filer-bench: small -->

<!-- filer-bench: end -->

## Memory

Peak heap and allocations per stream, and the size of the decoder
itself.

<!-- filer-bench: memory -->

<!-- filer-bench: end -->

## Checkpoints

The time to take a checkpoint and to resume from one, and the bytes each
holds, in streams decoded in pieces of 32 KiB, per machine: a row per
input and level, with our decoder's µs to take one and to resume from
one, each the median over several processes of the median over the
places they were taken at, and the most heap one held and its
allocations. No counterpart offers them. It offers them between
blocks, so a stream of one block has none.

<!-- filer-bench: checkpoints -->

<!-- filer-bench: end -->

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Target | `opt-level` 3 | `opt-level` "s" |
| --- | ---: | ---: |
| aarch64-apple-darwin | 30512 | 25132 |
| aarch64-unknown-linux-gnu | 30020 | 24776 |
| x86_64-unknown-linux-gnu | 38634 | 29431 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`, `decode-xz`
0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-16, at `d73cac4`,
`decode-xz` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-16, at `d73cac4`,
`decode-xz` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
