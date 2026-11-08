# Downloads PhotoRec 7.2 for Windows from cgsecurity.org and keeps only what
# Open Recovr needs in vendor/testdisk-7.2 (photorec_win.exe, its Cygwin
# libraries and the licence files). Every kept file is checked against a known
# SHA-256, so a changed upstream archive is noticed instead of shipped.
#
#   pwsh scripts/fetch-photorec.ps1            # or: powershell -File scripts\fetch-photorec.ps1
#
# The binaries are not stored in the Git repository: run this once before
# `cargo run` if you want the deep search on Windows, and the release workflow
# runs it before packaging.

param(
    [string]$Url = "https://www.cgsecurity.org/testdisk-7.2.win64.zip",
    [string]$Zip = ""   # an already downloaded archive, to skip the download
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root "vendor/testdisk-7.2"

# path inside the kept folder -> SHA-256
$expected = [ordered]@{
    "photorec_win.exe"    = "d75b9a6ff60f91479085a578aca518abb1cd7405f24506a61afa2f7e222147bf"
    "cygewf-2.dll"        = "e904b5a3f55572fec2d08d222a6c1a54dd768bbb72e666f8eebffa0c68c19214"
    "cyggcc_s-seh-1.dll"  = "38425ee608500b6808bcfe4569ea7f6387a436e8b779979c9c3c045201b87260"
    "cygiconv-2.dll"      = "b5674bb002302ab874521499ca07edc30ace5a6beded599591743a2d9656a9cf"
    "cygjpeg-8.dll"       = "49c02ebeeb3d02ae5ef7760aba80b07f070e27100b7ac48fd5cd3fc77323df04"
    "cygncursesw-10.dll"  = "b2f0f7b56356244a4530934d95de622c58772b66bd356963ab345741a21d41a9"
    "cygwin1.dll"         = "52062be7ef8f14445a81a5dff3b3ac3c7b74466159fffd542b22a83af8ac6b86"
    "cygz.dll"            = "78d499f1d956edf00dd988c95f7f76174425c0fb47e3ddb56ba6af447f18f4f1"
    "63/cygwin"           = "1a9b75481dba09d8f73b9b13f39d52db026bfc7ae8eb4de1d250388756445dbc"
    "COPYING.txt"         = "9f2e250993c6206fac643824e05e5a0d7d3e0895d9e09a5ce4b12bc2610afc11"
    "AUTHORS.txt"         = "24f8f773d2f79b50b15acb86c2960a63eb87236bb9498358540289a116688c90"
    "THANKS.txt"          = "12b2bd649cdb8df8a649ffed716af77570982194b96a1fffabc18d830153ccc5"
    "NEWS.txt"            = "6cd6b0d6c2ac3a413ccba78533105b3987df50017569dc0481fa72311c9432d8"
    "VERSION.txt"         = "81bdb4615fad4dd3953406c3505ed2bea8cd550274bd16cbe9576f45a786a385"
    "INFO"                = "45c3a355cd86b60f4647c715dc9c6a5fd8216047416f04b32cf273b1216ed442"
    "readme.txt"          = "68af43175eeba5e1104074600f807492391f2928da944786ea1acc507dc5ce02"
}

function Test-Kept {
    foreach ($rel in $expected.Keys) {
        $p = Join-Path $dest $rel
        if (-not (Test-Path $p)) { return $false }
        if ((Get-FileHash -Algorithm SHA256 $p).Hash.ToLower() -ne $expected[$rel]) { return $false }
    }
    return $true
}

if (Test-Kept) {
    Write-Host "PhotoRec is already in $dest"
    exit 0
}

$work = Join-Path ([IO.Path]::GetTempPath()) ("open-recovr-photorec-" + [guid]::NewGuid())
New-Item -ItemType Directory -Force $work | Out-Null
try {
    if (-not $Zip) {
        $Zip = Join-Path $work "testdisk.zip"
        Write-Host "Downloading $Url"
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Invoke-WebRequest -Uri $Url -OutFile $Zip -UseBasicParsing
    }
    $unpacked = Join-Path $work "unpacked"
    Expand-Archive -Path $Zip -DestinationPath $unpacked

    $exe = Get-ChildItem -Recurse -File -Path $unpacked -Filter "photorec_win.exe" | Select-Object -First 1
    if (-not $exe) { throw "photorec_win.exe not found in the archive" }
    $src = $exe.DirectoryName

    if (Test-Path $dest) { Remove-Item -Recurse -Force $dest }
    New-Item -ItemType Directory -Force $dest | Out-Null
    foreach ($rel in $expected.Keys) {
        $from = Join-Path $src $rel
        if (-not (Test-Path $from)) {
            # the terminal description may sit under terminfo/ in the archive
            $leaf = Split-Path -Leaf $rel
            $parent = Split-Path -Leaf (Split-Path -Parent $rel)
            $hit = Get-ChildItem -Recurse -File -Path $unpacked -Filter $leaf |
                Where-Object { $parent -eq "" -or (Split-Path -Leaf $_.DirectoryName) -eq $parent } |
                Select-Object -First 1
            if (-not $hit) { throw "$rel not found in the archive" }
            $from = $hit.FullName
        }
        $to = Join-Path $dest $rel
        New-Item -ItemType Directory -Force (Split-Path -Parent $to) | Out-Null
        Copy-Item $from $to
    }
} finally {
    Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
}

if (-not (Test-Kept)) {
    throw "The downloaded files do not match the expected SHA-256: the upstream archive changed. Check it, then update the hashes in this script."
}
Write-Host "PhotoRec 7.2 is ready in $dest"
