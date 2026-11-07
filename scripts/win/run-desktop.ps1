# Starts a program on the logged-in user's visible desktop of the Windows test
# machine. Processes started over SSH run in a non-interactive session (no
# visible window, no real display scale, unreliable GPU present), so this goes
# through a one-shot scheduled task (/IT) instead.
#
#   powershell -File scripts/win/run-desktop.ps1 -Exe zig-out\bin\zant-demo.exe
#   ... -Env "ZANT_SNAPSHOT=$env:TEMP\a.png;ZANT_SNAPSHOT_EXIT=1" -Wait 15
#
# -Wait N waits up to N seconds for the program to exit and prints its log.
# -Log <file> writes the program's output there (default: a shared log in
# %TEMP%\zant-desktop), e.g. for an app that keeps running while other
# programs are started with this script; give such a program its own -Name
# (the scheduled task: one name runs one program at a time).
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [string]$WorkDir = (Get-Location).Path,
    [string]$Arguments = "",
    [string]$Env = "",
    [int]$Wait = 0,
    [string]$Log = "",
    [string]$Name = "ZantDesktopRun"
)
$ErrorActionPreference = "Stop"
$cache = Join-Path $env:TEMP "zant-desktop"
New-Item -ItemType Directory -Force $cache | Out-Null
$log = if ($Log) { $Log } else { Join-Path $cache "desktop-run.log" }
$done = Join-Path $cache "$Name.done"
Remove-Item -Force -ErrorAction SilentlyContinue $log, $done

# a bare name ("powershell.exe") is looked up on PATH
$exePath = if ([IO.Path]::IsPathRooted($Exe) -or !($Exe -match "[\\/]")) { $Exe } else { Join-Path $WorkDir $Exe }
$lines = @("@echo off", "chcp 65001 >nul", "cd /d `"$WorkDir`"")
foreach ($pair in ($Env -split ";")) {
    if ($pair.Trim()) { $lines += "set `"$($pair.Trim())`"" }
}
$lines += "`"$exePath`" $Arguments > `"$log`" 2>&1"
$lines += "echo %ERRORLEVEL% > `"$done`""
$cmd = Join-Path $cache "$Name.cmd"
Set-Content -Encoding ASCII $cmd ($lines -join "`r`n")

$task = $Name
schtasks /Create /TN $task /TR "cmd /c `"$cmd`"" /SC ONCE /ST 00:00 /F /IT | Out-Null
schtasks /Run /TN $task | Out-Null

if ($Wait -gt 0) {
    $deadline = (Get-Date).AddSeconds($Wait)
    while (!(Test-Path $done) -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 300 }
    if (Test-Path $done) { "exit: $((Get-Content $done).Trim())" } else { "still running after $Wait s" }
    if (Test-Path $log) { Get-Content -Encoding UTF8 $log }
}
