#!/bin/sh
# This is free and unencumbered software released into the public domain.
#
# Makes the Windows fixtures with Windows itself: `windows/make.ps1` runs
# in Windows 11 in a container (dockur/windows, under KVM), formats and
# fills each volume, and writes Windows' own manifest of it. This keeps,
# for each, `<name>.sparse` (the volume, without the VHD's partition
# table and footer, in `make.sh`'s sparse format) and `<name>.manifest`.
#
# The first run starts the container `filer-windows`, which downloads and
# installs Windows (half an hour or more); `windows/install.bat` then
# registers `windows/runner.ps1`, which runs each job put in the shared
# folder. Later runs reuse the container. Its state is kept in
# `$FILER_WINDOWS` (`~/.cache/filer-windows` by default).
#
# Usage: tests/fixtures/make-windows.sh [output-dir [fixture...]], on Linux
# with Docker, KVM (about 4 GB of memory and 40 GB of disk for Windows),
# and python3.

set -eu
here=$(cd "$(dirname "$0")" && pwd)
out=$(cd "${1:-$here}" && pwd)
[ $# -gt 0 ] && shift
only=$(printf '%s,' "$@" | sed 's/,$//')
state=${FILER_WINDOWS:-$HOME/.cache/filer-windows}
shared="$state/shared"
mkdir -p "$state/storage" "$shared"

if ! docker ps --format '{{.Names}}' | grep -qx filer-windows; then
    docker start filer-windows 2>/dev/null || docker run -d --name filer-windows \
        -e VERSION=11 -e RAM_SIZE=4G -e CPU_CORES=3 -e DISK_SIZE=40G \
        -p 127.0.0.1:8006:8006 --device=/dev/kvm --device=/dev/net/tun \
        --cap-add NET_ADMIN --stop-timeout 120 \
        -v "$state/storage:/storage" -v "$here/windows:/oem:ro" \
        -v "$shared:/shared" docker.io/dockurr/windows
fi

# The job: `make.ps1`, with the byte-order mark Windows PowerShell needs.
rm -rf "$shared/out" "$shared"/job.*
mkdir "$shared/out"
{ printf '\357\273\277'; cat "$here/windows/make.ps1"; } > "$shared/make.ps1"
args='-Out "$share\out"'
[ -n "$only" ] && args="$args -Only $only"
{
    printf '$share = if (Test-Path Z:\\make.ps1) { "Z:" } else { "\\\\host.lan\\Data" }\r\n'
    printf '& "$share\\make.ps1" %s\r\nexit $LASTEXITCODE\r\n' "$args"
} > "$shared/job.tmp"
mv "$shared/job.tmp" "$shared/job.ps1"
echo "waiting for Windows to run the job (see http://127.0.0.1:8006/)"
while [ ! -e "$shared/job.done" ]; do sleep 10; done
cat "$shared/job.log" 2>/dev/null || true
[ "$(tr -d '\r\n' < "$shared/job.done")" = 0 ] || { echo "the job failed"; exit 1; }

for vhd in "$shared"/out/*.vhd; do
    name=$(basename "$vhd" .vhd)
    python3 -I - "$vhd" "$out/$name.sparse" <<'PY'
import struct, sys
data = open(sys.argv[1], "rb").read()
# A fixed VHD is the disk, then a 512-byte footer; the disk has an MBR
# with one partition, the volume.
assert data[-512:-504] == b"conectix", "not a VHD"
mbr = data[:512]
assert mbr[510:512] == b"\x55\xaa", "no MBR"
start, count = struct.unpack("<II", mbr[0x1C6:0x1CE])
volume = data[start * 512:(start + count) * 512]
with open(sys.argv[2], "wb") as out:
    out.write(b"FILER-SPARSE-2\n" + struct.pack("<Q", len(volume)))
    sectors = len(volume) // 512
    i = 0
    while i < sectors:
        sector = volume[i * 512:(i + 1) * 512]
        n = 1
        while i + n < sectors and volume[(i + n) * 512:(i + n + 1) * 512] == sector:
            n += 1
        if any(sector):
            out.write(struct.pack("<II", i, n) + sector)
        i += n
PY
    cp "$shared/out/$name.manifest" "$out/$name.manifest"
    echo "made $name"
done
