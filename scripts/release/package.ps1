# Builds the Windows release archive that install.ps1 downloads:
#
#   powershell -File scripts/release/package.ps1 -Version v0.1.0 [-Out dist]
#
# Run from a clean checkout after scripts/fetch_skia_windows.ps1, on a machine
# with Visual Studio (C++) and Rust on PATH (or $env:CARGO): rustup installs
# the toolchain pinned in rust-toolchain.toml, so no Zig is needed any more.
# Produces <out>/zant-windows-x64.zip and its .sha256:
#   bin/zant.exe                         the CLI (finds zant-ui\ next to bin\)
#   zant-ui/                             the committed Rust workspace (git archive):
#                                        Cargo.toml, Cargo.lock, rust-toolchain.toml,
#                                        crates/ and the native sources it compiles
#                                        (src/renderer/skia, src/platform)
#   zant-ui/third_party/                 Skia libraries, headers, PDFium and fonts
#   zant-ui/.zant-version
#
# The Skia bridge is linked into the app statically in the Rust port, so there
# is no zant_skia.dll / zant-skia-prebuilt/ in the archive (Zig: both existed).
param([Parameter(Mandatory = $true)][string]$Version, [string]$Out = 'dist')
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Cargo = if ($env:CARGO) { $env:CARGO } else { 'cargo' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Name = 'zant-windows-x64.zip'
Set-Location $Root

if (!(Test-Path 'third_party/skia-win/skia-binaries/skia.lib')) { throw 'run scripts/fetch_skia_windows.ps1 first' }
if (!(Test-Path 'third_party/skia-headers/include/core/SkSurface.h')) { throw 'run scripts/fetch_skia_windows.ps1 first' }
# --locked keeps the release on the committed Cargo.lock; Rust targets the
# baseline CPU by default (Zig needed -Dcpu=baseline).
& $Cargo build --release --locked -p zant-cli --bin zant
if ($LASTEXITCODE -ne 0) { throw 'cargo build -p zant-cli failed' }

$stage = Join-Path ([IO.Path]::GetTempPath()) ('zant-release-' + [guid]::NewGuid().ToString('N'))
try {
    $ui = Join-Path $stage 'zant-ui'
    New-Item -ItemType Directory -Force (Join-Path $stage 'bin'), $ui | Out-Null
    Copy-Item (Join-Path $TargetDir 'release/zant.exe') (Join-Path $stage 'bin/zant.exe')
    git archive --format=tar -o (Join-Path $stage 'src.tar') HEAD
    if ($LASTEXITCODE -ne 0) { throw 'git archive failed' }
    tar -xf (Join-Path $stage 'src.tar') -C $ui
    Remove-Item (Join-Path $stage 'src.tar')

    $tp = Join-Path $ui 'third_party'
    New-Item -ItemType Directory -Force $tp | Out-Null
    foreach ($dir in 'skia-win', 'pdfium-win', 'fonts') { Copy-Item -Recurse "third_party/$dir" (Join-Path $tp $dir) }
    robocopy 'third_party/skia-headers' (Join-Path $tp 'skia-headers') /E /XD .git /NFL /NDL /NJH /NJS /NP | Out-Null
    if ($LASTEXITCODE -ge 8) { throw 'copying skia-headers failed' }
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
