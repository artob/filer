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
1.99.1, 2026-11-11; and AMD EPYC 7551P (Zen 1), 4 vCPUs under KVM, by
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
| 16 | whole | 1.91 ± 2.0% | 1.84 ± 1.8% | 1.17 ± 1.0% | 1.99 ± 0.9% |
| 256 | whole | 24.35 ± 0.8% | 4.39 ± 0.1% | 5.46 ± 0.1% | 28.02 ± 0.8% |
| 4096 | whole | 129.95 ± 0.2% | 4.44 ± 0.1% | 6.88 ± 0.0% | 61.97 ± 0.0% |
| 65536 | whole | 121.42 ± 0.4% | 4.47 ± 0.1% | 7.92 ± 0.1% | 58.85 ± 0.1% |
| 1048576 | whole | 120.50 ± 0.1% | 4.47 ± 0.0% | 8.83 ± 0.2% | 58.18 ± 0.2% |
| 16 | by 7 | 1.24 ± 0.2% | 1.31 ± 0.4% | 0.74 ± 0.2% | 1.34 ± 0.8% |
| 256 | by 7 | 1.82 ± 0.3% | 1.33 ± 0.6% | 0.91 ± 0.2% | 1.50 ± 0.5% |
| 4096 | by 7 | 1.98 ± 0.2% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.59 ± 0.1% |
| 65536 | by 7 | 1.99 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |
| 1048576 | by 7 | 1.98 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.10x, adler2
0.13x, simd-adler32 0.67x; by 7: zlib 0.76x, adler2 0.50x, simd-adler32
0.86x.

### Zen 5, ours without `intrinsics`, without dependencies

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.96 ± 1.2% | 1.82 ± 0.7% | 1.16 ± 1.1% | 1.97 ± 0.8% |
| 256 | whole | 11.63 ± 0.4% | 4.40 ± 0.1% | 5.47 ± 0.1% | 28.20 ± 0.5% |
| 4096 | whole | 16.48 ± 1.2% | 4.45 ± 0.1% | 6.89 ± 0.2% | 62.05 ± 0.1% |
| 65536 | whole | 17.84 ± 0.1% | 4.47 ± 0.1% | 7.92 ± 0.2% | 58.87 ± 0.2% |
| 1048576 | whole | 17.82 ± 0.1% | 4.47 ± 0.1% | 8.84 ± 0.1% | 58.45 ± 0.2% |
| 16 | by 7 | 1.29 ± 0.3% | 1.32 ± 0.6% | 0.74 ± 0.2% | 1.32 ± 0.7% |
| 256 | by 7 | 1.86 ± 0.4% | 1.33 ± 0.3% | 0.91 ± 0.2% | 1.50 ± 0.7% |
| 4096 | by 7 | 2.00 ± 0.0% | 1.36 ± 0.0% | 0.93 ± 0.0% | 1.59 ± 0.1% |
| 65536 | by 7 | 2.01 ± 0.1% | 1.37 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |
| 1048576 | by 7 | 2.01 ± 0.1% | 1.36 ± 0.1% | 0.93 ± 0.1% | 1.60 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.36x, adler2
0.48x, simd-adler32 2.51x; by 7: zlib 0.75x, adler2 0.49x, simd-adler32
0.84x.

### Granite Rapids, ours with `intrinsics`, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.58 ± 0.6% | 1.50 ± 2.4% | 0.96 ± 0.3% | 1.58 ± 2.6% |
| 256 | whole | 19.31 ± 1.2% | 3.75 ± 0.0% | 4.09 ± 0.1% | 22.39 ± 1.2% |
| 4096 | whole | 82.97 ± 0.8% | 3.73 ± 0.1% | 6.10 ± 0.0% | 45.38 ± 0.1% |
| 65536 | whole | 86.23 ± 0.2% | 3.72 ± 0.1% | 8.47 ± 0.1% | 41.60 ± 0.0% |
| 1048576 | whole | 86.08 ± 0.4% | 3.72 ± 0.1% | 10.18 ± 0.2% | 41.60 ± 0.2% |
| 16 | by 7 | 0.97 ± 0.5% | 1.06 ± 0.9% | 0.60 ± 0.2% | 0.97 ± 0.8% |
| 256 | by 7 | 1.25 ± 0.4% | 1.09 ± 0.1% | 0.77 ± 0.1% | 1.20 ± 0.2% |
| 4096 | by 7 | 1.54 ± 0.1% | 1.08 ± 0.2% | 0.79 ± 0.1% | 1.23 ± 0.1% |
| 65536 | by 7 | 1.52 ± 0.4% | 1.08 ± 0.2% | 0.79 ± 0.1% | 1.23 ± 0.0% |
| 1048576 | by 7 | 1.40 ± 0.9% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.11x, adler2
0.16x, simd-adler32 0.68x; by 7: zlib 0.82x, adler2 0.56x, simd-adler32
0.89x.

### Granite Rapids, ours without `intrinsics`, without dependencies

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.60 ± 1.3% | 1.52 ± 1.2% | 0.96 ± 0.4% | 1.59 ± 1.3% |
| 256 | whole | 8.09 ± 0.1% | 3.75 ± 0.1% | 4.11 ± 0.1% | 22.85 ± 1.2% |
| 4096 | whole | 12.13 ± 0.1% | 3.73 ± 0.1% | 6.10 ± 0.1% | 45.68 ± 0.2% |
| 65536 | whole | 12.55 ± 0.1% | 3.72 ± 0.0% | 8.47 ± 0.1% | 41.38 ± 0.1% |
| 1048576 | whole | 12.55 ± 0.2% | 3.71 ± 0.1% | 10.17 ± 0.0% | 41.34 ± 0.1% |
| 16 | by 7 | 1.05 ± 1.4% | 1.05 ± 1.2% | 0.60 ± 0.1% | 1.01 ± 0.3% |
| 256 | by 7 | 1.50 ± 0.2% | 1.09 ± 0.0% | 0.77 ± 0.1% | 1.20 ± 0.1% |
| 4096 | by 7 | 1.65 ± 0.1% | 1.08 ± 0.1% | 0.79 ± 0.1% | 1.23 ± 0.1% |
| 65536 | by 7 | 1.67 ± 0.2% | 1.08 ± 0.3% | 0.79 ± 0.2% | 1.23 ± 0.2% |
| 1048576 | by 7 | 1.65 ± 0.1% | 1.08 ± 0.1% | 0.79 ± 0.0% | 1.23 ± 0.1% |

Each counterpart's speed relative to ours: whole: zlib 0.41x, adler2
0.61x, simd-adler32 2.59x; by 7: zlib 0.73x, adler2 0.50x, simd-adler32
0.79x.

### Neoverse V3, ours with `intrinsics`, as by default

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.64 ± 2.4% | 2.37 ± 0.6% | 1.28 ± 0.6% | 1.52 ± 0.5% |
| 256 | whole | 17.23 ± 0.5% | 3.25 ± 0.3% | 5.11 ± 0.6% | 17.25 ± 0.9% |
| 4096 | whole | 47.67 ± 0.2% | 3.19 ± 0.2% | 5.99 ± 1.2% | 34.96 ± 0.2% |
| 65536 | whole | 53.40 ± 0.1% | 3.18 ± 0.2% | 5.90 ± 0.4% | 35.67 ± 0.0% |
| 1048576 | whole | 48.13 ± 1.1% | 3.18 ± 0.0% | 6.27 ± 0.1% | 35.05 ± 0.1% |
| 16 | by 7 | 1.11 ± 1.0% | 1.44 ± 0.5% | 0.64 ± 0.4% | 0.94 ± 0.2% |
| 256 | by 7 | 1.20 ± 0.0% | 1.13 ± 0.6% | 0.74 ± 0.7% | 0.79 ± 0.3% |
| 4096 | by 7 | 1.21 ± 0.1% | 1.11 ± 0.5% | 0.75 ± 0.1% | 0.80 ± 0.9% |
| 65536 | by 7 | 1.20 ± 0.4% | 1.10 ± 0.0% | 0.75 ± 0.1% | 0.78 ± 0.4% |
| 1048576 | by 7 | 1.20 ± 0.3% | 1.10 ± 0.2% | 0.76 ± 0.3% | 0.80 ± 0.6% |

Each counterpart's speed relative to ours: whole: zlib 0.15x, adler2
0.21x, simd-adler32 0.80x; by 7: zlib 0.99x, adler2 0.62x, simd-adler32
0.69x.

### Neoverse V3, ours without `intrinsics`, without dependencies

#### Adler-32

| Bytes | Fed | ours | zlib | adler2 | simd-adler32 |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.62 ± 1.6% | 2.35 ± 0.3% | 1.28 ± 1.2% | 1.50 ± 0.4% |
| 256 | whole | 10.94 ± 0.9% | 3.26 ± 0.2% | 5.07 ± 1.5% | 17.18 ± 1.3% |
| 4096 | whole | 17.35 ± 0.0% | 3.19 ± 0.1% | 5.97 ± 0.9% | 34.88 ± 0.0% |
| 65536 | whole | 18.12 ± 0.0% | 3.18 ± 0.0% | 5.91 ± 0.6% | 35.67 ± 0.2% |
| 1048576 | whole | 17.89 ± 0.3% | 3.18 ± 0.2% | 6.26 ± 0.1% | 34.99 ± 0.4% |
| 16 | by 7 | 1.10 ± 1.1% | 1.44 ± 0.3% | 0.63 ± 0.2% | 0.95 ± 0.4% |
| 256 | by 7 | 1.13 ± 0.2% | 1.13 ± 0.2% | 0.75 ± 0.1% | 0.79 ± 0.4% |
| 4096 | by 7 | 1.15 ± 0.1% | 1.10 ± 0.1% | 0.75 ± 0.1% | 0.76 ± 0.3% |
| 65536 | by 7 | 1.15 ± 0.1% | 1.10 ± 0.0% | 0.75 ± 0.0% | 0.77 ± 0.7% |
| 1048576 | by 7 | 1.15 ± 0.1% | 1.10 ± 0.1% | 0.76 ± 0.2% | 0.79 ± 0.5% |

Each counterpart's speed relative to ours: whole: zlib 0.30x, adler2
0.43x, simd-adler32 1.62x; by 7: zlib 1.03x, adler2 0.64x, simd-adler32
0.71x.

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
| adler32 | aarch64-unknown-linux-gnu | 708 | 376 |
| adler32 | x86_64-unknown-linux-gnu | 1147 | 430 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-adler32` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-10, at `b674208`,
`compute-adler32` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-adler32` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
