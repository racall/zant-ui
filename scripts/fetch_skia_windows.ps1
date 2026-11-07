# Fetches the pinned Windows dependencies of Zant UI into third_party/:
#   skia-win/     rust-skia 0.157.0 prebuilt Skia (MSVC, D3D12 + textlayout)
#   skia-headers/ matching Skia headers (m157.2), shared with macOS
#   pdfium-win/   PDFium (ui.pdfPreview)
#   fonts/        STIX Two Math (OFL 1.1; macOS has it, Windows does not)
# Usage (PowerShell 7 or Windows PowerShell 5.1):
#   pwsh -File scripts/fetch_skia_windows.ps1
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$Root = Split-Path -Parent $PSScriptRoot
$ThirdParty = Join-Path $Root "third_party"
New-Item -ItemType Directory -Force $ThirdParty | Out-Null

$Release = "0.157.0"
$Asset = "skia-binaries-30669978c3f39a73e8a7-x86_64-pc-windows-msvc-d3d-ganesh-gl-jpegd-jpege-pdf-textlayout.tar.gz"
$SkiaSha = "d9a2970ca3929659f83d161c8e4a1a001f57c1fbfb40fee1d347705809d56c0e"
$HeadersRef = "m157.2"
$HeadersCommit = "6a8fe80eadfac737c12e64229a1b9530bc78f3d1"
$PdfiumRelease = "chromium/8111"
$PdfiumSha = "3452d11031050ec65e57f180215b57b9e6d465e3fbedafa8a77ad90885c7d23e"
$StixUrl = "https://mirrors.ctan.org/fonts/stix2-otf/STIXTwoMath-Regular.otf"
$StixSha = "95bc2729e41faf93b0bcae9e96c4dc4da45855067fd0581e621e30734fe8d90b"

function Get-Pinned([string]$Url, [string]$Path, [string]$Sha) {
    if (Test-Path $Path) {
        if ((Get-FileHash $Path -Algorithm SHA256).Hash.ToLower() -eq $Sha) { return }
        Remove-Item -Force $Path
    }
    Write-Host "Downloading $Url"
    curl.exe -fsSL -o $Path $Url
    if ($LASTEXITCODE -ne 0) { throw "download failed: $Url" }
    $actual = (Get-FileHash $Path -Algorithm SHA256).Hash.ToLower()
    if ($actual -ne $Sha) {
        Remove-Item -Force $Path
        throw "checksum mismatch for $Path (got $actual)"
    }
}

# A folder's .zant-release records the archive unpacked there, so a newer
# pin (after `zant update` pulls) downloads again; a folder from before the
# stamp existed is taken as the current pin.
function Sync-Pin([string]$Dir, [string]$Marker, [string]$Pin) {
    $stamp = Join-Path $Dir ".zant-release"
    if ((Test-Path $Marker) -and !(Test-Path $stamp)) { Set-Content -NoNewline -Path $stamp -Value $Pin }
    if ((Test-Path $stamp) -and ((Get-Content -Raw $stamp) -ne $Pin)) {
        Write-Host "Pin changed: replacing $Dir"
        Remove-Item -Recurse -Force $Dir
    }
}

# Skia binaries
$SkiaDir = Join-Path $ThirdParty "skia-win"
$SkiaArchive = Join-Path $ThirdParty $Asset
Sync-Pin $SkiaDir (Join-Path $SkiaDir "skia-binaries/skia.lib") "$Release $SkiaSha"
if (!(Test-Path (Join-Path $SkiaDir "skia-binaries/skia.lib"))) {
    Get-Pinned "https://github.com/rust-skia/skia-binaries/releases/download/$Release/$Asset" $SkiaArchive $SkiaSha
    New-Item -ItemType Directory -Force $SkiaDir | Out-Null
    tar -xzf $SkiaArchive -C $SkiaDir
    if ($LASTEXITCODE -ne 0) { throw "could not unpack $SkiaArchive" }
    Set-Content -NoNewline -Path (Join-Path $SkiaDir ".zant-release") -Value "$Release $SkiaSha"
}

# Skia headers (sparse checkout, same as scripts/fetch_skia.sh)
$Headers = Join-Path $ThirdParty "skia-headers"
if ((Test-Path (Join-Path $Headers ".git")) -and ((git -C $Headers rev-parse HEAD).Trim() -ne $HeadersCommit)) {
    Write-Host "Skia headers pin changed: fetching again"
    Remove-Item -Recurse -Force $Headers
}
if (!(Test-Path (Join-Path $Headers "include/core/SkCanvas.h"))) {
    if (Test-Path $Headers) { Remove-Item -Recurse -Force $Headers }
    git clone --depth 1 --branch $HeadersRef --filter=blob:none --sparse https://github.com/rust-skia/skia.git $Headers
    if ($LASTEXITCODE -ne 0) { throw "git clone failed" }
    git -C $Headers sparse-checkout set include modules/skparagraph/include modules/skshaper/include modules/skunicode/include modules/skcms src/core src/utils
    if ($LASTEXITCODE -ne 0) { throw "sparse checkout failed" }
}
$head = (git -C $Headers rev-parse HEAD).Trim()
if ($head -ne $HeadersCommit) { throw "skia-headers is at $head, expected $HeadersCommit" }

# PDFium
$PdfiumDir = Join-Path $ThirdParty "pdfium-win"
Sync-Pin $PdfiumDir (Join-Path $PdfiumDir "bin/pdfium.dll") "$PdfiumRelease $PdfiumSha"
if (!(Test-Path (Join-Path $PdfiumDir "bin/pdfium.dll"))) {
    $PdfiumArchive = Join-Path $ThirdParty "pdfium-win-x64.tgz"
    Get-Pinned "https://github.com/bblanchon/pdfium-binaries/releases/download/$PdfiumRelease/pdfium-win-x64.tgz" $PdfiumArchive $PdfiumSha
    New-Item -ItemType Directory -Force $PdfiumDir | Out-Null
    tar -xzf $PdfiumArchive -C $PdfiumDir
    if ($LASTEXITCODE -ne 0) { throw "could not unpack $PdfiumArchive" }
    Set-Content -NoNewline -Path (Join-Path $PdfiumDir ".zant-release") -Value "$PdfiumRelease $PdfiumSha"
}

# STIX Two Math
$Fonts = Join-Path $ThirdParty "fonts"
New-Item -ItemType Directory -Force $Fonts | Out-Null
Get-Pinned $StixUrl (Join-Path $Fonts "STIXTwoMath-Regular.otf") $StixSha

Write-Host "Windows dependencies ready in $ThirdParty"
