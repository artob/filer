#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the FAT fixtures with real tools: `mkfs.fat` (dosfstools) formats
# each volume, and `mtools` fills it with the tree `tests/fixtures.rs`
# expects. Each image is stored sparsely, as `<name>.sparse`: the magic
# `FILER-SPARSE-1\n`, the image size (u64, little-endian), then each
# nonzero 512-byte sector as its index (u32, little-endian) and its bytes.
#
# Usage: tests/fixtures/make.sh [output-dir], on Linux with dosfstools,
# mtools, and python3. Times are UTC and IDs fixed, so output is stable.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
export TZ=UTC LC_ALL=C.UTF-8 MTOOLS_SKIP_CHECK=1

# Writes `size` bytes of the pattern `tests/fixtures.rs` checks to `file`.
pattern() {
    python3 -c 'import sys
size, seed = int(sys.argv[1]), int(sys.argv[2])
sys.stdout.buffer.write(bytes((i * 7 + seed) % 251 for i in range(size)))' "$1" "$2" > "$3"
}

sparse() {
    python3 -c 'import struct, sys
data = open(sys.argv[1], "rb").read()
with open(sys.argv[2], "wb") as out:
    out.write(b"FILER-SPARSE-1\n" + struct.pack("<Q", len(data)))
    for i in range(0, len(data), 512):
        sector = data[i:i + 512]
        if any(sector):
            out.write(struct.pack("<I", i // 512) + sector)' "$1" "$2"
}

make() {
    name=$1 bits=$2 kib=$3 label=$4
    img="$work/$name.img"
    mkfs.fat -C -F "$bits" -s 1 -n "$label" -i 12345678 --invariant "$img" "$kib" > /dev/null
    files="$work/files"
    rm -rf "$files" && mkdir -p "$files"
    put() { # put SIZE SEED PATH
        pattern "$1" "$2" "$files/f"
        touch -d '2024-01-02 03:04:06' "$files/f"
        mcopy -m -i "$img" "$files/f" "::/$3"
    }
    put 9 1 'README.TXT'
    put 10 2 'notes.txt'
    put 100 3 'A long file name with spaces.txt'
    put 7 4 'Ünïcödé ✓.txt'
    put 0 5 'empty'
    put 20000 6 'big.bin'
    mmd -i "$img" '::/Sub' '::/Sub/Deeper' '::/many'
    put 5000 7 'Sub/nested.bin'
    put 5 8 'Sub/Deeper/leaf.txt'
    i=0
    while [ $i -lt 100 ]; do
        put $((i * 3)) $((100 + i)) "many/f$(printf %03d $i)"
        i=$((i + 1))
    done
    # Deleted entries, and a file fragmented around a freed hole (except
    # on FAT32, where mtools follows the FSInfo next-free hint past it).
    put 512 9 'gone.txt'
    put 3000 10 'spacer.bin'
    mmd -i "$img" '::/GoneDir'
    mdel -i "$img" '::/gone.txt'
    mrd -i "$img" '::/GoneDir'
    put 4000 11 'frag.bin'
    fsck.fat -n "$img" > /dev/null
    sparse "$img" "$out/$name.sparse"
}

make fat12 12 1440 FLOPPY12
make fat16 16 16384 VOLUME16
make fat32 32 40960 VOLUME32
