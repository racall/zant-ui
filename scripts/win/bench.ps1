# Runs the headless frame benchmark (`zant-bench`) on the Windows test machine:
# the Rust port (target\release) and the Zig baseline (zig-out\bin), every
# scenario, in both render modes, -Repeat rounds each. Logs land in -Out and
# scripts/win/bench-compare.sh turns them into the plan §7.3 gate table.
#
#   powershell -File scripts/win/bench.ps1 -Out target\win-bench -Repeat 3
#   ... -Scenarios "open,stream"   -Messages 30
param(
    [string]$Repo = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$Out = "",
    [int]$Repeat = 3,
    [string]$Scenarios = "",
    [string]$Messages = "30",
    [string]$RustBin = "",
    [string]$ZigBin = ""
)
$ErrorActionPreference = "Continue"
if (!$Out) { $Out = Join-Path $env:TEMP "zant-bench" }
if (!$RustBin) { $RustBin = Join-Path $Repo "target/release/zant-bench.exe" }
if (!$ZigBin) { $ZigBin = Join-Path $Repo "zig-out/bin/zant-bench.exe" }
foreach ($exe in @($RustBin, $ZigBin)) {
    if (!(Test-Path $exe)) { throw "missing benchmark: $exe" }
}
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$args = @()
if ($Scenarios) { $args = @($Scenarios -split "," | Where-Object { $_ }) }
$env:ZANT_BENCH_MESSAGES = $Messages

foreach ($mode in @("default", "list")) {
    if ($mode -eq "list") { $env:ZANT_BENCH_LIST = "1" } else { Remove-Item Env:ZANT_BENCH_LIST -ErrorAction SilentlyContinue }
    for ($round = 1; $round -le $Repeat; $round++) {
        # alternate which side runs first (machine noise should hit both)
        $sides = if ($round % 2 -eq 0) { @("rust", "zig") } else { @("zig", "rust") }
        foreach ($side in $sides) {
            $exe = if ($side -eq "rust") { $RustBin } else { $ZigBin }
            $log = Join-Path $Out ("{0}-{1}-r{2}.log" -f $side, $mode, $round)
            $started = Get-Date
            (& $exe @args 2>&1) | ForEach-Object { "$_" } | Set-Content -Encoding UTF8 $log
            $ms = [int]((Get-Date) - $started).TotalMilliseconds
            Write-Output ("{0} {1} round {2}: exit {3} in {4} ms" -f $side, $mode, $round, $LASTEXITCODE, $ms)
        }
    }
}
Remove-Item Env:ZANT_BENCH_LIST -ErrorAction SilentlyContinue
Write-Output ("wrote " + $Out)
