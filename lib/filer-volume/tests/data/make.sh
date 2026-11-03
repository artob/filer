#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes `hello.img`, the smallest FAT volume `mkfs.fat` makes (64 KiB,
# FAT12), holding `hello/world.txt`, for the crate's doc examples. The
# tests read the other file systems' fixtures from their own crates.
#
# Usage: tests/data/make.sh [output-dir], on Linux with dosfstools and
# mtools.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
image="$out/hello.img"
rm -f "$image"
mkfs.fat --invariant -i 1D2E3F40 -C "$image" 64 >/dev/null
printf 'Hello, world!\n' > "$work/world.txt"
touch -d @1700000000 "$work/world.txt"
export MTOOLS_SKIP_CHECK=1
mmd -i "$image" ::/hello
mcopy -m -i "$image" "$work/world.txt" ::/hello/world.txt
