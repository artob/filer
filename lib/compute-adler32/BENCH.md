# compute-adler32 benchmarks

The figures of `compute-adler32`'s Adler-32, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| zlib | `libz-sys` | C |
| adler2 | `adler2` | Rust |
| simd-adler32 | `simd-adler32` | Rust, with SIMD |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for Adler-32.

## Machines

64-bit x86 on a current AMD core and a current Intel core, both bare
metal, and on a cloud VM; and 64-bit ARM on Apple M4 and on a Neoverse
server.

Measured so far: AMD EPYC 7551P (Zen 1), 4 vCPUs under KVM, Linux 7.0,
Rust 1.97.0, 2026-11-06.

## Throughput

GB/s, each the median of five or more processes with its 95%
confidence interval: a row per size and way of feeding, a column per
implementation; then each counterpart's speed relative to ours, the
geometric mean over sizes. The counterparts pick the CPU's
instructions at run time.

### Ours portable, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.28 ± 0.4% | 0.35 ± 0.8% | 0.29 ± 0.9% | 0.63 ± 0.5% |
| 256 | whole | 2.95 ± 0.3% | 1.63 ± 0.1% | 1.74 ± 0.3% | 5.68 ± 0.7% |
| 4096 | whole | 7.44 ± 0.7% | 2.11 ± 0.3% | 2.72 ± 0.3% | 10.94 ± 0.5% |
| 65536 | whole | 8.21 ± 0.7% | 2.12 ± 0.6% | 4.54 ± 0.7% | 11.33 ± 0.3% |
| 1048576 | whole | 8.18 ± 0.2% | 2.12 ± 0.3% | 6.41 ± 0.1% | 11.27 ± 0.3% |
| 16 | by 7 | 0.17 ± 0.2% | 0.29 ± 0.4% | 0.19 ± 0.4% | 0.34 ± 0.2% |
| 256 | by 7 | 0.26 ± 0.3% | 0.51 ± 0.4% | 0.32 ± 0.4% | 0.51 ± 0.3% |
| 4096 | by 7 | 0.28 ± 0.2% | 0.54 ± 0.4% | 0.33 ± 0.4% | 0.53 ± 0.2% |
| 65536 | by 7 | 0.28 ± 0.2% | 0.53 ± 0.3% | 0.33 ± 0.2% | 0.54 ± 0.3% |
| 1048576 | by 7 | 0.28 ± 0.4% | 0.53 ± 0.5% | 0.33 ± 0.8% | 0.54 ± 0.8% |

Each counterpart's speed relative to ours: whole: zlib 0.42x, adler2
0.63x, simd-adler32 1.65x; by 7: zlib 1.89x, adler2 1.19x, simd-adler32
1.96x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| adler32 | aarch64-apple-darwin | 708 | 376 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-adler32` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
