# Builds the prebuilt native library that released apps link instead of
# compiling C++/C (WP-6.6, docs/rust-port/plan.md §5). Same idea as the Zig
# build's third_party/zant-skia-prebuilt: the release workflow compiles the
# native layer once per platform and ships the archive inside
# zant-ui\third_party\zant-native-prebuilt\.
#
#   powershell -File scripts/release/prebuilt.ps1 [-Out target\prebuilt\windows]
#
# Writes <out>\zant_native.lib + metadata.txt (default target\prebuilt\windows;
# a relative -Out is resolved against the repo root). scripts\release\package.ps1
# copies it into the release archive.
#
# Needs the pinned Skia (scripts\fetch_skia_windows.ps1), Visual Studio's C++
# tools (MSVC) and Rust on PATH (or $env:CARGO). Cargo finds MSVC through
# vswhere, so no vcvars shell is required. The archive itself is produced by
# crates\zant-sys\build.rs when ZANT_NATIVE_PREBUILT_OUT is set, so the source
# hash in the metadata is computed by the same code that verifies it on the
# user's machine.
#
# NOTE: 待验证 — the Windows prebuilt path has not been exercised on a Windows
# machine yet (the test machine is unreachable). prebuilt-check.sh only runs on
# macOS; treat a green Windows run of this script as unconfirmed until then.
param([string]$Out = 'target\prebuilt\windows')
$ErrorActionPreference = 'Stop'

$Root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$Cargo = if ($env:CARGO) { $env:CARGO } else { 'cargo' }
$OutDir = if ([IO.Path]::IsPathRooted($Out)) { $Out } else { Join-Path $Root $Out }
Set-Location $Root

function Die([string]$Text) { Write-Host "prebuilt.ps1: $Text" -ForegroundColor Red; exit 1 }

if (!(Test-Path 'third_party/skia-win/skia-binaries/skia.lib')) { Die 'Skia is missing; run scripts/fetch_skia_windows.ps1 first' }
if (!(Test-Path 'third_party/skia-headers/include/core/SkSurface.h')) { Die 'Skia headers are missing; run scripts/fetch_skia_windows.ps1 first' }

& $Cargo --version | Out-Null
if ($LASTEXITCODE -ne 0) { Die 'cargo not found; set $env:CARGO' }

# Best effort toolchain description for the metadata (not used for verification).
$commit = 'unknown'
try { $commit = (& git rev-parse HEAD) } catch { $commit = 'unknown' }
if (!$commit) { $commit = 'unknown' }
$rustc = (& rustc --version) -join ' '
$msvc = 'unknown'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path $vswhere) {
    $vsVersion = & $vswhere -latest -products * -property installationVersion 2>$null
    if ($vsVersion) { $msvc = "MSVC tools $vsVersion" }
}
$toolchain = "$rustc; $msvc"

if (Test-Path $OutDir) { Remove-Item -Recurse -Force $OutDir }
New-Item -ItemType Directory -Force $OutDir | Out-Null

Write-Host 'prebuilt.ps1: building the native layer (cargo build --release --locked -p zant-sys)' -ForegroundColor Cyan
$env:ZANT_NATIVE_PREBUILT_OUT = $OutDir
$env:ZANT_NATIVE_PREBUILT_COMMIT = $commit
$env:ZANT_NATIVE_PREBUILT_TOOLCHAIN = $toolchain
& $Cargo build --release --locked -p zant-sys
if ($LASTEXITCODE -ne 0) { Die 'cargo build --release --locked -p zant-sys failed' }

# cc's create_archive on MSVC writes both zant_native.lib (the real archive) and
# a libzant_native.a copy; package only the .lib and the metadata.
$extra = Join-Path $OutDir 'libzant_native.a'
if (Test-Path $extra) { Remove-Item $extra }
$lib = Join-Path $OutDir 'zant_native.lib'
$metadata = Join-Path $OutDir 'metadata.txt'
if (!(Test-Path $lib)) { Die "build.rs did not write $lib" }
if (!(Test-Path $metadata)) { Die "build.rs did not write $metadata" }

Write-Host ''
Write-Host "prebuilt native library: $OutDir"
Get-ChildItem $OutDir | Format-Table Name, Length
$sourceHash = (Get-Content $metadata | Select-String '^source_hash=' | Select-Object -First 1).Line
Write-Host "$sourceHash"
Write-Host "commit:    $commit"
Write-Host "toolchain: $toolchain"
Write-Host ''
Write-Host 'next: powershell -File scripts/release/package.ps1 -Version <version>'
