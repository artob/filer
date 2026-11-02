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

# Writes extended attributes of each kind and place, and inode flags,
# into directory `$1`, as root.
attrs() {
    sudo python3 -I - "$1" <<'PY'
import os, struct, subprocess, sys
root = sys.argv[1]
MTIME = 1_704_164_646_123_456_789

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def put(path, data=b""):
    full = os.path.join(root, path)
    with open(full, "wb") as f:
        f.write(data)
    os.utime(full, ns=(MTIME, MTIME))
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
# Too large for the inode, so in a block.
os.setxattr(put("big.txt", b"big"), "user.big", pattern(900, 1))
# More than the inode holds: some in it, the rest in a block.
full = put("many.txt", b"many")
for i in range(16):
    os.setxattr(full, f"user.a{i:02}", pattern(10, i))
os.setxattr(put("trusted.txt"), "trusted.secret", b"trusted")
os.setxattr(put("security.txt"), "security.label", b"label")
os.setxattr(put("foreign.txt").encode(), b"user.caf\xe9", b"foreign")
os.setxattr(put("acl.txt"), "system.posix_acl_access", ACL)
os.mkdir(os.path.join(root, "acldir"))
os.setxattr(os.path.join(root, "acldir"), "system.posix_acl_default", ACL)
os.mkdir(os.path.join(root, "tagged"))
os.setxattr(os.path.join(root, "tagged"), "user.tag", b"dir")
# Identical sets of attributes too large for an inode: one shared block.
for name in ("shared1", "shared2"):
    os.setxattr(put(name), "user.same", pattern(500, 5))
# Inode flags, of which immutable and append-only are reported.
for name, flags in [("immutable", "+i"), ("append", "+a"), ("nodump", "+d"),
                    ("noatime", "+A"), ("both", "+ia")]:
    put(name, name.encode())
    subprocess.run(["chattr", flags, os.path.join(root, name)], check=True)
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

# Writes directory `big` into directory `$1`: 700 files of long names,
# with bytes past ASCII, so that its index is hashed by every byte, signed
# or not, and is two levels deep in blocks of 1 KiB.
big() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
big = os.path.join(sys.argv[1], "big")
os.mkdir(big)
for i in range(700):
    open(os.path.join(big, f"{i:04}-{'é' * (i % 50)}{'x' * 100}"), "wb").close()
PY
}

# Writes files of extents into directory `$1`: `fragmented`, 2000 blocks
# with a hole after each, which an extent tree of 1 KiB blocks holds two
# levels deep, each 100th block a pattern and the rest written zeros; and
# `unwritten`, 1 MiB allocated but not written, but for 4 KiB in its
# middle.
deep() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
with open(os.path.join(root, "fragmented"), "wb") as f:
    for i in range(2000):
        f.seek(i * 2048)
        f.write(bytes([i % 251 + 1]) * 1024 if i % 100 == 0 else bytes(1024))
with open(os.path.join(root, "unwritten"), "wb") as f:
    os.posix_fallocate(f.fileno(), 0, 1 << 20)
    f.seek(512 << 10)
    f.write(bytes(range(256)) * 16)
PY
}

# Writes inline data that continues past `i_block` into directory `$1`:
# `spill`, a directory of six entries, past the 56 bytes `i_block` has
# room for, and files of 60, 61, and 120 bytes, each byte its offset.
inline() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
root = sys.argv[1]
spill = os.path.join(root, "spill")
os.mkdir(spill)
for i in range(6):
    open(os.path.join(spill, f"entry{i}"), "wb").close()
for size in [60, 61, 120]:
    with open(os.path.join(root, f"bytes{size}"), "wb") as f:
        f.write(bytes(range(size)))
PY
}

# Writes a file whose integrity fs-verity seals into directory `$1`:
# `sealed`, a pattern of 100000 bytes, whose Merkle tree Linux keeps past
# its end.
verity() {
    sudo python3 -I - "$1" <<'PY'
import fcntl, os, struct, sys
path = os.path.join(sys.argv[1], "sealed")
with open(path, "wb") as f:
    f.write(bytes(i * 7 % 251 for i in range(100000)))
# FS_IOC_ENABLE_VERITY: version 1, SHA-256, blocks of 4 KiB, no salt or
# signature.
arg = struct.pack("<IIIIQII Q88x", 1, 1, 4096, 0, 0, 0, 0, 0)
fd = os.open(path, os.O_RDONLY)
try:
    fcntl.ioctl(fd, 0x40806685, arg)
finally:
    os.close(fd)
PY
}

# Writes a file whose attribute values are too large for a block into
# directory `$1`, so that `ea_inode` keeps them in inodes of their own:
# `holder`, with `user.large` of 8000 bytes and `user.huge` of 60000, and
# `user.small`, in the inode.
ea_inode() {
    sudo python3 -I - "$1" <<'PY'
import os, sys
path = os.path.join(sys.argv[1], "holder")
open(path, "wb").close()
os.setxattr(path, "user.large", bytes(i * 3 % 251 for i in range(8000)))
os.setxattr(path, "user.huge", bytes(i * 5 % 251 for i in range(60000)))
os.setxattr(path, "user.small", b"small")
PY
}

# Makes fixture `$1` as `make` does, filled by `big`, its directories
# hashed by version `$4` (`legacy`, `half_md4`, or `tea`) over signed or,
# if `$5` is `unsigned`, unsigned bytes.
hashed() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    truncate -s "$2" "$img"
    # shellcheck disable=SC2086
    mke2fs -q -F -L FILER $3 "$img"
    flags=1
    [ "$5" = unsigned ] && flags=2
    debugfs -w -R "ssv def_hash_version $4" "$img" 2>/dev/null
    debugfs -w -R "ssv flags $flags" "$img" 2>/dev/null
    sudo mount -o loop "$img" "$mnt"
    big "$mnt"
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

# Makes volume `$1` of size `$2` with `mke2fs` options `$3` whose journal
# needs recovery, as a machine losing power leaves it: the spec's tree,
# written back in place, then transactions that commit but are not yet
# written back (files made, renamed, and removed; a directory and a file
# with an indirect block made and removed, so later transactions revoke
# their blocks), copied while still mounted. Files are synced, so their
# data and sizes are in the journal's transactions; commits come each
# second, and writing back only after 30; nor do `sync` or `umount` run
# first: both write back what the journal holds.
dirty() {
    case " $only " in "  " | *" $1 "*) ;; *) return ;; esac
    img="$work/$1.img"
    truncate -s "$2" "$img"
    # shellcheck disable=SC2086
    mke2fs -q -F -L FILER $3 "$img"
    sudo mount -o loop,commit=1 "$img" "$mnt"
    spec "$mnt"
    sync
    sudo python3 -I - "$mnt" <<'PY'
import os, shutil, sys, time
root = sys.argv[1]

def write(path, data):
    with open(path, "wb") as f:
        f.write(data)
        os.fsync(f.fileno())

gone = os.path.join(root, "gone")
os.mkdir(gone)
for i in range(200):
    write(os.path.join(gone, f"g{i:03}"), b"")
write(os.path.join(root, "large"), b"x" * (64 << 10))
time.sleep(2)
shutil.rmtree(gone)
os.remove(os.path.join(root, "large"))
more = os.path.join(root, "more")
os.mkdir(more)
for i in range(40):
    write(os.path.join(more, f"m{i:02}"), bytes([i]) * (i * 100))
os.rename(os.path.join(more, "m00"), os.path.join(more, "renamed"))
os.remove(os.path.join(more, "m01"))
time.sleep(3)
PY
    cp --sparse=always "$img" "$img.lost"
    sudo umount "$mnt"
    sparse "$img.lost" "$out/$1.sparse"
}

make ext2-1k 32M "-t ext2 -b 1024" fill
make ext2-2k 32M "-t ext2 -b 2048" fill
make ext2-4k 32M "-t ext2 -b 4096" fill
make ext2-128 32M "-t ext2 -b 4096 -I 128" fill
make ext2-rev0 32M "-t ext2 -b 1024 -E revision=0" fill
populate ext2-64k 64M "-t ext2 -b 65536" fill
make ext2-attrs 8M "-t ext2 -b 4096" attrs
make ext2-attrs-128 8M "-t ext2 -b 1024 -I 128" attrs
make spec 32M "-t ext2 -b 4096" spec
make ext3-1k 32M "-t ext3 -b 1024" fill
make ext3-4k 32M "-t ext3 -b 4096" fill
make ext3-extents-1k 32M "-t ext3 -b 1024 -O extent" fill
make ext4-1k 32M "-t ext4 -b 1024" fill
make ext4-4k 32M "-t ext4 -b 4096" fill
make ext4-meta-bg 256M "-t ext4 -b 1024 -O meta_bg,^resize_inode" fill
make ext4-gdt-csum 32M "-t ext4 -b 4096 -O ^metadata_csum,^metadata_csum_seed,uninit_bg" fill
make ext4-attrs 8M "-t ext4 -b 4096" attrs
make ext4-extents-deep 16M "-t ext4 -b 1024" deep
make ext4-inline 32M "-t ext4 -b 4096 -O inline_data" fill
make ext4-inline-spill 8M "-t ext4 -b 4096 -O inline_data" inline
make ext4-bigalloc 64M "-t ext4 -b 1024 -O bigalloc -C 16384" fill
make ext4-features 32M "-t ext4 -b 4096 -O large_dir,mmp,sparse_super2,^resize_inode" fill
make ext4-verity 8M "-t ext4 -b 4096 -O verity" verity
make ext4-ea-inode 8M "-t ext4 -b 4096 -O ea_inode" ea_inode
make ext3-extents-4k 32M "-t ext3 -b 4096 -O extent" fill
make ext3-extents-deep 16M "-t ext3 -b 1024 -O extent" deep
hashed ext2-htree-tea 16M "-t ext2 -b 1024" tea unsigned
hashed ext2-htree-legacy 16M "-t ext2 -b 4096" legacy signed
hashed ext4-htree 16M "-t ext4 -b 1024 -O ^has_journal" half_md4 signed
dirty ext3-dirty 32M "-t ext3 -b 4096"
dirty ext3-dirty-1k 32M "-t ext3 -b 1024"
dirty ext3-dirty-csum 32M "-t ext3 -b 4096 -O metadata_csum"
