# compute-xxhash benchmarks

The figures of `compute-xxhash`'s xxHash hashes, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| xxHash | `xxhash-c-sys` | C |
| xxhash-rust | `xxhash-rust` | Rust |
| twox-hash | `twox-hash` | Rust |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for XXH32 and XXH64.

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

### Zen 5, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.94 ± 0.9% | 0.70 ± 0.3% | 2.03 ± 0.7% | 2.03 ± 0.4% |
| 256 | whole | 10.36 ± 0.5% | 1.86 ± 0.1% | 10.67 ± 0.4% | 10.70 ± 0.6% |
| 4096 | whole | 12.18 ± 0.1% | 2.07 ± 0.2% | 12.27 ± 0.3% | 12.33 ± 0.2% |
| 65536 | whole | 12.64 ± 0.1% | 2.07 ± 0.2% | 12.60 ± 0.1% | 12.69 ± 0.2% |
| 1048576 | whole | 12.68 ± 0.1% | 2.07 ± 0.2% | 12.64 ± 0.1% | 12.71 ± 0.1% |
| 16 | by 7 | 0.86 ± 0.5% | 0.56 ± 0.1% | 0.84 ± 0.5% | 0.83 ± 0.5% |
| 256 | by 7 | 1.47 ± 0.2% | 0.90 ± 0.2% | 1.43 ± 0.1% | 1.55 ± 0.1% |
| 4096 | by 7 | 1.54 ± 0.1% | 0.93 ± 0.1% | 1.55 ± 0.5% | 1.63 ± 0.1% |
| 65536 | by 7 | 1.54 ± 0.2% | 0.92 ± 0.9% | 1.56 ± 0.1% | 1.64 ± 0.0% |
| 1048576 | by 7 | 1.54 ± 0.2% | 0.93 ± 0.1% | 1.56 ± 0.2% | 1.64 ± 0.1% |

Each counterpart's speed relative to ours: whole: xxHash 0.20x,
xxhash-rust 1.01x, twox-hash 1.02x; by 7: xxHash 0.61x, xxhash-rust
1.00x, twox-hash 1.04x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.88 ± 0.4% | 1.41 ± 0.1% | 1.96 ± 0.5% | 1.95 ± 0.3% |
| 256 | whole | 14.35 ± 0.3% | 12.06 ± 0.6% | 14.53 ± 0.9% | 14.54 ± 0.7% |
| 4096 | whole | 23.80 ± 0.1% | 23.42 ± 0.1% | 23.68 ± 0.0% | 23.96 ± 0.1% |
| 65536 | whole | 25.05 ± 0.1% | 25.06 ± 0.1% | 24.69 ± 0.8% | 25.15 ± 0.1% |
| 1048576 | whole | 25.17 ± 0.1% | 25.20 ± 0.1% | 24.96 ± 0.2% | 25.24 ± 0.2% |
| 16 | by 7 | 0.82 ± 0.1% | 0.76 ± 0.6% | 0.81 ± 0.3% | 0.81 ± 0.1% |
| 256 | by 7 | 1.94 ± 0.3% | 2.19 ± 0.1% | 2.00 ± 0.2% | 1.94 ± 0.2% |
| 4096 | by 7 | 2.13 ± 0.1% | 2.45 ± 0.8% | 2.09 ± 0.1% | 2.15 ± 0.1% |
| 65536 | by 7 | 2.15 ± 0.1% | 2.47 ± 0.2% | 2.10 ± 0.1% | 2.16 ± 0.1% |
| 1048576 | by 7 | 2.14 ± 0.1% | 2.43 ± 0.9% | 2.07 ± 0.8% | 2.16 ± 0.5% |

Each counterpart's speed relative to ours: whole: xxHash 0.91x,
xxhash-rust 1.00x, twox-hash 1.01x; by 7: xxHash 1.09x, xxhash-rust
0.99x, twox-hash 1.00x.

### Granite Rapids, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.54 ± 0.1% | 0.66 ± 1.1% | 1.56 ± 0.5% | 1.58 ± 1.3% |
| 256 | whole | 7.55 ± 0.2% | 2.73 ± 0.6% | 7.54 ± 0.2% | 7.57 ± 0.3% |
| 4096 | whole | 7.61 ± 0.1% | 3.42 ± 0.1% | 7.61 ± 0.2% | 7.64 ± 0.1% |
| 65536 | whole | 7.69 ± 0.1% | 3.47 ± 0.1% | 7.68 ± 0.1% | 7.69 ± 0.2% |
| 1048576 | whole | 7.69 ± 0.1% | 3.45 ± 0.2% | 7.69 ± 0.1% | 7.69 ± 0.1% |
| 16 | by 7 | 0.67 ± 0.4% | 0.52 ± 0.1% | 0.65 ± 0.3% | 0.67 ± 0.4% |
| 256 | by 7 | 1.24 ± 0.3% | 0.89 ± 0.8% | 1.22 ± 0.2% | 1.26 ± 0.1% |
| 4096 | by 7 | 1.34 ± 0.1% | 0.93 ± 0.8% | 1.29 ± 0.5% | 1.35 ± 0.2% |
| 65536 | by 7 | 1.35 ± 0.1% | 0.95 ± 0.1% | 1.30 ± 0.8% | 1.35 ± 0.1% |
| 1048576 | by 7 | 1.35 ± 0.1% | 0.95 ± 0.8% | 1.30 ± 0.8% | 1.35 ± 0.0% |

Each counterpart's speed relative to ours: whole: xxHash 0.43x,
xxhash-rust 1.00x, twox-hash 1.01x; by 7: xxHash 0.72x, xxhash-rust
0.97x, twox-hash 1.01x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.40 ± 0.9% | 0.65 ± 1.0% | 1.43 ± 0.4% | 1.46 ± 0.5% |
| 256 | whole | 12.01 ± 0.2% | 8.30 ± 0.6% | 12.39 ± 0.3% | 12.52 ± 0.2% |
| 4096 | whole | 14.88 ± 0.1% | 14.48 ± 0.1% | 15.05 ± 0.1% | 15.17 ± 0.1% |
| 65536 | whole | 15.35 ± 0.1% | 15.31 ± 0.1% | 15.36 ± 0.1% | 15.37 ± 0.1% |
| 1048576 | whole | 15.34 ± 0.2% | 15.32 ± 0.2% | 15.34 ± 0.3% | 15.34 ± 0.3% |
| 16 | by 7 | 0.65 ± 0.8% | 0.51 ± 1.7% | 0.62 ± 0.9% | 0.64 ± 1.3% |
| 256 | by 7 | 1.52 ± 0.4% | 1.49 ± 0.2% | 1.49 ± 0.4% | 1.55 ± 0.6% |
| 4096 | by 7 | 1.82 ± 0.2% | 1.61 ± 0.1% | 1.69 ± 0.7% | 1.77 ± 0.4% |
| 65536 | by 7 | 1.86 ± 0.2% | 1.60 ± 0.4% | 1.70 ± 0.4% | 1.78 ± 1.1% |
| 1048576 | by 7 | 1.85 ± 0.7% | 1.57 ± 0.6% | 1.71 ± 0.7% | 1.77 ± 0.3% |

Each counterpart's speed relative to ours: whole: xxHash 0.79x,
xxhash-rust 1.01x, twox-hash 1.02x; by 7: xxHash 0.87x, xxhash-rust
0.94x, twox-hash 0.98x.

### Neoverse V3, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.87 ± 1.3% | 2.13 ± 2.6% | 1.92 ± 3.4% | 1.92 ± 3.4% |
| 256 | whole | 7.69 ± 0.3% | 7.73 ± 0.8% | 5.96 ± 6.9% | 8.71 ± 0.1% |
| 4096 | whole | 9.27 ± 0.1% | 9.08 ± 2.2% | 4.34 ± 0.0% | 4.44 ± 0.0% |
| 65536 | whole | 9.43 ± 0.4% | 8.19 ± 0.9% | 4.26 ± 0.1% | 4.25 ± 0.1% |
| 1048576 | whole | 9.45 ± 1.0% | 7.96 ± 0.7% | 4.36 ± 0.0% | 4.35 ± 0.0% |
| 16 | by 7 | 0.62 ± 2.7% | 0.56 ± 0.8% | 0.53 ± 2.6% | 0.47 ± 13.7% |
| 256 | by 7 | 0.98 ± 0.1% | 0.74 ± 0.7% | 0.84 ± 0.2% | 0.74 ± 1.7% |
| 4096 | by 7 | 1.06 ± 1.5% | 0.75 ± 0.8% | 0.87 ± 0.1% | 0.75 ± 1.5% |
| 65536 | by 7 | 1.08 ± 1.5% | 0.75 ± 0.6% | 0.87 ± 0.6% | 0.77 ± 1.7% |
| 1048576 | by 7 | 1.08 ± 1.0% | 0.74 ± 0.7% | 0.87 ± 0.1% | 0.78 ± 19.2% |

Each counterpart's speed relative to ours: whole: xxHash 0.97x,
xxhash-rust 0.60x, twox-hash 0.65x; by 7: xxHash 0.75x, xxhash-rust
0.83x, twox-hash 0.74x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 1.15 ± 4.3% | 0.99 ± 2.9% | 1.07 ± 6.5% | 1.07 ± 2.2% |
| 256 | whole | 11.85 ± 1.3% | 11.48 ± 1.3% | 12.30 ± 0.6% | 12.01 ± 0.7% |
| 4096 | whole | 17.40 ± 0.6% | 16.85 ± 0.3% | 17.15 ± 2.9% | 18.26 ± 0.7% |
| 65536 | whole | 16.64 ± 1.8% | 17.29 ± 2.5% | 17.47 ± 3.6% | 17.10 ± 2.2% |
| 1048576 | whole | 24.46 ± 0.5% | 22.00 ± 1.1% | 23.23 ± 0.5% | 23.27 ± 0.5% |
| 16 | by 7 | 0.71 ± 0.5% | 0.73 ± 0.4% | 0.81 ± 0.5% | 0.72 ± 0.9% |
| 256 | by 7 | 1.13 ± 0.2% | 0.91 ± 0.3% | 1.23 ± 0.8% | 0.89 ± 0.9% |
| 4096 | by 7 | 1.50 ± 7.7% | 0.92 ± 0.4% | 1.30 ± 0.1% | 0.93 ± 1.2% |
| 65536 | by 7 | 1.68 ± 6.2% | 0.92 ± 0.2% | 1.31 ± 2.7% | 0.95 ± 0.5% |
| 1048576 | by 7 | 1.62 ± 5.9% | 0.92 ± 0.5% | 1.30 ± 2.3% | 0.95 ± 0.6% |

Each counterpart's speed relative to ours: whole: xxHash 0.95x,
xxhash-rust 1.00x, twox-hash 1.00x; by 7: xxHash 0.69x, xxhash-rust
0.92x, twox-hash 0.70x.

### Zen 1, ours portable, as by default

#### XXH32

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.42 ± 0.3% | 0.35 ± 0.3% | 0.42 ± 0.3% | 0.43 ± 0.4% |
| 256 | whole | 2.98 ± 0.3% | 1.65 ± 0.2% | 2.87 ± 0.4% | 2.88 ± 0.2% |
| 4096 | whole | 4.72 ± 0.4% | 2.14 ± 0.2% | 4.24 ± 0.3% | 4.48 ± 0.4% |
| 65536 | whole | 4.94 ± 0.2% | 2.18 ± 0.1% | 4.41 ± 0.1% | 4.66 ± 0.1% |
| 1048576 | whole | 4.94 ± 0.6% | 2.17 ± 0.4% | 4.40 ± 0.5% | 4.66 ± 0.6% |
| 16 | by 7 | 0.26 ± 0.6% | 0.22 ± 2.5% | 0.25 ± 0.3% | 0.25 ± 0.5% |
| 256 | by 7 | 0.51 ± 0.3% | 0.44 ± 0.7% | 0.49 ± 0.6% | 0.50 ± 0.4% |
| 4096 | by 7 | 0.56 ± 0.4% | 0.46 ± 1.0% | 0.53 ± 0.5% | 0.53 ± 0.9% |
| 65536 | by 7 | 0.55 ± 0.9% | 0.47 ± 1.0% | 0.52 ± 0.8% | 0.54 ± 0.7% |
| 1048576 | by 7 | 0.55 ± 0.7% | 0.46 ± 0.6% | 0.52 ± 0.9% | 0.53 ± 0.6% |

Each counterpart's speed relative to ours: whole: xxHash 0.53x,
xxhash-rust 0.93x, twox-hash 0.96x; by 7: xxHash 0.84x, xxhash-rust
0.95x, twox-hash 0.96x.

#### XXH64

| Bytes | Fed | ours | xxHash | xxhash-rust | twox-hash |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.36 ± 0.5% | 0.33 ± 0.5% | 0.37 ± 0.5% | 0.37 ± 0.9% |
| 256 | whole | 3.52 ± 0.6% | 3.30 ± 0.6% | 3.49 ± 0.3% | 3.61 ± 0.4% |
| 4096 | whole | 8.72 ± 0.4% | 8.32 ± 0.3% | 8.45 ± 0.3% | 8.83 ± 0.2% |
| 65536 | whole | 9.55 ± 0.7% | 9.33 ± 0.3% | 9.37 ± 0.7% | 9.81 ± 0.3% |
| 1048576 | whole | 9.58 ± 0.3% | 9.35 ± 0.5% | 9.45 ± 0.8% | 9.79 ± 0.1% |
| 16 | by 7 | 0.27 ± 0.7% | 0.23 ± 0.9% | 0.25 ± 0.7% | 0.26 ± 0.5% |
| 256 | by 7 | 0.57 ± 0.6% | 0.57 ± 0.2% | 0.54 ± 0.3% | 0.55 ± 0.5% |
| 4096 | by 7 | 0.65 ± 0.2% | 0.66 ± 0.4% | 0.60 ± 0.4% | 0.62 ± 1.5% |
| 65536 | by 7 | 0.65 ± 0.5% | 0.67 ± 0.3% | 0.61 ± 0.3% | 0.62 ± 0.7% |
| 1048576 | by 7 | 0.65 ± 0.2% | 0.67 ± 0.3% | 0.61 ± 0.3% | 0.62 ± 2.3% |

Each counterpart's speed relative to ours: whole: xxHash 0.95x,
xxhash-rust 0.99x, twox-hash 1.03x; by 7: xxHash 0.99x, xxhash-rust
0.93x, twox-hash 0.96x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| xxh32 | aarch64-apple-darwin | 500 | 400 |
| xxh32 | aarch64-unknown-linux-gnu | 500 | 408 |
| xxh32 | x86_64-unknown-linux-gnu | 614 | 355 |
| xxh64 | aarch64-apple-darwin | 632 | 552 |
| xxh64 | aarch64-unknown-linux-gnu | 636 | 560 |
| xxh64 | x86_64-unknown-linux-gnu | 864 | 570 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-xxhash` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On aarch64-unknown-linux-gnu, measured 2026-11-10, at `b674208`,
`compute-xxhash` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-xxhash` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
