#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-squashfs` with the Linux kernel's `squashfs` driver: for
# each image, the `squashfs-manifest` example's listing must equal what
# Linux shows with the image mounted: names, kinds, sizes, contents,
# symlink targets, modification times, and the extended attributes of
# files and directories. Linux cannot mount LZMA images, so those are
# compared with what `unsquashfs` extracts instead, but for directories'
# sizes, which extracting makes the host file system's. The images are
# the committed fixtures and fresh ones `tests/fixtures/make.sh` makes.
#
# Listings are compared sorted. Names that are not UTF-8 are shown with
# `\xHH` escapes, and such entries are compared by their metadata only:
# Filer's paths are UTF-8.
#
# Each image is a job of its own, as many at once as there are cores (or
# `FILER_JOBS`), each one's report printed whole, in order.
#
# Usage: lib/filer-squashfs/tests/differential.sh, from the repository
# root, on Linux with squashfs-tools, python3, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
# shellcheck source=lib/filer-spec/tests/jobs.sh
. lib/filer-spec/tests/jobs.sh

# Prints what Linux shows of the tree at `$1`, in the manifest's format.
kernel_manifest() {
    # As root, for trusted attributes.
    sudo python3 -I - "$1" <<'PY'
import errno, os, re, stat, sys
root = os.fsencode(sys.argv[1])

def fnv1a(data):
    # A zero byte multiplies the hash by the prime, so runs of zeros are
    # hashed at once.
    h, prime, mask = 0xCBF29CE484222325, 0x100000001B3, 0xFFFFFFFFFFFFFFFF
    for run in re.finditer(rb"\x00+|[^\x00]+", data):
        chunk = run.group()
        if chunk[0] == 0:
            h = h * pow(prime, len(chunk), 1 << 64) & mask
        else:
            for b in chunk:
                h = (h ^ b) * prime & mask
    return h

def shown(name):
    # As Filer displays names: bytes that are not UTF-8 as `\xHH`.
    text = name.decode("utf-8", "surrogateescape")
    return "".join(f"\\x{ord(c) - 0xDC00:02X}" if 0xDC80 <= ord(c) <= 0xDCFF
                   else c for c in text)

def joined(items):
    return ",".join(sorted(items)) or "-"

def xattrs(path):
    try:
        names = os.listxattr(path)
    except OSError as error:
        # An image without an attribute table has none to list.
        if error.errno == errno.EOPNOTSUPP:
            return "-"
        raise
    return joined(f"{shown(name.encode('utf-8', 'surrogateescape'))}="
                  f"{fnv1a(os.getxattr(path, name)):016x}"
                  for name in names)

KINDS = [(stat.S_ISDIR, "d"), (stat.S_ISLNK, "l"), (stat.S_ISFIFO, "p"),
         (stat.S_ISSOCK, "s"), (stat.S_ISCHR, "c"), (stat.S_ISBLK, "b")]

def walk(rel, rel_shown):
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        path_shown = shown(name) if not rel else f"{rel_shown}/{shown(name)}"
        st = os.lstat(full)
        kind = next((k for test, k in KINDS if test(st.st_mode)), "f")
        size = st.st_size if kind in "fdl" else 0
        mtime = f"{st.st_mtime_ns // 10**9}.{st.st_mtime_ns % 10**9:09}"
        foreign = path_shown != path.decode("utf-8", "replace")
        if foreign or kind in "pscbd":
            what = "-"
        elif kind == "l":
            what = os.readlink(full).decode("utf-8", "replace")
        else:
            what = f"{fnv1a(open(full, 'rb').read()):016x}"
        attributes = "-" if foreign or kind not in "fd" else xattrs(full)
        print(f"{path_shown}\t{kind}\t{size}\t{mtime}\t{what}\t{attributes}")
        if kind == "d" and not foreign:
            walk(path, path_shown)

walk(b"", "")
PY
}

# Blanks directories' sizes, for comparing with an extracted tree.
unsized() {
    awk -F '\t' 'BEGIN { OFS = FS } $2 == "d" { $3 = "-" } { print }'
}

# Compares image `$2`, named `$3`, with what Linux shows of it: the one
# kind of job.
perform() {
    "$manifest" "$2" | sort > "$scratch/filer"
    if sudo mount -o loop,ro "$2" "$mnt" 2>/dev/null; then
        kernel_manifest "$mnt" | sort > "$scratch/kernel"
        sudo umount "$mnt"
        how=mounted
    else
        # LZMA, which Linux does not read.
        sudo unsquashfs -q -n -d "$scratch/tree" "$2" >/dev/null
        kernel_manifest "$scratch/tree" | unsized | sort > "$scratch/kernel"
        unsized < "$scratch/filer" | sort > "$scratch/filer.unsized"
        mv "$scratch/filer.unsized" "$scratch/filer"
        sudo rm -rf "$scratch/tree"
        how=extracted
    fi
    if diff -u "$scratch/kernel" "$scratch/filer" > "$scratch/diff"; then
        echo "same: $3 ($(wc -l < "$scratch/filer") entries, $how)"
    else
        echo "DIFFERENT: $3 ($how)"
        head -40 "$scratch/diff"
        failed=1
    fi
}

if [ "${1:-}" = --job ]; then
    job "$2"
fi

work=$(mktemp -d -p /var/tmp)
trap clean_work EXIT
failed=0

# A release build, as a debug one decompresses many times slower.
cargo build -q --release -p filer-squashfs --example squashfs-manifest
manifest="$PWD/target/release/examples/squashfs-manifest"

# The committed fixtures, and fresh ones, each a job.
mkdir "$work/fresh"
lib/filer-squashfs/tests/fixtures/make.sh "$work/fresh"
for image in lib/filer-squashfs/tests/fixtures/*.sqfs "$work"/fresh/*.sqfs; do
    case $image in "$work"/*) kind=fresh ;; *) kind=fixture ;; esac
    add image "$image" "$kind-$(basename "$image" .sqfs)"
done
run
exit $failed
