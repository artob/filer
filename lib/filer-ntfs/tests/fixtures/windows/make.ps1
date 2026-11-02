# This is free and unencumbered software released into the public domain.
#
# Makes the Windows fixtures: Windows itself formats each volume (a fixed
# VHD with one partition) and fills it, then writes its own manifest of
# it, in the format of the `manifest` example, but for access times
# (which reading updates) and in Windows' listing order. Run elevated, on
# Windows 10 or later with EFS (Pro or better); `make-windows.sh` runs it
# in a Windows container and keeps the results.
#
# Usage: make.ps1 -Out OUTPUT-DIR [-Only NAME,...], making each fixture
# named, or all. It is kept as UTF-8 without a byte-order mark, which
# Windows PowerShell needs to read it as UTF-8: `make-windows.sh` adds
# one.

param(
    [Parameter(Mandatory = $true)][string]$Out,
    [string[]]$Only = @()
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version 3

Add-Type -TypeDefinition @"
using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

// Named data streams, of files and directories, through the Win32 API:
// .NET Framework refuses paths with a stream name.
public static class Ads {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct FindStreamData {
        public long StreamSize;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 296)]
        public string StreamName;
    }

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern IntPtr FindFirstStreamW(string path, int level, out FindStreamData data, int flags);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool FindNextStreamW(IntPtr find, out FindStreamData data);
    [DllImport("kernel32.dll")]
    static extern bool FindClose(IntPtr find);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern SafeFileHandle CreateFileW(string path, uint access, uint share,
        IntPtr security, uint disposition, uint flags, IntPtr template);

    const uint BackupSemantics = 0x02000000;

    // The names of the named streams of a file or directory.
    public static List<string> Names(string path) {
        var names = new List<string>();
        FindStreamData data;
        IntPtr find = FindFirstStreamW(path, 0, out data, 0);
        if (find == new IntPtr(-1)) {
            return names;
        }
        try {
            do {
                // ":name:" and the type, or just ":" and the type for the
                // unnamed stream.
                string name = data.StreamName;
                if (name != "::`$DATA") {
                    names.Add(name.Substring(1, name.Length - 7));
                }
            } while (FindNextStreamW(find, out data));
        } finally {
            FindClose(find);
        }
        return names;
    }

    static FileStream Open(string path, string name, uint access, uint disposition) {
        SafeFileHandle handle = CreateFileW(path + ":" + name, access, 7, IntPtr.Zero,
            disposition, BackupSemantics, IntPtr.Zero);
        if (handle.IsInvalid) {
            throw new Win32Exception(Marshal.GetLastWin32Error(), path + ":" + name);
        }
        return new FileStream(handle, access == 0x80000000 ? FileAccess.Read : FileAccess.Write);
    }

    public static byte[] Read(string path, string name) {
        using (FileStream stream = Open(path, name, 0x80000000, 3)) {
            var bytes = new MemoryStream();
            stream.CopyTo(bytes);
            return bytes.ToArray();
        }
    }

    public static void Write(string path, string name, byte[] bytes) {
        using (FileStream stream = Open(path, name, 0x40000000, 2)) {
            stream.Write(bytes, 0, bytes.Length);
        }
    }
}

public static class Fnv {
    public static string Hash(byte[] bytes) {
        ulong hash = 0xcbf29ce484222325;
        unchecked {
            foreach (byte b in bytes) {
                hash = (hash ^ b) * 0x100000001b3;
            }
        }
        return hash.ToString("x16");
    }
}
"@

$MTIME = [DateTime]::new(2024, 1, 2, 3, 4, 6, [DateTimeKind]::Utc)
$BTIME = [DateTime]::new(2023, 5, 6, 7, 8, 9, [DateTimeKind]::Utc).AddTicks(1234567)
$Letter = "X"
$Root = "${Letter}:\"

function Pattern([int]$Size, [int]$Seed) {
    $bytes = New-Object byte[] $Size
    for ($i = 0; $i -lt $Size; $i++) { $bytes[$i] = [byte](($i * 7 + $Seed) % 251) }
    return , $bytes
}

function Noise([int]$Size, [uint32]$Seed) {
    # Xorshift32, as `tests/common` has it.
    $bytes = New-Object byte[] $Size
    [uint64]$x = $Seed
    # Not `0xFFFFFFFF`, which Windows PowerShell reads as -1.
    [uint64]$mask = 4294967295
    for ($i = 0; $i -lt $Size; $i++) {
        $x = ($x -bxor ($x -shl 13)) -band $mask
        $x = ($x -bxor ($x -shr 17)) -band $mask
        $x = ($x -bxor ($x -shl 5)) -band $mask
        $bytes[$i] = [byte]($x -band 0xFF)
    }
    return , $bytes
}

function Put([string]$Path, [byte[]]$Bytes) {
    $full = Join-Path $Root $Path
    [IO.File]::WriteAllBytes($full, $Bytes)
    Stamp $full
}

function Stamp([string]$Full) {
    $item = Get-Item -Force -LiteralPath $Full
    $item.CreationTimeUtc = $BTIME
    $item.LastWriteTimeUtc = $MTIME
}

function Stream([string]$Path, [string]$Name, [byte[]]$Bytes) {
    [Ads]::Write((Join-Path $Root $Path), $Name, $Bytes)
}

function Run([string]$Command) {
    $output = cmd /c $Command 2>&1
    if ($LASTEXITCODE -ne 0) { throw "$Command failed: $output" }
}

function Invoke-Diskpart([string[]]$Lines) {
    $script = Join-Path $env:TEMP "diskpart.txt"
    Set-Content -Path $script -Value $Lines -Encoding Ascii
    $output = diskpart /s $script
    if ($LASTEXITCODE -ne 0) { throw "diskpart failed: $output" }
}

# The tree every layout holds; compressed files only where clusters are
# small enough for NTFS to compress (up to 4 KiB).
function Fill-Basic([bool]$Compress) {
    Put "README.TXT" (Pattern 9 1)
    Put "notes.txt" (Pattern 10 2)
    Put "A long file name with spaces.txt" (Pattern 100 3)
    Put "Ünïcödé ✓.txt" (Pattern 7 4)
    Put "emoji 😀.bin" (Pattern 5 5)
    Put "empty" (Pattern 0 6)
    Put "big.bin" (Pattern 300000 7)
    New-Item -ItemType Directory (Join-Path $Root "Sub\Deeper") | Out-Null
    Put "Sub\nested.bin" (Pattern 5000 8)
    Put "Sub\Deeper\leaf.txt" (Pattern 5 9)
    # A long name, with the DOS (8.3) name Windows makes for it.
    Put "Long name for DOS.txt" (Pattern 6 13)
    # Two files grown in turns, so that both fragment.
    $a = [IO.File]::Open((Join-Path $Root "frag.a"), "Create", "Write")
    $b = [IO.File]::Open((Join-Path $Root "frag.b"), "Create", "Write")
    for ($i = 0; $i -lt 12; $i++) {
        $chunk = Pattern 8192 (20 + $i); $a.Write($chunk, 0, $chunk.Length); $a.Flush($true)
        $chunk = Pattern 8192 (40 + $i); $b.Write($chunk, 0, $chunk.Length); $b.Flush($true)
    }
    $a.Close(); $b.Close()
    Stamp (Join-Path $Root "frag.a"); Stamp (Join-Path $Root "frag.b")
    # A sparse file: a megabyte of zeros, then three bytes.
    $sparse = Join-Path $Root "sparse.bin"
    [IO.File]::WriteAllBytes($sparse, [byte[]]@())
    Run "fsutil sparse setflag `"$sparse`""
    $stream = [IO.File]::Open($sparse, "Open", "Write")
    $stream.SetLength(1000003)
    $stream.Seek(1000000, "Begin") | Out-Null
    $stream.Write([byte[]](0x65, 0x6E, 0x64), 0, 3)
    $stream.Close()
    Run "fsutil sparse setrange `"$sparse`" 0 1000000"
    Stamp $sparse
    # Hard links.
    Put "small" (Pattern 4 10)
    New-Item -ItemType HardLink -Path (Join-Path $Root "Sub\hardlink") -Target (Join-Path $Root "small") | Out-Null
    # Named streams, on a file and a directory, small and large.
    Stream "README.TXT" "extra" ([Text.Encoding]::ASCII.GetBytes("stream data"))
    Stream "notes.txt" "Zone.Identifier" ([Text.Encoding]::ASCII.GetBytes("[ZoneTransfer]`r`nZoneId=3`r`n"))
    Stream "notes.txt" "large" (Pattern 200000 11)
    Stream "Sub" "tag" ([Text.Encoding]::ASCII.GetBytes("dir"))
    # File attributes.
    Put "hidden.txt" (Pattern 3 14)
    Put "system.txt" (Pattern 3 15)
    Put "readonly.txt" (Pattern 3 16)
    Put "plain.txt" (Pattern 3 17)
    Put "offline.txt" (Pattern 3 18)
    (Get-Item -Force (Join-Path $Root "hidden.txt")).Attributes = "Hidden, Archive"
    (Get-Item -Force (Join-Path $Root "system.txt")).Attributes = "System"
    (Get-Item -Force (Join-Path $Root "readonly.txt")).Attributes = "ReadOnly, NotContentIndexed"
    (Get-Item -Force (Join-Path $Root "plain.txt")).Attributes = "Normal"
    (Get-Item -Force (Join-Path $Root "offline.txt")).Attributes = "Offline"
    # Compression, inherited from a directory: compressible,
    # incompressible, sparse, exactly one unit, and mixed.
    if ($Compress) {
        $dir = Join-Path $Root "Compressed"
        New-Item -ItemType Directory $dir | Out-Null
        Run "compact /c /q `"$dir`""
        Put "Compressed\text.txt" (Pattern 200000 1)
        Put "Compressed\noise.bin" (Noise 100000 7)
        Put "Compressed\exact.bin" (Pattern 65536 3)
        Put "Compressed\mixed.bin" ((Noise 30000 9) + (New-Object byte[] 70000) + (Pattern 50000 4))
        Put "Compressed\small.txt" (Pattern 5 2)
    }
    # Links: relative, to a directory, absolute on the drive, a junction,
    # and dangling.
    Put "target.txt" ([Text.Encoding]::ASCII.GetBytes("target"))
    Push-Location $Root
    Run "mklink link target.txt"
    Run "mklink deep Sub\Deeper\leaf.txt"
    Run "mklink /D dirlink Sub"
    Run "mklink abs ${Letter}:\target.txt"
    Run "mklink /J junction ${Letter}:\Sub"
    Run "mklink dangling nope"
    Pop-Location
    # Deleted entries.
    Put "gone.txt" (Pattern 512 19)
    New-Item -ItemType Directory (Join-Path $Root "GoneDir") | Out-Null
    Remove-Item (Join-Path $Root "gone.txt")
    Remove-Item (Join-Path $Root "GoneDir")
}

# What only the main layout holds.
function Fill-More {
    # A directory whose index has more than one level, and names that
    # sort differently by case, accent, and punctuation.
    $dir = Join-Path $Root "many"
    New-Item -ItemType Directory $dir | Out-Null
    for ($i = 0; $i -lt 1000; $i++) {
        Put ("many\f{0:d4}" -f $i) (Pattern ($i % 64) $i)
    }
    $dir = Join-Path $Root "collation"
    New-Item -ItemType Directory $dir | Out-Null
    $names = @("a", "B", "c", "Ä", "ä0", "Z", "_", "-x", "~", "1", "10", "9",
        "É", "e", "f", "ß", "SS", "Ω", "ω1", "ǅ", "ﬃ", "İ", "i2", "ı3")
    $seed = 0
    foreach ($name in $names) {
        $seed++
        Put "collation\$name" (Pattern 1 $seed)
    }
    # More streams than one record holds, listed in `$ATTRIBUTE_LIST`.
    Put "many-streams.bin" ([Text.Encoding]::ASCII.GetBytes("many"))
    for ($i = 0; $i -lt 100; $i++) {
        Stream "many-streams.bin" ("s{0:d3}" -f $i) (Pattern 300 $i)
    }
    # Many hard links, which also need `$ATTRIBUTE_LIST`.
    New-Item -ItemType Directory (Join-Path $Root "links") | Out-Null
    Put "links\0" (Pattern 7 20)
    for ($i = 1; $i -lt 40; $i++) {
        New-Item -ItemType HardLink -Path (Join-Path $Root "links\$i") -Target (Join-Path $Root "links\0") | Out-Null
    }
    # Per-directory case sensitivity, where Windows has it.
    $dir = Join-Path $Root "case"
    New-Item -ItemType Directory $dir | Out-Null
    cmd /c "fsutil file setCaseSensitiveInfo `"$dir`" enable" | Out-Null
    if ($LASTEXITCODE -eq 0) {
        Put "case\Case.txt" (Pattern 3 11)
        Put "case\case.txt" (Pattern 4 12)
    }
    # An encrypted file (EFS), and one compressed by WOF (XPRESS), both of
    # which Filer refuses to read.
    Put "secret.txt" (Pattern 1000 21)
    Run "cipher /e /a /q `"$(Join-Path $Root 'secret.txt')`""
    Put "wof.bin" (Pattern 100000 22)
    Run "compact /c /q /exe:xpress4k `"$(Join-Path $Root 'wof.bin')`""
}

# Windows' view of everything below `$Rel` (a path with `/`), in the
# order Windows lists it: its index's, unlike `Get-ChildItem`, which lists
# directories first.
function Manifest([string]$Rel, [System.Collections.Generic.List[string]]$Lines) {
    $dir = [IO.DirectoryInfo]::new((Join-Path $Root $Rel.Replace("/", "\")))
    foreach ($item in $dir.EnumerateFileSystemInfos()) {
        $path = if ($Rel) { "$Rel/$($item.Name)" } else { $item.Name }
        $attrs = [int]$item.Attributes
        $flags = @()
        foreach ($pair in @(@(0x2, "hidden"), @(0x4, "system"), @(0x20, "archive"),
                @(0x200, "sparse"), @(0x800, "compressed"), @(0x1000, "offline"),
                @(0x4000, "encrypted"), @(0x440000, "dataless"))) {
            if ($attrs -band $pair[0]) { $flags += $pair[1] }
        }
        $flags = if ($flags) { ($flags | Sort-Object -CaseSensitive) -join "," } else { "-" }
        $mtime = [DateTimeOffset]::new($item.LastWriteTimeUtc).ToUnixTimeSeconds()
        $btime = [DateTimeOffset]::new($item.CreationTimeUtc).ToUnixTimeSeconds()
        $times = "$mtime`t$btime`t-"
        if ($item.LinkType -in @("SymbolicLink", "Junction")) {
            # As Filer maps targets: `\` as `/`, and a path on the drive
            # from the volume's root.
            $target = @($item.Target)[0] -replace "\\", "/"
            $target = $target -replace "^[A-Za-z]:/", "/"
            $Lines.Add("$path`tl`t$($target.Length)`t$times`t$target`t$flags`t-")
            continue
        }
        $pairs = @()
        foreach ($name in [Ads]::Names($item.FullName)) {
            $pairs += "user.$name=$([Fnv]::Hash([Ads]::Read($item.FullName, $name)))"
        }
        $xattrs = if ($pairs) { ($pairs | Sort-Object -CaseSensitive) -join "," } else { "-" }
        if ($item -is [IO.DirectoryInfo]) {
            $Lines.Add("$path`td`t-`t$times`t-`t$flags`t$xattrs")
            # Windows' own, which even administrators may not list.
            if ($path -eq "System Volume Information") { continue }
            Manifest $path $Lines
        } else {
            $hash = [Fnv]::Hash([IO.File]::ReadAllBytes($item.FullName))
            $Lines.Add("$path`tf`t$($item.Length)`t$times`t$hash`t$flags`t$xattrs")
        }
    }
}

# Makes fixture `$Name`, of `$SizeMB` megabytes, with clusters of `$Unit`.
function Make([string]$Name, [int]$SizeMB, [string]$Unit, [bool]$More) {
    if ($Only.Count -and $Only -notcontains $Name) { return }
    Write-Output "== $Name"
    $vhd = "C:\fixtures\$Name.vhd"
    New-Item -ItemType Directory -Force "C:\fixtures" | Out-Null
    if (Test-Path $vhd) {
        # Left attached by a failed run.
        try { Invoke-Diskpart @("select vdisk file=$vhd", "detach vdisk") } catch {}
        Remove-Item -Force $vhd
    }
    Invoke-Diskpart @(
        "create vdisk file=$vhd maximum=$SizeMB type=fixed",
        "select vdisk file=$vhd", "attach vdisk", "convert mbr",
        # Aligned for clusters over 64 KiB.
        "create partition primary align=1024",
        "format fs=ntfs unit=$Unit label=FILER quick",
        "assign letter=$Letter"
    )
    try {
        Run "fsutil 8dot3name set ${Letter}: 0"
        Fill-Basic ($Unit -in @("512", "4096"))
        if ($More) { Fill-More }
        $lines = New-Object System.Collections.Generic.List[string]
        Manifest "" $lines
        $utf8 = New-Object Text.UTF8Encoding $false
        [IO.File]::WriteAllLines((Join-Path $Out "$Name.manifest"), $lines, $utf8)
    } finally {
        Invoke-Diskpart @("select vdisk file=$vhd", "detach vdisk")
    }
    Copy-Item -Force $vhd (Join-Path $Out "$Name.vhd")
    Remove-Item -Force $vhd
}

Make "windows-4k" 48 "4096" $true
Make "windows-512" 32 "512" $false
Make "windows-64k" 48 "64K" $false
Make "windows-128k" 64 "128K" $false
Write-Output "done"
