#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the ISO 9660 fixtures with real tools: `xorriso` packs the tree
# `tests/common` expects, written here, into an image of each extension
# the driver reads: Rock Ridge, Joliet, both, neither, and Rock Ridge
# with every file zisofs compresses, and of ISO 9660:1999's names.
# `spec.iso` holds the specification's tree instead. The trees'
# directories nest deeper than ISO 9660 allows, so Rock Ridge relocates
# one. Images are stored as they are, `<name>.iso`.
#
# `hidden.iso`, with a hidden file, is made only when named: only
# `genisoimage` sets that flag, and it writes the time it runs, so the
# image is never the same twice, and is not committed;
# `tests/differential.sh` makes it fresh.
#
# Usage: tests/fixtures/make.sh [output-dir [fixture...]], on Linux with
# `xorriso`, python3, `sudo` for making devices, and `genisoimage` for
# `hidden.iso`; without fixture names, makes all but `hidden.iso`. Times
# are fixed, so output is stable.

set -eu
out=$(cd "${1:-$(dirname "$0")}" && pwd)
[ $# -gt 0 ] && shift
only="$*"
work=$(mktemp -d -p /var/tmp)
cleanup() {
    sudo rm -rf "$work"
}
trap cleanup EXIT
export TZ=UTC LC_ALL=C.UTF-8
# 2024-01-02T03:04:06Z, for every date the tools write.
export SOURCE_DATE_EPOCH=1704164646

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
# Across several blocks; and zeros, which zisofs shrinks to nothing.
put("big.bin", pattern(20_000, 7))
put("zeros.bin", bytes(20_000))
os.makedirs(os.path.join(root, "Sub", "Deeper"))
put("Sub/nested.bin", pattern(5000, 8))
put("Sub/Deeper/leaf.txt", pattern(5, 9))
os.mkdir(os.path.join(root, "many"))
for i in range(30):
    put(f"many/f{i:03}", pattern(i, 100 + i))
# Nine levels deep, one more than ISO 9660 allows.
deep = os.path.join(root, *"abcdefghi")
os.makedirs(deep)
put(os.path.join(deep, "bottom.txt"), pattern(6, 15))
# A hard link, names differing only in case, the longest name, and a
# name that is not UTF-8.
put("small", pattern(4, 10))
os.link(os.path.join(root, "small"), os.path.join(root, "Sub", "hardlink"))
put("Case.txt", pattern(3, 11))
put("case.txt", pattern(4, 12))
put("n" * 255, pattern(6, 13))
with open(os.path.join(root.encode(), b"caf\xe9.txt"), "wb") as f:
    f.write(pattern(3, 14))
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

# Makes fixture `$1` of the tree that `$2` writes, packed by `$3`, with
# options `$4`, and for `xorriso`, its own options `$5` first. `xorriso`
# writes Rock Ridge's entries unless told not to.
make() {
    case " $only " in *" $1 "*) ;; "  ") [ "$1" != hidden ] || return 0 ;; *) return 0 ;; esac
    tree="$work/$1"
    mkdir "$tree"
    $2 "$tree"
    settle "$tree"
    image="$work/$1.iso"
    case $3 in
    xorriso)
        # Every date, and every file's, fixed; no padding after the
        # image.
        # shellcheck disable=SC2086,SC2048
        sudo xorriso -report_about SORRY ${5:-} -as mkisofs -o "$image" -V FILER \
            -no-pad --modification-date=2024010203040600 \
            --set_all_file_dates 2024010203040600 $4 "$tree"
        ;;
    zisofs)
        # Rock Ridge, every file compressed.
        sudo xorriso -report_about SORRY -outdev "stdio:$image" -volid FILER \
            -compliance new_rr \
            -padding 0 -volume_date all_file_dates 2024010203040600 \
            -volume_date uuid 2024010203040600 \
            -map "$tree" / -set_filter_r --zisofs / -- -commit
        ;;
    genisoimage)
        # shellcheck disable=SC2086
        sudo genisoimage -quiet -o "$image" -V FILER -no-pad $4 "$tree"
        ;;
    esac
    sudo chown "$(id -u):$(id -g)" "$image"
    cp "$image" "$out/$1.iso"
}

# Rock Ridge 1.12, whose serial numbers hard links share; 1.10 has none.
# (`-hardlinks on` would record the time `xorriso` ran.)
make rock-ridge fill xorriso "-R" "-compliance new_rr"
make joliet fill xorriso "-J -joliet-long" "-rockridge off"
make rock-ridge-joliet fill xorriso "-R -J" "-compliance deep_paths_off"
make plain fill xorriso "" "-rockridge off"
make zisofs fill zisofs ""
make iso9660-1999 fill xorriso "-iso-level 4" "-rockridge off"
make hidden fill genisoimage "-D -hidden README.TXT"
make spec spec xorriso "-R" "-compliance new_rr"
