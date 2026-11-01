#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the NTFS fixtures with real tools: `mkntfs` formats each volume,
# and the ntfs-3g FUSE driver fills it with the tree `tests/fixtures.rs`
# expects. Each image is stored as `<name>.sparse`: the magic
# `FILER-SPARSE-2\n`, the image size (u64, little-endian), then runs of
# identical nonzero 512-byte sectors, each as its first sector (u32), its
# count (u32), and the sector's bytes. `spec.sparse` holds the
# specification's tree instead.
#
# Usage: tests/fixtures/make.sh [output-dir [fixture...]], on Linux with
# ntfs-3g, python3, and `sudo` for mounting; without fixture names, makes
# them all. Times are UTC, so output is stable but
# for the times `mkntfs` and ntfs-3g record themselves.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
[ $# -gt 0 ] && shift
only="$*"
work=$(mktemp -d)
mnt="$work/mnt"
mkdir "$mnt"
cleanup() {
    mountpoint -q "$mnt" && sudo umount "$mnt"
    rm -rf "$work"
}
trap cleanup EXIT
export TZ=UTC LC_ALL=C.UTF-8

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

# Formats `$1` as NTFS with sectors of `$2` bytes and clusters of `$3`.
format() {
    truncate -s 32M "$1"
    mkntfs -Q -F -q -L FILER -s "$2" -c "$3" "$1" 2>/dev/null
}

fill() {
    python3 -I - "$mnt" <<'PY'
import os, sys
root = sys.argv[1]
MTIME = 1_704_164_646  # 2024-01-02T03:04:06Z

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def put(path, size, seed):
    full = os.path.join(root, path)
    with open(full, "wb") as f:
        f.write(pattern(size, seed))
    os.utime(full, (MTIME, MTIME))

put("README.TXT", 9, 1)
put("notes.txt", 10, 2)
put("A long file name with spaces.txt", 100, 3)
put("Ünïcödé ✓.txt", 7, 4)
put("emoji 😀.bin", 5, 5)
put("empty", 0, 6)
put("big.bin", 100_000, 7)
os.makedirs(os.path.join(root, "Sub", "Deeper"))
put("Sub/nested.bin", 5000, 8)
put("Sub/Deeper/leaf.txt", 5, 9)
os.mkdir(os.path.join(root, "many"))
for i in range(150):
    put(f"many/f{i:03}", i, 100 + i)
# Two files grown in turns, so that both fragment.
with open(os.path.join(root, "frag.a"), "wb") as a, \
     open(os.path.join(root, "frag.b"), "wb") as b:
    for i in range(12):
        a.write(pattern(4096, 20 + i)); a.flush(); os.fsync(a.fileno())
        b.write(pattern(4096, 40 + i)); b.flush(); os.fsync(b.fileno())
for name in ("frag.a", "frag.b"):
    os.utime(os.path.join(root, name), (MTIME, MTIME))
# A sparse file: a megabyte of zeros, then three bytes.
with open(os.path.join(root, "sparse.bin"), "wb") as f:
    f.seek(1_000_000)
    f.write(b"end")
os.utime(os.path.join(root, "sparse.bin"), (MTIME, MTIME))
# A hard link.
put("small", 4, 10)
os.link(os.path.join(root, "small"), os.path.join(root, "Sub", "hardlink"))
# Names differing only in case, in the POSIX namespace.
put("Case.txt", 3, 11)
put("case.txt", 4, 12)
# A long name with a DOS (8.3) name, which listings skip.
put("Long name for DOS.txt", 6, 13)
os.setxattr(os.path.join(root, "Long name for DOS.txt"),
            "system.ntfs_dos_name", b"LONGNA~1.TXT")
# A named data stream, which is not exposed yet.
os.setxattr(os.path.join(root, "README.TXT"), "user.extra", b"stream data")
# Deleted entries.
put("gone.txt", 512, 14)
os.mkdir(os.path.join(root, "GoneDir"))
os.remove(os.path.join(root, "gone.txt"))
os.rmdir(os.path.join(root, "GoneDir"))
PY
}

# Compressed files, in a directory flagged compressed: compressible,
# incompressible (stored as is), sparse, exactly one unit, and mixed.
compressed() {
    python3 -I - "$mnt" <<'PY'
import os, sys
root = sys.argv[1]
MTIME = 1_704_164_646

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def noise(size, seed):
    # Xorshift32, as `tests/common` has it.
    out, x = bytearray(), seed
    for _ in range(size):
        x ^= (x << 13) & 0xFFFFFFFF
        x ^= x >> 17
        x ^= (x << 5) & 0xFFFFFFFF
        out.append(x & 0xFF)
    return bytes(out)

directory = os.path.join(root, "Compressed")
os.mkdir(directory)
flags = int.from_bytes(os.getxattr(directory, "system.ntfs_attrib_be"), "big")
os.setxattr(directory, "system.ntfs_attrib_be", (flags | 0x800).to_bytes(4, "big"))

def put(name, data):
    full = os.path.join(directory, name)
    with open(full, "wb") as f:
        f.write(data)
    os.utime(full, (MTIME, MTIME))

put("text.txt", pattern(200_000, 1))
put("noise.bin", noise(100_000, 7))
put("exact.bin", pattern(65536, 3))
put("mixed.bin", noise(30_000, 9) + bytes(70_000) + pattern(50_000, 4))
put("small.txt", pattern(5, 2))
with open(os.path.join(directory, "sparse.bin"), "wb") as f:
    f.seek(200_000)
    f.write(b"end")
os.utime(os.path.join(directory, "sparse.bin"), (MTIME, MTIME))
PY
}

spec() {
    python3 -I - "$mnt" <<'PY'
import os, sys
root = sys.argv[1]
os.makedirs(os.path.join(root, "a", "b"))
open(os.path.join(root, "a", "b", "c.txt"), "wb").write(b"contents")
open(os.path.join(root, "a", "z.txt"), "wb").close()
os.mkdir(os.path.join(root, "a", "empty"))
os.mkdir(os.path.join(root, "many"))
for i in range(600):
    open(os.path.join(root, "many", f"f{i:03}"), "wb").close()
PY
}

# Makes fixture `$1` with sectors of `$2` bytes and clusters of `$3`,
# filled by `$4`.
make() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    format "$img" "$2" "$3"
    sudo mount -t ntfs-3g -o "compression,uid=$(id -u),gid=$(id -g)" "$img" "$mnt"
    $4
    sudo umount "$mnt"
    sparse "$img" "$out/$1.sparse"
}

make ntfs-4k 512 4096 fill
make ntfs-512 512 512 fill
make ntfs-64k 512 65536 fill
make ntfs-4kn 4096 4096 fill
make ntfs-compressed 512 4096 compressed
make spec 512 4096 spec
