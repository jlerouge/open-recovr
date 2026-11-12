<#
.SYNOPSIS
  Runs each carving engine on a disk image, times it and scores what it recovered.

.DESCRIPTION
  Engines: "ours" (open-recovr --carve) and "photorec" (PhotoRec run directly, its own default
  file set, whole space, no partition table read). Each engine writes into its own folder under
  -Out; the scorer (open-recovr --bench-score) then compares that folder with the truth file.
  One line per engine is appended to <Out>\results.tsv:
  image engine seconds deleted intact exact trimmable damaged recovered junk junk_bytes

.EXAMPLE
  .\scripts\bench\run.ps1 -Image D:\bench\L1_Graphic.dd -Truth D:\bench\L1_Graphic.tsv -Out D:\bench\out
#>
param(
    [Parameter(Mandatory)] [string] $Image,
    [Parameter(Mandatory)] [string] $Truth,
    [Parameter(Mandatory)] [string] $Out,
    [string[]] $Engines = @('ours', 'photorec'),
    [string] $App = (Join-Path $PSScriptRoot '..\..\target\release\open-recovr.exe'),
    [string] $PhotoRec = (Join-Path $PSScriptRoot '..\..\vendor\testdisk-7.2\photorec_win.exe'),
    # keep the recovered files (they are removed once scored, to save space)
    [switch] $Keep
)
$ErrorActionPreference = 'Stop'
$Image = (Resolve-Path $Image).Path
$Truth = (Resolve-Path $Truth).Path
$App = (Resolve-Path $App).Path
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$name = [IO.Path]::GetFileNameWithoutExtension($Image)
$results = Join-Path $Out 'results.tsv'
if (-not (Test-Path $results)) {
    "image`tengine`tseconds`tdeleted`tintact`texact`ttrimmable`tdamaged`trecovered`tjunk`tjunk_bytes" |
        Out-File -Encoding utf8 $results
}

foreach ($engine in $Engines) {
    $dir = Join-Path $Out "$name.$engine"
    if (Test-Path $dir) { Remove-Item -Recurse -Force $dir }
    New-Item -ItemType Directory -Force $dir | Out-Null
    $watch = [Diagnostics.Stopwatch]::StartNew()
    switch ($engine) {
        'ours' {
            & $App --carve $Image $dir | Out-Host
        }
        'photorec' {
            # PhotoRec asks for elevation unless told the caller's rights are enough
            $env:__COMPAT_LAYER = 'RunAsInvoker'
            $p = Start-Process -FilePath (Resolve-Path $PhotoRec).Path -WorkingDirectory $dir -NoNewWindow -Wait -PassThru `
                -RedirectStandardOutput (Join-Path $Out "$name.photorec.screen") `
                -ArgumentList '/log', '/d', 'recup', '/cmd', "`"$Image`"", 'partition_none,options,keep_corrupted_file_no,wholespace,search'
            Remove-Item Env:__COMPAT_LAYER
            if ($p.ExitCode -ne 0) { Write-Warning "PhotoRec exited with code $($p.ExitCode)" }
        }
        default { throw "unknown engine $engine" }
    }
    $secs = [math]::Round($watch.Elapsed.TotalSeconds, 2).ToString([Globalization.CultureInfo]::InvariantCulture)
    # piped, or PowerShell would not wait for a program built for windows
    $report = @(& $App --bench-score $Truth $dir $Image "$name/$engine" | Out-String -Stream)
    $report | Where-Object { $_ -notmatch '^RESULT' } | Out-Host
    $line = $report | Where-Object { $_ -match '^RESULT' } | Select-Object -First 1
    if ($line) {
        $f = $line -split "`t"
        (@($name, $engine, $secs) + $f[2..9]) -join "`t" | Out-File -Append -Encoding utf8 $results
    }
    Write-Host "$engine : $secs s"
    if (-not $Keep) { Remove-Item -Recurse -Force $dir }
}
