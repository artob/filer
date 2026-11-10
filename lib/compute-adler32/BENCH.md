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

64-bit x86 on a current AMD core and a current Intel core, each a
cloud instance's own, and on an older AMD core, a VM's; and 64-bit ARM
on Apple M4 and on a Neoverse server.

Measured so far, each under Linux 7.0, by Rust 1.97.0: AMD EPYC 9R45
(Zen 5) and Intel Xeon 6975P-C (Granite Rapids), each 1 vCPU under EC2
Nitro, on a physical core of its own, 2026-11-10; and AMD EPYC 7551P
(Zen 1), 4 vCPUs under KVM, 2026-11-06.

## Throughput

GB/s, each the median of five or more processes with its 95%
confidence interval: a row per size and way of feeding, a column per
implementation; then each counterpart's speed relative to ours, the
geometric mean over sizes. The counterparts pick the CPU's
instructions at run time.

### Zen 5, ours portable, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.46 ± 0.1% | 1.90 ± 0.9% | 1.29 ± 0.3% | 2.08 ± 1.5% |
| 256 | whole | 11.48 ± 0.3% | 4.40 ± 0.1% | 4.07 ± 0.1% | 29.42 ± 0.1% |
| 4096 | whole | 18.12 ± 0.9% | 4.43 ± 0.4% | 4.40 ± 0.4% | 62.01 ± 0.6% |
| 65536 | whole | 19.34 ± 0.1% | 4.46 ± 0.0% | 6.58 ± 0.0% | 58.70 ± 0.2% |
| 1048576 | whole | 19.34 ± 0.1% | 4.47 ± 0.1% | 8.84 ± 0.1% | 58.47 ± 0.3% |
| 16 | by 7 | 0.70 ± 0.3% | 1.36 ± 0.6% | 0.75 ± 0.3% | 1.35 ± 0.1% |
| 256 | by 7 | 0.93 ± 0.6% | 1.31 ± 1.1% | 0.90 ± 0.5% | 1.48 ± 1.1% |
| 4096 | by 7 | 0.97 ± 0.1% | 1.36 ± 0.2% | 0.93 ± 0.1% | 1.59 ± 0.0% |
| 65536 | by 7 | 0.98 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |
| 1048576 | by 7 | 0.98 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.37x, adler2
0.41x, simd-adler32 2.59x; by 7: zlib 1.49x, adler2 0.98x, simd-adler32
1.68x.

### Granite Rapids, ours portable, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.15 ± 1.2% | 1.50 ± 2.8% | 0.96 ± 0.8% | 1.57 ± 2.3% |
| 256 | whole | 7.76 ± 0.1% | 3.74 ± 0.1% | 4.07 ± 0.2% | 22.48 ± 1.2% |
| 4096 | whole | 12.11 ± 0.1% | 3.74 ± 0.1% | 6.10 ± 0.0% | 45.54 ± 0.2% |
| 65536 | whole | 12.55 ± 0.1% | 3.72 ± 0.1% | 8.69 ± 0.2% | 41.29 ± 0.1% |
| 1048576 | whole | 12.52 ± 0.3% | 3.71 ± 0.2% | 10.69 ± 0.2% | 41.08 ± 0.2% |
| 16 | by 7 | 0.54 ± 0.2% | 1.06 ± 0.8% | 0.62 ± 0.6% | 0.98 ± 1.5% |
| 256 | by 7 | 0.72 ± 0.3% | 1.10 ± 0.1% | 0.77 ± 0.1% | 1.21 ± 0.1% |
| 4096 | by 7 | 0.78 ± 0.1% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.1% |
| 65536 | by 7 | 0.78 ± 0.1% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.24 ± 0.1% |
| 1048576 | by 7 | 0.78 ± 0.1% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.0% |

Each counterpart's speed relative to ours: whole: zlib 0.44x, adler2
0.67x, simd-adler32 2.76x; by 7: zlib 1.51x, adler2 1.05x, simd-adler32
1.64x.

### Zen 1, ours portable, as by default

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
| adler32 | x86_64-unknown-linux-gnu | 1147 | 430 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-adler32` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-adler32` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
