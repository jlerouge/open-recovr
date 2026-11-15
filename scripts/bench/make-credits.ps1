<#
.SYNOPSIS
  Writes the credits of a set of images: the author, licence and source of every file on them.

.DESCRIPTION
  Every file on the images comes from one of three places, each with its list:
    manifest.tsv  (collect-commons.ps1)  a file of Wikimedia Commons, public domain or CC0
    derived.tsv   (make-derived.ps1)     a file made from one or more Commons files, or from none
    govdocs.tsv   (collect-govdocs.ps1)  a file of govdocs1 (Digital Corpora)
  The truth files (.tsv next to each image, from make-vhd.ps1) give the SHA-256 of every file on an
  image, deleted or live. Each one is traced back to its sources, and only the works that end up
  on an image are credited.

  Writes <Out>\CREDITS.md, to read, and <Out>\credits.tsv, one line per credited work:
    source  work  author  licence  page  used-as  images
  A file found in no list is reported and makes the script fail: it would be on an image with no
  known licence.

.EXAMPLE
  .\scripts\bench\make-credits.ps1 -Manifest D:\bench\corpus-free\manifest.tsv -Derived D:\bench\derived\derived.tsv -Govdocs D:\bench\derived\govdocs.tsv -Truth D:\bench\free\*.tsv -Out D:\bench\release
#>
param(
    [Parameter(Mandatory)] [string] $Manifest,
    [Parameter(Mandatory)] [string] $Derived,
    [string] $Govdocs,
    # the truth files of the images
    [Parameter(Mandatory)] [string[]] $Truth,
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'
function Rows($path) {
    foreach ($l in Get-Content -Encoding utf8 $path | Select-Object -Skip 1) { if ($l.Trim()) { , ($l.TrimEnd("`r") -split "`t") } }
}

$commons = @{}
foreach ($f in Rows $Manifest) {
    if ($f[0] -eq '-') { continue }
    $lic = if ($f[6] -match '^public domain$') { 'Public domain' } else { $f[6] }
    $commons[$f[0]] = [pscustomobject]@{ Title = $f[4]; Author = $f[5]; Licence = $lic; Url = $f[7]; Uses = @{}; Images = @{} }
}
$conv = @{}
foreach ($f in Rows $Derived) { $conv[$f[0]] = [pscustomobject]@{ Format = $f[2]; File = $f[3]; Sources = @($f[4] -split ',' | Where-Object { $_ -ne '-' }); Tool = $f[5] } }
$gov = @{}
if ($Govdocs) { foreach ($f in Rows $Govdocs) { $gov[$f[0]] = [pscustomobject]@{ Format = $f[2]; Source = $f[4]; Images = @{} } } }

# --- every file on every image, traced back to its sources --------------------------------
$unknown = [Collections.Generic.List[string]]::new()
$made = @{}     # derived files with no source: made here, nothing to credit
$truthFiles = @(foreach ($t in $Truth) { Get-ChildItem $t })
foreach ($t in $truthFiles) {
    $image = $t.BaseName
    foreach ($l in Get-Content -Encoding utf8 $t.FullName) {
        if ($l -match '^\s*(#|$)' -or $l -match '^\uFEFF#') { continue }
        $f = $l -split "`t"
        $sha = $f[0]
        if ($commons.ContainsKey($sha)) {
            $commons[$sha].Uses['itself'] = $true; $commons[$sha].Images[$image] = $true
        } elseif ($conv.ContainsKey($sha)) {
            $d = $conv[$sha]
            if (-not $d.Sources.Count) { $made[$d.File] = $d.Tool; continue }
            foreach ($s in $d.Sources) {
                if (-not $commons.ContainsKey($s)) { $unknown.Add("$image $($f[6]): source $s of $($d.File) is not in the manifest"); continue }
                $commons[$s].Uses[$d.Format] = $true; $commons[$s].Images[$image] = $true
            }
        } elseif ($gov.ContainsKey($sha)) {
            $gov[$sha].Images[$image] = $true
        } else {
            $unknown.Add("$image $($f[6]): $sha is in no list")
        }
    }
}
if ($unknown.Count) {
    $unknown | Select-Object -First 20 | ForEach-Object { Write-Warning $_ }
    throw "$($unknown.Count) files on the images have no known source"
}

# --- the lists ----------------------------------------------------------------------------
function Page($title) {
    # the description page of a Commons file, where its full licence and history are
    $t = $title -replace ' ', '_'
    'https://commons.wikimedia.org/wiki/File:' + ([Uri]::EscapeDataString(($t -replace '^File:', '')) -replace '\(', '%28' -replace '\)', '%29')
}
function Cell($s) { ($s -replace '\s+', ' ' -replace '\|', '\|').Trim() }
function UsedAs($w) { (@($w.Uses.Keys) | Sort-Object { if ($_ -eq 'itself') { '' } else { $_ } }) -join ', ' }

$works = @($commons.Values | Where-Object { $_.Images.Count } | Sort-Object Title)
$docs = @($gov.Values | Where-Object { $_.Images.Count } | Sort-Object Source)
New-Item -ItemType Directory -Force $Out | Out-Null
$images = @($truthFiles | ForEach-Object BaseName | Sort-Object)

$tsv = [Collections.Generic.List[string]]::new()
$tsv.Add("source`twork`tauthor`tlicence`tpage`tused-as`timages")
foreach ($w in $works) {
    $author = if ("$($w.Author)".Trim() -notmatch '^-?$') { (Cell $w.Author) } else { 'unknown' }
    $tsv.Add((@('Wikimedia Commons', ($w.Title -replace '^File:', ''), $author, $w.Licence, (Page $w.Title), (UsedAs $w), ((@($w.Images.Keys) | Sort-Object) -join ',')) -join "`t"))
}
foreach ($g in $docs) {
    $tsv.Add((@('govdocs1', $g.Source, 'US government (see the citation)', 'Public domain (US government work)', 'https://digitalcorpora.org/corpora/file-corpora/files/', 'itself', ((@($g.Images.Keys) | Sort-Object) -join ',')) -join "`t"))
}
[IO.File]::WriteAllLines((Join-Path $Out 'credits.tsv'), [string[]]$tsv, [Text.UTF8Encoding]::new($false))

$md = [Collections.Generic.List[string]]::new()
$md.AddRange([string[]]@(
    '# Credits',
    '',
    "These are the sources of every file on the images $($images -join ', '), deleted or live. Each file was taken because it is in the public domain or under CC0 (for govdocs1, in the public domain in the US), so none of them needs to be credited. They are credited all the same, out of courtesy and so that anyone can check where a file comes from. ``credits.tsv`` holds the same list in a form a program can read.",
    '',
    'If you hold rights on a file listed here and did not mean to release it, open an issue and it will be removed from the images.',
    '',
    '## Wikimedia Commons',
    '',
    "$($works.Count) files of [Wikimedia Commons](https://commons.wikimedia.org/). Each was taken because both its structured data and its description page said CC0 or public domain, with no further restriction. The licence below is the one Commons gave when the file was collected; the linked page holds the full statement.",
    '',
    '"Used as" says how the file appears on the images: "itself" for the file as downloaded, otherwise the formats it was converted into, packed into (archives) or placed in (Word and PowerPoint documents) by `make-derived.ps1`.',
    '',
    '| File | Author | Licence | Used as |',
    '|---|---|---|---|'
))
foreach ($w in $works) {
    $author = if ("$($w.Author)".Trim() -notmatch '^-?$') { Cell $w.Author } else { 'unknown' }
    $md.Add("| [$(Cell ($w.Title -replace '^File:', ''))]($(Page $w.Title)) | $author | $($w.Licence) | $(UsedAs $w) |")
}
if ($docs.Count) {
    $md.AddRange([string[]]@(
        '',
        '## govdocs1',
        '',
        "$($docs.Count) files of the govdocs1 corpus, distributed by [Digital Corpora](https://digitalcorpora.org/corpora/file-corpora/files/). They were gathered from web servers of the US government. Works of the US federal government are not protected by copyright in the US. A few files may have been written by contractors or quote other works; outside the US the rules may differ.",
        '',
        'The corpus is described in: Simson Garfinkel, Paul Farrell, Vassil Roussev and George Dinolt, "Bringing science to digital forensics with standardized forensic corpora", *Digital Investigation* 6 (2009), DFRWS 2009, pages S2-S11.',
        '',
        'Files, by their name in govdocs1 (thread/file):',
        ''
    ))
    $md.Add((($docs | ForEach-Object { '`' + ($_.Source -replace '^govdocs1/', '') + '`' }) -join ', '))
}
$md.AddRange([string[]]@(
    '',
    '## Made for the data set',
    '',
    "Some files have no source to credit: the Excel tables are random figures, and the text of the Word reports is made of random syllables, both generated by ``make-derived.ps1``$(if ($made.Count) { " ($($made.Count) files with no source on the images)" }). The conversions were made with ffmpeg, ImageMagick, the Windows HEIF encoder, the tar and makecab of Windows and Microsoft Office; these programs claim no rights on what they write.",
    '',
    'The disk images themselves, made by `make-vhd.ps1`, are released under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).'
))
[IO.File]::WriteAllLines((Join-Path $Out 'CREDITS.md'), [string[]]$md, [Text.UTF8Encoding]::new($false))
Write-Host "$($works.Count) Commons works and $($docs.Count) govdocs1 files credited in $Out"
