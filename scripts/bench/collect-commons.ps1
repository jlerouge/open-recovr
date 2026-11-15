<#
.SYNOPSIS
  Gathers a corpus of freely reusable files from Wikimedia Commons, for a carving benchmark that
  can be published with its files.

.DESCRIPTION
  Only files in the public domain or under CC0 are taken, so that a disk image built from them can
  be shared without conditions. A file is kept when both hold:
    - its structured data says so (licence CC0, P275=Q6938433, or copyright status public domain,
      P6216=Q19652), which is what the search asks for;
    - the licence on its description page (extmetadata) is CC0 or public domain, and it carries no
      further restriction (trademark, personality rights...).

  For each format of the plan, files are searched by MIME type and size, in a random order, until
  the count is reached. Each file is downloaded once, its SHA-1 checked against the one Commons
  gives, and recorded in <Out>\manifest.tsv:
    sha256 size mime file title author licence url sha1

  With -DryRun nothing is downloaded: the manifest lists the files that would be, with their sizes,
  so the total is known first. Without it, files already in the manifest are not fetched again,
  and a manifest made by a dry run is downloaded as it is (same files).

  With -Recover nothing is searched or downloaded: files already in <Out> but missing from the
  manifest (a lost or damaged manifest) are looked up on Commons by their SHA-1 and listed again,
  under the same licence checks.

.EXAMPLE
  .\scripts\bench\collect-commons.ps1 -Out D:\bench\corpus-free -DryRun
  .\scripts\bench\collect-commons.ps1 -Out D:\bench\corpus-free
#>
param(
    [Parameter(Mandatory)] [string] $Out,
    [switch] $DryRun,
    [switch] $Recover,
    # mime, count, smallest and largest size in KB; old and new formats alike
    [string[]] $Plan = @(
        'image/jpeg,150,50,8000',
        'image/png,80,10,8000',
        'image/gif,40,5,10000',
        'image/tiff,30,100,40000',
        'image/webp,40,10,8000',
        'image/svg+xml,40,5,4000',
        'image/vnd.djvu,15,50,20000',
        'image/x-xcf,10,50,20000',
        'application/pdf,40,50,30000',
        'audio/x-flac,20,200,40000',
        'audio/wav,20,50,40000',
        'audio/mpeg,30,100,20000',
        'audio/midi,30,2,500',
        'application/ogg,40,50,30000',
        'video/webm,15,500,80000',
        'video/mpeg,10,500,80000',
        'application/sla,10,20,20000'
    )
)
$ErrorActionPreference = 'Stop'
# Wikimedia asks automated clients to name themselves with a way to reach their authors
$ua = 'open-recovr-bench-collector/0.1 (https://github.com/jlerouge/open-recovr; corpus of public-domain files)'
$api = 'https://commons.wikimedia.org/w/api.php'
New-Item -ItemType Directory -Force $Out | Out-Null
$Out = (Resolve-Path $Out).Path
$manifestPath = Join-Path $Out 'manifest.tsv'
$header = "sha256`tsize`tmime`tfile`ttitle`tauthor`tlicence`turl`tsha1"

# what is already listed, by Commons title
$rows = [ordered]@{}
if (Test-Path $manifestPath) {
    foreach ($l in Get-Content -Encoding utf8 $manifestPath | Select-Object -Skip 1) {
        $f = $l -split "`t"
        if ($f.Count -ge 9) { $rows[$f[4]] = $f }
    }
}
function Save {
    $lines = @($header) + @($rows.Values | ForEach-Object { $_ -join "`t" })
    [IO.File]::WriteAllLines($manifestPath, $lines, [Text.UTF8Encoding]::new($false))
}
function Plain($html) {
    if (-not $html) { return '-' }
    $t = [Net.WebUtility]::HtmlDecode(($html -replace '<[^>]+>', ' '))
    ($t -replace '[\t\r\n]+', ' ' -replace '\s{2,}', ' ').Trim()
}
function Ask($params) {
    $params.format = 'json'; $params.maxlag = '5'
    for ($try = 0; $try -lt 6; $try++) {
        try {
            $r = Invoke-RestMethod -Uri $api -Body $params -UserAgent $ua
            if ($r.error) { throw $r.error.code }
            return $r
        } catch {
            Start-Sleep -Seconds (5 * ($try + 1))
        }
    }
    throw "Commons API keeps failing: $($params | ConvertTo-Json -Compress)"
}
function Sha256File($p) { (Get-FileHash -Algorithm SHA256 $p).Hash.ToLower() }

$exts = @{
    'image/jpeg' = 'jpg'; 'image/png' = 'png'; 'image/gif' = 'gif'; 'image/tiff' = 'tif'; 'image/webp' = 'webp'
    'image/svg+xml' = 'svg'; 'image/vnd.djvu' = 'djvu'; 'image/x-xcf' = 'xcf'; 'application/pdf' = 'pdf'
    'audio/x-flac' = 'flac'; 'audio/wav' = 'wav'; 'audio/mpeg' = 'mp3'; 'audio/midi' = 'mid'
    'application/ogg' = 'ogg'; 'video/webm' = 'webm'; 'video/mpeg' = 'mpg'; 'application/sla' = 'stl'
}
# a manifest row for a Commons file, or nothing when its description page does not say CC0 or
# public domain, or adds a restriction
function Row($title, $ii) {
    $m = $ii.extmetadata
    $lic = "$($m.License.value)".ToLower()
    $short = Plain $m.LicenseShortName.value
    if (-not ($lic -eq 'cc0' -or $lic -like 'pd*' -or $short -match '^(CC0|Public domain)')) { return }
    if ("$($m.Restrictions.value)".Trim()) { return }
    $name = '{0}.{1}' -f $ii.sha1.Substring(0, 16), $exts[$ii.mime]
    , @('-', $ii.size, $ii.mime, $name, $title, (Plain $m.Artist.value), $short, ($ii.url -replace '\?.*', ''), $ii.sha1)
}

# --- listing again the files already there ------------------------------------------------
if ($Recover) {
    $listed = @{}; foreach ($v in $rows.Values) { $listed[$v[3]] = $true }
    $found = 0; $dropped = 0
    foreach ($f in Get-ChildItem -Recurse -File $Out | Where-Object { $_.Name -ne 'manifest.tsv' -and -not $listed[$_.Name] }) {
        $sha1 = (Get-FileHash -Algorithm SHA1 $f.FullName).Hash.ToLower()
        $r = Ask @{
            action = 'query'; list = 'allimages'; aisha1 = $sha1; ailimit = '1'
            aiprop = 'url|size|sha1|mime|extmetadata'
            aiextmetadatafilter = 'License|LicenseShortName|Artist|Restrictions'
        }
        $ii = @($r.query.allimages)[0]
        $row = if ($ii) { Row $ii.title $ii }
        if (-not $row -or $row[3] -ne $f.Name) { Write-Warning "not found or not free: $($f.FullName)"; $dropped++; continue }
        $row[0] = Sha256File $f.FullName
        $rows[$row[4]] = $row
        $found++
        if ($found % 50 -eq 0) { Save; Write-Host "$found listed again" }
    }
    Save
    Write-Host "$found files listed again, $dropped not ($($rows.Count) in the manifest)"
    return
}

# --- choosing the files -------------------------------------------------------------------
foreach ($entry in $Plan) {
    $mime, $count, $minKB, $maxKB = $entry -split ','
    $count = [int]$count
    $have = @($rows.Values | Where-Object { $_[2] -eq $mime }).Count
    $seen = 0
    while ($have -lt $count -and $seen -lt 20) {
        # the random order is the same for the same search, so it is read page by page
        $r = Ask @{
            action = 'query'; generator = 'search'; gsrnamespace = '6'; gsrlimit = '50'; gsrsort = 'random'
            gsroffset = "$(50 * $seen++)"
            gsrsearch = "filemime:$mime filesize:>$minKB filesize:<$maxKB haswbstatement:P275=Q6938433|P6216=Q19652"
            prop = 'imageinfo'; iiprop = 'url|size|sha1|mime|extmetadata'
            iiextmetadatafilter = 'License|LicenseShortName|Artist|Restrictions'
        }
        if (-not $r.query) { break }
        foreach ($p in $r.query.pages.PSObject.Properties.Value) {
            if ($have -ge $count) { break }
            if ($rows.Contains($p.title)) { continue }
            $ii = $p.imageinfo[0]
            if (-not $ii -or $ii.mime -ne $mime) { continue }
            $row = Row $p.title $ii
            if (-not $row) { continue }
            $rows[$p.title] = $row
            $have++
        }
        Save
    }
    $mb = 0; foreach ($v in $rows.Values) { if ($v[2] -eq $mime) { $mb += [long]$v[1] / 1MB } }
    Write-Host ("{0,-16} {1,4} / {2} files, {3,8:N1} MB" -f $mime, $have, $count, $mb)
}
$total = 0; foreach ($v in $rows.Values) { $total += [long]$v[1] }
Write-Host ("{0} files, {1:N0} MB in all" -f $rows.Count, ($total / 1MB))
if ($DryRun) { Save; return }

# --- downloading --------------------------------------------------------------------------
$done = 0
$headers = Join-Path $env:TEMP 'collect-commons-headers.txt'
foreach ($row in @($rows.Values)) {
    $dir = Join-Path $Out ($row[2] -replace '[/+]', '-')
    $path = Join-Path $dir $row[3]
    if ($row[0] -ne '-' -and (Test-Path $path)) { continue }
    # fetched by a run stopped before it saved the manifest
    if ((Test-Path $path) -and (Get-FileHash -Algorithm SHA1 $path).Hash.ToLower() -eq $row[8]) { $row[0] = Sha256File $path; continue }
    New-Item -ItemType Directory -Force $dir | Out-Null
    # Commons lets a client fetch a few originals, then answers 429 with the time to wait
    # (Retry-After, often ten minutes): it is waited out and the same file asked again
    $code = ''
    for ($try = 0; $try -lt 8 -and $code -ne '200'; $try++) {
        $code = curl.exe -sSL -A $ua -o $path -D $headers -w '%{http_code}' $row[7]
        if ($code -eq '200') { break }
        $wait = 60
        $ra = Select-String -Path $headers -Pattern '^retry-after:\s*(\d+)' -ErrorAction SilentlyContinue
        if ($ra) { $wait = [int]$ra.Matches[0].Groups[1].Value }
        Write-Host ("{0:HH:mm} HTTP {1}, waiting {2} s ({3} downloaded)" -f (Get-Date), $code, $wait, $done)
        Start-Sleep -Seconds $wait
    }
    if ($code -ne '200') { Write-Warning "failed: $($row[4])"; Remove-Item -ErrorAction SilentlyContinue $path; continue }
    $sha1 = (Get-FileHash -Algorithm SHA1 $path).Hash.ToLower()
    if ($sha1 -ne $row[8]) { Write-Warning "SHA-1 differs, dropped: $($row[4])"; Remove-Item $path; continue }
    $row[0] = Sha256File $path
    $done++
    if ($done % 25 -eq 0) { Save; Write-Host "$done downloaded" }
    Start-Sleep -Seconds 2
}
Save
Write-Host "$done files downloaded into $Out"
