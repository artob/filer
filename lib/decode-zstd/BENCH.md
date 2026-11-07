# decode-zstd benchmarks

The figures of `decode-zstd`'s decoder, alone and against the main
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
| libzstd, the reference | `zstd` | C |
| ruzstd | `ruzstd` | Rust |

## Inputs

The protocol's corpus and synthetic cases, encoded by zstd 1.5 at levels
1, 3, 9, 19, and 22 (`--ultra`), and at 19 with a 128 MiB window
(`--long=27`) on the large inputs; and as Ubuntu's initramfs-tools packs
initrd images.

## Machines

64-bit x86 on a current AMD core and a current Intel core, both bare
metal, and on a cloud VM; and 64-bit ARM on Apple M4 and on a Neoverse
server.

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
decoder's construction included.

## Memory

Peak heap and allocations per stream, and the size of the decoder
itself.

<!-- filer-bench: memory -->
<!-- filer-bench: end -->

## Checkpoints

The time to take a checkpoint and to resume from one, and the bytes each
holds.

## Code size

The bytes of `.text` the decoder adds to a minimal `#![no_std]` program
that decodes a stream with it, from construction to `finish`, built
portable with fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->
<!-- filer-bench: end -->
