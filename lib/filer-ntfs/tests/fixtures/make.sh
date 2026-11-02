#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the NTFS fixtures with real tools: `mkntfs` formats each volume,
# and the ntfs-3g FUSE driver fills it with the tree `tests/fixtures.rs`
# expects, writing named data streams as `user.` attributes. Each image is stored as `<name>.sparse`: the magic
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
# A named data stream, read as the attribute `user.extra`.
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

# Reparse points: links of each kind, and a tag Filer cannot read; and
# Interix special files, as ntfs-3g makes FIFOs and sockets, and as older
# ntfs-3g made symlinks and devices.
links() {
    python3 -I - "$mnt" <<'PY'
import os, socket, struct, sys
root = sys.argv[1]
MTIME = 1_704_164_646

def utf16(s):
    return s.encode("utf-16-le")

def windows_link(substitute, print_name, flags):
    """A symbolic link (`flags` given) or junction (`flags` None)."""
    sub, pr = utf16(substitute), utf16(print_name)
    data = struct.pack("<HHHH", 0, len(sub), len(sub), len(pr))
    if flags is not None:
        data += struct.pack("<I", flags)
    data += sub + pr
    tag = 0xA000000C if flags is not None else 0xA0000003
    return struct.pack("<IHH", tag, len(data), 0) + data

def wsl_link(target):
    data = struct.pack("<I", 2) + target.encode()
    return struct.pack("<IHH", 0xA000001D, len(data), 0) + data

def reparse(path, value, directory=False):
    full = os.path.join(root, path)
    if directory:
        os.mkdir(full)
    else:
        open(full, "wb").close()
    os.setxattr(full, "system.ntfs_reparse_data", value)

def put(path, data):
    full = os.path.join(root, path)
    with open(full, "wb") as f:
        f.write(data)
    os.utime(full, (MTIME, MTIME))

put("target.txt", b"target")
os.mkdir(os.path.join(root, "Sub"))
put("Sub/nested.txt", b"nested")
reparse("link", windows_link("target.txt", "target.txt", 1))
reparse("deep", windows_link("Sub\\nested.txt", "Sub\\nested.txt", 1))
reparse("dirlink", windows_link("Sub", "Sub", 1), directory=True)
reparse("abs", windows_link("\\??\\C:\\target.txt", "C:\\target.txt", 0))
reparse("junction", windows_link("\\??\\C:\\Sub", "C:\\Sub", None), directory=True)
reparse("wsl", wsl_link("target.txt"))
reparse("dangling", windows_link("nope", "nope", 1))
reparse("cloud.bin", struct.pack("<IHH", 0x9000001A, 4, 0) + b"data")

def interix(path, data):
    full = os.path.join(root, path)
    with open(full, "wb") as f:
        f.write(data)
    os.setxattr(full, "system.ntfs_attrib_be", (0x4).to_bytes(4, "big"))

interix("ilink", b"IntxLNK\x01" + utf16("target.txt"))
interix("idirlink", b"IntxLNK\x01" + utf16("Sub"))
interix("ichr", b"IntxCHR\x00" + struct.pack("<QQ", 1, 3))
interix("iblk", b"IntxBLK\x00" + struct.pack("<QQ", 8, 1))
interix("system.bin", b"IntxFOO\x00, a plain system file")
os.mkfifo(os.path.join(root, "fifo"))
sock = socket.socket(socket.AF_UNIX)
sock.bind(os.path.join(root, "sock"))
sock.close()
PY
}

# Named data streams, read as extended attributes, and file attributes.
streams() {
    python3 -I - "$mnt" <<'PY'
import os, sys
root = sys.argv[1]
MTIME = 1_704_164_646

def pattern(size, seed):
    return bytes((i * 7 + seed) % 251 for i in range(size))

def put(path, data):
    full = os.path.join(root, path)
    with open(full, "wb") as f:
        f.write(data)
    os.utime(full, (MTIME, MTIME))
    return full

# Resident, non-resident (up to Linux's limit on attribute values),
# empty, and non-ASCII streams.
full = put("streams.txt", b"main")
os.setxattr(full, "user.small", b"x")
os.setxattr(full, "user.big", pattern(60_000, 1))
os.setxattr(full, "user.empty", b"")
os.setxattr(full, "user.Ünï ✓", b"unicode")
os.setxattr(full, "user.MixedCase", b"m")
# More streams than one record holds, listed in `$ATTRIBUTE_LIST`.
full = put("many.bin", b"many")
for i in range(100):
    os.setxattr(full, f"user.s{i:03}", pattern(300, i))
# A directory's stream.
os.mkdir(os.path.join(root, "Tagged"))
os.setxattr(os.path.join(root, "Tagged"), "user.tag", b"dir")
# File attributes, set exactly: read-only (0x1) is a permission, and not
# content indexed (0x2000) is not reported.
for name, bits in [
    ("hidden.txt", 0x2),
    ("system.txt", 0x4),
    ("archive.txt", 0x20),
    ("readonly.txt", 0x1),
    ("offline.txt", 0x1000),
    ("all.txt", 0x1 | 0x2 | 0x4 | 0x20 | 0x1000 | 0x2000),
]:
    full = put(name, name.encode())
    os.setxattr(full, "system.ntfs_attrib_be", bits.to_bytes(4, "big"))
PY
}

spec() {
    python3 -I - "$mnt" <<'PY'
import os, struct, sys
root = sys.argv[1]

def utf16(s):
    return s.encode("utf-16-le")

def windows_link(substitute, print_name, flags):
    """A symbolic link (`flags` given) or junction (`flags` None)."""
    sub, pr = utf16(substitute), utf16(print_name)
    data = struct.pack("<HHHH", 0, len(sub), len(sub), len(pr))
    if flags is not None:
        data += struct.pack("<I", flags)
    data += sub + pr
    tag = 0xA000000C if flags is not None else 0xA0000003
    return struct.pack("<IHH", tag, len(data), 0) + data

def wsl_link(target):
    data = struct.pack("<I", 2) + target.encode()
    return struct.pack("<IHH", 0xA000001D, len(data), 0) + data

def reparse(path, value, directory=False):
    full = os.path.join(root, path)
    if directory:
        os.mkdir(full)
    else:
        open(full, "wb").close()
    os.setxattr(full, "system.ntfs_reparse_data", value)

os.makedirs(os.path.join(root, "a", "b"))
open(os.path.join(root, "a", "b", "c.txt"), "wb").write(b"contents")
open(os.path.join(root, "a", "z.txt"), "wb").close()
os.mkdir(os.path.join(root, "a", "empty"))
# The specification's attributes, as named data streams.
os.setxattr(os.path.join(root, "a", "b", "c.txt"), "user.comment", b"a comment")
os.setxattr(os.path.join(root, "a", "b", "c.txt"), "user.empty", b"")
os.setxattr(os.path.join(root, "a"), "user.tag", b"dir")
# The specification's symlinks, as Windows links where Windows can
# express them, and an absolute one on drive C:.
reparse("a/link", windows_link("b\\c.txt", "b\\c.txt", 1))
reparse("a/up", windows_link("..\\a\\b\\c.txt", "..\\a\\b\\c.txt", 1))
reparse("a/abs", windows_link("\\??\\C:\\a\\b\\c.txt", "C:\\a\\b\\c.txt", 0))
reparse("a/escape", windows_link("..\\..\\c.txt", "..\\..\\c.txt", 1))
reparse("a/loop", windows_link("loop", "loop", 1))
reparse("a/dangling", windows_link("nope", "nope", 1))
os.mkdir(os.path.join(root, "many"))
for i in range(600):
    open(os.path.join(root, "many", f"f{i:03}"), "wb").close()
os.mkdir(os.path.join(root, "special"))
os.mkfifo(os.path.join(root, "special", "fifo"))
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
make ntfs-links 512 4096 links
make ntfs-streams 512 4096 streams
make spec 512 4096 spec
