# This is free and unencumbered software released into the public domain.
#
# What `tests/differential.sh` and `tests/external.sh` share, sourced by
# them: printing what the Linux kernel shows of an image in the `manifest`
# example's format, and comparing the two; and running comparisons at
# once. They set `work`, a directory of their own; `mnt`, a mount point in
# it; `manifest`, the example; and `failed`, set to 1 by any difference.
# Comparisons keep their files in `scratch`, or in `work` if it is unset,
# so that several can run at once, each with a `scratch` and `mnt` of its
# own. If `FILER_XFS_KEEP` names a directory, the image of each comparison
# that fails is kept there.

# Prints what the Linux kernel shows, in the manifest's format.
kernel_manifest() {
    # As root, as the fixtures' files are root's.
    sudo python3 -I - "$1" <<'PY'
import ctypes, fcntl, os, re, stat, struct, sys
root = os.fsencode(sys.argv[1])
libc = ctypes.CDLL(None, use_errno=True)
AT_FDCWD, AT_SYMLINK_NOFOLLOW, STATX_ALL = -100, 0x100, 0xFFF
STATX_BTIME = 0x800

def birth(path):
    buf = ctypes.create_string_buffer(256)
    if libc.statx(AT_FDCWD, path, AT_SYMLINK_NOFOLLOW, STATX_ALL, buf) != 0:
        raise OSError(ctypes.get_errno(), "statx")
    mask, = struct.unpack_from("<I", buf.raw, 0)
    if not mask & STATX_BTIME:
        return "-"
    secs, nanos = struct.unpack_from("<qI", buf.raw, 0x50)
    return f"{secs}.{nanos:09}"

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

# The inode flags Filer reports, by `FS_IOC_GETFLAGS` bit.
FLAGS = [(0x4, "compressed"), (0x10, "immutable"), (0x20, "append_only"),
         (0x800, "encrypted"), (0x100000, "verity")]
FS_IOC_GETFLAGS = 0x80086601

def joined(items):
    return ",".join(sorted(items)) or "-"

def flags(path):
    fd = os.open(path, os.O_RDONLY | os.O_NONBLOCK)
    try:
        bits, = struct.unpack_from("<I", fcntl.ioctl(fd, FS_IOC_GETFLAGS, bytes(8)))
    finally:
        os.close(fd)
    return joined(name for bit, name in FLAGS if bits & bit)

def xattrs(path):
    return joined(f"{shown(name.encode('utf-8', 'surrogateescape'))}="
                  f"{fnv1a(os.getxattr(path, name)):016x}"
                  for name in os.listxattr(path))

def time(ns):
    return f"{ns // 10**9}.{ns % 10**9:09}"

KINDS = [(stat.S_ISDIR, "d"), (stat.S_ISLNK, "l"), (stat.S_ISFIFO, "p"),
         (stat.S_ISSOCK, "s"), (stat.S_ISCHR, "c"), (stat.S_ISBLK, "b")]

def walk(rel, rel_shown):
    # `listdir` returns entries in the order the directory holds them.
    for name in os.listdir(os.path.join(root, rel)):
        path = os.path.join(rel, name)
        full = os.path.join(root, path)
        path_shown = shown(name) if not rel else f"{rel_shown}/{shown(name)}"
        st = os.lstat(full)
        kind = next((k for test, k in KINDS if test(st.st_mode)), "f")
        times = "\t".join([time(st.st_mtime_ns), time(st.st_ctime_ns),
                           time(st.st_atime_ns), birth(full)])
        foreign = path_shown != path.decode("utf-8", "replace")
        if foreign or kind in "pscb":
            what = "-"
        elif kind == "l":
            what = os.readlink(full).decode("utf-8", "replace")
        elif kind == "d":
            what = "-"
        elif st.st_size > 1 << 30:
            # As the manifest: too large to hash.
            what = "-"
        else:
            what = f"{fnv1a(open(full, 'rb').read()):016x}"
        attributes = "-" if foreign or kind not in "fd" else xattrs(full)
        bits = flags(full) if kind in "fd" else "-"
        print(f"{path_shown}\t{kind}\t{st.st_size}\t{times}\t{what}\t{bits}\t{attributes}")
        if kind == "d" and not foreign:
            walk(path, path_shown)

walk(b"", "")
PY
}

# Compares the manifest of image `$1`, called `$2`, or of image `$3` if
# given, with what the kernel shows of `$1`.
compare() {
    files=${scratch:-$work}
    if ! sudo mount -o loop,ro "$1" "$mnt" 2>/dev/null; then
        echo "skipped: $2 (Linux cannot mount it here)"
        return
    fi
    kernel_manifest "$mnt" | sort > "$files/kernel"
    sudo umount "$mnt"
    if ! "$manifest" "${3:-$1}" > "$files/filer" 2> "$files/error"; then
        echo "FAILED: $2: $(cat "$files/error")"
        keep "${3:-$1}" "$2"
        failed=1
        return
    fi
    sort -o "$files/filer" "$files/filer"
    if diff -u "$files/kernel" "$files/filer" > "$files/diff"; then
        echo "same: $2 ($(wc -l < "$files/filer") entries)"
    else
        echo "DIFFERENT: $2"
        head -40 "$files/diff"
        keep "${3:-$1}" "$2"
        failed=1
    fi
}

# Keeps a copy of image `$1`, called `$2`, in `FILER_XFS_KEEP`, if set.
keep() {
    [ -n "${FILER_XFS_KEEP:-}" ] || return 0
    mkdir -p "$FILER_XFS_KEEP"
    kept="$FILER_XFS_KEEP/$(echo "$2" | tr -c 'A-Za-z0-9\n' -).img"
    cp --sparse=always "$1" "$kept"
    echo "kept: $kept"
}

# Compares the manifest of image `$1`, whose log needs recovery, called
# `$2`, with what the kernel shows of a copy it recovered.
recovered() {
    files=${scratch:-$work}
    cp "$1" "$files/recovered.img"
    sudo mount -o loop "$files/recovered.img" "$mnt"
    sudo umount "$mnt"
    compare "$files/recovered.img" "$2" "$1"
}

# Jobs: a script lists them with `add`, and runs them with `run`, which
# runs the script again as `$0 --job N` for each, as many at once as there
# are cores (or `FILER_XFS_JOBS`), then prints their reports in order. Run
# so, the script sets `work` and `manifest` from `FILER_XFS_WORK` and
# `FILER_XFS_MANIFEST` and calls `job`, which calls the script's `perform`
# with the job's kind and arguments.
tab=$(printf '\t')

# Adds a job of kind `$1`, with arguments `$2` and `$3`, to the list.
add() {
    printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$work/jobs"
}

# Runs the jobs listed, then prints their reports in order.
run() {
    mkdir "$work/out"
    export FILER_XFS_WORK="$work" FILER_XFS_MANIFEST="$manifest"
    count=$(wc -l < "$work/jobs")
    seq "$count" | xargs -P "${FILER_XFS_JOBS:-$(nproc)}" -n 1 sh "$0" --job || failed=1
    for n in $(seq "$count"); do
        cat "$work/out/$n"
    done
    rm -r "$work/jobs" "$work/out"
}

# Runs job `$1` of the list, with a `scratch` and `mnt` of its own: its
# report goes to `$work/out/$1`, and it fails if anything differs.
job() {
    exec > "$work/out/$1" 2>&1
    scratch=$(mktemp -d -p "$work")
    mnt="$scratch/mnt"
    mkdir "$mnt"
    trap 'if mountpoint -q "$mnt"; then sudo umount "$mnt"; fi; sudo rm -rf "$scratch"' EXIT
    failed=0
    spec=$(sed -n "$1p" "$work/jobs")
    kind=${spec%%"$tab"*}
    rest=${spec#*"$tab"}
    perform "$kind" "${rest%%"$tab"*}" "${rest#*"$tab"}"
    exit $failed
}

# Unmounts what jobs left mounted in `$work`, and removes it.
clean_work() {
    for mounted in "$work"/mnt "$work"/*/mnt; do
        if mountpoint -q "$mounted"; then sudo umount "$mounted"; fi
    done
    sudo rm -rf "$work"
}
