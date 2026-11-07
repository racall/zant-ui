# Builds the Windows release archive that install.ps1 downloads:
#
#   powershell -File scripts/release/package.ps1 -Version v0.1.0 [-Out dist]
#
# Run from a clean checkout after scripts/fetch_skia_windows.ps1, on a
# machine with Visual Studio (C++) and Zig on PATH (or $env:ZIG). Produces
# <out>/zant-windows-x64.zip and its .sha256:
#   bin/zant.exe                         the CLI (finds zant-ui\ next to bin\)
#   zant-ui/                             the committed sources (git archive), plus
#   zant-ui/third_party/                 Skia, headers, PDFium, fonts and
#   zant-ui/third_party/zant-skia-prebuilt/  zant_skia.dll built here with the
#                                        MSVC runtime, so app developers need no Visual Studio
#   zant-ui/.zant-version
param([Parameter(Mandatory = $true)][string]$Version, [string]$Out = 'dist')
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Zig = if ($env:ZIG) { $env:ZIG } else { 'zig' }
$Name = 'zant-windows-x64.zip'
Set-Location $Root

if (!(Test-Path 'third_party/skia-win/skia-binaries/skia.lib')) { throw 'run scripts/fetch_skia_windows.ps1 first' }
& $Zig build cli -Doptimize=ReleaseSafe
if ($LASTEXITCODE -ne 0) { throw 'zig build cli failed' }
# the bridge (MSVC) and pdfium.dll, into zig-out/bin
& $Zig build zant-runtime
if ($LASTEXITCODE -ne 0) { throw 'zig build zant-runtime failed' }

$stage = Join-Path ([IO.Path]::GetTempPath()) ('zant-release-' + [guid]::NewGuid().ToString('N'))
try {
    $ui = Join-Path $stage 'zant-ui'
    New-Item -ItemType Directory -Force (Join-Path $stage 'bin'), $ui | Out-Null
    Copy-Item 'zig-out/bin/zant.exe' (Join-Path $stage 'bin/zant.exe')
    git archive --format=tar -o (Join-Path $stage 'src.tar') HEAD
    if ($LASTEXITCODE -ne 0) { throw 'git archive failed' }
    tar -xf (Join-Path $stage 'src.tar') -C $ui
    Remove-Item (Join-Path $stage 'src.tar')

    $tp = Join-Path $ui 'third_party'
    New-Item -ItemType Directory -Force $tp | Out-Null
    foreach ($dir in 'skia-win', 'pdfium-win', 'fonts') { Copy-Item -Recurse "third_party/$dir" (Join-Path $tp $dir) }
    robocopy 'third_party/skia-headers' (Join-Path $tp 'skia-headers') /E /XD .git /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw 'copying skia-headers failed' }
    $prebuilt = Join-Path $tp 'zant-skia-prebuilt'
    New-Item -ItemType Directory -Force $prebuilt | Out-Null
    foreach ($dll in 'zant_skia.dll', 'pdfium.dll') { Copy-Item "zig-out/bin/$dll" $prebuilt }
    foreach ($dll in 'vcruntime140.dll', 'vcruntime140_1.dll', 'msvcp140.dll') {
        Copy-Item (Join-Path $env:SystemRoot "System32/$dll") $prebuilt
    }
    Set-Content -NoNewline -Path (Join-Path $ui '.zant-version') -Value $Version

    New-Item -ItemType Directory -Force $Out | Out-Null
    $zip = Join-Path (Resolve-Path $Out) $Name
    if (Test-Path $zip) { Remove-Item $zip }
    Push-Location $stage
    try { tar.exe -a -c -f $zip bin zant-ui } finally { Pop-Location }
    if ($LASTEXITCODE -ne 0) { throw 'zip failed' }
    (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower() | Set-Content -NoNewline -Path "$zip.sha256"
    Write-Host "$zip ($([math]::Round((Get-Item $zip).Length / 1MB)) MB)"
} finally {
    Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
}
