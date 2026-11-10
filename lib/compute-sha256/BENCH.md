# compute-sha256 benchmarks

The figures of `compute-sha256`'s SHA-256, alone and against the main
alternatives, measured by the protocol of this repository's
[`bench/METHOD.md`](../../bench/METHOD.md): the same inputs, machines,
and builds for every implementation, each figure a median with its 95%
confidence interval, and every result checked against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| sha2 | `sha2` | Rust, with SHA instructions |
| ring | `ring` | C and assembly |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for SHA-256.

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

### Zen 5, ours with `intrinsics`, as by default

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.22 ± 0.2% | 0.31 ± 1.1% | 0.25 ± 0.6% |
| 256 | whole | 1.35 ± 0.1% | 1.67 ± 0.1% | 1.54 ± 0.5% |
| 4096 | whole | 2.13 ± 0.1% | 2.17 ± 0.1% | 2.15 ± 0.5% |
| 65536 | whole | 2.21 ± 0.1% | 2.21 ± 0.1% | 2.21 ± 0.1% |
| 1048576 | whole | 2.21 ± 0.1% | 2.21 ± 0.1% | 2.21 ± 0.1% |
| 16 | by 7 | 0.22 ± 0.2% | 0.30 ± 0.8% | 0.23 ± 2.3% |
| 256 | by 7 | 0.87 ± 0.2% | 1.13 ± 0.4% | 0.89 ± 0.4% |
| 4096 | by 7 | 1.12 ± 0.3% | 1.40 ± 0.4% | 1.12 ± 0.5% |
| 65536 | by 7 | 1.14 ± 0.2% | 1.43 ± 0.5% | 1.14 ± 0.3% |
| 1048576 | by 7 | 1.15 ± 0.3% | 1.43 ± 0.2% | 1.15 ± 0.3% |

Each counterpart's speed relative to ours: whole: sha2 1.12x, ring
1.05x; by 7: sha2 1.29x, ring 1.01x.

### Zen 5, ours without `intrinsics`, without dependencies

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.08 ± 0.1% | 0.31 ± 1.0% | 0.24 ± 1.5% |
| 256 | whole | 0.30 ± 0.7% | 1.68 ± 0.2% | 1.53 ± 0.4% |
| 4096 | whole | 0.38 ± 0.0% | 2.16 ± 0.0% | 2.14 ± 0.1% |
| 65536 | whole | 0.39 ± 0.1% | 2.21 ± 0.1% | 2.20 ± 0.1% |
| 1048576 | whole | 0.39 ± 0.0% | 2.21 ± 0.0% | 2.21 ± 0.0% |
| 16 | by 7 | 0.08 ± 0.1% | 0.31 ± 0.5% | 0.22 ± 1.5% |
| 256 | by 7 | 0.27 ± 0.3% | 1.14 ± 0.1% | 0.86 ± 0.5% |
| 4096 | by 7 | 0.33 ± 0.0% | 1.40 ± 0.1% | 1.12 ± 0.2% |
| 65536 | by 7 | 0.33 ± 0.1% | 1.42 ± 0.5% | 1.14 ± 0.3% |
| 1048576 | by 7 | 0.33 ± 0.1% | 1.43 ± 0.2% | 1.15 ± 0.3% |

Each counterpart's speed relative to ours: whole: sha2 5.20x, ring
4.86x; by 7: sha2 4.16x, ring 3.24x.

### Granite Rapids, ours with `intrinsics`, as by default

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.20 ± 0.3% | 0.26 ± 0.8% | 0.19 ± 0.8% |
| 256 | whole | 1.18 ± 0.2% | 1.41 ± 0.1% | 1.28 ± 0.2% |
| 4096 | whole | 1.80 ± 0.1% | 1.83 ± 0.1% | 1.81 ± 0.1% |
| 65536 | whole | 1.88 ± 0.1% | 1.88 ± 0.2% | 1.88 ± 0.1% |
| 1048576 | whole | 1.88 ± 0.2% | 1.88 ± 0.2% | 1.88 ± 0.1% |
| 16 | by 7 | 0.18 ± 0.4% | 0.25 ± 0.0% | 0.18 ± 0.8% |
| 256 | by 7 | 0.73 ± 0.2% | 0.98 ± 0.3% | 0.75 ± 0.3% |
| 4096 | by 7 | 0.99 ± 0.2% | 1.22 ± 0.1% | 1.00 ± 0.3% |
| 65536 | by 7 | 1.01 ± 4.0% | 1.25 ± 0.2% | 1.03 ± 0.1% |
| 1048576 | by 7 | 1.02 ± 0.2% | 1.25 ± 0.2% | 1.03 ± 0.2% |

Each counterpart's speed relative to ours: whole: sha2 1.09x, ring
1.01x; by 7: sha2 1.29x, ring 1.01x.

### Granite Rapids, ours without `intrinsics`, without dependencies

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.2% | 0.26 ± 0.9% | 0.19 ± 0.7% |
| 256 | whole | 0.23 ± 0.1% | 1.41 ± 0.1% | 1.27 ± 0.8% |
| 4096 | whole | 0.29 ± 0.3% | 1.83 ± 0.1% | 1.81 ± 0.2% |
| 65536 | whole | 0.30 ± 0.3% | 1.88 ± 0.2% | 1.87 ± 0.1% |
| 1048576 | whole | 0.30 ± 0.2% | 1.88 ± 0.1% | 1.88 ± 0.2% |
| 16 | by 7 | 0.06 ± 0.1% | 0.25 ± 0.2% | 0.18 ± 0.6% |
| 256 | by 7 | 0.21 ± 0.1% | 0.98 ± 0.2% | 0.75 ± 0.2% |
| 4096 | by 7 | 0.25 ± 0.2% | 1.22 ± 0.5% | 1.00 ± 0.3% |
| 65536 | by 7 | 0.26 ± 0.3% | 1.25 ± 0.1% | 1.03 ± 0.1% |
| 1048576 | by 7 | 0.26 ± 0.1% | 1.25 ± 0.2% | 1.03 ± 0.3% |

Each counterpart's speed relative to ours: whole: sha2 5.68x, ring
5.20x; by 7: sha2 4.64x, ring 3.67x.

### Zen 1, ours with `intrinsics`, as by default

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.12 ± 0.9% | 0.16 ± 1.0% | 0.10 ± 0.6% |
| 256 | whole | 0.74 ± 0.8% | 0.82 ± 0.1% | 0.70 ± 0.8% |
| 4096 | whole | 1.18 ± 0.2% | 1.19 ± 0.2% | 1.17 ± 0.3% |
| 65536 | whole | 1.23 ± 0.2% | 1.23 ± 0.3% | 1.23 ± 0.2% |
| 1048576 | whole | 1.23 ± 0.2% | 1.23 ± 0.4% | 1.22 ± 0.3% |
| 16 | by 7 | 0.11 ± 0.6% | 0.14 ± 1.0% | 0.09 ± 0.3% |
| 256 | by 7 | 0.36 ± 1.0% | 0.46 ± 0.3% | 0.32 ± 0.5% |
| 4096 | by 7 | 0.45 ± 0.7% | 0.55 ± 0.5% | 0.39 ± 0.8% |
| 65536 | by 7 | 0.46 ± 0.4% | 0.56 ± 0.6% | 0.40 ± 0.7% |
| 1048576 | by 7 | 0.46 ± 0.8% | 0.57 ± 0.4% | 0.39 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.09x, ring
0.96x; by 7: sha2 1.24x, ring 0.86x.

### Zen 1, ours without `intrinsics`, without dependencies

#### SHA-256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.5% | 0.16 ± 0.4% | 0.10 ± 0.9% |
| 256 | whole | 0.12 ± 0.4% | 0.81 ± 0.2% | 0.70 ± 0.2% |
| 4096 | whole | 0.16 ± 0.5% | 1.20 ± 0.2% | 1.18 ± 0.4% |
| 65536 | whole | 0.16 ± 0.5% | 1.23 ± 0.2% | 1.23 ± 0.1% |
| 1048576 | whole | 0.16 ± 0.3% | 1.23 ± 0.1% | 1.23 ± 0.2% |
| 16 | by 7 | 0.03 ± 0.5% | 0.14 ± 0.3% | 0.09 ± 2.5% |
| 256 | by 7 | 0.11 ± 0.4% | 0.46 ± 0.2% | 0.33 ± 0.9% |
| 4096 | by 7 | 0.13 ± 0.4% | 0.55 ± 0.2% | 0.39 ± 0.5% |
| 65536 | by 7 | 0.13 ± 0.2% | 0.56 ± 0.6% | 0.40 ± 0.6% |
| 1048576 | by 7 | 0.13 ± 0.7% | 0.56 ± 0.3% | 0.40 ± 0.7% |

Each counterpart's speed relative to ours: whole: sha2 6.73x, ring
5.92x; by 7: sha2 4.34x, ring 3.02x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| sha256 | aarch64-apple-darwin | 1168 | 960 |
| sha256 | x86_64-unknown-linux-gnu | 1784 | 1030 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-sha256` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-sha256` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
