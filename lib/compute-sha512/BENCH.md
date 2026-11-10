# compute-sha512 benchmarks

The figures of `compute-sha512`'s SHA-512 and SHA-512/256, alone and
against the main alternatives, measured by the protocol of this
repository's [`bench/METHOD.md`](../../bench/METHOD.md): the same
inputs, machines, and builds for every implementation, each figure a
median with its 95% confidence interval, and every result checked
against the others'.

## Counterparts

| Implementation | Crate | Language |
| --- | --- | --- |
| sha2 | `sha2` | Rust, with SHA instructions |
| ring | `ring` | C and assembly |

## Inputs

Buffers of random bytes, of 16 B, 256 B, 4 KiB, 64 KiB, and 1 MiB, fed
whole, by one update, and in pieces of 7 bytes, for SHA-512 and
SHA-512/256.

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

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.8% | 0.11 ± 0.8% | 0.09 ± 0.6% |
| 256 | whole | 0.38 ± 0.5% | 0.63 ± 0.8% | 0.57 ± 0.6% |
| 4096 | whole | 0.59 ± 0.1% | 0.98 ± 0.2% | 0.90 ± 0.1% |
| 65536 | whole | 0.61 ± 0.1% | 1.02 ± 0.1% | 0.94 ± 0.1% |
| 1048576 | whole | 0.61 ± 0.1% | 1.02 ± 0.1% | 0.94 ± 0.1% |
| 16 | by 7 | 0.06 ± 0.5% | 0.11 ± 1.1% | 0.09 ± 0.6% |
| 256 | by 7 | 0.34 ± 0.1% | 0.53 ± 1.3% | 0.46 ± 0.4% |
| 4096 | by 7 | 0.49 ± 0.5% | 0.76 ± 0.2% | 0.62 ± 1.6% |
| 65536 | by 7 | 0.50 ± 0.2% | 0.78 ± 0.2% | 0.64 ± 1.2% |
| 1048576 | by 7 | 0.50 ± 0.1% | 0.78 ± 0.8% | 0.65 ± 1.1% |

Each counterpart's speed relative to ours: whole: sha2 1.67x, ring
1.51x; by 7: sha2 1.59x, ring 1.32x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.3% | 0.11 ± 0.6% | 0.09 ± 0.1% |
| 256 | whole | 0.38 ± 0.2% | 0.63 ± 1.5% | 0.56 ± 0.5% |
| 4096 | whole | 0.59 ± 0.1% | 0.98 ± 0.1% | 0.90 ± 0.2% |
| 65536 | whole | 0.61 ± 0.1% | 1.02 ± 0.0% | 0.94 ± 0.1% |
| 1048576 | whole | 0.61 ± 0.0% | 1.02 ± 0.0% | 0.94 ± 0.0% |
| 16 | by 7 | 0.06 ± 0.4% | 0.11 ± 0.8% | 0.09 ± 0.7% |
| 256 | by 7 | 0.34 ± 0.1% | 0.52 ± 0.9% | 0.46 ± 0.4% |
| 4096 | by 7 | 0.48 ± 0.3% | 0.74 ± 0.1% | 0.62 ± 0.9% |
| 65536 | by 7 | 0.49 ± 0.1% | 0.76 ± 0.1% | 0.64 ± 0.6% |
| 1048576 | by 7 | 0.49 ± 0.2% | 0.76 ± 1.1% | 0.65 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.67x, ring
1.50x; by 7: sha2 1.58x, ring 1.34x.

### Zen 5, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.1% | 0.11 ± 0.8% | 0.09 ± 0.8% |
| 256 | whole | 0.38 ± 0.1% | 0.61 ± 0.5% | 0.56 ± 0.4% |
| 4096 | whole | 0.59 ± 0.1% | 0.92 ± 0.1% | 0.90 ± 0.1% |
| 65536 | whole | 0.61 ± 0.1% | 0.95 ± 0.1% | 0.94 ± 0.2% |
| 1048576 | whole | 0.61 ± 0.1% | 0.95 ± 0.0% | 0.94 ± 0.1% |
| 16 | by 7 | 0.06 ± 0.6% | 0.11 ± 1.3% | 0.09 ± 0.5% |
| 256 | by 7 | 0.34 ± 0.3% | 0.55 ± 0.5% | 0.46 ± 0.8% |
| 4096 | by 7 | 0.48 ± 0.5% | 0.78 ± 0.2% | 0.62 ± 0.5% |
| 65536 | by 7 | 0.49 ± 0.5% | 0.80 ± 0.2% | 0.64 ± 0.2% |
| 1048576 | by 7 | 0.49 ± 0.4% | 0.78 ± 1.2% | 0.64 ± 1.0% |

Each counterpart's speed relative to ours: whole: sha2 1.60x, ring
1.51x; by 7: sha2 1.63x, ring 1.34x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.06 ± 0.5% | 0.11 ± 0.6% | 0.09 ± 0.1% |
| 256 | whole | 0.38 ± 0.2% | 0.61 ± 0.7% | 0.56 ± 0.6% |
| 4096 | whole | 0.59 ± 0.1% | 0.92 ± 0.2% | 0.90 ± 0.2% |
| 65536 | whole | 0.61 ± 0.1% | 0.95 ± 0.1% | 0.93 ± 0.1% |
| 1048576 | whole | 0.61 ± 0.1% | 0.95 ± 0.1% | 0.94 ± 0.2% |
| 16 | by 7 | 0.06 ± 0.3% | 0.11 ± 1.0% | 0.09 ± 0.6% |
| 256 | by 7 | 0.34 ± 0.4% | 0.55 ± 0.6% | 0.46 ± 0.7% |
| 4096 | by 7 | 0.48 ± 0.3% | 0.79 ± 0.3% | 0.62 ± 1.0% |
| 65536 | by 7 | 0.49 ± 0.4% | 0.81 ± 0.3% | 0.64 ± 0.4% |
| 1048576 | by 7 | 0.49 ± 0.2% | 0.81 ± 0.9% | 0.64 ± 0.4% |

Each counterpart's speed relative to ours: whole: sha2 1.59x, ring
1.51x; by 7: sha2 1.66x, ring 1.34x.

### Granite Rapids, ours with `intrinsics`, as by default

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.3% | 0.07 ± 0.9% | 0.07 ± 0.2% |
| 256 | whole | 0.32 ± 0.8% | 0.46 ± 0.4% | 0.45 ± 0.2% |
| 4096 | whole | 0.48 ± 0.1% | 0.71 ± 0.3% | 0.72 ± 0.3% |
| 65536 | whole | 0.50 ± 0.2% | 0.74 ± 0.4% | 0.74 ± 0.2% |
| 1048576 | whole | 0.50 ± 0.1% | 0.73 ± 0.1% | 0.75 ± 0.2% |
| 16 | by 7 | 0.05 ± 0.2% | 0.08 ± 0.8% | 0.07 ± 0.6% |
| 256 | by 7 | 0.27 ± 0.6% | 0.36 ± 0.4% | 0.37 ± 0.9% |
| 4096 | by 7 | 0.39 ± 0.2% | 0.51 ± 0.9% | 0.52 ± 0.3% |
| 65536 | by 7 | 0.40 ± 0.1% | 0.51 ± 0.8% | 0.54 ± 0.1% |
| 1048576 | by 7 | 0.40 ± 0.5% | 0.52 ± 0.8% | 0.54 ± 0.2% |

Each counterpart's speed relative to ours: whole: sha2 1.47x, ring
1.46x; by 7: sha2 1.34x, ring 1.36x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.3% | 0.08 ± 0.7% | 0.07 ± 0.2% |
| 256 | whole | 0.32 ± 0.4% | 0.46 ± 0.4% | 0.46 ± 0.1% |
| 4096 | whole | 0.48 ± 0.1% | 0.71 ± 0.2% | 0.72 ± 0.1% |
| 65536 | whole | 0.50 ± 0.3% | 0.74 ± 0.2% | 0.75 ± 0.1% |
| 1048576 | whole | 0.50 ± 0.1% | 0.73 ± 0.3% | 0.75 ± 0.0% |
| 16 | by 7 | 0.05 ± 0.2% | 0.08 ± 0.9% | 0.07 ± 0.3% |
| 256 | by 7 | 0.27 ± 0.6% | 0.36 ± 0.7% | 0.37 ± 0.4% |
| 4096 | by 7 | 0.39 ± 0.2% | 0.51 ± 0.7% | 0.53 ± 0.2% |
| 65536 | by 7 | 0.40 ± 0.3% | 0.52 ± 0.4% | 0.55 ± 0.3% |
| 1048576 | by 7 | 0.40 ± 0.1% | 0.51 ± 0.9% | 0.55 ± 0.1% |

Each counterpart's speed relative to ours: whole: sha2 1.46x, ring
1.47x; by 7: sha2 1.34x, ring 1.38x.

### Granite Rapids, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.2% | 0.08 ± 1.0% | 0.07 ± 0.2% |
| 256 | whole | 0.31 ± 0.2% | 0.46 ± 0.4% | 0.45 ± 0.1% |
| 4096 | whole | 0.46 ± 0.2% | 0.71 ± 0.3% | 0.72 ± 0.1% |
| 65536 | whole | 0.48 ± 0.2% | 0.73 ± 0.2% | 0.74 ± 0.1% |
| 1048576 | whole | 0.48 ± 0.1% | 0.73 ± 0.2% | 0.75 ± 0.1% |
| 16 | by 7 | 0.05 ± 0.3% | 0.07 ± 1.1% | 0.07 ± 1.0% |
| 256 | by 7 | 0.27 ± 0.7% | 0.36 ± 0.5% | 0.37 ± 0.3% |
| 4096 | by 7 | 0.38 ± 0.1% | 0.52 ± 0.5% | 0.53 ± 0.1% |
| 65536 | by 7 | 0.39 ± 0.2% | 0.53 ± 0.4% | 0.55 ± 0.3% |
| 1048576 | by 7 | 0.39 ± 0.1% | 0.53 ± 0.6% | 0.55 ± 0.1% |

Each counterpart's speed relative to ours: whole: sha2 1.51x, ring
1.51x; by 7: sha2 1.38x, ring 1.40x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.05 ± 0.2% | 0.08 ± 1.4% | 0.07 ± 0.2% |
| 256 | whole | 0.31 ± 0.3% | 0.46 ± 0.6% | 0.45 ± 0.5% |
| 4096 | whole | 0.47 ± 0.1% | 0.71 ± 0.2% | 0.72 ± 0.1% |
| 65536 | whole | 0.48 ± 0.2% | 0.74 ± 0.2% | 0.74 ± 0.1% |
| 1048576 | whole | 0.48 ± 0.2% | 0.73 ± 0.1% | 0.75 ± 0.2% |
| 16 | by 7 | 0.05 ± 0.4% | 0.07 ± 1.0% | 0.07 ± 0.5% |
| 256 | by 7 | 0.27 ± 0.3% | 0.36 ± 0.8% | 0.37 ± 0.1% |
| 4096 | by 7 | 0.38 ± 0.1% | 0.52 ± 0.5% | 0.53 ± 0.5% |
| 65536 | by 7 | 0.39 ± 0.2% | 0.53 ± 0.8% | 0.55 ± 0.1% |
| 1048576 | by 7 | 0.39 ± 0.2% | 0.53 ± 0.5% | 0.55 ± 0.1% |

Each counterpart's speed relative to ours: whole: sha2 1.51x, ring
1.51x; by 7: sha2 1.38x, ring 1.40x.

### Zen 1, ours with `intrinsics`, as by default

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.7% | 0.03 ± 1.0% | 0.03 ± 0.4% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.9% | 0.21 ± 0.9% |
| 4096 | whole | 0.23 ± 1.0% | 0.26 ± 0.5% | 0.34 ± 0.7% |
| 65536 | whole | 0.24 ± 0.7% | 0.27 ± 0.2% | 0.35 ± 0.4% |
| 1048576 | whole | 0.24 ± 0.7% | 0.27 ± 0.3% | 0.36 ± 0.4% |
| 16 | by 7 | 0.02 ± 0.3% | 0.03 ± 0.5% | 0.03 ± 0.9% |
| 256 | by 7 | 0.12 ± 0.8% | 0.15 ± 0.6% | 0.16 ± 0.7% |
| 4096 | by 7 | 0.18 ± 0.4% | 0.21 ± 0.6% | 0.22 ± 0.6% |
| 65536 | by 7 | 0.18 ± 0.3% | 0.22 ± 0.8% | 0.23 ± 0.4% |
| 1048576 | by 7 | 0.18 ± 0.4% | 0.22 ± 0.7% | 0.23 ± 0.8% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.42x; by 7: sha2 1.20x, ring 1.27x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.02 ± 0.5% | 0.03 ± 0.9% | 0.03 ± 0.7% |
| 256 | whole | 0.15 ± 0.8% | 0.17 ± 0.5% | 0.21 ± 0.6% |
| 4096 | whole | 0.23 ± 0.9% | 0.26 ± 0.5% | 0.34 ± 0.7% |
| 65536 | whole | 0.24 ± 0.3% | 0.27 ± 0.2% | 0.36 ± 0.6% |
| 1048576 | whole | 0.24 ± 1.0% | 0.27 ± 0.7% | 0.35 ± 0.8% |
| 16 | by 7 | 0.02 ± 0.9% | 0.03 ± 0.5% | 0.03 ± 1.0% |
| 256 | by 7 | 0.13 ± 0.5% | 0.16 ± 0.9% | 0.16 ± 0.6% |
| 4096 | by 7 | 0.18 ± 0.3% | 0.22 ± 0.4% | 0.21 ± 0.4% |
| 65536 | by 7 | 0.18 ± 0.2% | 0.22 ± 0.4% | 0.22 ± 0.3% |
| 1048576 | by 7 | 0.19 ± 0.8% | 0.22 ± 0.5% | 0.22 ± 0.9% |

Each counterpart's speed relative to ours: whole: sha2 1.15x, ring
1.43x; by 7: sha2 1.22x, ring 1.22x.

### Zen 1, ours without `intrinsics`, without dependencies

#### SHA-512

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.02 ± 0.3% | 0.03 ± 0.5% | 0.03 ± 0.8% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.4% | 0.22 ± 0.8% |
| 4096 | whole | 0.24 ± 0.4% | 0.26 ± 0.4% | 0.34 ± 0.6% |
| 65536 | whole | 0.24 ± 0.7% | 0.27 ± 0.5% | 0.36 ± 0.8% |
| 1048576 | whole | 0.24 ± 0.4% | 0.27 ± 0.5% | 0.35 ± 0.4% |
| 16 | by 7 | 0.02 ± 0.7% | 0.03 ± 0.7% | 0.03 ± 0.7% |
| 256 | by 7 | 0.13 ± 0.5% | 0.15 ± 0.2% | 0.16 ± 0.7% |
| 4096 | by 7 | 0.18 ± 1.0% | 0.20 ± 0.7% | 0.22 ± 0.8% |
| 65536 | by 7 | 0.19 ± 0.5% | 0.21 ± 0.4% | 0.23 ± 1.0% |
| 1048576 | by 7 | 0.19 ± 0.3% | 0.21 ± 0.3% | 0.23 ± 0.6% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.43x; by 7: sha2 1.14x, ring 1.25x.

#### SHA-512/256

| Bytes | Fed | ours | sha2 | ring |
| ---: | --- | ---: | ---: | ---: |
| 16 | whole | 0.03 ± 0.4% | 0.03 ± 0.3% | 0.03 ± 0.8% |
| 256 | whole | 0.15 ± 0.4% | 0.17 ± 0.6% | 0.22 ± 0.6% |
| 4096 | whole | 0.24 ± 0.8% | 0.26 ± 0.2% | 0.34 ± 0.8% |
| 65536 | whole | 0.24 ± 0.3% | 0.27 ± 0.4% | 0.35 ± 0.4% |
| 1048576 | whole | 0.24 ± 0.9% | 0.27 ± 0.9% | 0.35 ± 0.6% |
| 16 | by 7 | 0.02 ± 1.0% | 0.03 ± 0.4% | 0.03 ± 0.4% |
| 256 | by 7 | 0.13 ± 0.6% | 0.15 ± 0.3% | 0.16 ± 0.5% |
| 4096 | by 7 | 0.18 ± 0.2% | 0.21 ± 0.4% | 0.21 ± 0.3% |
| 65536 | by 7 | 0.19 ± 0.3% | 0.21 ± 0.1% | 0.21 ± 0.2% |
| 1048576 | by 7 | 0.19 ± 0.6% | 0.21 ± 0.5% | 0.21 ± 0.4% |

Each counterpart's speed relative to ours: whole: sha2 1.14x, ring
1.43x; by 7: sha2 1.16x, ring 1.21x.

## Code size

The bytes of `.text` each algorithm adds to a minimal `#![no_std]`
program that computes a checksum or hash with it, built portable with
fat LTO; read-only data, such as tables, is not counted.

<!-- filer-bench: size -->

| Algorithm | Target | `opt-level` 3 | `opt-level` "s" |
| --- | --- | ---: | ---: |
| sha512 | aarch64-apple-darwin | 1404 | 1000 |
| sha512 | x86_64-unknown-linux-gnu | 2214 | 1272 |
| sha512-256 | aarch64-apple-darwin | 1412 | 992 |
| sha512-256 | x86_64-unknown-linux-gnu | 2150 | 1254 |

On aarch64-apple-darwin, measured 2026-11-07, at `86d2427`,
`compute-sha512` 0.0.4, by rustc 1.99.1 (58cf8f9c3 2026-10-13).

On x86_64-unknown-linux-gnu, measured 2026-11-10, at `037ffbb`,
`compute-sha512` 0.0.4, by rustc 1.97.0 (2d8144b78 2026-07-07).

<!-- filer-bench: end -->
