<#
.SYNOPSIS
  Adds the formats Wikimedia Commons does not hold, made from the free corpus: video containers,
  image formats and office documents.

.DESCRIPTION
  The sources are files of the corpus gathered by collect-commons.ps1 (public domain or CC0), so
  what is made from them can be shared the same way. Each output is listed in <Out>\derived.tsv:
    sha256 size format file source tool
  where source is the SHA-256 of the corpus file it was made from (several, comma-separated, for
  an archive or a document holding pictures; - for none), and tool what wrote it. The
  scorer cares only about contents; the list lets results be split between files written by the
  programs that usually write them (Word, Excel, PowerPoint) and files converted by free tools.

    video   ffmpeg: MP4 (H.264, H.265), MKV, AVI, MOV, 3GP, MPEG-TS, FLV, WMV, MXF, DV, cut to -Seconds
    sound   ffmpeg: M4A (AAC), AIFF, AC-3, WMA
    image   ImageMagick: AVIF, JPEG XL, PSD, BMP, ICO, TGA, PCX, JPEG 2000, DDS, SGI, PPM, PICT, EPS
            Windows HEIF encoder: HEIC (needs the HEIF and HEVC extensions of the Microsoft Store)
    archive Windows tar (libarchive): ZIP, 7z, TAR (plain, gzip, bzip2, xz), ISO 9660, cpio; makecab: CAB
    office  Microsoft Word (reports with corpus pictures): DOCX, DOC, RTF, ODT
            Microsoft Excel (tables of random figures, with a chart): XLSX, XLS, ODS
            Microsoft PowerPoint (slides of corpus pictures): PPTX, PPT, ODP

.EXAMPLE
  .\scripts\bench\make-derived.ps1 -Corpus D:\bench\corpus-free -Out D:\bench\derived -Tools D:\bench\tools
#>
param(
    [Parameter(Mandatory)] [string] $Corpus,
    [Parameter(Mandatory)] [string] $Out,
    [Parameter(Mandatory)] [string] $Tools,
    [string[]] $Only = @('video', 'sound', 'image', 'archive', 'office'),
    # sources taken per output format
    [int] $PerFormat = 4,
    [int] $Seconds = 20,
    [int] $Seed = 7
)
$ErrorActionPreference = 'Stop'
$rng = [Random]::new($Seed)
$ffmpeg = Get-ChildItem -Recurse -Filter ffmpeg.exe $Tools | Select-Object -First 1 -ExpandProperty FullName
$magick = Get-ChildItem -Recurse -Filter magick.exe $Tools | Select-Object -First 1 -ExpandProperty FullName
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$listPath = Join-Path $Out 'derived.tsv'
$list = [Collections.Generic.List[string]]::new()
if (Test-Path $listPath) { $list.AddRange([string[]]@(Get-Content -Encoding utf8 $listPath | Select-Object -Skip 1)) }
$done = @{}; foreach ($l in $list) { $done[($l -split "`t")[3]] = $true }
function Save {
    [IO.File]::WriteAllLines($listPath, [string[]](@("sha256`tsize`tformat`tfile`tsource`ttool") + $list), [Text.UTF8Encoding]::new($false))
}

# the corpus files actually downloaded, by MIME type
$byMime = @{}
foreach ($l in Get-Content -Encoding utf8 (Join-Path $Corpus 'manifest.tsv') | Select-Object -Skip 1) {
    $f = $l -split "`t"
    if ($f[0] -eq '-') { continue }
    $path = Join-Path (Join-Path $Corpus ($f[2] -replace '[/+]', '-')) $f[3]
    if (-not (Test-Path $path)) { continue }
    if (-not $byMime.ContainsKey($f[2])) { $byMime[$f[2]] = [Collections.Generic.List[object]]::new() }
    $byMime[$f[2]].Add([pscustomobject]@{ Sha = $f[0]; Path = $path; Size = [long]$f[1] })
}
function Pick($mimes, $n, $maxMB = 1e9) {
    $all = @(foreach ($m in $mimes) { if ($byMime.ContainsKey($m)) { $byMime[$m] | Where-Object { $_.Size -le $maxMB * 1MB } } })
    @($all | Sort-Object { $rng.Next() } | Select-Object -First $n)
}
# $src: the corpus file it was made from, or several (an archive, a document with pictures)
function Record($path, $format, $src, $tool) {
    if (-not (Test-Path $path) -or (Get-Item $path).Length -lt 1024) {
        Write-Warning "no output: $path"; Remove-Item -ErrorAction SilentlyContinue $path; return
    }
    $h = (Get-FileHash -Algorithm SHA256 $path).Hash.ToLower()
    $list.Add((@($h, (Get-Item $path).Length, $format, (Split-Path -Leaf $path), ((@($src) | ForEach-Object Sha) -join ','), $tool) -join "`t"))
}
function Target($format, $ext, $src) {
    $dir = Join-Path $Out $format
    New-Item -ItemType Directory -Force $dir | Out-Null
    Join-Path $dir ('{0}.{1}' -f $src.Sha.Substring(0, 16), $ext)
}
function FF($src, $path, [string[]] $opts) {
    # ffmpeg writes its progress to stderr: kept out of the way, the exit code says enough
    Native $ffmpeg (@('-hide_banner', '-loglevel', 'error', '-y', '-i', $src.Path) + $opts + @($path))
}
function Native($exe, [string[]] $a) {
    # Windows PowerShell turns any line a program writes on stderr into an error, fatal under Stop
    $ErrorActionPreference = 'Continue'
    & $exe @a 2>$null | Out-Null
    $LASTEXITCODE -eq 0
}

# --- video and sound ------------------------------------------------------------------------
$t = @('-t', "$Seconds")
$small = @('-vf', 'scale=-2:min(480\,ih)')
$videos = @(
    @('mp4-h264', 'mp4', $t + $small + @('-c:v', 'libx264', '-preset', 'fast', '-c:a', 'aac', '-movflags', '+faststart')),
    @('mp4-h265', 'mp4', $t + $small + @('-c:v', 'libx265', '-preset', 'fast', '-tag:v', 'hvc1', '-c:a', 'aac')),
    @('mkv-vp9', 'mkv', $t + $small + @('-c:v', 'libvpx-vp9', '-b:v', '800k', '-deadline', 'realtime', '-c:a', 'libopus')),
    @('mkv-h264', 'mkv', $t + $small + @('-c:v', 'libx264', '-preset', 'fast', '-c:a', 'ac3')),
    @('avi-mpeg4', 'avi', $t + $small + @('-c:v', 'mpeg4', '-vtag', 'XVID', '-q:v', '5', '-c:a', 'libmp3lame')),
    @('avi-mjpeg', 'avi', $t + $small + @('-c:v', 'mjpeg', '-q:v', '5', '-c:a', 'pcm_s16le')),
    @('mov-h264', 'mov', $t + $small + @('-c:v', 'libx264', '-preset', 'fast', '-c:a', 'aac')),
    @('mov-prores', 'mov', @('-t', '5', '-vf', 'scale=-2:min(360\,ih)', '-c:v', 'prores_ks', '-profile:v', '0', '-c:a', 'pcm_s16le')),
    @('3gp', '3gp', $t + @('-vf', 'scale=352:288', '-c:v', 'mpeg4', '-c:a', 'aac', '-ar', '22050', '-ac', '1')),
    @('ts-h264', 'ts', $t + $small + @('-c:v', 'libx264', '-preset', 'fast', '-c:a', 'ac3')),
    @('flv', 'flv', $t + $small + @('-c:v', 'flv', '-c:a', 'libmp3lame', '-ar', '44100')),
    @('wmv', 'wmv', $t + $small + @('-c:v', 'wmv2', '-c:a', 'wmav2')),
    @('mxf', 'mxf', @('-t', '10', '-vf', 'scale=720:576,fps=25', '-c:v', 'mpeg2video', '-b:v', '8M', '-c:a', 'pcm_s16le', '-ar', '48000')),
    @('dv', 'dv', @('-t', '5', '-vf', 'scale=720:576,fps=25', '-pix_fmt', 'yuv420p', '-c:v', 'dvvideo', '-c:a', 'pcm_s16le', '-ar', '48000', '-ac', '2'))
)
$sounds = @(
    @('m4a', 'm4a', $t + @('-vn', '-c:a', 'aac', '-b:a', '160k')),
    @('aiff', 'aiff', $t + @('-vn', '-c:a', 'pcm_s16be')),
    @('ac3', 'ac3', $t + @('-vn', '-c:a', 'ac3')),
    @('wma', 'wma', $t + @('-vn', '-c:a', 'wmav2'))
)
if ('video' -in $Only) {
    foreach ($v in $videos) {
        foreach ($src in (Pick @('video/webm', 'video/mpeg', 'application/ogg') $PerFormat 200)) {
            $path = Target $v[0] $v[1] $src
            if ($done[(Split-Path -Leaf $path)]) { continue }
            if (FF $src $path $v[2]) { Record $path $v[0] $src "ffmpeg $($v[0])" } else { Remove-Item -ErrorAction SilentlyContinue $path }
        }
        Save; Write-Host "video $($v[0])"
    }
}
if ('sound' -in $Only) {
    foreach ($s in $sounds) {
        foreach ($src in (Pick @('audio/x-flac', 'audio/wav', 'audio/mpeg', 'application/ogg') $PerFormat 100)) {
            $path = Target $s[0] $s[1] $src
            if ($done[(Split-Path -Leaf $path)]) { continue }
            if (FF $src $path $s[2]) { Record $path $s[0] $src "ffmpeg $($s[0])" } else { Remove-Item -ErrorAction SilentlyContinue $path }
        }
        Save; Write-Host "sound $($s[0])"
    }
}

# --- pictures -------------------------------------------------------------------------------
$images = @(
    @('avif', 'avif', @()), @('jxl', 'jxl', @()), @('psd', 'psd', @()), @('bmp', 'bmp', @()),
    @('ico', 'ico', @('-resize', '256x256>')), @('tga', 'tga', @()), @('pcx', 'pcx', @()), @('jp2', 'jp2', @()),
    @('dds', 'dds', @('-resize', '1024x1024>')), @('sgi', 'sgi', @()), @('ppm', 'ppm', @()), @('pict', 'pct', @()),
    @('eps', 'eps', @('-resize', '1024x1024>'))
)
if ('image' -in $Only) {
    foreach ($i in $images) {
        foreach ($src in (Pick @('image/jpeg', 'image/png') $PerFormat 8)) {
            $path = Target $i[0] $i[1] $src
            if ($done[(Split-Path -Leaf $path)]) { continue }
            # big pictures are brought down to a few megapixels: uncompressed formats would be huge
            $a = @("$($src.Path)[0]", '-resize', '2400x2400>') + $i[2] + @($path)
            if (Native $magick $a) { Record $path $i[0] $src "ImageMagick $($i[0])" } else { Remove-Item -ErrorAction SilentlyContinue $path }
        }
        Save; Write-Host "image $($i[0])"
    }
}

# --- HEIC, written by the HEIF encoder of Windows (the HEIF and HEVC extensions of the Store) ---
if ('image' -in $Only) {
    Add-Type -AssemblyName System.Runtime.WindowsRuntime
    $ext = [System.WindowsRuntimeSystemExtensions].GetMethods()
    $asTask = $ext | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' } | Select-Object -First 1
    $asAction = $ext | Where-Object { $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncAction' } | Select-Object -First 1
    function Await($op, [type] $t) { $task = $asTask.MakeGenericMethod($t).Invoke($null, @($op)); $task.Wait(-1) | Out-Null; $task.Result }
    [Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime] | Out-Null
    [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics.Imaging, ContentType = WindowsRuntime] | Out-Null
    [Windows.Storage.Streams.IRandomAccessStream, Windows.Storage.Streams, ContentType = WindowsRuntime] | Out-Null
    $stream = [Windows.Storage.Streams.IRandomAccessStream]
    function Heic($from, $to) {
        $in = Await ((Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($from)) ([Windows.Storage.StorageFile])).OpenAsync('Read')) $stream
        $out = $null
        try {
            $bmp = Await ((Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($in)) ([Windows.Graphics.Imaging.BitmapDecoder])).GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
            $dir = Await ([Windows.Storage.StorageFolder]::GetFolderFromPathAsync((Split-Path $to))) ([Windows.Storage.StorageFolder])
            $file = Await ($dir.CreateFileAsync((Split-Path -Leaf $to), 'ReplaceExisting')) ([Windows.Storage.StorageFile])
            $out = Await ($file.OpenAsync('ReadWrite')) $stream
            $enc = Await ([Windows.Graphics.Imaging.BitmapEncoder]::CreateAsync([Windows.Graphics.Imaging.BitmapEncoder]::HeifEncoderId, $out)) ([Windows.Graphics.Imaging.BitmapEncoder])
            $enc.SetSoftwareBitmap($bmp)
            $asAction.Invoke($null, @($enc.FlushAsync())).Wait(-1) | Out-Null
        } finally { if ($out) { $out.Dispose() }; $in.Dispose() }
    }
    foreach ($src in (Pick @('image/jpeg') ($PerFormat * 2) 8)) {
        $path = Target 'heic' 'heic' $src
        if ($done[(Split-Path -Leaf $path)]) { continue }
        try { Heic $src.Path $path; Record $path 'heic' $src 'Windows HEIF encoder' }
        catch { Write-Warning "no HEIC from $($src.Path): $_"; Remove-Item -ErrorAction SilentlyContinue $path }
    }
    Save; Write-Host 'image heic'
}

# --- archives of a few corpus files, written by the tar and makecab of Windows ---------------
if ('archive' -in $Only) {
    $tar = Join-Path $env:SystemRoot 'System32\tar.exe'
    $makecab = Join-Path $env:SystemRoot 'System32\makecab.exe'
    $archives = @(
        @('zip', 'zip', @('--format', 'zip')), @('7z', '7z', @('--format', '7zip')),
        @('tar', 'tar', @('--format', 'ustar')), @('tar-gz', 'tar.gz', @('-z')), @('tar-bz2', 'tar.bz2', @('-j')),
        @('tar-xz', 'tar.xz', @('-J')), @('iso', 'iso', @('--format', 'iso9660')), @('cpio', 'cpio', @('--format', 'cpio')),
        @('cab', 'cab', $null)
    )
    $kinds = @('image/jpeg', 'image/png', 'image/gif', 'image/svg+xml', 'application/pdf', 'audio/midi', 'audio/mpeg')
    foreach ($x in $archives) {
        for ($k = 0; $k -lt $PerFormat; $k++) {
            $pics = Pick $kinds (2 + $rng.Next(5)) 5
            if (-not $pics.Count) { break }
            $path = Target $x[0] $x[1] $pics[0]
            if ($done[(Split-Path -Leaf $path)]) { continue }
            # the files are copied side by side first, so the archive holds plain names
            $stage = Join-Path $env:TEMP ('make-derived-' + [guid]::NewGuid())
            New-Item -ItemType Directory $stage | Out-Null
            try {
                foreach ($p in $pics) { Copy-Item $p.Path $stage }
                $names = @(Get-ChildItem -File $stage | ForEach-Object Name)
                if ($x[0] -eq 'cab') {
                    $ddf = Join-Path $stage 'files.ddf'
                    @(".Set CabinetNameTemplate=$(Split-Path -Leaf $path)", ".Set DiskDirectoryTemplate=$(Split-Path $path)",
                      '.Set Cabinet=on', '.Set Compress=on', '.Set CompressionType=MSZIP', '.Set MaxDiskSize=0',
                      ".Set InfFileName=$stage\setup.inf", ".Set RptFileName=$stage\setup.rpt") + $names | Set-Content -Encoding ascii $ddf
                    Push-Location $stage; $ok = Native $makecab @('/F', $ddf); Pop-Location
                } else {
                    $ok = Native $tar (@('-C', $stage, '-cf', $path) + $x[2] + $names)
                }
                if ($ok) { Record $path $x[0] $pics "Windows $(if ($x[0] -eq 'cab') { 'makecab' } else { 'tar (libarchive)' })" }
                else { Remove-Item -ErrorAction SilentlyContinue $path }
            } finally { Remove-Item -Recurse -Force $stage }
        }
        Save; Write-Host "archive $($x[0])"
    }
}

# --- office documents, written by Microsoft Office itself -----------------------------------
# Office keeps the file it last saved open, so a document is saved in every format, closed, and
# only then are its files hashed.
function Saved($files, $src, $tool) {
    foreach ($f in $files) { Record $f[0] $f[1] $src $tool }
    Save
}
if ('office' -in $Only) {
    # Word: a report made here (made-up words, so free of rights) with headings, a table and corpus
    # pictures. Opening the corpus PDFs in Word was tried first, but Word crashes on some of them and
    # then waits, hidden, on a question about the file.
    $syll = 'ka', 'ro', 'mi', 'tel', 'an', 'su', 'ver', 'do', 'li', 'pra', 'ne', 'os', 'tu', 'ga', 'ber', 'il', 'mo', 're'
    function Words($n) { (1..$n | ForEach-Object { -join (1..(1 + $rng.Next(3)) | ForEach-Object { $syll[$rng.Next($syll.Count)] }) }) -join ' ' }
    $word = New-Object -ComObject Word.Application
    $word.Visible = $false; $word.DisplayAlerts = 0
    try {
        $wordFormats = @(@('docx', 'docx', 16), @('doc', 'doc', 0), @('rtf', 'rtf', 6), @('odt', 'odt', 23))
        for ($k = 0; $k -lt $PerFormat; $k++) {
            $name = 'report-{0:D3}' -f $k
            $pics = Pick @('image/jpeg', 'image/png') (1 + $rng.Next(3)) 4
            if ($done["$name.docx"] -or -not $pics.Count) { continue }
            $doc = $word.Documents.Add()
            $doc.RemovePersonalInformation = $true   # no author or last-saved-by from the Office account
            $sel = $word.Selection
            foreach ($p in $pics) {
                $sel.Style = $doc.Styles.Item(-2); $sel.TypeText((Words 4)); $sel.TypeParagraph()
                for ($i = 0; $i -lt 3 + $rng.Next(8); $i++) {
                    $sel.Style = $doc.Styles.Item(-1); $sel.TypeText((Words (40 + $rng.Next(120))) + '.'); $sel.TypeParagraph()
                }
                $sel.InlineShapes.AddPicture($p.Path) | Out-Null; $sel.TypeParagraph()
            }
            $rows = 3 + $rng.Next(15)
            $t = $doc.Tables.Add($sel.Range, $rows, 4); $t.Borders.Enable = 1
            for ($r = 1; $r -le $rows; $r++) { for ($c = 1; $c -le 4; $c++) { $t.Cell($r, $c).Range.Text = if ($c -eq 1) { Words 2 } else { '{0:N2}' -f ($rng.NextDouble() * 1000) } } }
            $files = @()
            foreach ($x in $wordFormats) {
                $path = Join-Path (New-Item -ItemType Directory -Force (Join-Path $Out $x[0])) "$name.$($x[1])"
                $doc.SaveAs2([string]$path, [int]$x[2])
                $files += , @($path, $x[0])
            }
            $doc.Close(0)
            Saved $files $pics "Microsoft Word $($word.Version)"
        }
    } finally { $word.Quit(); [Runtime.InteropServices.Marshal]::ReleaseComObject($word) | Out-Null }
    Write-Host 'office word'

    # Excel: a table of random figures (made here, so free of rights) with a chart
    $excel = New-Object -ComObject Excel.Application
    $excel.Visible = $false; $excel.DisplayAlerts = $false
    try {
        $xlFormats = @(@('xlsx', 'xlsx', 51), @('xls', 'xls', 56), @('ods', 'ods', 60))
        for ($k = 0; $k -lt $PerFormat; $k++) {
            # no corpus file behind it: named after its number
            $src = [pscustomobject]@{ Sha = '-'; Name = 'table-{0:D3}' -f $k }
            $rows = 50 + $rng.Next(2000)
            if ($done["$($src.Name).xlsx"]) { continue }
            $book = $excel.Workbooks.Add()
            $book.RemovePersonalInformation = $true
            $sheet = $book.Worksheets.Item(1)
            $data = New-Object 'object[,]' $rows, 6
            for ($r = 0; $r -lt $rows; $r++) {
                $data[$r, 0] = "Item $r"; for ($c = 1; $c -lt 6; $c++) { $data[$r, $c] = [math]::Round($rng.NextDouble() * 10000, 2) }
            }
            $sheet.Range($sheet.Cells.Item(1, 1), $sheet.Cells.Item($rows, 6)).Value2 = $data
            $chart = $sheet.Shapes.AddChart2(-1, 4).Chart
            $chart.SetSourceData($sheet.Range($sheet.Cells.Item(1, 1), $sheet.Cells.Item([math]::Min($rows, 30), 3)))
            $files = @()
            foreach ($x in $xlFormats) {
                $path = Join-Path (New-Item -ItemType Directory -Force (Join-Path $Out $x[0])) "$($src.Name).$($x[1])"
                $book.SaveAs([string]$path, [int]$x[2])
                $files += , @($path, $x[0])
            }
            $book.Close($false)
            Saved $files $src "Microsoft Excel $($excel.Version)"
        }
    } finally { $excel.Quit(); [Runtime.InteropServices.Marshal]::ReleaseComObject($excel) | Out-Null }
    Write-Host 'office excel'

    # PowerPoint: slides holding pictures of the corpus
    $ppt = New-Object -ComObject PowerPoint.Application
    try {
        $pptFormats = @(@('pptx', 'pptx', 24), @('ppt', 'ppt', 1), @('odp', 'odp', 35))
        for ($k = 0; $k -lt $PerFormat; $k++) {
            $name = 'slides-{0:D3}' -f $k
            $pics = Pick @('image/jpeg', 'image/png') (2 + $rng.Next(4)) 4
            if ($done["$name.pptx"] -or -not $pics.Count) { continue }
            $pres = $ppt.Presentations.Add(0)
            $pres.RemovePersonalInformation = -1
            foreach ($p in $pics) {
                $slide = $pres.Slides.Add($pres.Slides.Count + 1, 12)
                $slide.Shapes.AddPicture($p.Path, 0, -1, 40, 40, 640, 400) | Out-Null
            }
            $files = @()
            foreach ($x in $pptFormats) {
                $path = Join-Path (New-Item -ItemType Directory -Force (Join-Path $Out $x[0])) "$name.$($x[1])"
                $pres.SaveAs([string]$path, [int]$x[2])
                $files += , @($path, $x[0])
            }
            $pres.Close()
            Saved $files $pics "Microsoft PowerPoint $($ppt.Version)"
        }
    } finally { $ppt.Quit(); [Runtime.InteropServices.Marshal]::ReleaseComObject($ppt) | Out-Null }
    Write-Host 'office powerpoint'
}
Save
Write-Host "$($list.Count) files in $listPath"
