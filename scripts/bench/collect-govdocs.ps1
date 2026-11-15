<#
.SYNOPSIS
  Lists files of the govdocs1 corpus to join the free corpus: old office documents as written by
  the programs of their time (Word 97, Excel 97, PowerPoint 97, PostScript, dBase...).

.DESCRIPTION
  govdocs1 (Digital Corpora, https://digitalcorpora.org/corpora/file-corpora/files/) holds about a
  million files gathered from web servers of the US government. Works of the US federal
  government are not protected by copyright in the US, and Digital Corpora distributes the corpus
  for free reuse. It is cut into 1,000 zip files of about 1,000 files each ("threads").

  Files of the threads given in -Source (the extracted zip files) are copied into <Out>\govdocs\<ext>
  and listed in <Out>\govdocs.tsv:
    sha256 size format file source
  where source is the thread and the file name in govdocs1. Files with no signature a carver could
  find (plain text, CSV, logs, unknown) are left out: they are a different problem.

.EXAMPLE
  .\scripts\bench\collect-govdocs.ps1 -Source D:\bench\govdocs\001 -Out D:\bench\derived
#>
param(
    [Parameter(Mandatory)] [string[]] $Source,
    [Parameter(Mandatory)] [string] $Out,
    [string[]] $Skip = @('txt', 'csv', 'log', 'unk', 'f', 'text')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$rows = [Collections.Generic.List[string]]::new()
foreach ($s in $Source) {
    foreach ($f in Get-ChildItem -Recurse -File $s) {
        $ext = $f.Extension.TrimStart('.').ToLower()
        if (-not $ext -or $ext -in $Skip) { continue }
        $dir = Join-Path (Join-Path $Out 'govdocs') $ext
        New-Item -ItemType Directory -Force $dir | Out-Null
        Copy-Item $f.FullName $dir -Force
        $h = (Get-FileHash -Algorithm SHA256 $f.FullName).Hash.ToLower()
        $rows.Add((@($h, $f.Length, $ext, $f.Name, "govdocs1/$($f.Directory.Name)/$($f.Name)") -join "`t"))
    }
}
[IO.File]::WriteAllLines((Join-Path $Out 'govdocs.tsv'), [string[]](@("sha256`tsize`tformat`tfile`tsource") + $rows), [Text.UTF8Encoding]::new($false))
$mb = 0; foreach ($r in $rows) { $mb += [long]($r -split "`t")[1] / 1MB }
Write-Host ("{0} files, {1:N0} MB" -f $rows.Count, $mb)
