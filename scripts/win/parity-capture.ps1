# Captures the Zig baseline and the Rust port of the parity examples on the
# Windows test machine: PNG screenshot + layout dump per theme/window size
# (docs/rust-port/plan.md §6 / §7.2). The diffs run on the Mac with
# scripts/parity/{layoutdiff,imgdiff}.py after copying the output directory
# back (scripts/win/parity-compare.sh does both).
#
#   powershell -File scripts/win/parity-capture.ps1 -Out target\win-parity
#   ... -Apps zant-demo,zant-components -Sizes 600x620 -Themes light
#
# The snapshot path works in an SSH session (the renderer falls back to the
# CPU raster backend; D3D12 needs a real desktop). Windows note: Zig needs
# zant_skia.dll / pdfium.dll next to the exe, which `zig build` installs into
# zig-out\bin.
param(
    [string]$Apps = "zant-demo,zant-gallery,zant-components,zant-markdown,zant-math,zant-md-stream,zant-pdf",
    [string]$Repo = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ZigDir = "",
    [string]$RustDir = "",
    [string]$Out = "",
    [string]$Sizes = "600x620,900x640,1200x800",
    [string]$Themes = "light,dark",
    [int]$TimeoutSec = 120
)
$ErrorActionPreference = "Stop"
if (!$ZigDir) { $ZigDir = Join-Path $Repo "zig-out/bin" }
if (!$RustDir) { $RustDir = Join-Path $Repo "target/release" }
if (!$Out) { $Out = Join-Path $env:TEMP "zant-parity" }
New-Item -ItemType Directory -Force $Out | Out-Null
$summary = New-Object System.Collections.Generic.List[string]
$appList = @($Apps -split "," | Where-Object { $_ })
$sizeList = @($Sizes -split "," | Where-Object { $_ })
$themeList = @($Themes -split "," | Where-Object { $_ })

# zant-pdf opens a document given on the command line (the Zig example too).
function App-Arguments([string]$app) {
    if ($app -eq "zant-pdf") { return "tests/fixtures/widgets.pdf" }
    return ""
}

# The Zig tree names two TSX examples differently from the Rust workspace.
function Zig-Name([string]$app) {
    switch ($app) {
        "zant-tsx-demo" { return "zant-tsx-app" }
        "zant-tsx-bench-app" { return "zant-tsx-bench" }
        default { return $app }
    }
}

# zant-md-stream streams text over time; the 600 ms snapshot would catch a
# different amount of text on the two implementations (different rebuild
# speed), so the parity matrix feeds the whole reply in one delta.
function App-Env([string]$app) {
    if ($app -eq "zant-md-stream") {
        return @{ ZANT_STREAM_CHARS = "1000000"; ZANT_STREAM_MS = "0" }
    }
    return @{}
}

function Run-Capture([string]$exe, [string]$dir, [string]$theme, [string]$size, [string]$arguments, [hashtable]$extraEnv) {
    New-Item -ItemType Directory -Force $dir | Out-Null
    $shot = Join-Path $dir "shot.png"
    $layout = Join-Path $dir "layout.txt"
    $stdout = Join-Path $dir "stdout.log"
    $stderr = Join-Path $dir "stderr.log"
    Remove-Item -Force -ErrorAction SilentlyContinue $shot, $layout, $stdout, $stderr

    Remove-Item Env:ZANT_STREAM_CHARS -ErrorAction SilentlyContinue
    Remove-Item Env:ZANT_STREAM_MS -ErrorAction SilentlyContinue
    foreach ($name in $extraEnv.Keys) { Set-Item "env:$name" $extraEnv[$name] }

    $env:ZANT_NO_MOTION = "1"
    $env:ZANT_SNAPSHOT = $shot
    $env:ZANT_SNAPSHOT_EXIT = "1"
    $env:ZANT_LAYOUT_FILE = $layout
    $env:ZANT_WINDOW_SIZE = $size
    $env:ZANT_THEME = $theme
    if ($theme -eq "dark") { $env:ZANT_DARK = "1" } else { Remove-Item Env:ZANT_DARK -ErrorAction SilentlyContinue }

    $exit = -1
    try {
        $startArgs = @{
            FilePath = $exe
            WorkingDirectory = $Repo
            NoNewWindow = $true
            PassThru = $true
            RedirectStandardOutput = $stdout
            RedirectStandardError = $stderr
        }
        if ($arguments) { $startArgs.ArgumentList = $arguments }
        $p = Start-Process @startArgs
        if ($p.WaitForExit($TimeoutSec * 1000)) {
            $exit = $p.ExitCode
        } else {
            $p.Kill()
            $exit = -999
        }
    } catch {
        Add-Content -Path $stderr -Value ("start failed: " + $_.Exception.Message)
    }

    $png = (Test-Path $shot) -and ((Get-Item $shot).Length -gt 0)
    $lay = (Test-Path $layout) -and ((Get-Item $layout).Length -gt 0)
    return "exit=$exit png=$([int]$png) layout=$([int]$lay)"
}

foreach ($app in $appList) {
    $zig = Join-Path $ZigDir "$(Zig-Name $app).exe"
    $rust = Join-Path $RustDir "$app.exe"
    foreach ($side in @("zig", "rust")) {
        $exe = if ($side -eq "zig") { $zig } else { $rust }
        if (!(Test-Path $exe)) {
            $summary.Add("MISSING $app $side $exe")
            Write-Output "MISSING $app $side $exe"
            continue
        }
        foreach ($theme in $themeList) {
            foreach ($size in $sizeList) {
                $name = "$theme-$size"
                $dir = Join-Path (Join-Path $Out $app) "$side-$name"
                $result = Run-Capture $exe $dir $theme $size (App-Arguments $app) (App-Env $app)
                $line = "$app|$side|$name|$result"
                $summary.Add($line)
                Write-Output $line
            }
        }
    }
}

$summaryPath = Join-Path $Out "summary.txt"
Set-Content -Path $summaryPath -Value $summary -Encoding UTF8
Write-Output ("wrote " + $summaryPath)
