<#
.SYNOPSIS
  Builds a realistic test disk: Windows itself formats it, fills it, ages it and deletes files.
  Run as administrator (attaching a virtual disk needs it).

.DESCRIPTION
  1. creates a fixed-size VHD (a raw disk image followed by a 512-byte footer: any carver reads it
     as is), puts one partition on it and formats it with Windows' own format;
  2. ages it: rounds of copies from a corpus of real files into nested folders, deletions and
     overwrites, so that files end up fragmented the way a used disk fragments them;
  3. before each deletion, records where the file lay (asked from the file system, on NTFS, FAT32
     and exFAT alike) with its hashes;
  4. applies the scenario, then lists the files still alive and detaches the disk.

  Writes <Out>\<Name>.vhd and <Out>\<Name>.tsv, the truth file read by `open-recovr --bench-score`.

  Scenarios:
    deleted      files deleted for good (Shift+Del, or the bin emptied) during the aging
    quickformat  the same, then the whole volume quick-formatted: every file is a deleted one
    camera       FAT32/exFAT card, photos and videos written in sequence, some deleted, then refilled
    fragmented   like deleted, but the free space is first cut into small holes (filler files of
                 random bytes, half of them deleted), so that the files written after it are split
                 the way they are on a full, much-used disk

  The first file described is read back from the raw volume to check its extents: the script
  stops if they do not match the disk.

.EXAMPLE
  .\scripts\bench\make-vhd.ps1 -Corpus D:\corpus -Out D:\bench -Name ntfs-aged -SizeMB 2048
.EXAMPLE
  .\scripts\bench\make-vhd.ps1 -Corpus D:\photos -Out D:\bench -Name sd-card -FileSystem exFAT -Scenario camera
#>
#Requires -RunAsAdministrator
param(
    # folders of real files (photos, documents, archives, music, videos...), read only; lists (.tsv) are skipped
    [Parameter(Mandatory)] [string[]] $Corpus,
    [Parameter(Mandatory)] [string] $Out,
    [string] $Name = 'bench',
    [int] $SizeMB = 1024,
    [ValidateSet('NTFS', 'exFAT', 'FAT32')] [string] $FileSystem = 'NTFS',
    [ValidateSet('deleted', 'quickformat', 'camera', 'fragmented')] [string] $Scenario = 'deleted',
    # allocation unit in bytes, 0 for Windows' default
    [int] $Cluster = 0,
    [int] $Rounds = 12,
    # how full the volume gets at most during the aging
    [double] $Fill = 0.85,
    [int] $Seed = 1
)
$ErrorActionPreference = 'Stop'
$rng = [Random]::new($Seed)
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$vhd = Join-Path $Out "$Name.vhd"
$truthPath = Join-Path $Out "$Name.tsv"
if (Test-Path $vhd) { throw "$vhd already exists" }

$files = @(Get-ChildItem -Recurse -File $Corpus | Where-Object { $_.Length -ge 2048 -and $_.Extension -ne '.tsv' })
if ($Scenario -eq 'camera') {
    $files = @($files | Where-Object { $_.Extension -match '^\.(jpe?g|png|heic|mp4|mov|avi|cr2|nef|arw|dng)$' })
}
if ($files.Count -lt 10) { throw "not enough files in $Corpus" }
Write-Host "$($files.Count) files in the corpus"

# Where a file lies, asked from the file system itself (FSCTL_GET_RETRIEVAL_POINTERS, which
# fsutil file queryextents also uses, but on NTFS only): NTFS, FAT32 and exFAT all answer it.
Add-Type @"
using System; using System.Collections.Generic; using System.ComponentModel; using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class Extents {
    [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern SafeFileHandle CreateFile(string name, uint access, uint share, IntPtr sa, uint disposition, uint flags, IntPtr template);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool DeviceIoControl(SafeFileHandle h, uint code, ref long inBuf, int inSize, byte[] outBuf, int outSize, out int got, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool ReadFile(SafeFileHandle h, byte[] buf, int len, out int got, IntPtr ov);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool SetFilePointerEx(SafeFileHandle h, long to, out long at, int how);
    [DllImport("kernel32.dll", SetLastError = true)]
    static extern bool FlushFileBuffers(SafeFileHandle h);

    static SafeFileHandle Open(string name, uint access) {
        var h = CreateFile(name, access, 7, IntPtr.Zero, 3, 0, IntPtr.Zero);
        if (h.IsInvalid) throw new Win32Exception(Marshal.GetLastWin32Error(), name);
        return h;
    }

    // the runs of a file in file order: { lcn, clusters }, lcn -1 for a hole
    public static List<long[]> Of(string path) {
        var runs = new List<long[]>();
        using (var h = Open(path, 0x80)) {
            long vcn = 0;
            var buf = new byte[64 * 1024];
            while (true) {
                int got;
                bool ok = DeviceIoControl(h, 0x90073, ref vcn, 8, buf, buf.Length, out got, IntPtr.Zero);
                int err = ok ? 0 : Marshal.GetLastWin32Error();
                if (err == 38) break; // nothing on disk: empty, or inside its MFT record
                if (!ok && err != 234) throw new Win32Exception(err, path);
                int count = BitConverter.ToInt32(buf, 0);
                long start = BitConverter.ToInt64(buf, 8);
                for (int i = 0; i < count; i++) {
                    long next = BitConverter.ToInt64(buf, 16 + 16 * i), lcn = BitConverter.ToInt64(buf, 24 + 16 * i);
                    runs.Add(new long[] { lcn, next - start });
                    start = next;
                }
                if (ok || count == 0) break;
                vcn = start;
            }
        }
        return runs;
    }

    static byte[] Sector(SafeFileHandle h, long at) {
        var b = new byte[512];
        long pos; int got;
        if (!SetFilePointerEx(h, at, out pos, 0) || !ReadFile(h, b, 512, out got, IntPtr.Zero) || got != 512)
            throw new Win32Exception(Marshal.GetLastWin32Error());
        return b;
    }

    // the byte of the volume where cluster (LCN) 0 starts
    public static long DataStart(char letter, string fs) {
        if (fs == "NTFS") return 0;
        using (var h = Open(@"\\.\" + letter + ":", 0x80000000)) {
            var b = Sector(h, 0);
            long bps = BitConverter.ToUInt16(b, 11);
            if (fs == "exFAT") return (long)BitConverter.ToUInt32(b, 88) << b[108];
            long fat = BitConverter.ToUInt16(b, 22);
            if (fat == 0) fat = BitConverter.ToUInt32(b, 36);
            long root = (BitConverter.ToUInt16(b, 17) * 32 + bps - 1) / bps;
            return (BitConverter.ToUInt16(b, 14) + b[16] * fat + root) * bps;
        }
    }

    // a sector of the volume, once the file's data is written out
    public static byte[] Read(char letter, string path, long at) {
        using (var f = Open(path, 0x40000000)) { FlushFileBuffers(f); }
        using (var h = Open(@"\\.\" + letter + ":", 0x80000000)) { return Sector(h, at); }
    }
}
"@

# --- the disk -----------------------------------------------------------------------------
$dp = [IO.Path]::GetTempFileName()
"create vdisk file=`"$vhd`" maximum=$SizeMB type=fixed" | Out-File -Encoding ascii $dp
diskpart /s $dp | Out-Null
Remove-Item $dp
$disk = Mount-DiskImage -ImagePath $vhd -PassThru | Get-Disk
try {
    Initialize-Disk -Number $disk.Number -PartitionStyle MBR
    # no letter until formatted, or Explorer pops up "you must format the disk"
    $part = New-Partition -DiskNumber $disk.Number -UseMaximumSize
    $fmt = @{ FileSystem = $FileSystem; NewFileSystemLabel = 'BENCH'; Confirm = $false; Force = $true }
    if ($Cluster -gt 0) { $fmt.AllocationUnitSize = $Cluster }
    $vol = $part | Format-Volume @fmt
    $part | Add-PartitionAccessPath -AssignDriveLetter
    $part = Get-Partition -DiskNumber $disk.Number -PartitionNumber $part.PartitionNumber
    $root = "$($part.DriveLetter):\"
    $cl = $vol.AllocationUnitSize
    $base = $part.Offset
    Write-Host "$FileSystem on $root, cluster $cl, partition at byte $base"

    # Where the clusters of the volume start: cluster numbers (LCN) count from the start of the
    # volume on NTFS, from the start of the data area on FAT32 and exFAT (its first cluster, number 2)
    $data = [Extents]::DataStart($part.DriveLetter, $FileSystem)
    $checked = $false

    $sha = [Security.Cryptography.SHA256]::Create()
    function Hex($b) { -join ($b | ForEach-Object { $_.ToString('x2') }) }
    function Describe($path, $state) {
        $bytes = [IO.File]::ReadAllBytes($path)
        $head = if ($bytes.Length -gt 4096) { $sha.ComputeHash($bytes, 0, 4096) } else { $sha.ComputeHash($bytes) }
        $ext = '-'; $pieces = 1
        # the runs of the file as the file system lists them (lcn, clusters), holes left out
        $runs = [Collections.Generic.List[object]]::new()
        foreach ($r in [Extents]::Of($path)) { if ($r[0] -ge 0) { $runs.Add($r) } }
        if ($runs.Count -gt 0) {
            # once, the first cluster read back from the volume itself must be the file's
            if (-not $script:checked -and $bytes.Length -ge 512) {
                $raw = [Extents]::Read($part.DriveLetter, $path, $data + $runs[0][0] * $cl)
                for ($i = 0; $i -lt 512; $i++) { if ($raw[$i] -ne $bytes[$i]) { throw "the extents of $path do not match the disk" } }
                $script:checked = $true
            }
            # runs that follow each other on disk are one piece
            $merged = @(); foreach ($r in $runs) {
                if ($merged.Count -and $merged[-1][0] + $merged[-1][1] -eq $r[0]) { $merged[-1][1] += $r[1] } else { $merged += , @($r[0], $r[1]) }
            }
            $pieces = $merged.Count
            $ext = ($merged | ForEach-Object { "$($base + $data + $_[0] * $cl)+$($_[1] * $cl)" }) -join ','
        } elseif ($FileSystem -eq 'NTFS') { $pieces = 0 } # small enough to live inside its MFT record
        @((Hex $sha.ComputeHash($bytes)), (Hex $head), $bytes.Length, $state, $pieces, $ext, $path.Substring(3)) -join "`t"
    }

    $truth = [Collections.Generic.List[string]]::new()
    $capacity = $vol.Size
    # read from the file system, not Get-Volume: that one asks the storage service, which takes
    # seconds per call (more when a disk of the machine is failing) and is called for every file
    $drive = [IO.DriveInfo]::new($root)
    function Used { $drive.TotalSize - $drive.TotalFreeSpace }
    function Copy-Some($share) {
        $dirs = @('DCIM\100CAMERA', 'Documents', 'Documents\Projets', 'Musique', 'Videos', 'Downloads', 'Archives\2024')
        while ((Used) -lt $capacity * $share) {
            $f = $files[$rng.Next($files.Count)]
            if ((Used) + $f.Length -gt $capacity * $Fill) { break }
            $d = if ($Scenario -eq 'camera') { Join-Path $root 'DCIM\100CAMERA' } else { Join-Path $root $dirs[$rng.Next($dirs.Count)] }
            New-Item -ItemType Directory -Force $d | Out-Null
            $dst = Join-Path $d ("{0:D5}_{1}" -f $rng.Next(100000), $f.Name)
            if (-not (Test-Path $dst)) { Copy-Item $f.FullName $dst }
        }
    }
    # filler: random bytes, not part of the truth (no carver should return it, and if one does it is junk)
    $fillerDir = Join-Path $root 'Temp\cache'
    function Real-Files {
        Get-ChildItem -Recurse -File $root -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -notmatch 'System Volume Information' -and -not $_.FullName.StartsWith($fillerDir) }
    }
    function Chop-Free($share) {
        New-Item -ItemType Directory -Force $fillerDir | Out-Null
        $buf = [byte[]]::new(64 * $cl)
        $n = 0
        while ((Used) -lt $capacity * $share) {
            $rng.NextBytes($buf)
            $len = (1 + $rng.Next(64)) * $cl
            $fs = [IO.File]::Create((Join-Path $fillerDir ('{0:D6}.tmp' -f $n)))
            $fs.Write($buf, 0, $len); $fs.Close(); $n++
        }
        # every other hole freed, at random: the free space is now many small pieces
        foreach ($f in Get-ChildItem -File $fillerDir) { if ($rng.NextDouble() -lt 0.5) { Remove-Item -Force $f.FullName } }
        Write-Host ("free space cut by {0} filler files, {1:N0} MB used" -f $n, ((Used) / 1MB))
    }
    function Delete-Some($share) {
        $live = @(Real-Files)
        foreach ($f in $live) {
            if ($rng.NextDouble() -lt $share) {
                $truth.Add((Describe $f.FullName 'deleted'))
                Remove-Item -Force $f.FullName
            }
        }
    }

    # --- the aging ------------------------------------------------------------------------
    if ($Scenario -eq 'fragmented') { Chop-Free 0.9 }
    for ($r = 1; $r -le $Rounds; $r++) {
        Copy-Some ([math]::Min($Fill, 0.3 + $r * ($Fill - 0.3) / $Rounds))
        Delete-Some 0.25
        Write-Host ("round {0}/{1}: {2:N0} MB used, {3} files deleted so far" -f $r, $Rounds, ((Used) / 1MB), $truth.Count)
    }
    # the last files deleted are the ones a user wants back: nothing has been written since
    Delete-Some 0.15

    $live = @(Real-Files)
    if ($Scenario -eq 'quickformat') {
        foreach ($f in $live) { $truth.Add((Describe $f.FullName 'deleted')) }
        Format-Volume -DriveLetter $part.DriveLetter -FileSystem $FileSystem -NewFileSystemLabel 'BENCH' -Confirm:$false -Force | Out-Null
    } else {
        foreach ($f in $live) { $truth.Add((Describe $f.FullName 'live')) }
    }

    $header = @(
        "# $Name : $FileSystem, cluster $cl, partition at byte $base, $SizeMB MB, scenario $Scenario, $Rounds rounds, seed $Seed",
        "# corpus: $($files.Count) files from $Corpus",
        "# sha256`thead`tsize`tstate`tpieces`textents`tname"
    )
    ($header + $truth) | Out-File -Encoding utf8 $truthPath
    Write-Host "$($truth.Count) files in the truth: $truthPath"
} finally {
    Dismount-DiskImage -ImagePath $vhd | Out-Null
}
Write-Host "image: $vhd"
