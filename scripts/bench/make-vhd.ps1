<#
.SYNOPSIS
  Builds a realistic test disk: Windows itself formats it, fills it, ages it and deletes files.
  Run as administrator (attaching a virtual disk needs it).

.DESCRIPTION
  1. creates a fixed-size VHD (a raw disk image followed by a 512-byte footer: any carver reads it
     as is), puts one partition on it and formats it with Windows' own format;
  2. ages it: rounds of copies from a corpus of real files into nested folders, deletions and
     overwrites, so that files end up fragmented the way a used disk fragments them;
  3. before each deletion, records where the file lay (fsutil file queryextents) with its hashes;
  4. applies the scenario, then lists the files still alive and detaches the disk.

  Writes <Out>\<Name>.vhd and <Out>\<Name>.tsv, the truth file read by `open-recovr --bench-score`.

  Scenarios:
    deleted      files deleted for good (Shift+Del, or the bin emptied) during the aging
    quickformat  the same, then the whole volume quick-formatted: every file is a deleted one
    camera       FAT32/exFAT card, photos and videos written in sequence, some deleted, then refilled

  fsutil only reports where files lie on NTFS (and ReFS). On FAT32 and exFAT the truth has no
  extents: the scorer cannot tell overwritten files apart, and counts them all as recoverable.

.EXAMPLE
  .\scripts\bench\make-vhd.ps1 -Corpus D:\corpus -Out D:\bench -Name ntfs-aged -SizeMB 2048
.EXAMPLE
  .\scripts\bench\make-vhd.ps1 -Corpus D:\photos -Out D:\bench -Name sd-card -FileSystem exFAT -Scenario camera
#>
#Requires -RunAsAdministrator
param(
    # a folder of real files (photos, documents, archives, music, videos...), read only
    [Parameter(Mandatory)] [string] $Corpus,
    [Parameter(Mandatory)] [string] $Out,
    [string] $Name = 'bench',
    [int] $SizeMB = 1024,
    [ValidateSet('NTFS', 'exFAT', 'FAT32')] [string] $FileSystem = 'NTFS',
    [ValidateSet('deleted', 'quickformat', 'camera')] [string] $Scenario = 'deleted',
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

$files = @(Get-ChildItem -Recurse -File $Corpus | Where-Object { $_.Length -ge 2048 })
if ($Scenario -eq 'camera') {
    $files = @($files | Where-Object { $_.Extension -match '^\.(jpe?g|png|heic|mp4|mov|avi|cr2|nef|arw|dng)$' })
}
if ($files.Count -lt 10) { throw "not enough files in $Corpus" }
Write-Host "$($files.Count) files in the corpus"

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

    $sha = [Security.Cryptography.SHA256]::Create()
    function Hex($b) { -join ($b | ForEach-Object { $_.ToString('x2') }) }
    function Describe($path, $state) {
        $bytes = [IO.File]::ReadAllBytes($path)
        $head = if ($bytes.Length -gt 4096) { $sha.ComputeHash($bytes, 0, 4096) } else { $sha.ComputeHash($bytes) }
        $ext = '-'; $pieces = 1
        if ($FileSystem -eq 'NTFS') {
            # "VCN: 0x0  Clusters: 0x10  LCN: 0x2a4f" (the labels are translated, the numbers are not)
            $runs = @()
            foreach ($l in (fsutil file queryextents $path)) {
                $n = [regex]::Matches($l, '0x([0-9a-fA-F]+)')
                if ($n.Count -eq 3) {
                    $runs += , @([Convert]::ToInt64($n[2].Groups[1].Value, 16), [Convert]::ToInt64($n[1].Groups[1].Value, 16))
                }
            }
            if ($runs.Count -gt 0) {
                # runs that follow each other on disk are one piece
                $merged = @(); foreach ($r in $runs) {
                    if ($merged.Count -and $merged[-1][0] + $merged[-1][1] -eq $r[0]) { $merged[-1][1] += $r[1] } else { $merged += , @($r[0], $r[1]) }
                }
                $pieces = $merged.Count
                $ext = ($merged | ForEach-Object { "$($base + $_[0] * $cl)+$($_[1] * $cl)" }) -join ','
            } else { $pieces = 0 } # small enough to live inside its MFT record
        }
        @((Hex $sha.ComputeHash($bytes)), (Hex $head), $bytes.Length, $state, $pieces, $ext, $path.Substring(3)) -join "`t"
    }

    $truth = [Collections.Generic.List[string]]::new()
    $capacity = $vol.Size
    function Used { (Get-Volume -DriveLetter $part.DriveLetter).Size - (Get-Volume -DriveLetter $part.DriveLetter).SizeRemaining }
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
    function Delete-Some($share) {
        $live = @(Get-ChildItem -Recurse -File $root -ErrorAction SilentlyContinue | Where-Object { $_.FullName -notmatch 'System Volume Information' })
        foreach ($f in $live) {
            if ($rng.NextDouble() -lt $share) {
                $truth.Add((Describe $f.FullName 'deleted'))
                Remove-Item -Force $f.FullName
            }
        }
    }

    # --- the aging ------------------------------------------------------------------------
    for ($r = 1; $r -le $Rounds; $r++) {
        Copy-Some ([math]::Min($Fill, 0.3 + $r * ($Fill - 0.3) / $Rounds))
        Delete-Some 0.25
        Write-Host ("round {0}/{1}: {2:N0} MB used, {3} files deleted so far" -f $r, $Rounds, ((Used) / 1MB), $truth.Count)
    }
    # the last files deleted are the ones a user wants back: nothing has been written since
    Delete-Some 0.15

    $live = @(Get-ChildItem -Recurse -File $root | Where-Object { $_.FullName -notmatch 'System Volume Information' })
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
