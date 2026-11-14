# Runs the headless frame benchmark (`zant-bench`, target\release) on the
# Windows test machine: every scenario, in both render modes, -Repeat rounds
# each. Logs (`<mode>-r<round>.log`, one summary line per scenario) land in
# -Out.
#
#   powershell -File scripts/win/bench.ps1 -Out target\win-bench -Repeat 3
#   ... -Scenarios "open,stream"   -Messages 30
param(
    [string]$Repo = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$Out = "",
    [int]$Repeat = 3,
    [string]$Scenarios = "",
    [string]$Messages = "30",
    [string]$Exe = ""
)
$ErrorActionPreference = "Continue"
if (!$Out) { $Out = Join-Path $env:TEMP "zant-bench" }
if (!$Exe) { $Exe = Join-Path $Repo "target/release/zant-bench.exe" }
if (!(Test-Path $Exe)) { throw "missing benchmark: $Exe" }
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$args = @()
if ($Scenarios) { $args = @($Scenarios -split "," | Where-Object { $_ }) }
$env:ZANT_BENCH_MESSAGES = $Messages

foreach ($mode in @("default", "list")) {
    if ($mode -eq "list") { $env:ZANT_BENCH_LIST = "1" } else { Remove-Item Env:ZANT_BENCH_LIST -ErrorAction SilentlyContinue }
    for ($round = 1; $round -le $Repeat; $round++) {
        $log = Join-Path $Out ("{0}-r{1}.log" -f $mode, $round)
        $started = Get-Date
        (& $Exe @args 2>&1) | ForEach-Object { "$_" } | Set-Content -Encoding UTF8 $log
        $ms = [int]((Get-Date) - $started).TotalMilliseconds
        Write-Output ("{0} round {1}: exit {2} in {3} ms" -f $mode, $round, $LASTEXITCODE, $ms)
    }
}
Remove-Item Env:ZANT_BENCH_LIST -ErrorAction SilentlyContinue
Write-Output ("wrote " + $Out)
