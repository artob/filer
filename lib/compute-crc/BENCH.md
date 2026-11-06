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
| 16 | whole | 0.76 ± 0.9% | 0.27 ± 0.4% | 0.67 ± 0.9% |
| 256 | whole | 1.21 ± 0.4% | 2.87 ± 0.9% | 1.96 ± 0.6% |
| 4096 | whole | 1.26 ± 0.3% | 8.59 ± 1.0% | 2.26 ± 0.4% |
| 65536 | whole | 1.26 ± 0.4% | 9.75 ± 0.7% | 2.30 ± 1.0% |
| 1048576 | whole | 1.26 ± 0.5% | 9.76 ± 0.9% | 2.29 ± 0.6% |
| 16 | by 7 | 0.30 ± 0.7% | 0.11 ± 0.2% | 0.22 ± 0.6% |
| 256 | by 7 | 0.30 ± 0.1% | 0.18 ± 0.2% | 0.26 ± 0.1% |
| 4096 | by 7 | 0.30 ± 0.2% | 0.18 ± 0.2% | 0.26 ± 0.2% |
| 65536 | by 7 | 0.30 ± 0.3% | 0.18 ± 0.2% | 0.26 ± 0.1% |
| 1048576 | by 7 | 0.30 ± 0.5% | 0.18 ± 0.5% | 0.26 ± 0.3% |

Each counterpart's speed relative to ours: whole: crc-fast 3.23x, crc
1.54x; by 7: crc-fast 0.54x, crc 0.83x.

#### CRC-32

| Bytes | Fed | ours | zlib | crc32fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 16 | whole | 0.75 ± 0.6% | 0.25 ± 0.4% | 0.79 ± 0.5% | 0.27 ± 0.5% | 0.71 ± 0.4% |
| 256 | whole | 1.30 ± 0.3% | 0.94 ± 0.4% | 4.58 ± 1.0% | 2.89 ± 0.1% | 1.88 ± 1.0% |
| 4096 | whole | 1.34 ± 0.2% | 2.35 ± 0.6% | 8.61 ± 0.3% | 8.63 ± 0.4% | 2.03 ± 0.9% |
| 65536 | whole | 1.34 ± 0.2% | 2.61 ± 0.3% | 9.24 ± 0.2% | 9.75 ± 0.4% | 2.03 ± 0.4% |
| 1048576 | whole | 1.34 ± 0.3% | 2.60 ± 0.3% | 9.19 ± 0.3% | 9.78 ± 0.6% | 2.02 ± 0.6% |
| 16 | by 7 | 0.30 ± 0.5% | 0.24 ± 0.7% | 0.34 ± 0.8% | 0.11 ± 0.4% | 0.26 ± 0.8% |
| 256 | by 7 | 0.30 ± 0.3% | 0.29 ± 0.3% | 0.44 ± 0.3% | 0.18 ± 0.1% | 0.30 ± 0.3% |
| 4096 | by 7 | 0.30 ± 0.3% | 0.29 ± 1.0% | 0.45 ± 0.6% | 0.18 ± 0.3% | 0.31 ± 0.2% |
| 65536 | by 7 | 0.30 ± 0.2% | 0.29 ± 0.4% | 0.45 ± 0.2% | 0.18 ± 0.3% | 0.31 ± 0.4% |
| 1048576 | by 7 | 0.30 ± 0.3% | 0.29 ± 0.3% | 0.45 ± 0.6% | 0.18 ± 0.2% | 0.31 ± 0.4% |

Each counterpart's speed relative to ours: whole: zlib 1.10x, crc32fast
4.08x, crc-fast 3.07x, crc 1.36x; by 7: zlib 0.91x, crc32fast 1.40x,
crc-fast 0.54x, crc 0.98x.

#### CRC-32C

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.76 ± 1.0% | 0.42 ± 0.5% | 0.70 ± 0.6% |
| 256 | whole | 1.29 ± 0.4% | 3.35 ± 0.2% | 1.86 ± 0.4% |
| 4096 | whole | 1.34 ± 0.5% | 5.88 ± 0.3% | 2.01 ± 1.0% |
| 65536 | whole | 1.35 ± 0.1% | 7.42 ± 0.2% | 2.03 ± 0.9% |
| 1048576 | whole | 1.35 ± 0.3% | 7.56 ± 0.6% | 2.02 ± 0.9% |
| 16 | by 7 | 0.30 ± 0.8% | 0.23 ± 0.9% | 0.26 ± 0.3% |
| 256 | by 7 | 0.30 ± 0.3% | 0.43 ± 0.8% | 0.30 ± 0.4% |
| 4096 | by 7 | 0.30 ± 0.6% | 0.47 ± 0.9% | 0.31 ± 0.5% |
| 65536 | by 7 | 0.30 ± 0.1% | 0.47 ± 0.2% | 0.31 ± 0.2% |
| 1048576 | by 7 | 0.30 ± 0.6% | 0.47 ± 0.6% | 0.31 ± 0.3% |

Each counterpart's speed relative to ours: whole: crc-fast 2.88x, crc
1.35x; by 7: crc-fast 1.32x, crc 0.98x.

#### CRC-32/BZIP2

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.74 ± 1.3% | 0.26 ± 0.3% | 0.68 ± 0.3% |
| 256 | whole | 1.20 ± 0.2% | 2.78 ± 0.2% | 1.87 ± 1.3% |
| 4096 | whole | 1.24 ± 0.9% | 8.32 ± 0.6% | 2.02 ± 0.8% |
| 65536 | whole | 1.24 ± 0.2% | 9.44 ± 0.3% | 2.04 ± 1.0% |
| 1048576 | whole | 1.23 ± 0.3% | 9.41 ± 0.6% | 2.01 ± 0.9% |
| 16 | by 7 | 0.28 ± 0.8% | 0.11 ± 0.4% | 0.26 ± 0.2% |
| 256 | by 7 | 0.28 ± 0.2% | 0.16 ± 0.8% | 0.31 ± 0.3% |
| 4096 | by 7 | 0.28 ± 0.7% | 0.17 ± 1.0% | 0.31 ± 0.6% |
| 65536 | by 7 | 0.28 ± 0.3% | 0.17 ± 1.0% | 0.31 ± 0.2% |
| 1048576 | by 7 | 0.28 ± 0.2% | 0.17 ± 1.0% | 0.31 ± 0.2% |

Each counterpart's speed relative to ours: whole: crc-fast 3.18x, crc
1.44x; by 7: crc-fast 0.56x, crc 1.07x.

#### CRC-64/XZ

| Bytes | Fed | ours | crc64fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.76 ± 0.3% | 0.42 ± 0.9% | 0.25 ± 0.6% | 0.41 ± 0.3% |
| 256 | whole | 1.16 ± 0.9% | 3.93 ± 0.2% | 2.59 ± 0.3% | 1.33 ± 1.0% |
| 4096 | whole | 1.14 ± 1.0% | 9.02 ± 0.5% | 8.38 ± 0.3% | 1.37 ± 0.3% |
| 65536 | whole | 1.14 ± 1.0% | 9.83 ± 0.3% | 9.77 ± 0.2% | 1.38 ± 0.4% |
| 1048576 | whole | 1.13 ± 0.8% | 9.78 ± 0.6% | 9.76 ± 0.7% | 1.37 ± 0.6% |
| 16 | by 7 | 0.30 ± 0.9% | 0.21 ± 0.1% | 0.10 ± 0.6% | 0.19 ± 0.5% |
| 256 | by 7 | 0.30 ± 0.4% | 0.29 ± 0.9% | 0.16 ± 0.4% | 0.27 ± 0.2% |
| 4096 | by 7 | 0.30 ± 0.5% | 0.30 ± 0.4% | 0.16 ± 0.5% | 0.27 ± 0.5% |
| 65536 | by 7 | 0.30 ± 0.4% | 0.30 ± 0.4% | 0.16 ± 0.2% | 0.27 ± 0.4% |
| 1048576 | by 7 | 0.30 ± 0.3% | 0.30 ± 0.3% | 0.16 ± 0.2% | 0.27 ± 0.2% |

Each counterpart's speed relative to ours: whole: crc64fast 4.07x, crc-
fast 3.32x, crc 1.02x; by 7: crc64fast 0.91x, crc-fast 0.48x, crc 0.84x.

### Ours with `intrinsics`

#### CRC-16/MODBUS

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.75 ± 1.1% | 0.27 ± 0.4% | 0.67 ± 0.7% |
| 256 | whole | 1.23 ± 0.6% | 2.89 ± 0.4% | 1.99 ± 0.8% |
| 4096 | whole | 1.26 ± 0.5% | 8.62 ± 0.4% | 2.27 ± 0.5% |
| 65536 | whole | 1.26 ± 0.5% | 9.73 ± 0.5% | 2.29 ± 0.6% |
| 1048576 | whole | 1.25 ± 0.3% | 9.74 ± 0.7% | 2.27 ± 0.3% |
| 16 | by 7 | 0.30 ± 0.8% | 0.11 ± 0.5% | 0.22 ± 0.9% |
| 256 | by 7 | 0.30 ± 0.1% | 0.18 ± 0.2% | 0.26 ± 0.1% |
| 4096 | by 7 | 0.30 ± 0.3% | 0.18 ± 0.2% | 0.26 ± 0.2% |
| 65536 | by 7 | 0.30 ± 0.5% | 0.18 ± 0.4% | 0.26 ± 0.3% |
| 1048576 | by 7 | 0.30 ± 0.3% | 0.18 ± 0.3% | 0.26 ± 0.2% |

Each counterpart's speed relative to ours: whole: crc-fast 3.22x, crc
1.54x; by 7: crc-fast 0.54x, crc 0.83x.

#### CRC-32

| Bytes | Fed | ours | zlib | crc32fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 16 | whole | 0.77 ± 0.7% | 0.25 ± 0.2% | 0.79 ± 0.9% | 0.27 ± 0.2% | 0.71 ± 0.4% |
| 256 | whole | 1.28 ± 0.3% | 0.94 ± 0.4% | 4.58 ± 0.6% | 2.89 ± 0.3% | 1.82 ± 1.0% |
| 4096 | whole | 1.33 ± 0.5% | 2.34 ± 0.6% | 8.57 ± 0.4% | 8.58 ± 0.4% | 1.99 ± 0.6% |
| 65536 | whole | 1.34 ± 0.2% | 2.59 ± 0.6% | 9.20 ± 0.4% | 9.74 ± 0.3% | 2.00 ± 1.0% |
| 1048576 | whole | 1.34 ± 0.3% | 2.60 ± 0.6% | 9.20 ± 0.3% | 9.80 ± 0.3% | 2.02 ± 0.1% |
| 16 | by 7 | 0.30 ± 0.9% | 0.24 ± 0.2% | 0.34 ± 0.8% | 0.11 ± 0.4% | 0.26 ± 0.3% |
| 256 | by 7 | 0.30 ± 0.5% | 0.29 ± 0.7% | 0.45 ± 0.5% | 0.18 ± 0.6% | 0.30 ± 0.6% |
| 4096 | by 7 | 0.30 ± 0.1% | 0.29 ± 0.8% | 0.45 ± 0.2% | 0.18 ± 0.3% | 0.31 ± 0.2% |
| 65536 | by 7 | 0.30 ± 0.3% | 0.29 ± 0.1% | 0.45 ± 0.1% | 0.18 ± 0.4% | 0.31 ± 0.2% |
| 1048576 | by 7 | 0.30 ± 0.2% | 0.29 ± 0.3% | 0.45 ± 0.2% | 0.18 ± 0.3% | 0.31 ± 0.2% |

Each counterpart's speed relative to ours: whole: zlib 1.10x, crc32fast
4.07x, crc-fast 3.07x, crc 1.35x; by 7: zlib 0.91x, crc32fast 1.40x,
crc-fast 0.54x, crc 0.98x.

#### CRC-32C

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.67 ± 0.7% | 0.42 ± 0.3% | 0.70 ± 0.3% |
| 256 | whole | 4.27 ± 0.5% | 3.34 ± 0.7% | 1.88 ± 0.5% |
| 4096 | whole | 5.84 ± 0.3% | 5.90 ± 0.9% | 2.01 ± 0.6% |
| 65536 | whole | 6.22 ± 0.9% | 7.41 ± 0.2% | 2.03 ± 0.4% |
| 1048576 | whole | 6.11 ± 0.6% | 7.60 ± 0.4% | 2.03 ± 0.9% |
| 16 | by 7 | 0.34 ± 0.9% | 0.23 ± 0.9% | 0.26 ± 0.7% |
| 256 | by 7 | 0.48 ± 0.8% | 0.43 ± 0.5% | 0.30 ± 0.3% |
| 4096 | by 7 | 0.51 ± 0.6% | 0.47 ± 0.6% | 0.31 ± 0.4% |
| 65536 | by 7 | 0.51 ± 0.1% | 0.47 ± 0.4% | 0.31 ± 0.1% |
| 1048576 | by 7 | 0.51 ± 0.2% | 0.47 ± 0.4% | 0.31 ± 0.3% |

Each counterpart's speed relative to ours: whole: crc-fast 0.94x, crc
0.44x; by 7: crc-fast 0.86x, crc 0.64x.

#### CRC-32/BZIP2

| Bytes | Fed | ours | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.74 ± 0.9% | 0.26 ± 0.4% | 0.69 ± 0.2% |
| 256 | whole | 1.20 ± 0.3% | 2.77 ± 0.5% | 1.88 ± 1.0% |
| 4096 | whole | 1.23 ± 0.2% | 8.30 ± 0.3% | 2.01 ± 0.6% |
| 65536 | whole | 1.23 ± 0.6% | 9.41 ± 0.5% | 2.01 ± 0.9% |
| 1048576 | whole | 1.23 ± 0.2% | 9.44 ± 0.3% | 2.02 ± 0.6% |
| 16 | by 7 | 0.28 ± 0.3% | 0.11 ± 0.8% | 0.26 ± 0.3% |
| 256 | by 7 | 0.28 ± 0.3% | 0.17 ± 0.9% | 0.30 ± 0.2% |
| 4096 | by 7 | 0.28 ± 0.3% | 0.17 ± 1.0% | 0.31 ± 0.3% |
| 65536 | by 7 | 0.28 ± 0.3% | 0.17 ± 0.7% | 0.31 ± 0.3% |
| 1048576 | by 7 | 0.27 ± 0.4% | 0.17 ± 0.9% | 0.31 ± 0.3% |

Each counterpart's speed relative to ours: whole: crc-fast 3.18x, crc
1.45x; by 7: crc-fast 0.56x, crc 1.08x.

#### CRC-64/XZ

| Bytes | Fed | ours | crc64fast | crc-fast | crc |
| ---: | --- | ---: | ---: | ---: | ---: |
| 16 | whole | 0.76 ± 0.2% | 0.43 ± 0.4% | 0.25 ± 0.6% | 0.41 ± 0.9% |
| 256 | whole | 1.18 ± 0.9% | 3.91 ± 0.3% | 2.66 ± 0.9% | 1.33 ± 1.0% |
| 4096 | whole | 1.15 ± 0.7% | 9.08 ± 0.3% | 8.51 ± 0.2% | 1.38 ± 0.2% |
| 65536 | whole | 1.13 ± 0.9% | 9.78 ± 0.6% | 9.72 ± 0.6% | 1.38 ± 0.5% |
| 1048576 | whole | 1.13 ± 0.7% | 9.79 ± 0.9% | 9.80 ± 0.5% | 1.38 ± 0.3% |
| 16 | by 7 | 0.31 ± 0.7% | 0.21 ± 0.5% | 0.10 ± 0.7% | 0.19 ± 0.7% |
| 256 | by 7 | 0.30 ± 0.6% | 0.29 ± 0.5% | 0.16 ± 0.8% | 0.27 ± 0.5% |
| 4096 | by 7 | 0.30 ± 0.6% | 0.30 ± 0.5% | 0.16 ± 0.4% | 0.27 ± 0.4% |
| 65536 | by 7 | 0.30 ± 0.3% | 0.30 ± 0.2% | 0.16 ± 0.2% | 0.27 ± 0.3% |
| 1048576 | by 7 | 0.30 ± 0.3% | 0.30 ± 0.4% | 0.16 ± 0.4% | 0.27 ± 0.5% |

Each counterpart's speed relative to ours: whole: crc64fast 4.06x, crc-
fast 3.34x, crc 1.02x; by 7: crc64fast 0.91x, crc-fast 0.48x, crc 0.83x.

## Code size

The bytes of `.text` it adds to a minimal `#![no_std]` binary: not
measured yet.
