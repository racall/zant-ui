# Builds the Windows release archive that install.ps1 downloads:
#
#   powershell -File scripts/release/package.ps1 -Version v0.1.0 [-Out dist] [-AllowNoPrebuilt]
#
# Run from a clean checkout after scripts/fetch_skia_windows.ps1, on a machine
# with Visual Studio (C++) and Rust on PATH (or $env:CARGO): rustup installs
# the toolchain pinned in rust-toolchain.toml, so no Zig is needed any more.
# WP-6.6: apps in the archive link the prebuilt native layer instead of
# compiling C++/C, so this needs scripts/release/prebuilt.ps1 to have run (or
# $env:ZANT_NATIVE_PREBUILT set to a directory made by it); -AllowNoPrebuilt
# packages without it.
# Produces <out>/zant-windows-x64.zip and its .sha256:
#   bin/zant.exe                         the CLI (finds zant-ui\ next to bin\)
#   zant-ui/                             the committed Rust workspace (git archive):
#                                        Cargo.toml, Cargo.lock, rust-toolchain.toml,
#                                        crates/ and the native sources it compiles
#                                        (src/renderer/skia, src/platform)
#   zant-ui/third_party/                 Skia libraries, headers, PDFium and fonts
#   zant-ui/third_party/zant-native-prebuilt/
#                                        zant_native.lib + metadata.txt (WP-6.6):
#                                        app builds link this instead of compiling
#                                        the native layer; build.rs falls back to
#                                        compiling if it is stale
#   zant-ui/.zant-version
#
# The Skia bridge is linked into the app statically in the Rust port, so there
# is no zant_skia.dll / zant-skia-prebuilt/ in the archive (Zig: both existed).
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [string]$Out = 'dist',
    [switch]$AllowNoPrebuilt
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$Cargo = if ($env:CARGO) { $env:CARGO } else { 'cargo' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Name = 'zant-windows-x64.zip'
Set-Location $Root

if (!(Test-Path 'third_party/skia-win/skia-binaries/skia.lib')) { throw 'run scripts/fetch_skia_windows.ps1 first' }
if (!(Test-Path 'third_party/skia-headers/include/core/SkSurface.h')) { throw 'run scripts/fetch_skia_windows.ps1 first' }

# WP-6.6: the prebuilt native library the archive ships. $env:ZANT_NATIVE_PREBUILT
# is the same variable build.rs reads, so setting it here also makes the
# packaging build skip the C++ compile. Otherwise use the directory
# scripts/release/prebuilt.ps1 wrote by default.
$prebuilt = $null
foreach ($candidate in @($env:ZANT_NATIVE_PREBUILT, (Join-Path $TargetDir 'prebuilt/windows'))) {
    if ($candidate -and (Test-Path (Join-Path $candidate 'zant_native.lib'))) { $prebuilt = $candidate; break }
}
if (!$prebuilt) {
    if ($AllowNoPrebuilt) {
        Write-Host 'warning: no prebuilt native library: the archive will need a native C++/C compile' -ForegroundColor Yellow
    } else {
        throw 'no prebuilt native library: run scripts/release/prebuilt.ps1 first (or set $env:ZANT_NATIVE_PREBUILT, or pass -AllowNoPrebuilt to ship without it)'
    }
}
if ($prebuilt) {
    $metadata = Join-Path $prebuilt 'metadata.txt'
    if (!(Test-Path $metadata)) { throw "$metadata is missing (not a directory made by scripts/release/prebuilt.ps1)" }
    if (!(Select-String -Quiet -Path $metadata -Pattern '^platform=windows$')) { throw "$metadata is not a Windows archive" }
    if (!(Select-String -Quiet -Path $metadata -Pattern '^source_hash=.+')) { throw "$metadata has no source_hash" }
    # build.rs reads this variable too: the packaging build below links it.
    $env:ZANT_NATIVE_PREBUILT = $prebuilt
}

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

    if ($prebuilt) {
        $prebuiltOut = Join-Path $tp 'zant-native-prebuilt'
        New-Item -ItemType Directory -Force $prebuiltOut | Out-Null
        Copy-Item (Join-Path $prebuilt 'zant_native.lib'), (Join-Path $prebuilt 'metadata.txt') $prebuiltOut
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
