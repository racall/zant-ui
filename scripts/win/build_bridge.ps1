# Builds zant_skia.dll: the Skia bridge (skia_bridge.cpp + skia_bridge_win.cpp)
# and the PDF backend (pdf_win.cpp), compiled with MSVC against the pinned
# rust-skia Windows archive, which is built with MSVC's STL and /MD. The rest
# of Zant UI is compiled by Zig and talks to this DLL only through the C ABI
# in skia_bridge.h / pdf.h, so the two C++ runtimes never meet.
#
# Called by build.zig on Windows hosts (cached by Zig; it reruns when a source
# file changes). Needs Visual Studio 2022+ or Build Tools with the C++
# workload, and third_party/ from scripts/fetch_skia_windows.ps1.
param(
    [Parameter(Mandatory = $true)][string]$OutDir
)
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "../..")).Path
$Skia = Join-Path $Root "third_party/skia-win/skia-binaries"
$Headers = Join-Path $Root "third_party/skia-headers"
if (!(Test-Path (Join-Path $Skia "skia.lib")) -or !(Test-Path (Join-Path $Headers "include/core/SkCanvas.h"))) {
    throw "Skia for Windows is missing; run: powershell -File scripts/fetch_skia_windows.ps1"
}

# MSVC environment (vcvars64) for this process
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
if (!(Test-Path $vswhere)) { throw "vswhere.exe not found: install Visual Studio or the Build Tools (C++ workload)" }
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (!$vs) { throw "no Visual Studio installation with the MSVC x64 tools" }
$vcvars = Join-Path $vs "VC/Auxiliary/Build/vcvars64.bat"
$envDump = cmd /c "`"$vcvars`" >nul 2>&1 && set"
foreach ($line in $envDump) {
    $eq = $line.IndexOf("=")
    if ($eq -gt 0) { [Environment]::SetEnvironmentVariable($line.Substring(0, $eq), $line.Substring($eq + 1)) }
}

New-Item -ItemType Directory -Force $OutDir | Out-Null
$Obj = Join-Path $OutDir "obj"
New-Item -ItemType Directory -Force $Obj | Out-Null

$defines = @("/DNDEBUG", "/DSK_RELEASE", "/DSK_GANESH", "/DSK_DIRECT3D", "/D_HAS_EXCEPTIONS=0",
             "/DWIN32_LEAN_AND_MEAN", "/DNOMINMAX", "/D_CRT_SECURE_NO_WARNINGS")
$includes = @("/I$Root", "/I$(Join-Path $Root 'src/renderer/skia')", "/I$Headers",
              "/I$(Join-Path $Headers 'include/private')", "/I$(Join-Path $Headers 'src')")
$sources = @(
    (Join-Path $Root "src/renderer/skia/skia_bridge.cpp"),
    (Join-Path $Root "src/renderer/skia/skia_bridge_win.cpp"),
    (Join-Path $Root "src/platform/windows/pdf_win.cpp")
)
& cl.exe /nologo /c /std:c++20 /O2 /MD /EHs-c- /GR- /utf-8 /permissive- /Zc:__cplusplus /W3 /wd4244 /wd4267 /wd4800 /wd5030 `
    @defines @includes "/Fo$Obj\" @sources
if ($LASTEXITCODE -ne 0) { throw "cl.exe failed" }

# ICU common data as an RCDATA resource (see zant_skia_resources.h)
# and STIX Two Math, which Windows does not install (KaTeX layout)
$icu = (Join-Path $Skia "icudtl.dat").Replace("\", "\\")
$mathFont = Join-Path $Root "third_party/fonts/STIXTwoMath-Regular.otf"
if (!(Test-Path $mathFont)) { throw "STIX Two Math is missing; run: powershell -File scripts/fetch_skia_windows.ps1" }
$math = $mathFont.Replace("\", "\\").Replace("/", "\\")
$rc = Join-Path $Obj "zant_skia.rc"
Set-Content -Encoding ASCII $rc "#include `"zant_skia_resources.h`"`r`nZANT_ICU_DATA RCDATA `"$icu`"`r`nZANT_MATH_FONT RCDATA `"$math`"`r`n"
& rc.exe /nologo "/I$(Join-Path $Root 'src/renderer/skia')" "/fo$(Join-Path $Obj 'zant_skia.res')" $rc
if ($LASTEXITCODE -ne 0) { throw "rc.exe failed" }

$objects = @(Get-ChildItem $Obj -Filter *.obj | ForEach-Object { $_.FullName }) + (Join-Path $Obj "zant_skia.res")
$skiaLibs = @("skparagraph.lib", "skshaper.lib", "skunicode_icu.lib", "skunicode_core.lib", "skia.lib") |
    ForEach-Object { Join-Path $Skia $_ }
$systemLibs = @("d3d12.lib", "dxgi.lib", "d3dcompiler.lib", "dwrite.lib", "opengl32.lib", "usp10.lib",
                "fontsub.lib", "ole32.lib", "oleaut32.lib", "user32.lib", "gdi32.lib", "advapi32.lib",
                "shell32.lib", "windowscodecs.lib")
& link.exe /nologo /DLL /OPT:REF /OPT:ICF /INCREMENTAL:NO `
    "/OUT:$(Join-Path $OutDir 'zant_skia.dll')" "/IMPLIB:$(Join-Path $OutDir 'zant_skia_msvc.lib')" `
    "/DEF:$(Join-Path $Root 'src/renderer/skia/zant_skia.def')" @objects @skiaLibs @systemLibs
if ($LASTEXITCODE -ne 0) { throw "link.exe failed" }

# PDFium for ui.pdfPreview, next to the bridge
$pdfium = Join-Path $Root "third_party/pdfium-win/bin/pdfium.dll"
if (Test-Path $pdfium) { Copy-Item -Force $pdfium (Join-Path $OutDir "pdfium.dll") }
Write-Host "zant_skia.dll -> $OutDir"
