#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the UDF fixtures with real tools: `mkudffs` makes an empty volume
# of each revision and layout, and the Linux kernel's `udf` driver fills
# it with the tree `tests/common` expects, written here, from which one
# file is then deleted, leaving its identifier. `udf-1.02` has short
# allocation descriptors, and `udf-2.01` long ones, as the kernel's
# `longad` writes them, and extended file entries; small files are
# embedded in their entries. `udf-2.50` is a BD-R's, of a virtual
# partition, which Linux does not write, so it is empty, and ends at its
# virtual allocation table. `spec` holds the specification's tree.
#
# `udf-2048` (blocks of 2 KiB), `udf-cdrw` (a sparable partition, of
# 40 MB), and `udf-cdr` (a CD-R's virtual partition) are
# made only when named, by `tests/differential.sh`, and are not
# committed.
#
# Each volume is stored as `<name>.sparse`: the magic `FILER-SPARSE-2\n`,
# the volume's size (u64, little-endian), then runs of identical nonzero
# 512-byte sectors, each as its first sector (u32), its count (u32), and
# the sector's bytes.
#
# Linux writes the time it runs into what it records, as each file's
# creation time, so the volumes differ each time they are made.
#
# Usage: tests/fixtures/make.sh [output-dir [fixture...]], on Linux with
# `udftools`, python3, and `sudo` for mounting and making devices;
# without fixture names, makes the committed fixtures.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
[ $# -gt 0 ] && shift
only="$*"
work=$(mktemp -d -p /var/tmp)
cleanup() {
    if mountpoint -q "$work/mnt"; then sudo umount "$work/mnt"; fi
    sudo rm -rf "$work"
}
trap cleanup EXIT
export TZ=UTC LC_ALL=C.UTF-8
mkdir "$work/mnt"

# Writes the tree `tests/common` expects into directory `$1`, as root.
fill() {
    sudo python3 -I - "$1" <<'PY'
import os, socket, sys
root = sys.argv[1]

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def put(path, data):
    with open(os.path.join(root, path), "wb") as f:
        f.write(data)

put("README.TXT", pattern(9, 1))
put("notes.txt", pattern(10, 2))
put("A long file name with spaces.txt", pattern(100, 3))
put("Ünïcödé ✓.txt", pattern(7, 4))
put("emoji 😀.bin", pattern(5, 5))
put("empty", b"")
# Across several blocks, in extents.
put("big.bin", pattern(20_000, 7))
os.makedirs(os.path.join(root, "Sub", "Deeper"))
put("Sub/nested.bin", pattern(5000, 8))
put("Sub/Deeper/leaf.txt", pattern(5, 9))
os.mkdir(os.path.join(root, "many"))
for i in range(30):
    put(f"many/f{i:03}", pattern(i, 100 + i))
deep = os.path.join(root, *"abcdefghi")
os.makedirs(deep)
put(os.path.join(deep, "bottom.txt"), pattern(6, 15))
# A hard link, names differing only in case, and the longest name UDF
# holds in 8-bit characters.
put("small", pattern(4, 10))
os.link(os.path.join(root, "small"), os.path.join(root, "Sub", "hardlink"))
put("Case.txt", pattern(3, 11))
put("case.txt", pattern(4, 12))
put("n" * 254, pattern(6, 13))
for link, target in [
    ("link", "README.TXT"),
    ("deeplink", "Sub/Deeper/leaf.txt"),
    ("dirlink", "Sub"),
    ("abs", "/notes.txt"),
    ("dangling", "nope"),
    ("long", "Sub/" + "x" * 100),
]:
    os.symlink(target, os.path.join(root, link))
os.mkdir(os.path.join(root, "special"))
os.mkfifo(os.path.join(root, "special", "fifo"))
sock = socket.socket(socket.AF_UNIX)
sock.bind(os.path.join(root, "special", "sock"))
sock.close()
os.mknod(os.path.join(root, "special", "chr"), 0o20644, os.makedev(1, 3))
os.mknod(os.path.join(root, "special", "blk"), 0o60644, os.makedev(7, 0))
# A file deleted, whose identifier stays, marked.
put("gone.txt", b"gone")
os.remove(os.path.join(root, "gone.txt"))
PY
}

# Writes the specification's tree into directory `$1`, but for its name
# that is not UTF-8, which UDF cannot hold.
spec() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
os.makedirs(os.path.join(root, "a", "b"))
open(os.path.join(root, "a", "b", "c.txt"), "wb").write(b"contents")
open(os.path.join(root, "a", "z.txt"), "wb").close()
os.mkdir(os.path.join(root, "a", "empty"))
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
PY
}

# Writes volume `$1` as sparse fixture `$2`.
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

# An empty tree.
none() {
    :
}

# Fixes the times of everything in directory `$1`, directories after
# what they hold, as filling them changed theirs.
settle() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
MTIME = 1_704_164_646_000_000_000
for parent, dirs, files in os.walk(sys.argv[1].encode(), topdown=False):
    for name in dirs + files + [b"."]:
        path = os.path.join(parent, name)
        os.utime(path, ns=(MTIME, MTIME), follow_symlinks=False)
PY
}

# Makes fixture `$1`, of `$2` bytes, with `mkudffs` options `$3`, filled
# by the tree `$4` writes, mounted with options `$5`. A volume of a
# virtual partition is cut to its virtual allocation table.
make() {
    case " $only " in
    *" $1 "*) ;;
    "  ") case $1 in udf-2048 | udf-cdrw | udf-cdr) return 0 ;; esac ;;
    *) return 0 ;;
    esac
    image="$work/$1.img"
    truncate -s "$2" "$image"
    # shellcheck disable=SC2086
    made=$(mkudffs $3 --label=FILER --uuid=0123456789abcdef "$image" 2>&1)
    vat=$(echo "$made" | sed -n 's/.*vatblock=\([0-9]*\).*/\1/p')
    if [ -n "$vat" ]; then
        block=$(echo "$made" | sed -n 's/.*blocksize=\([0-9]*\).*/\1/p')
        truncate -s $(((vat + 1) * block)) "$image"
    else
        sudo mount -t udf -o "loop,rw,iocharset=utf8${5:+,$5}" "$image" "$work/mnt"
        $4 "$work/mnt"
        settle "$work/mnt"
        sudo umount "$work/mnt"
    fi
    sparse "$image" "$out/$1.sparse"
}

make udf-1.02 400K "--media-type=hd --blocksize=512 --udfrev=1.02" fill
make udf-2.01 400K "--media-type=hd --blocksize=512 --udfrev=2.01" fill longad
make udf-2.50 4M "--media-type=bdr --udfrev=2.50" none
make spec 512K "--media-type=hd --blocksize=512 --udfrev=2.01" spec
make udf-2048 1M "--media-type=hd --blocksize=2048 --udfrev=2.01" fill
make udf-cdrw 40M "--media-type=cdrw --udfrev=2.01" fill
make udf-cdr 4M "--media-type=cdr --udfrev=2.01" none
