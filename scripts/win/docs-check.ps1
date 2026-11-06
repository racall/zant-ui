# Opens every Markdown file under docs/ in the still. example on the Windows
# desktop (preview and edit mode), snapshots each and records exit code,
# snapshot and any panic / error output. Run in the logged-in session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\docs-check.ps1" -Wait 900
# -App: a Markdown app built on Zant UI that takes a file argument and the
#  STILL_MODE / STILL_SIZE variables (the still. example app)
param(
    [Parameter(Mandatory = $true)][string]$App,
    [string]$Docs = "",
    [string]$Out = "",
    [string[]]$Modes = @("preview", "edit")
)
# defaults here: $PSScriptRoot is empty inside param() on Windows PowerShell 5.1
if (!$Docs) { $Docs = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "docs") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\docs") }
$ErrorActionPreference = "Stop"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$report = Join-Path $Out "report.txt"

# "._*" are macOS AppleDouble metadata files, not documents
$files = Get-ChildItem -Recurse -Filter *.md $Docs | Where-Object { $_.Name -notlike "._*" } | Sort-Object FullName
foreach ($file in $files) {
    $rel = $file.FullName.Substring($Docs.Length + 1)
    $name = ($rel -replace "[\\/]", "_") -replace "\.md$", ""
    foreach ($mode in $Modes) {
        $png = Join-Path $Out "$name-$mode.png"
        $log = Join-Path $Out "$name-$mode.log"
        $psi = New-Object Diagnostics.ProcessStartInfo $App
        $psi.Arguments = "`"$($file.FullName)`""
        $psi.UseShellExecute = $false
        $psi.RedirectStandardError = $true
        $psi.RedirectStandardOutput = $true
        $psi.WorkingDirectory = $file.DirectoryName
        $psi.EnvironmentVariables["ZANT_SNAPSHOT"] = $png
        $psi.EnvironmentVariables["ZANT_SNAPSHOT_EXIT"] = "1"
        $psi.EnvironmentVariables["STILL_MODE"] = $mode
        $psi.EnvironmentVariables["STILL_SIZE"] = "1050x1300"
        $p = [Diagnostics.Process]::Start($psi)
        $stderr = $p.StandardError.ReadToEndAsync()
        $stdout = $p.StandardOutput.ReadToEndAsync()
        $exited = $p.WaitForExit(30000)
        if (!$exited) { $p.Kill(); $p.WaitForExit() }
        $text = $stdout.Result + $stderr.Result
        [IO.File]::WriteAllText($log, $text)
        $problems = @()
        if (!$exited) { $problems += "timeout" }
        elseif ($p.ExitCode -ne 0) { $problems += "exit $($p.ExitCode)" }
        if (!(Test-Path $png)) { $problems += "no snapshot" }
        if ($text -match "panic|error\(|leaked|failed") { $problems += "log: " + (($text -split "`n" | Where-Object { $_ -match "panic|error\(|leaked|failed" } | Select-Object -First 1).Trim()) }
        $status = if ($problems.Count -eq 0) { "PASS" } else { "FAIL" }
        Add-Content -Encoding UTF8 $report ("{0} {1} [{2}] {3}" -f $status, $rel, $mode, ($problems -join "; "))
    }
}
Add-Content -Encoding UTF8 $report "done"
