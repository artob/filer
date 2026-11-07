# compute-crc benchmarks

The figures of `compute-crc`'s CRCs, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| zlib | `libz-sys` | C |
| crc-fast | `crc-fast` | Rust, with SIMD |
| crc | `crc` | Rust, portable |
| crc32fast | `crc32fast` | Rust, with SIMD |
| crc64fast | `crc64fast` | Rust, with SIMD |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for CRC-16/MODBUS,
CRC-32, CRC-32C, CRC-32/BZIP2, and CRC-64/XZ.

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

#### CRC-16/MODBUS

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.63 ± 0.9% | 0.27 ± 0.5% | 0.67 ± 0.9% |
| 256 | whole | 2.23 ± 0.5% | 2.90 ± 0.4% | 1.97 ± 0.3% |
| 4096 | whole | 2.63 ± 0.8% | 8.52 ± 0.8% | 2.23 ± 0.9% |
| 65536 | whole | 2.70 ± 0.6% | 9.72 ± 0.3% | 2.29 ± 0.7% |
| 1048576 | whole | 2.63 ± 0.8% | 9.73 ± 0.9% | 2.27 ± 0.7% |
| 16 | by 7 | 0.33 ± 0.5% | 0.11 ± 0.6% | 0.22 ± 0.2% |
| 256 | by 7 | 0.48 ± 0.4% | 0.18 ± 0.5% | 0.26 ± 0.7% |
| 4096 | by 7 | 0.49 ± 0.3% | 0.18 ± 0.3% | 0.26 ± 0.3% |
| 65536 | by 7 | 0.49 ± 0.2% | 0.18 ± 0.6% | 0.26 ± 0.2% |
| 1048576 | by 7 | 0.49 ± 0.7% | 0.18 ± 0.7% | 0.26 ± 0.7% |

Each counterpart's speed relative to ours: whole: crc-fast 1.89x, crc
0.90x; by 7: crc-fast 0.36x, crc 0.56x.

#### CRC-32

| Bytes | Fed | ours | zlib | crc32fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 16 | whole | 0.64 ± 1.0% | 0.25 ± 0.2% | 0.79 ± 0.2% | 0.27 ± 0.4% | 0.71 ± 0.2% |
| 256 | whole | 2.26 ± 0.9% | 0.95 ± 0.2% | 4.58 ± 1.0% | 2.90 ± 0.1% | 1.89 ± 0.9% |
| 4096 | whole | 2.54 ± 0.5% | 2.33 ± 0.3% | 8.57 ± 0.1% | 8.58 ± 0.3% | 2.01 ± 0.6% |
| 65536 | whole | 2.54 ± 1.0% | 2.60 ± 0.7% | 9.18 ± 0.5% | 9.74 ± 0.3% | 2.02 ± 0.9% |
| 1048576 | whole | 2.55 ± 0.9% | 2.60 ± 0.5% | 9.16 ± 0.5% | 9.74 ± 0.6% | 2.01 ± 0.4% |
| 16 | by 7 | 0.31 ± 1.0% | 0.24 ± 0.6% | 0.33 ± 0.4% | 0.11 ± 0.7% | 0.25 ± 0.2% |
| 256 | by 7 | 0.44 ± 0.3% | 0.29 ± 0.4% | 0.44 ± 0.3% | 0.17 ± 0.8% | 0.30 ± 0.2% |
| 4096 | by 7 | 0.45 ± 0.3% | 0.29 ± 0.9% | 0.45 ± 0.2% | 0.18 ± 0.2% | 0.31 ± 0.4% |
| 65536 | by 7 | 0.45 ± 0.3% | 0.29 ± 0.3% | 0.45 ± 0.4% | 0.18 ± 0.2% | 0.31 ± 0.2% |
| 1048576 | by 7 | 0.45 ± 0.6% | 0.29 ± 0.6% | 0.45 ± 0.7% | 0.18 ± 1.0% | 0.31 ± 0.5% |

Each counterpart's speed relative to ours: whole: zlib 0.69x, crc32fast
2.56x, crc-fast 1.93x, crc 0.85x; by 7: zlib 0.67x, crc32fast 1.01x,
crc-fast 0.39x, crc 0.71x.

#### CRC-32C

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.64 ± 0.9% | 0.42 ± 0.6% | 0.69 ± 0.6% |
| 256 | whole | 2.22 ± 0.9% | 3.33 ± 0.3% | 1.86 ± 1.2% |
| 4096 | whole | 2.53 ± 1.0% | 5.87 ± 0.3% | 1.99 ± 0.3% |
| 65536 | whole | 2.55 ± 1.0% | 7.36 ± 0.6% | 2.00 ± 0.7% |
| 1048576 | whole | 2.54 ± 0.4% | 7.48 ± 0.3% | 1.99 ± 0.6% |
| 16 | by 7 | 0.31 ± 0.7% | 0.23 ± 0.9% | 0.25 ± 0.5% |
| 256 | by 7 | 0.44 ± 0.4% | 0.43 ± 0.3% | 0.30 ± 0.2% |
| 4096 | by 7 | 0.44 ± 0.9% | 0.46 ± 0.9% | 0.30 ± 0.8% |
| 65536 | by 7 | 0.45 ± 0.5% | 0.46 ± 0.7% | 0.31 ± 0.6% |
| 1048576 | by 7 | 0.44 ± 0.8% | 0.46 ± 0.8% | 0.30 ± 0.9% |

Each counterpart's speed relative to ours: whole: crc-fast 1.81x, crc
0.85x; by 7: crc-fast 0.96x, crc 0.71x.

#### CRC-32/BZIP2

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.64 ± 0.4% | 0.26 ± 0.3% | 0.67 ± 0.8% |
| 256 | whole | 2.13 ± 0.9% | 2.74 ± 0.4% | 1.84 ± 0.6% |
| 4096 | whole | 2.48 ± 0.5% | 8.20 ± 0.7% | 2.01 ± 1.0% |
| 65536 | whole | 2.46 ± 0.9% | 9.22 ± 0.6% | 1.98 ± 0.7% |
| 1048576 | whole | 2.46 ± 0.7% | 9.34 ± 1.0% | 1.97 ± 0.5% |
| 16 | by 7 | 0.29 ± 1.0% | 0.11 ± 0.8% | 0.25 ± 0.7% |
| 256 | by 7 | 0.42 ± 0.6% | 0.16 ± 0.9% | 0.30 ± 0.7% |
| 4096 | by 7 | 0.42 ± 0.8% | 0.17 ± 0.3% | 0.31 ± 0.7% |
| 65536 | by 7 | 0.43 ± 0.4% | 0.17 ± 1.0% | 0.31 ± 0.6% |
| 1048576 | by 7 | 0.43 ± 0.8% | 0.17 ± 1.3% | 0.31 ± 0.8% |

Each counterpart's speed relative to ours: whole: crc-fast 1.89x, crc
0.86x; by 7: crc-fast 0.39x, crc 0.75x.

#### CRC-64/XZ

| Bytes | Fed | ours | crc64fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.62 ± 0.7% | 0.42 ± 0.5% | 0.25 ± 0.7% | 0.41 ± 0.6% |
| 256 | whole | 1.70 ± 1.1% | 3.87 ± 0.2% | 2.64 ± 0.4% | 1.30 ± 0.5% |
| 4096 | whole | 1.59 ± 0.9% | 8.95 ± 0.3% | 8.41 ± 0.5% | 1.35 ± 1.0% |
| 65536 | whole | 1.60 ± 0.6% | 9.67 ± 0.9% | 9.61 ± 0.6% | 1.36 ± 0.7% |
| 1048576 | whole | 1.61 ± 0.8% | 9.69 ± 0.9% | 9.68 ± 0.9% | 1.36 ± 0.9% |
| 16 | by 7 | 0.30 ± 0.4% | 0.21 ± 0.4% | 0.10 ± 0.8% | 0.19 ± 0.2% |
| 256 | by 7 | 0.44 ± 0.0% | 0.29 ± 0.3% | 0.16 ± 0.6% | 0.27 ± 0.7% |
| 4096 | by 7 | 0.44 ± 0.7% | 0.29 ± 0.9% | 0.16 ± 0.6% | 0.27 ± 0.8% |
| 65536 | by 7 | 0.44 ± 0.6% | 0.30 ± 0.4% | 0.16 ± 0.5% | 0.27 ± 0.6% |
| 1048576 | by 7 | 0.44 ± 0.7% | 0.29 ± 0.4% | 0.16 ± 0.4% | 0.27 ± 0.7% |

Each counterpart's speed relative to ours: whole: crc64fast 3.15x, crc-
fast 2.60x, crc 0.79x; by 7: crc64fast 0.67x, crc-fast 0.35x, crc 0.62x.

### Ours with `intrinsics`

#### CRC-16/MODBUS

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.64 ± 0.8% | 0.27 ± 0.7% | 0.66 ± 0.8% |
| 256 | whole | 2.20 ± 0.5% | 2.88 ± 0.7% | 1.95 ± 0.2% |
| 4096 | whole | 2.63 ± 0.6% | 8.49 ± 0.8% | 2.23 ± 0.8% |
| 65536 | whole | 2.69 ± 1.1% | 9.70 ± 0.6% | 2.29 ± 0.8% |
| 1048576 | whole | 2.64 ± 0.8% | 9.76 ± 0.5% | 2.28 ± 0.6% |
| 16 | by 7 | 0.33 ± 0.4% | 0.11 ± 0.5% | 0.22 ± 1.2% |
| 256 | by 7 | 0.48 ± 0.4% | 0.17 ± 0.5% | 0.26 ± 0.3% |
| 4096 | by 7 | 0.49 ± 0.3% | 0.18 ± 0.4% | 0.26 ± 0.5% |
| 65536 | by 7 | 0.49 ± 0.2% | 0.18 ± 0.5% | 0.26 ± 0.4% |
| 1048576 | by 7 | 0.49 ± 0.5% | 0.18 ± 0.5% | 0.26 ± 0.3% |

Each counterpart's speed relative to ours: whole: crc-fast 1.88x, crc
0.90x; by 7: crc-fast 0.36x, crc 0.56x.

#### CRC-32

| Bytes | Fed | ours | zlib | crc32fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 16 | whole | 0.64 ± 0.7% | 0.25 ± 0.7% | 0.78 ± 1.0% | 0.27 ± 0.5% | 0.70 ± 0.4% |
| 256 | whole | 2.25 ± 0.9% | 0.94 ± 0.4% | 4.56 ± 0.7% | 2.88 ± 0.5% | 1.87 ± 0.9% |
| 4096 | whole | 2.55 ± 1.0% | 2.33 ± 0.4% | 8.55 ± 0.2% | 8.56 ± 0.5% | 2.00 ± 0.8% |
| 65536 | whole | 2.52 ± 0.9% | 2.55 ± 0.7% | 9.03 ± 0.8% | 9.59 ± 0.9% | 1.99 ± 0.8% |
| 1048576 | whole | 2.55 ± 0.4% | 2.58 ± 0.7% | 9.08 ± 0.5% | 9.64 ± 0.6% | 1.99 ± 0.4% |
| 16 | by 7 | 0.31 ± 0.3% | 0.24 ± 0.4% | 0.33 ± 0.9% | 0.10 ± 0.6% | 0.25 ± 0.6% |
| 256 | by 7 | 0.44 ± 0.4% | 0.28 ± 0.5% | 0.44 ± 0.4% | 0.17 ± 0.9% | 0.30 ± 0.4% |
| 4096 | by 7 | 0.45 ± 0.2% | 0.29 ± 1.0% | 0.45 ± 0.4% | 0.18 ± 0.2% | 0.31 ± 0.1% |
| 65536 | by 7 | 0.45 ± 0.2% | 0.29 ± 0.2% | 0.45 ± 0.2% | 0.18 ± 0.3% | 0.31 ± 0.6% |
| 1048576 | by 7 | 0.45 ± 0.3% | 0.29 ± 0.2% | 0.45 ± 0.1% | 0.18 ± 0.2% | 0.31 ± 0.4% |

Each counterpart's speed relative to ours: whole: zlib 0.69x, crc32fast
2.54x, crc-fast 1.92x, crc 0.85x; by 7: zlib 0.67x, crc32fast 1.01x,
crc-fast 0.38x, crc 0.71x.

#### CRC-32C

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.51 ± 0.7% | 0.42 ± 0.7% | 0.69 ± 0.7% |
| 256 | whole | 3.81 ± 0.4% | 3.35 ± 0.6% | 1.86 ± 0.6% |
| 4096 | whole | 5.74 ± 0.4% | 5.88 ± 0.7% | 2.00 ± 1.0% |
| 65536 | whole | 6.29 ± 0.2% | 7.39 ± 0.3% | 2.02 ± 0.9% |
| 1048576 | whole | 6.05 ± 0.5% | 7.55 ± 0.4% | 2.03 ± 0.8% |
| 16 | by 7 | 0.28 ± 0.6% | 0.23 ± 1.0% | 0.26 ± 0.3% |
| 256 | by 7 | 0.45 ± 0.4% | 0.43 ± 0.4% | 0.30 ± 0.1% |
| 4096 | by 7 | 0.48 ± 0.5% | 0.46 ± 0.6% | 0.31 ± 0.3% |
| 65536 | by 7 | 0.48 ± 0.5% | 0.47 ± 0.5% | 0.31 ± 0.3% |
| 1048576 | by 7 | 0.48 ± 0.6% | 0.46 ± 0.4% | 0.31 ± 0.4% |

Each counterpart's speed relative to ours: whole: crc-fast 1.01x, crc
0.48x; by 7: crc-fast 0.94x, crc 0.69x.

#### CRC-32/BZIP2

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.63 ± 0.8% | 0.26 ± 0.7% | 0.70 ± 0.6% |
| 256 | whole | 2.23 ± 0.9% | 2.76 ± 0.9% | 1.87 ± 0.9% |
| 4096 | whole | 2.47 ± 1.0% | 8.26 ± 0.4% | 1.99 ± 0.9% |
| 65536 | whole | 2.51 ± 0.8% | 9.39 ± 0.3% | 2.01 ± 0.8% |
| 1048576 | whole | 2.50 ± 0.7% | 9.40 ± 0.5% | 2.00 ± 0.8% |
| 16 | by 7 | 0.30 ± 0.2% | 0.11 ± 0.5% | 0.26 ± 0.3% |
| 256 | by 7 | 0.42 ± 0.1% | 0.16 ± 1.0% | 0.30 ± 0.2% |
| 4096 | by 7 | 0.43 ± 0.3% | 0.17 ± 0.7% | 0.31 ± 0.1% |
| 65536 | by 7 | 0.43 ± 0.5% | 0.17 ± 0.6% | 0.31 ± 0.7% |
| 1048576 | by 7 | 0.43 ± 0.2% | 0.17 ± 0.8% | 0.31 ± 0.2% |

Each counterpart's speed relative to ours: whole: crc-fast 1.89x, crc
0.86x; by 7: crc-fast 0.38x, crc 0.74x.

#### CRC-64/XZ

| Bytes | Fed | ours | crc64fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.63 ± 0.9% | 0.43 ± 0.6% | 0.25 ± 0.6% | 0.41 ± 0.8% |
| 256 | whole | 1.75 ± 0.9% | 3.92 ± 0.4% | 2.67 ± 0.4% | 1.32 ± 0.8% |
| 4096 | whole | 1.62 ± 0.7% | 9.02 ± 0.7% | 8.39 ± 0.5% | 1.37 ± 0.8% |
| 65536 | whole | 1.63 ± 0.6% | 9.79 ± 0.7% | 9.75 ± 0.6% | 1.38 ± 0.8% |
| 1048576 | whole | 1.62 ± 0.3% | 9.80 ± 0.5% | 9.84 ± 0.6% | 1.38 ± 0.4% |
| 16 | by 7 | 0.32 ± 0.3% | 0.21 ± 0.5% | 0.10 ± 0.9% | 0.19 ± 0.3% |
| 256 | by 7 | 0.43 ± 0.4% | 0.29 ± 0.8% | 0.16 ± 0.6% | 0.26 ± 1.0% |
| 4096 | by 7 | 0.44 ± 0.7% | 0.29 ± 0.6% | 0.16 ± 0.4% | 0.27 ± 0.8% |
| 65536 | by 7 | 0.45 ± 0.6% | 0.30 ± 0.3% | 0.16 ± 0.2% | 0.27 ± 0.2% |
| 1048576 | by 7 | 0.44 ± 0.4% | 0.30 ± 0.3% | 0.16 ± 0.4% | 0.27 ± 0.4% |

Each counterpart's speed relative to ours: whole: crc64fast 3.14x, crc-
fast 2.57x, crc 0.78x; by 7: crc64fast 0.66x, crc-fast 0.35x, crc 0.61x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->
<!-- filer-bench: end -->
