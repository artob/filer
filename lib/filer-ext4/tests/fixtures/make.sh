#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the ext2/3/4 fixtures with real tools: `mke2fs` formats each
# volume, and the Linux kernel fills it with the tree `tests/fixtures.rs`
# expects, through a loop mount. Volumes of 64 KiB blocks, which x86 Linux
# cannot mount, are filled by `mke2fs -d` from the same tree instead.
# Each image is stored as `<name>.sparse`: the magic `FILER-SPARSE-2\n`,
# the image size (u64, little-endian), then runs of identical nonzero
# 512-byte sectors, each as its first sector (u32), its count (u32), and
# the sector's bytes. `spec.sparse` holds the specification's tree
# instead.
#
# Usage: tests/fixtures/make.sh [output-dir [fixture...]], on Linux with
# e2fsprogs, python3, and `sudo` for mounting and making devices; without
# fixture names, makes them all. Times are UTC, so output is stable but
# for the times `mke2fs` and the kernel record themselves.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
[ $# -gt 0 ] && shift
only="$*"
work=$(mktemp -d)
mnt="$work/mnt"
mkdir "$mnt"
cleanup() {
    mountpoint -q "$mnt" && sudo umount "$mnt"
    sudo rm -rf "$work"
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

# Writes the tree `tests/common` expects into directory `$1`, as root.
fill() {
    sudo python3 -I - "$1" <<'PY'
import os, socket, sys
root = sys.argv[1]
# 2024-01-02T03:04:06.123456789Z, in nanoseconds.
MTIME = 1_704_164_646_123_456_789

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def touch(path):
    os.utime(os.path.join(root, path), ns=(MTIME, MTIME), follow_symlinks=False)

def put(path, data):
    with open(os.path.join(root, path), "wb") as f:
        f.write(data)
    touch(path)

put("README.TXT", pattern(9, 1))
put("notes.txt", pattern(10, 2))
put("A long file name with spaces.txt", pattern(100, 3))
put("Ünïcödé ✓.txt", pattern(7, 4))
put("emoji 😀.bin", pattern(5, 5))
put("empty", b"")
put("big.bin", pattern(100_000, 7))
os.makedirs(os.path.join(root, "Sub", "Deeper"))
put("Sub/nested.bin", pattern(5000, 8))
put("Sub/Deeper/leaf.txt", pattern(5, 9))
os.mkdir(os.path.join(root, "many"))
for i in range(150):
    put(f"many/f{i:03}", pattern(i, 100 + i))
# Two files grown in turns, so that both fragment.
with open(os.path.join(root, "frag.a"), "wb") as a, \
     open(os.path.join(root, "frag.b"), "wb") as b:
    for i in range(12):
        a.write(pattern(4096, 20 + i)); a.flush(); os.fsync(a.fileno())
        b.write(pattern(4096, 40 + i)); b.flush(); os.fsync(b.fileno())
touch("frag.a")
touch("frag.b")
# Sparse files: a megabyte of zeros, then three bytes; and data far
# enough out to need double and, with 1 KiB blocks, triple indirect
# blocks.
with open(os.path.join(root, "sparse.bin"), "wb") as f:
    f.seek(1_000_000)
    f.write(b"end")
touch("sparse.bin")
with open(os.path.join(root, "deep.bin"), "wb") as f:
    f.write(b"start")
    f.seek(300_000)
    f.write(b"middle")
    f.seek(70_000_000)
    f.write(b"end")
touch("deep.bin")
# A hard link, names differing only in case, the longest name, and a
# name that is not UTF-8.
put("small", pattern(4, 10))
os.link(os.path.join(root, "small"), os.path.join(root, "Sub", "hardlink"))
put("Case.txt", pattern(3, 11))
put("case.txt", pattern(4, 12))
put("n" * 255, pattern(6, 13))
with open(os.path.join(root.encode(), b"caf\xe9.txt"), "wb") as f:
    f.write(pattern(3, 14))
os.utime(os.path.join(root.encode(), b"caf\xe9.txt"), ns=(MTIME, MTIME))
# Symlinks: fast (in the inode) and slow (in a block).
for link, target in [
    ("link", "README.TXT"),
    ("deeplink", "Sub/Deeper/leaf.txt"),
    ("dirlink", "Sub"),
    ("abs", "/notes.txt"),
    ("dangling", "nope"),
    ("long", "Sub/" + "x" * 100),
]:
    os.symlink(target, os.path.join(root, link))
    touch(link)
# Special files.
os.mkdir(os.path.join(root, "special"))
os.mkfifo(os.path.join(root, "special", "fifo"))
sock = socket.socket(socket.AF_UNIX)
sock.bind(os.path.join(root, "special", "sock"))
sock.close()
os.mknod(os.path.join(root, "special", "chr"), 0o20644, os.makedev(1, 3))
os.mknod(os.path.join(root, "special", "blk"), 0o60644, os.makedev(7, 0))
# Deleted entries.
put("gone.txt", pattern(512, 15))
os.mkdir(os.path.join(root, "GoneDir"))
os.remove(os.path.join(root, "gone.txt"))
os.rmdir(os.path.join(root, "GoneDir"))
PY
}

# Writes the specification's tree into directory `$1`, as root.
spec() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
os.makedirs(os.path.join(root, "a", "b"))
open(os.path.join(root, "a", "b", "c.txt"), "wb").write(b"contents")
open(os.path.join(root, "a", "z.txt"), "wb").close()
os.mkdir(os.path.join(root, "a", "empty"))
os.setxattr(os.path.join(root, "a", "b", "c.txt"), "user.comment", b"a comment")
os.setxattr(os.path.join(root, "a", "b", "c.txt"), "user.empty", b"")
os.setxattr(os.path.join(root, "a"), "user.tag", b"dir")
for link, target in [
    ("a/link", "b/c.txt"),
    ("a/up", "../a/b/c.txt"),
    ("a/abs", "/a/b/c.txt"),
    ("a/escape", "../../c.txt"),
    ("a/loop", "loop"),
    ("a/dangling", "nope"),
]:
    os.symlink(target, os.path.join(root, link))
os.mkdir(os.path.join(root, "many"))
for i in range(600):
    open(os.path.join(root, "many", f"f{i:03}"), "wb").close()
os.mkdir(os.path.join(root, "special"))
os.mkfifo(os.path.join(root, "special", "fifo"))
os.mkdir(os.path.join(root, "foreign"))
open(os.path.join(root.encode(), b"foreign", b"caf\xe9"), "wb").write(b"foreign")
PY
}

# Makes fixture `$1`, formatted by `mke2fs` with options `$3`, of size
# `$2`, filled by `$4` through the kernel.
make() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    truncate -s "$2" "$img"
    # shellcheck disable=SC2086
    mke2fs -q -F -L FILER $3 "$img"
    sudo mount -o loop "$img" "$mnt"
    $4 "$mnt"
    sudo umount "$mnt"
    sparse "$img" "$out/$1.sparse"
}

# Makes fixture `$1` as `make` does, filled by `mke2fs -d` from a tree
# written by `$4`.
populate() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    tree="$work/$1.tree"
    mkdir "$tree"
    $4 "$tree"
    truncate -s "$2" "$img"
    # shellcheck disable=SC2086
    sudo mke2fs -q -F -L FILER $3 -d "$tree" "$img" 2>/dev/null
    sparse "$img" "$out/$1.sparse"
}

make ext2-1k 32M "-t ext2 -b 1024" fill
make ext2-2k 32M "-t ext2 -b 2048" fill
make ext2-4k 32M "-t ext2 -b 4096" fill
make ext2-128 32M "-t ext2 -b 4096 -I 128" fill
make ext2-rev0 32M "-t ext2 -b 1024 -E revision=0" fill
populate ext2-64k 64M "-t ext2 -b 65536" fill
make spec 32M "-t ext2 -b 4096" spec
