#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the SquashFS fixtures with real tools: `mksquashfs` packs the tree
# `tests/common` expects, written here, into an image of each compressor,
# and of the block sizes and layouts that exercise the reader: 4 KiB and
# 1 MiB blocks, nothing compressed, no fragments, and every tail in a
# fragment. `spec.sqfs` holds the specification's tree instead, and
# `attrs.sqfs` extended attributes of each namespace and size, some
# sharing their values. Images are stored as they are, `<name>.sqfs`.
#
# Usage: tests/fixtures/make.sh [output-dir [fixture...]], on Linux with
# squashfs-tools 4.6 or later, python3, and `sudo` for making devices and
# trusted attributes; without fixture names, makes them all. Times are
# fixed, so output is stable.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
[ $# -gt 0 ] && shift
only="$*"
# On tmpfs, which holds attributes of every namespace and size.
work=$(mktemp -d -p /dev/shm)
cleanup() {
    sudo rm -rf "$work"
}
trap cleanup EXIT
export TZ=UTC LC_ALL=C.UTF-8

# Writes the tree `tests/common` expects into directory `$1`, as root.
fill() {
    sudo python3 -I - "$1" <<'PY'
import os, socket, sys
root = sys.argv[1]
# 2024-01-02T03:04:06Z, in nanoseconds.
MTIME = 1_704_164_646_000_000_000

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
put("frag.a", b"".join(pattern(4096, 20 + i) for i in range(12)))
put("frag.b", b"".join(pattern(4096, 40 + i) for i in range(12)))
# Sparse files: a megabyte of zeros, then three bytes; and data far out.
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
# Pseudo-random bytes, which no compressor shrinks, so stored as they are.
state, noise = 1, bytearray()
for _ in range(135_000):
    state = (state * 6364136223846793005 + 1442695040888963407) % 2**64
    noise.append(state >> 56)
put("random.bin", bytes(noise))
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
os.mkdir(os.path.join(root, "special"))
os.mkfifo(os.path.join(root, "special", "fifo"))
sock = socket.socket(socket.AF_UNIX)
sock.bind(os.path.join(root, "special", "sock"))
sock.close()
os.mknod(os.path.join(root, "special", "chr"), 0o20644, os.makedev(1, 3))
os.mknod(os.path.join(root, "special", "blk"), 0o60644, os.makedev(7, 0))
for path in ["special/fifo", "special/sock", "special/chr", "special/blk"]:
    touch(path)
# Directories last, as filling them changed their times.
for path in ["Sub/Deeper", "Sub", "many", "special", "."]:
    touch(path)
PY
}

# Writes extended attributes of each namespace and size into directory
# `$1`, as root: on files, a directory, and a symlink, many enough to
# span metadata blocks, and some sharing a value.
attrs() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
MTIME = 1_704_164_646_000_000_000

def path(name):
    return os.path.join(root, name)

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

for name in ["plain", "one", "many", "shared.a", "shared.b", "large"]:
    open(path(name), "wb").write(name.encode())
os.mkdir(path("dir"))
os.symlink("plain", path("link"))
os.setxattr(path("one"), "user.comment", b"a comment")
os.setxattr(path("one"), "user.empty", b"")
os.setxattr(path("one"), "trusted.secret", b"hidden")
os.setxattr(path("one"), "security.label", b"label")
for i in range(200):
    os.setxattr(path("many"), f"user.attribute.{i:03}", pattern(40, i))
for name in ["shared.a", "shared.b"]:
    os.setxattr(path(name), "user.same", pattern(1000, 1))
    os.setxattr(path(name), "user.own", name.encode())
os.setxattr(path("large"), "user.large", pattern(20_000, 2))
os.setxattr(path("dir"), "user.tag", b"dir")
os.setxattr(path("link"), "trusted.link", b"symlink", follow_symlinks=False)
for name in ["plain", "one", "many", "shared.a", "shared.b", "large", "link", "dir", "."]:
    os.utime(path(name), ns=(MTIME, MTIME), follow_symlinks=False)
PY
}

# Writes the specification's tree into directory `$1`.
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

# Makes fixture `$1` of the tree that `$2` writes, packed by `mksquashfs`
# with options `$3`.
make() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    tree="$work/$1"
    mkdir "$tree"
    $2 "$tree"
    # shellcheck disable=SC2086
    sudo mksquashfs "$tree" "$work/$1.sqfs" -quiet -noappend -no-progress \
        -mkfs-time 1704164646 $3 >/dev/null
    cp "$work/$1.sqfs" "$out/$1.sqfs"
}

make gzip fill "-comp gzip"
make lzma fill "-comp lzma"
make lzo fill "-comp lzo"
make lzo-1 fill "-comp lzo -Xalgorithm lzo1x_1"
make xz fill "-comp xz"
make xz-bcj fill "-comp xz -Xbcj x86"
make lz4 fill "-comp lz4"
make lz4-hc fill "-comp lz4 -Xhc"
make zstd fill "-comp zstd"
make small-blocks fill "-comp gzip -b 4K"
make large-blocks fill "-comp zstd -b 1M"
make uncompressed fill "-noI -noId -noD -noF -noX"
make no-fragments fill "-comp gzip -no-fragments"
make all-fragments fill "-comp gzip -always-use-fragments"
make attrs attrs "-comp gzip"
make spec spec "-comp gzip"
