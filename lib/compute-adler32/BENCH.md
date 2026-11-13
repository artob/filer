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

Measured so far, each under Linux 7.0: AMD EPYC 9R45 (Zen 5), Intel
Xeon 6975P-C (Granite Rapids), and ARM Neoverse V3 (AWS Graviton5),
each 1 vCPU under EC2 Nitro, on a physical core of its own, by Rust
1.99.1, 2026-11-13; and AMD EPYC 7551P (Zen 1), 4 vCPUs under KVM, by
Rust 1.97.0, 2026-11-06.

## Throughput

GB/s, each the median of five or more processes with its 95%
confidence interval: a row per size and way of feeding, a column per
implementation; then each counterpart's speed relative to ours, the
geometric mean over sizes. The counterparts pick the CPU's
instructions at run time.

### Zen 5, ours with `intrinsics`, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.91 ± 1.5% | 1.84 ± 1.2% | 1.22 ± 0.4% | 1.99 ± 1.2% |
| 256 | whole | 24.10 ± 0.5% | 4.39 ± 0.1% | 4.07 ± 0.0% | 28.01 ± 0.5% |
| 4096 | whole | 129.72 ± 0.9% | 4.44 ± 0.1% | 4.41 ± 0.2% | 61.89 ± 0.1% |
| 65536 | whole | 120.98 ± 0.8% | 4.46 ± 0.1% | 6.56 ± 0.1% | 58.83 ± 0.2% |
| 1048576 | whole | 119.21 ± 0.3% | 4.46 ± 0.1% | 8.78 ± 0.1% | 58.05 ± 0.1% |
| 16 | by 7 | 1.24 ± 0.1% | 1.31 ± 0.6% | 0.74 ± 0.2% | 1.32 ± 0.7% |
| 256 | by 7 | 1.64 ± 0.9% | 1.33 ± 0.2% | 0.91 ± 0.1% | 1.51 ± 0.2% |
| 4096 | by 7 | 1.75 ± 0.7% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.59 ± 0.1% |
| 65536 | by 7 | 1.76 ± 0.7% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |
| 1048576 | by 7 | 1.80 ± 1.0% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.10x, adler2
0.11x, simd-adler32 0.67x; by 7: zlib 0.83x, adler2 0.54x, simd-adler32
0.93x.

### Zen 5, ours without `intrinsics`, without dependencies

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.96 ± 1.0% | 1.84 ± 0.7% | 1.22 ± 0.3% | 1.99 ± 0.8% |
| 256 | whole | 11.64 ± 0.4% | 4.39 ± 0.2% | 5.47 ± 0.1% | 28.14 ± 0.4% |
| 4096 | whole | 18.35 ± 0.9% | 4.44 ± 0.1% | 6.89 ± 0.2% | 62.07 ± 0.2% |
| 65536 | whole | 19.32 ± 0.1% | 4.46 ± 0.1% | 7.90 ± 0.1% | 58.78 ± 0.2% |
| 1048576 | whole | 19.32 ± 0.1% | 4.46 ± 0.1% | 8.80 ± 0.2% | 57.91 ± 0.2% |
| 16 | by 7 | 1.29 ± 0.3% | 1.31 ± 0.4% | 0.74 ± 0.2% | 1.34 ± 0.8% |
| 256 | by 7 | 1.85 ± 0.6% | 1.33 ± 0.3% | 0.91 ± 0.1% | 1.49 ± 0.9% |
| 4096 | by 7 | 2.00 ± 0.2% | 1.36 ± 0.1% | 0.93 ± 0.2% | 1.59 ± 0.1% |
| 65536 | by 7 | 2.01 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |
| 1048576 | by 7 | 2.01 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.34x, adler2
0.46x, simd-adler32 2.37x; by 7: zlib 0.74x, adler2 0.49x, simd-adler32
0.84x.

### Granite Rapids, ours with `intrinsics`, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.58 ± 0.6% | 1.54 ± 2.5% | 0.96 ± 0.3% | 1.62 ± 2.6% |
| 256 | whole | 19.68 ± 1.0% | 3.75 ± 0.0% | 4.10 ± 0.1% | 22.88 ± 1.1% |
| 4096 | whole | 83.11 ± 1.0% | 3.74 ± 0.1% | 6.11 ± 0.1% | 45.58 ± 0.2% |
| 65536 | whole | 86.38 ± 0.4% | 3.72 ± 0.1% | 8.48 ± 0.1% | 41.46 ± 0.1% |
| 1048576 | whole | 86.19 ± 1.6% | 3.72 ± 0.1% | 10.14 ± 1.4% | 41.35 ± 1.9% |
| 16 | by 7 | 0.81 ± 0.8% | 1.06 ± 0.9% | 0.61 ± 0.2% | 1.07 ± 0.7% |
| 256 | by 7 | 0.98 ± 0.3% | 1.10 ± 0.0% | 0.77 ± 0.1% | 1.21 ± 0.1% |
| 4096 | by 7 | 1.10 ± 0.3% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.1% |
| 65536 | by 7 | 1.09 ± 0.7% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.24 ± 0.0% |
| 1048576 | by 7 | 1.04 ± 0.9% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.11x, adler2
0.16x, simd-adler32 0.68x; by 7: zlib 1.08x, adler2 0.75x, simd-adler32
1.20x.

### Granite Rapids, ours without `intrinsics`, without dependencies

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.58 ± 2.5% | 1.50 ± 2.4% | 0.96 ± 1.0% | 1.58 ± 2.5% |
| 256 | whole | 8.08 ± 0.1% | 3.75 ± 0.0% | 4.09 ± 0.1% | 22.43 ± 1.6% |
| 4096 | whole | 12.14 ± 0.1% | 3.74 ± 0.1% | 6.11 ± 0.1% | 45.66 ± 0.1% |
| 65536 | whole | 12.56 ± 0.0% | 3.73 ± 0.1% | 8.49 ± 0.0% | 41.44 ± 0.0% |
| 1048576 | whole | 12.28 ± 1.4% | 3.72 ± 0.1% | 9.99 ± 1.0% | 39.66 ± 3.3% |
| 16 | by 7 | 1.02 ± 1.0% | 1.05 ± 1.4% | 0.60 ± 0.2% | 1.00 ± 0.7% |
| 256 | by 7 | 1.25 ± 0.4% | 1.10 ± 0.2% | 0.77 ± 0.1% | 1.21 ± 0.1% |
| 4096 | by 7 | 1.60 ± 0.6% | 1.08 ± 0.1% | 0.79 ± 0.0% | 1.23 ± 0.1% |
| 65536 | by 7 | 1.59 ± 0.9% | 1.08 ± 0.0% | 0.79 ± 0.0% | 1.24 ± 0.0% |
| 1048576 | by 7 | 1.48 ± 0.9% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.41x, adler2
0.61x, simd-adler32 2.57x; by 7: zlib 0.78x, adler2 0.54x, simd-adler32
0.86x.

### Neoverse V3, ours with `intrinsics`, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.57 ± 2.5% | 2.35 ± 2.0% | 1.28 ± 2.3% | 1.50 ± 2.3% |
| 256 | whole | 17.26 ± 0.1% | 3.26 ± 0.2% | 5.04 ± 1.8% | 17.35 ± 0.3% |
| 4096 | whole | 47.68 ± 0.0% | 3.19 ± 0.0% | 5.97 ± 1.0% | 34.86 ± 0.1% |
| 65536 | whole | 53.33 ± 0.1% | 3.18 ± 0.0% | 5.88 ± 0.6% | 35.64 ± 0.0% |
| 1048576 | whole | 48.56 ± 0.9% | 3.18 ± 0.0% | 6.25 ± 0.1% | 35.18 ± 0.2% |
| 16 | by 7 | 1.09 ± 0.7% | 1.45 ± 0.4% | 0.63 ± 0.6% | 0.95 ± 0.3% |
| 256 | by 7 | 1.17 ± 0.4% | 1.13 ± 0.2% | 0.74 ± 0.1% | 0.81 ± 0.1% |
| 4096 | by 7 | 1.20 ± 0.3% | 1.11 ± 0.5% | 0.75 ± 0.1% | 0.80 ± 0.8% |
| 65536 | by 7 | 1.20 ± 0.2% | 1.11 ± 0.8% | 0.75 ± 0.1% | 0.78 ± 0.5% |
| 1048576 | by 7 | 1.20 ± 0.0% | 1.10 ± 0.1% | 0.76 ± 0.1% | 0.80 ± 0.8% |

Each counterpart's speed relative to ours: whole: zlib 0.15x, adler2
0.21x, simd-adler32 0.80x; by 7: zlib 1.00x, adler2 0.62x, simd-adler32
0.70x.

### Neoverse V3, ours without `intrinsics`, without dependencies

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.68 ± 0.9% | 2.35 ± 0.6% | 1.28 ± 0.3% | 1.50 ± 0.8% |
| 256 | whole | 10.94 ± 1.3% | 3.26 ± 0.2% | 5.07 ± 1.1% | 17.33 ± 1.5% |
| 4096 | whole | 17.34 ± 0.1% | 3.19 ± 0.0% | 5.98 ± 1.2% | 34.96 ± 0.1% |
| 65536 | whole | 18.11 ± 0.0% | 3.18 ± 0.0% | 5.88 ± 0.4% | 35.65 ± 0.0% |
| 1048576 | whole | 17.99 ± 0.1% | 3.18 ± 0.2% | 6.30 ± 0.2% | 35.46 ± 0.1% |
| 16 | by 7 | 1.14 ± 0.8% | 1.44 ± 0.5% | 0.63 ± 0.6% | 0.95 ± 0.6% |
| 256 | by 7 | 1.17 ± 0.2% | 1.13 ± 0.2% | 0.74 ± 0.8% | 0.79 ± 0.5% |
| 4096 | by 7 | 1.18 ± 0.4% | 1.10 ± 0.8% | 0.75 ± 0.1% | 0.80 ± 0.6% |
| 65536 | by 7 | 1.19 ± 0.5% | 1.10 ± 0.5% | 0.75 ± 0.1% | 0.80 ± 0.8% |
| 1048576 | by 7 | 1.18 ± 0.6% | 1.10 ± 0.1% | 0.76 ± 0.1% | 0.80 ± 0.6% |

Each counterpart's speed relative to ours: whole: zlib 0.30x, adler2
0.43x, simd-adler32 1.61x; by 7: zlib 1.00x, adler2 0.62x, simd-adler32
0.70x.

### Zen 1, ours without `intrinsics`, without dependencies

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
| adler32 | aarch64-unknown-linux-gnu | 732 | 408 |
| adler32 | x86_64-unknown-linux-gnu | 1274 | 461 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-adler32` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`compute-adler32` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-13, at `de4f027`,
`compute-adler32` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

<!-- filer-bench: end -->
