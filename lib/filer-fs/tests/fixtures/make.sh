#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the partitioned disk images `tests/partition.rs` reads, with
# `sfdisk`, and records what `sfdisk -d` shows of each as `<name>.sfdisk`:
#
# - `mbr`: an MBR table of two primary partitions and an extended one,
#   holding two logical partitions;
# - `gpt`: a GPT of three named partitions, of 512-byte sectors;
# - `gpt-4k`: a GPT of two partitions, of 4096-byte sectors, made on a
#   loop device of that sector size;
# - `gpt-backup`: `gpt` with its primary header wiped, so that the table
#   is read by its backup header, at the end of the disk.
#
# Each image is stored as `<name>.sparse`: the magic `FILER-SPARSE-2\n`,
# the image size (u64, little-endian), then runs of identical nonzero
# 512-byte sectors, each as its first sector (u32), its count (u32), and
# the sector's bytes.
#
# Usage: tests/fixtures/make.sh [output-dir], on Linux with util-linux
# (`sfdisk`, `losetup`), python3, and `sudo` for the loop device.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
work=$(mktemp -d -p /var/tmp)
loop=
cleanup() {
    if [ -n "$loop" ]; then sudo losetup -d "$loop"; fi
    rm -rf "$work"
}
trap cleanup EXIT

# Stores image `$1` as sparse file `$2`.
sparse() {
    python3 -I - "$1" "$2" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
with open(sys.argv[2], "wb") as out:
    out.write(b"FILER-SPARSE-2\n" + struct.pack("<Q", len(data)))
    sectors = len(data) // 512
    i = 0
    while i < sectors:
        sector = data[i * 512:(i + 1) * 512]
        n = 1
        while i + n < sectors and data[(i + n) * 512:(i + n + 1) * 512] == sector:
            n += 1
        if any(sector):
            out.write(struct.pack("<II", i, n) + sector)
        i += n
PY
}

# Partitions image `$1` by the `sfdisk` script on standard input, and
# records it as `$2`.
make() {
    sfdisk -q "$1" > /dev/null
    sfdisk -d "$1" | sed "s|$1|$2|" > "$out/$2.sfdisk"
    sparse "$1" "$out/$2.sparse"
}

truncate -s 64M "$work/mbr.img"
make "$work/mbr.img" mbr <<'EOF'
label: dos
label-id: 0x12345678
start=2048, size=16MiB, type=c, bootable
size=16MiB, type=83
type=5
size=8MiB, type=7
type=83
EOF

truncate -s 64M "$work/gpt.img"
make "$work/gpt.img" gpt <<'EOF'
label: gpt
label-id: 01234567-89AB-CDEF-0123-456789ABCDEF
start=2048, size=16MiB, type=uefi, uuid=11111111-2222-3333-4444-555555555555, name="EFI system"
size=16MiB, type=linux, uuid=66666666-7777-8888-9999-AAAAAAAAAAAA, name="Linux data"
type=EBD0A0A2-B9E5-4433-87C0-68B6B72699C7, uuid=BBBBBBBB-CCCC-DDDD-EEEE-FFFFFFFFFFFF, name="Basic data ✓"
EOF

# The same table, its primary header wiped: sector 1.
cp "$work/gpt.img" "$work/gpt-backup.img"
dd if=/dev/zero of="$work/gpt-backup.img" bs=512 seek=1 count=1 conv=notrunc status=none
sed 's|^device: gpt$|device: gpt-backup|; s|^gpt\([0-9]\)|gpt-backup\1|' \
    "$out/gpt.sfdisk" > "$out/gpt-backup.sfdisk"
sparse "$work/gpt-backup.img" "$out/gpt-backup.sparse"

truncate -s 64M "$work/gpt-4k.img"
loop=$(sudo losetup -f --show -b 4096 "$work/gpt-4k.img")
sudo sfdisk -q "$loop" > /dev/null <<'EOF'
label: gpt
label-id: FEDCBA98-7654-3210-FEDC-BA9876543210
start=256, size=4MiB, type=uefi, uuid=12121212-3434-5656-7878-909090909090, name="EFI"
type=linux, uuid=ABABABAB-CDCD-EFEF-ABAB-CDCDCDCDCDCD, name="root"
EOF
sudo sfdisk -d "$loop" | sed "s|$loop|gpt-4k|" > "$out/gpt-4k.sfdisk"
sudo losetup -d "$loop"
loop=
sparse "$work/gpt-4k.img" "$out/gpt-4k.sparse"
