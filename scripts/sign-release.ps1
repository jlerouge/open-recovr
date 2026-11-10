<#
Signs a release's SHA256SUMS.txt with the Open Recovr key, on your own computer.

The release workflow publishes each version as a draft. Download SHA256SUMS.txt from the draft's
page, run this script, attach the SHA256SUMS.txt.minisig it writes to the draft, then publish it.
Open Recovr installs an update only when that signature checks out against keys/open-recovr.pub.

  ./scripts/sign-release.ps1 -Tag v0.3.0
  ./scripts/sign-release.ps1 -Tag v0.3.0 -Sums D:\SHA256SUMS.txt -SecretKey E:\open-recovr.key

Needs minisign (winget install jedisct1.minisign) and the secret key made as keys/README.md says.
#>
param(
    [Parameter(Mandatory = $true)][string]$Tag,
    [string]$Sums = (Join-Path $env:USERPROFILE 'Downloads\SHA256SUMS.txt'),
    [string]$SecretKey = (Join-Path $env:USERPROFILE '.minisign\open-recovr.key')
)
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$pub = Join-Path $root 'keys\open-recovr.pub'

if (-not (Get-Command minisign -ErrorAction SilentlyContinue)) {
    throw 'minisign is missing: winget install jedisct1.minisign'
}
if (-not ((Get-Content $pub -TotalCount 2) -match '^RW')) {
    throw "keys\open-recovr.pub holds no key yet: see keys\README.md"
}
foreach ($f in @($Sums, $SecretKey)) {
    if (-not (Test-Path $f)) { throw "Not found: $f" }
}

# 1. The release was built from the commit you have here, not from one pushed by someone else.
$local = (git -C $root rev-parse "$Tag^{commit}").Trim()
if ($LASTEXITCODE -ne 0) { throw "No tag $Tag here: git fetch --tags" }
$refs = git -C $root ls-remote origin "refs/tags/$Tag" "refs/tags/$Tag^{}"
$peeled = $refs | Where-Object { $_ -match '\^\{\}$' }
$line = if ($peeled) { $peeled } else { $refs | Select-Object -First 1 }
$remote = if ($line) { ($line -split '\s+')[0] } else { '' }
if ($remote -ne $local) {
    throw "On GitHub, $Tag points to '$remote', here to '$local'. Find out why before signing anything."
}
Write-Host "Built from $local : $(git -C $root log -1 --format=%s $local)"

# 2. The checksums are this version's, for the files the update fetches.
$text = Get-Content $Sums
$version = $Tag.TrimStart('v')
foreach ($want in @("open-recovr-$Tag-windows-x64.zip", "open-recovr-v$version-windows-x64-setup.exe")) {
    if (-not ($text | Where-Object { $_ -match ([regex]::Escape($want) + '$') })) {
        throw "$want is not in ${Sums}: is it the file of the $Tag draft?"
    }
}
$text | ForEach-Object { Write-Host "  $_" }

# 3. Sign: the trusted comment names the version, which Open Recovr checks.
& minisign -S -s $SecretKey -m $Sums -t "open-recovr $Tag"
if ($LASTEXITCODE -ne 0) { throw 'minisign could not sign' }
& minisign -V -p $pub -m $Sums
if ($LASTEXITCODE -ne 0) { throw 'The signature does not check out against keys\open-recovr.pub' }

Write-Host ""
Write-Host "Attach $Sums.minisig to the $Tag draft on GitHub, then publish it."
