#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Compares `filer-fat` with the Linux kernel's FAT driver: for each image,
# the `manifest` example's listing must equal what the kernel shows with
# the image loop-mounted read-only. The images are the fixtures, the test
# writer's samples, and volumes made by `mkfs.fat` and written by the
# kernel itself, with names and sizes chosen to probe edge cases.
#
# Usage: lib/filer-fat/tests/differential.sh, from the repository root, on
# Linux with dosfstools, python3, GNU coreutils, and `sudo` for mounting.

set -eu
export TZ=UTC LC_ALL=C.UTF-8
work=$(mktemp -d)
mnt="$work/mnt"
mkdir "$mnt"
cleanup() {
    mountpoint -q "$mnt" && sudo umount "$mnt"
    rm -rf "$work"
}
trap cleanup EXIT
options="loop,utf8,tz=UTC,uid=$(id -u),gid=$(id -g)"

cargo build -q -p filer-fat --example manifest --features code-pages
manifest="$PWD/target/debug/examples/manifest"

# The fixtures.
for sparse in lib/filer-fat/tests/fixtures/*.sparse; do
    python3 -I - "$sparse" "$work/fixture-$(basename "$sparse" .sparse).img" <<'EOF'
import struct, sys
data = open(sys.argv[1], "rb").read()
assert data[:15] == b"FILER-SPARSE-1\n"
image = bytearray(struct.unpack("<Q", data[15:23])[0])
rest = data[23:]
while rest:
    i = struct.unpack("<I", rest[:4])[0]
    image[i * 512:(i + 1) * 512] = rest[4:516]
    rest = rest[516:]
open(sys.argv[2], "wb").write(image)
EOF
done

# The test writer's samples.
FILER_FAT_DUMP="$work" cargo test -q -p filer-fat --test driver -- \
    --ignored --exact dump_images > /dev/null

# Volumes written by the kernel.
kernel() {
    name=$1 bits=$2 sectors=$3 kib=$4
    img="$work/kernel-$name.img"
    mkfs.fat -C -F "$bits" -s "$sectors" -i 12345678 "$img" "$kib" > /dev/null
    sudo mount -o "$options" "$img" "$mnt"
    python3 -I - "$mnt" "$((sectors * 512))" <<'EOF'
import os, sys
root, cluster = sys.argv[1], int(sys.argv[2])

def put(path, size, seed):
    with open(os.path.join(root, path), "wb") as f:
        f.write(bytes((i * 7 + seed) % 251 for i in range(size)))
    # Even seconds, as FAT keeps modification times in 2-second steps.
    os.utime(os.path.join(root, path), (1_700_000_000, 1_700_000_000 + seed * 2))

names = [
    "UPPER.TXT", "lower.txt", "Mixed.Txt", "lowerbase.TXT", "UPPERBASE.txt",
    "with space.txt", "a.b.c.d", ".hidden", "Ünïcödé ✓.txt", "emoji 😀.bin",
    "x" * 250 + ".txt", "NOEXT", "lowerext", "12345678.123", "123456789.1234",
]
for seed, name in enumerate(names):
    put(name, seed * 3, seed)
for seed, size in enumerate([1, cluster - 1, cluster, cluster + 1, 3 * cluster - 1, 100_000]):
    put(f"size {size}.bin", size, 100 + seed)
deep = os.path.join(*"abcdefghij")
os.makedirs(os.path.join(root, deep))
put(os.path.join(deep, "bottom.txt"), 6, 200)
os.mkdir(os.path.join(root, "Many Files"))
for i in range(300):
    put(os.path.join("Many Files", f"file number {i:03}.dat"), i, i % 200)
for i in range(0, 300, 7):
    os.remove(os.path.join(root, "Many Files", f"file number {i:03}.dat"))
os.mkdir(os.path.join(root, "gone"))
put(os.path.join("gone", "inside.txt"), 10, 201)
os.remove(os.path.join(root, "gone", "inside.txt"))
os.rmdir(os.path.join(root, "gone"))
EOF
    sudo umount "$mnt"
    fsck.fat -n "$img" > /dev/null
}
kernel fat12 12 1 1440
kernel fat16 16 4 32768
kernel fat32 32 1 40960

# Volumes written by the kernel in other code pages: uppercase 8.3 names
# that the kernel stores as short names alone, in the code page's bytes.
skipped=0
codepage() {
    page=$1
    shift
    if ! modinfo "nls_cp$page" > /dev/null 2>&1; then
        echo "SKIPPED: codepage-$page (the kernel lacks nls_cp$page;" \
            "install linux-modules-extra-$(uname -r))"
        skipped=1
        return
    fi
    img="$work/codepage-$page.img"
    mkfs.fat -C -F 16 -s 4 -i 12345678 "$img" 32768 > /dev/null
    sudo mount -o "$options,codepage=$page" "$img" "$mnt"
    for name in "$@"; do
        printf '%s' "$name" > "$mnt/$name"
    done
    sudo umount "$mnt"
    fsck.fat -n "$img" > /dev/null
}
codepage 850 'ØRE.TXT' 'ÆBLE' 'ÅR.ÉTÉ' 'NIÑO.ÇÃ' 'ÜBER.ÖL'
codepage 866 'ПРИВЕТ.TXT' 'МИР' 'ЁЖ.ДОК' 'ЩУКА.ЯЯЯ'

# Prints what the kernel shows, in the manifest's format.
kernel_manifest() {
    python3 -I - "$1" <<'EOF'
import os, stat, subprocess, sys
root = sys.argv[1]
lines = []

def fnv1a(data):
    h = 0xCBF29CE484222325
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h

def walk(rel):
    # `listdir` returns entries in the order the directory holds them.
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        st = os.lstat(full)
        birth = subprocess.run(["stat", "-c", "%W", "--", full], check=True,
                               capture_output=True, text=True).stdout.strip()
        birth = "-" if birth == "0" else birth
        times = f"{int(st.st_mtime)}\t{birth}\t{int(st.st_atime)}"
        if stat.S_ISDIR(st.st_mode):
            lines.append(f"{path}\td\t-\t{times}\t-")
            walk(path)
        else:
            data = open(full, "rb").read()
            lines.append(f"{path}\tf\t{len(data)}\t{times}\t{fnv1a(data):016x}")

walk("")
print("\n".join(lines))
EOF
}

failed=0
for img in "$work"/*.img; do
    name=$(basename "$img" .img)
    page=437
    case $name in codepage-*) page=${name#codepage-} ;; esac
    sudo mount -o "ro,$options,codepage=$page" "$img" "$mnt"
    kernel_manifest "$mnt" > "$work/$name.kernel"
    sudo umount "$mnt"
    "$manifest" --code-page "$page" "$img" > "$work/$name.filer"
    if diff -u "$work/$name.kernel" "$work/$name.filer" > "$work/$name.diff"; then
        echo "same: $name ($(wc -l < "$work/$name.filer") entries)"
    else
        echo "DIFFERENT: $name"
        head -40 "$work/$name.diff"
        failed=1
    fi
done
# 1 for a difference, 2 if some case could not run.
if [ $failed -eq 0 ] && [ $skipped -ne 0 ]; then
    exit 2
fi
exit $failed
