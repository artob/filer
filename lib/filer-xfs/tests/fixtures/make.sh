#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the XFS fixtures with real tools: `mkfs.xfs` formats each volume,
# version 4 (`xfs-*`), version 5 with all later features off (`xfs5-*`), as it
# does by default (`xfs5d-*`), or with its newest features (`xfs5n-*`), and
# the Linux kernel fills it with the tree `tests/fixtures.rs` expects, through
# a loop mount. Version 4 volumes of 64 KiB blocks, which x86 Linux cannot
# mount, are filled by `mkfs.xfs -p` from the same tree instead. Each image is
# stored as `<name>.sparse`: the magic `FILER-SPARSE-2\n`, the image size
# (u64, little-endian), then runs of identical nonzero 512-byte sectors, each
# as its first sector (u32), its count (u32), and the sector's bytes.
# `spec*.sparse` hold the specification's tree instead, `*-dirs.sparse` a
# directory of each form, `*-attrs*.sparse` extended attributes of each form,
# and inode flags, `xfs-ci.sparse` names on an `ASCII_CI` volume,
# `xfs5-uuid.sparse` the specification's tree on a volume whose UUID changed,
# so that its metadata carries `sb_meta_uuid`, `xfs5d-later.sparse` what later
# features change, `xfs5n-rt.sparse` the data device of a volume with a
# realtime device, and `*-dirty.sparse` the specification's tree on a volume
# shut down before its log was written back, whose log is kept but for the
# empty headers past its head. No tool makes `attr1` attribute forks: Linux
# and `mkfs.xfs -p` both turn a volume to `attr2` on its first attribute.
#
# Usage: tests/fixtures/make.sh [output-dir [fixture...]], on Linux with
# xfsprogs, python3, and `sudo` for mounting and making devices; without
# fixture names, makes them all. Times are UTC, so output is stable but
# for the times `mkfs.xfs` and the kernel record themselves.
#
# `mkfs.xfs` makes no volume smaller than 300 MiB, nor a log smaller than
# 64 MiB; sectors that stay zero cost nothing in a sparse image. Once
# filled, each volume's log is reformatted, clean and empty, by `xfs_db
# logformat`, as the records the kernel wrote would be most of its
# nonzero sectors; `tests/differential.sh` checks logs the kernel leaves,
# clean and not. Linux refuses to mount version 5 volumes so formatted,
# whose metadata records log positions past the new log's start; with
# `KEEP_LOG=1`, as `tests/differential.sh` sets, the kernel's log stays.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
[ $# -gt 0 ] && shift
only="$*"
# Not in `/tmp`, which may forbid devices, that `mkfs.xfs -p` opens.
work=$(mktemp -d -p /var/tmp)
mnt="$work/mnt"
mkdir "$mnt"
rtloop=
cleanup() {
    if mountpoint -q "$mnt"; then sudo umount "$mnt"; fi
    if [ -n "$rtloop" ]; then sudo losetup -d "$rtloop"; fi
    sudo rm -rf "$work"
}
trap cleanup EXIT
export TZ=UTC LC_ALL=C.UTF-8
SIZE=320M

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
# out, past holes.
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
# Symlinks: in the inode, and, too long for any, in blocks.
for link, target in [
    ("link", "README.TXT"),
    ("deeplink", "Sub/Deeper/leaf.txt"),
    ("dirlink", "Sub"),
    ("abs", "/notes.txt"),
    ("dangling", "nope"),
    ("long", "Sub/" + "x" * 100),
    ("remote", "x/" * 450),
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

# Writes extended attributes of each kind and form, and inode flags,
# into directory `$1`, as root. Forks: short form, in the inode; one leaf
# block; nodes over leaves; and that mapped by a B+ tree, its blocks
# scattered among a file's. Values: in the inode, in a leaf, and remote,
# in blocks of their own.
attrs() {
    sudo python3 -I - "$1" <<'PY'
import os, shutil, struct, subprocess, sys
root = sys.argv[1]

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def put(path, data=b""):
    full = os.path.join(root, path)
    with open(full, "wb") as f:
        f.write(data)
    return full

def acl(entries):
    # Linux's form: version 2, then tag, permissions, and id, with
    # u32::MAX for entries without one.
    return struct.pack("<I", 2) + b"".join(
        struct.pack("<HHI", tag, perm, 0xFFFFFFFF if uid is None else uid)
        for tag, perm, uid in entries)

ACL = acl([(1, 6, None), (2, 4, 1000), (4, 4, None), (0x10, 4, None), (0x20, 0, None)])

put("plain.txt", b"plain")
full = put("small.txt", b"small")
os.setxattr(full, "user.small", b"x")
os.setxattr(full, "user.empty", b"")
os.setxattr(put("big.txt", b"big"), "user.big", pattern(900, 1))
os.setxattr(put("remote.txt", b"remote"), "user.remote", pattern(10000, 2))
full = put("leaf.txt", b"leaf")
for i in range(16):
    os.setxattr(full, f"user.a{i:02}", pattern(30, i))
full = put("node.txt", b"node")
for i in range(400):
    os.setxattr(full, f"user.n{i:03}" + "n" * 40, pattern(20, i))
# Each new attribute block lands in a hole between files, or after the
# filler's last, so that few are contiguous; holes and filler are then
# deleted.
full = put("btree.txt", b"btree")
holes = os.path.join(root, "holes")
os.mkdir(holes)
for i in range(400):
    with open(os.path.join(holes, str(i)), "wb") as f:
        f.write(b"\0")
os.sync()
for i in range(0, 400, 2):
    os.remove(os.path.join(holes, str(i)))
os.sync()
filler = os.path.join(root, "filler")
with open(filler, "wb") as f:
    # Fewer than Linux lists: their names in 64 KiB.
    for i in range(1200):
        os.setxattr(full, f"user.b{i:03}" + "b" * 40, pattern(20, i))
        if i % 10 == 0:
            f.write(bytes(65536)); f.flush(); os.fsync(f.fileno())
os.remove(filler)
shutil.rmtree(holes)
os.setxattr(put("trusted.txt"), "trusted.secret", b"trusted")
os.setxattr(put("security.txt"), "security.label", b"label")
os.setxattr(put("foreign.txt").encode(), b"user.caf\xe9", b"foreign")
os.setxattr(put("acl.txt"), "system.posix_acl_access", ACL)
os.mkdir(os.path.join(root, "acldir"))
os.setxattr(os.path.join(root, "acldir"), "system.posix_acl_default", ACL)
os.mkdir(os.path.join(root, "tagged"))
os.setxattr(os.path.join(root, "tagged"), "user.tag", b"dir")
# Inode flags, of which immutable and append-only are reported.
for name, flags in [("immutable", "+i"), ("append", "+a"), ("nodump", "+d"),
                    ("noatime", "+A"), ("both", "+ia")]:
    put(name, name.encode())
    subprocess.run(["chattr", flags, os.path.join(root, name)], check=True)
PY
}

# Writes names differing from those looked up in case into directory
# `$1`, as root, on an `ASCII_CI` volume, and checks that Linux finds them
# so: in a short-form directory, and in one of nodes over leaves, whose
# entries are hard links to one file.
ci() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
def put(path, data):
    with open(os.path.join(root.encode(), path), "wb") as f:
        f.write(data)
put(b"Hello.TXT", b"hello")
put("Ünïcödé".encode(), b"unicode")
put(b"caf\xc9", b"latin-1")
os.mkdir(os.path.join(root, "Large"))
for i in range(2000):
    os.link(os.path.join(root, "Hello.TXT"), os.path.join(root, f"Large/File{i:04}"))
# Bytes fold, Latin-1's letters too, but not UTF-8's.
for name, found in [(b"hello.txt", True), (b"HELLO.TXT", True),
                    (b"CAF\xe9", True), ("ünïcödé".encode(), False),
                    (b"large/file1999", True), (b"LARGE/FILE0000", True)]:
    assert os.path.exists(os.path.join(root.encode(), name)) == found, name
PY
}

# Writes the specification's tree into directory `$1`, as `spec` does,
# then shuts its volume down, so that its log needs recovery.
spec_dirty() {
    spec "$1"
    sync
    # A shutdown reaches the volume holding `$1`: never let that be the
    # root's.
    mountpoint -q "$1"
    sudo xfs_io -x -c shutdown "$1"
}

# Zeroes the empty record headers of cycle 0 in the log of image `$1`,
# which Linux writes past the head: sectors of no cycle either way.
trim() {
    python3 -I - "$1" <<'PY'
import struct, subprocess, sys
img = sys.argv[1]
def field(name):
    out = subprocess.run(["xfs_db", "-r", "-c", "sb 0", "-c", f"print {name}", img],
                         capture_output=True, text=True, check=True).stdout
    return int(out.split("=")[1])
start, ag_blocks, ag_log = field("logstart"), field("agblocks"), field("agblklog")
block, blocks = field("blocksize"), field("logblocks")
base = ((start >> ag_log) * ag_blocks + (start & ((1 << ag_log) - 1))) * block
with open(img, "r+b") as f:
    for i in range(blocks * block // 512):
        f.seek(base + i * 512)
        sector = f.read(16)
        if sector[:8] == b"\xfe\xed\xba\xbe\0\0\0\0" and sector[12:16] == bytes(4):
            f.seek(base + i * 512)
            f.write(bytes(512))
PY
}

# Prints the attribute fork format of the inode at path `$2` of image
# `$1`, and the magic of its first block, if any.
fork() {
    format=$(field "$1" "$2" core.aformat)
    case $format in
        *"(local)") echo local ;;
        *) printf '%s ' "$format" | sed 's/.*(\(.*\)) /\1 /'
           xfs_db -r -c "path /$2" -c "ablock 0" -c "type text" -c print "$1" |
               awk 'NR == 1 { print $10 $11 }' ;;
    esac
}

# Fails unless the attribute forks `attrs` wrote in image `$1` are of the
# forms the files are named for, whose leaves have magic `$2`, and nodes
# `$3`; but with 512-byte blocks, one leaf does not hold `leaf.txt`'s.
check_attrs() {
    found="$(fork "$1" small.txt), $(fork "$1" leaf.txt), $(fork "$1" node.txt)"
    found="$found, $(fork "$1" btree.txt)"
    leaf=$2
    if [ "$(xfs_db -r -c "sb 0" -c "print blocksize" "$1" | sed 's/.*= //')" = 512 ]; then
        leaf=$3
    fi
    if [ "$found" != "local, extents $leaf, extents $3, btree $3" ]; then
        echo "attribute forms: $found" >&2
        exit 1
    fi
}

# Writes what version 5's later features change into directory `$1`, as
# root: times past 2038 and before 1970, which inodes hold as 64-bit
# nanoseconds with `BIGTIME`; reflinked copies, which share their blocks;
# and a file of many extents, counted wider with `NREXT64`.
later() {
    sudo python3 -I - "$1" <<'PY'
import os, subprocess, sys
root = sys.argv[1]

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def put(path, data):
    with open(os.path.join(root, path), "wb") as f:
        f.write(data)

for name, ns in [("y1901", -2_147_483_648 * 10**9),
                 ("y1960", -315_619_200 * 10**9 + 999_999_999),
                 ("y2038", 2_147_483_648 * 10**9 + 1),
                 ("y2400", 13_569_465_600 * 10**9 + 7)]:
    put(name, name.encode())
    os.utime(os.path.join(root, name), ns=(ns, ns))
put("original", pattern(60_000, 3))
for copy in ("copy", "changed"):
    subprocess.run(["cp", "--reflink=always", os.path.join(root, "original"),
                    os.path.join(root, copy)], check=True)
with open(os.path.join(root, "changed"), "r+b") as f:
    f.seek(20_000)
    f.write(pattern(5000, 9))
with open(os.path.join(root, "extents"), "wb") as f:
    for i in range(0, 600, 2):
        f.seek(i * 4096)
        f.write(pattern(16, i % 251))
PY
}

# Writes files on the realtime device into directory `$1`, as root: one
# made so, and one in a directory whose new files are; and one not.
realtime() {
    sudo sh -c "echo data > '$1/normal'; touch '$1/rtfile'"
    sudo xfs_io -c "chattr +r" "$1/rtfile"
    sudo sh -c "echo realtime > '$1/rtfile'"
    sudo mkdir "$1/rtdir"
    sudo xfs_io -c "chattr +t" "$1/rtdir"
    sudo sh -c "echo inherited > '$1/rtdir/f'"
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

# Writes a directory of each form into directory `$1`, as root: short
# form, in the inode; a single block; leaf; leaf, shrunk to one data
# block, which keeps its leaf; node; and node with its map a B+ tree, its
# blocks scattered among a file's. Entries are hard links to one file.
dirs() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
target = os.path.join(root, "target")
with open(target, "wb") as f:
    f.write(b"target")
for form, count in [("short", 3), ("block", 40), ("leaf", 400), ("node", 1000)]:
    os.mkdir(os.path.join(root, form))
    for i in range(count):
        os.link(target, os.path.join(root, form, f"f{i:05}"))
os.mkdir(os.path.join(root, "shrunk"))
for i in range(400):
    os.link(target, os.path.join(root, "shrunk", f"f{i:05}"))
for i in range(150, 400):
    os.remove(os.path.join(root, "shrunk", f"f{i:05}"))
# Each new directory block lands after the filler's last, so that few are
# contiguous; the filler is then deleted.
os.mkdir(os.path.join(root, "btree"))
filler = os.path.join(root, "filler")
with open(filler, "wb") as f:
    for i in range(2000):
        os.link(target, os.path.join(root, "btree", f"f{i:05}"))
        if i % 50 == 0:
            f.write(bytes(65536)); f.flush(); os.fsync(f.fileno())
os.remove(filler)
PY
}

# Prints field `$3` of the inode at path `$2` of image `$1`.
field() {
    xfs_db -r -c "path /$2" -c "print $3" "$1" | sed 's/.*= //'
}

# Prints the magic of the block at 32 GiB in directory `$2` of image
# `$1`: its leaf, or the root of its leaf nodes.
leaf() {
    xfs_db -r -c "path /$2" -c "dblock 8388608" -c "type text" -c print "$1" |
        awk 'NR == 1 { print $10 $11 }'
}

# Fails unless the directories `dirs` wrote in image `$1` are of the forms
# they are named for, whose leaves have magic `$2`, and nodes `$3`.
check_dirs() {
    found="$(field "$1" short core.format) $(field "$1" block core.size)"
    found="$found $(leaf "$1" leaf) $(leaf "$1" node) $(leaf "$1" btree)"
    found="$found $(field "$1" btree core.format)"
    found="$found $(field "$1" shrunk core.size) $(leaf "$1" shrunk)"
    if [ "$found" != "1 (local) 4096 $2 $3 $3 3 (btree) 4096 $2" ]; then
        echo "directory forms: $found" >&2
        exit 1
    fi
}

# Gives image `$1` a new UUID, so that its metadata carries the old one.
uuid() {
    xfs_admin -U generate "$1" >/dev/null 2>&1
}

# Makes fixture `$1`, formatted by `mkfs.xfs` with options `$2`, filled by
# `$3` through the kernel, then changed by `$4`, if given.
make() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    truncate -s $SIZE "$img"
    # shellcheck disable=SC2086
    mkfs.xfs -q -f -L FILER $2 "$img" 2>/dev/null
    sudo mount -o loop "$img" "$mnt"
    $3 "$mnt"
    sudo umount "$mnt"
    if [ $# -gt 3 ]; then
        $4 "$img"
    fi
    case $3 in
        *_dirty) trim "$img" ;;
        *) if [ -z "${KEEP_LOG:-}" ]; then
            xfs_db -x -c "logformat -c 1" "$img" >/dev/null
        fi ;;
    esac
    # Leaves', attribute leaves', and nodes' magic numbers.
    case $2 in
        *crc=0*) leaf=d2f1 attr=fbee node=febe ;;
        *) leaf=3df1 attr=3bee node=3ebe ;;
    esac
    case $3 in
        dirs) check_dirs "$img" "$leaf" "$node" ;;
        attrs) check_attrs "$img" "$attr" "$node" ;;
    esac
    sparse "$img" "$out/$1.sparse"
}

# Makes fixture `$1` as `make` does, on a volume with a realtime device,
# of which only the data device is kept.
make_rt() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    truncate -s $SIZE "$img"
    truncate -s 64M "$work/$1.rt"
    # shellcheck disable=SC2086
    mkfs.xfs -q -f -L FILER $2 -r rtdev="$work/$1.rt" "$img" 2>/dev/null
    rtloop=$(sudo losetup -f --show "$work/$1.rt")
    sudo mount -o loop,rtdev="$rtloop" "$img" "$mnt"
    $3 "$mnt"
    sudo umount "$mnt"
    sudo losetup -d "$rtloop"
    rtloop=
    if [ -z "${KEEP_LOG:-}" ]; then
        xfs_db -x -c "logformat -c 1" "$img" >/dev/null
    fi
    sparse "$img" "$out/$1.sparse"
}

# Writes out the holes of every file below directory `$1` as zeros,
# keeping its times.
dense() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
for dir, _, names in os.walk(sys.argv[1]):
    for name in names:
        path = os.path.join(dir, name)
        st = os.lstat(path)
        if not os.path.isfile(path) or os.path.islink(path):
            continue
        if st.st_blocks * 512 < st.st_size:
            data = open(path, "rb").read()
            with open(path, "r+b") as f:
                f.write(data)
                f.flush()
                os.fsync(f.fileno())
            os.utime(path, ns=(st.st_atime_ns, st.st_mtime_ns))
PY
}

# Makes fixture `$1` as `make` does, filled by `mkfs.xfs -p` from a tree
# written by `$3`. With 64 KiB blocks, `mkfs.xfs` 6.18 drops or garbles
# data that starts within a block, after a hole, so files are copied
# without holes.
populate() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    tree="$work/$1.tree"
    mkdir "$tree"
    $3 "$tree"
    dense "$tree"
    truncate -s $SIZE "$img"
    # shellcheck disable=SC2086
    sudo mkfs.xfs -q -f -L FILER $2 -p "$tree" "$img" 2>/dev/null
    sparse "$img" "$out/$1.sparse"
}

v4="-m crc=0"
make xfs-512 "$v4 -b size=512" fill
make xfs-1k "$v4 -b size=1024 -i size=512 -n ftype=0" fill
make xfs-2k "$v4 -b size=2048 -i size=1024 -n size=8192" fill
make xfs-4k "$v4" fill
make xfs-4k-big "$v4 -i size=2048 -n size=65536" fill
populate xfs-64k "$v4 -b size=65536 -i size=2048" fill
make xfs-dirs "$v4" dirs
make xfs-attrs "$v4" attrs
make xfs-ci "$v4 -n version=ci" ci
make xfs-attrs-512 "$v4 -b size=512" attrs
make spec "$v4" spec
make xfs-dirty "$v4" spec_dirty

v5="-m crc=1,finobt=0,rmapbt=0,reflink=0,bigtime=0,inobtcount=0"
v5="$v5 -i sparse=0,nrext64=0"
make xfs5-1k "$v5 -b size=1024" fill
make xfs5-4k "$v5" fill
make xfs5-64k "$v5 -b size=65536" fill
make xfs5-dirs "$v5" dirs
make xfs5-attrs "$v5" attrs
make xfs5-attrs-1k "$v5 -b size=1024" attrs
make xfs5-uuid "$v5" spec uuid
make spec5 "$v5" spec

# As `mkfs.xfs` makes them by default.
make xfs5d-4k "" fill
make xfs5d-dirs "" dirs
make xfs5d-attrs "" attrs
make xfs5d-later "" later
make xfs5d-dirty "" spec_dirty

# With the newest features: parent pointers, which bring range exchange,
# and the metadata directory, which brings realtime groups.
newest="-n parent=1 -m metadir=1"
make xfs5n-4k "$newest" fill
make xfs5n-dirs "$newest" dirs
make xfs5n-attrs "$newest" attrs
make_rt xfs5n-rt "$newest" realtime
