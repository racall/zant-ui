# Runs the Windows desktop interaction checks one after another in the
# logged-in session (only one app can own the mouse / foreground at a time, so
# they must not overlap). Called through run-desktop.ps1:
#
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File E:\dev\XyUI\scripts\win\desktop-checks.ps1" -Wait 3600
#
# -Checks selects a subset (default: all). Missing executables are SKIPped
# (`smoke` / `docs` drive the stillmd app passed as -StillExe).
# Every check writes its own report and screenshots under -Out; this driver
# writes summary.txt and prints it at the end.
param(
    [string]$Repo = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$Out = "",
    [string]$ComponentsExe = "",
    [string]$MdStreamExe = "",
    [string]$PdfExe = "",
    [string]$StillExe = "",
    [string]$Sse = "",
    [string]$PdfFolder = "",
    [string]$ZantExe = "",
    [string]$Checks = "components,resize,stream,pdf,scaffold,smoke,docs"
)
$ErrorActionPreference = "Continue"
if (!$Out) { $Out = Join-Path $env:TEMP "zant-checks/desktop" }
if (!$ComponentsExe) { $ComponentsExe = Join-Path $Repo "target/release/zant-components.exe" }
if (!$MdStreamExe) { $MdStreamExe = Join-Path $Repo "target/release/zant-md-stream.exe" }
if (!$PdfExe) { $PdfExe = Join-Path $Repo "target/release/zant-pdf.exe" }
if (!$Sse) { $Sse = Join-Path $Repo "scripts/win/fixtures/stream-replay.sse" }
if (!$PdfFolder) { $PdfFolder = Join-Path $Repo "tests/fixtures" }
if (!$ZantExe) { $ZantExe = Join-Path $Repo "target/release/zant.exe" }
New-Item -ItemType Directory -Force $Out | Out-Null
$summary = Join-Path $Out "summary.txt"
Remove-Item -Force -ErrorAction SilentlyContinue $summary
$wanted = @($Checks -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ })

function Record([string]$name, [string]$status, [string]$detail) {
    $line = "{0} {1} {2}" -f $status, $name, $detail
    Add-Content -Encoding UTF8 $summary $line
    Write-Output $line
}

# report.txt (every check) or result.txt (smoke-stillmd.ps1).
function Report-Text([string]$dir) {
    foreach ($name in @("report.txt", "result.txt")) {
        $path = Join-Path $dir $name
        if (Test-Path $path) { return (Get-Content -Encoding UTF8 $path) -join "`n" }
    }
    return ""
}

function Run-Check([string]$name, [string]$script, [string]$argLine, [string]$exe, [string]$reportDir, [switch]$Metrics) {
    if (!$wanted.Contains($name)) { return }
    if (!(Test-Path $exe)) { Record $name SKIP "missing $exe"; return }
    Record $name START ""
    $log = Join-Path $Out "$name-console.log"
    try {
        Invoke-Expression ("& '$PSScriptRoot/$script' $argLine") *>&1 |
            Tee-Object -FilePath $log | Out-Null
    } catch {
        Record $name FAIL ("exception: " + $_.Exception.Message)
        return
    }
    $text = Report-Text $reportDir
    if (!$text) { Record $name FAIL "no report in $reportDir"; return }
    # keep a copy next to the driver summary (smoke writes into %TEMP%)
    New-Item -ItemType Directory -Force (Join-Path $Out $name) | Out-Null
    Set-Content -Encoding UTF8 (Join-Path (Join-Path $Out $name) "report.txt") $text
    $lines = $text -split "`n"
    $passes = @($lines | Where-Object { $_ -match "^\s*PASS" })
    $fails = @($lines | Where-Object { $_ -match "^\s*FAIL" })
    if ($Metrics) {
        $bad = @($lines | Where-Object { $_ -match "FAIL|error|panic" })
        if ($bad.Count -gt 0) {
            Record $name FAIL ("{0} problem line(s)" -f $bad.Count)
            foreach ($b in ($bad | Select-Object -First 5)) { Record $name "  FAIL" $b }
        } else {
            $head = ($text -split "`n" | Where-Object { $_.Trim() } | Select-Object -First 3) -join " | "
            Record $name PASS ("metrics: " + $head)
        }
        return
    }
    if ($fails.Count -eq 0 -and $passes.Count -gt 0) {
        Record $name PASS ("{0} PASS" -f $passes.Count)
    } else {
        Record $name FAIL ("{0} PASS, {1} FAIL" -f $passes.Count, $fails.Count)
        foreach ($f in $fails) { Record $name "  FAIL" $f }
    }
}

Set-Location $Repo
$d = Join-Path $Out "components"
Run-Check "components" "components-check.ps1" "-Exe '$ComponentsExe' -Out '$d'" $ComponentsExe $d
$d = Join-Path $Out "resize"
Run-Check "resize" "resize-check.ps1" "-Exe '$ComponentsExe' -Out '$d'" $ComponentsExe $d -Metrics
$d = Join-Path $Out "stream"
Run-Check "stream" "stream-resize-check.ps1" "-Sse '$Sse' -Exe '$MdStreamExe' -Out '$d'" $MdStreamExe $d -Metrics
$d = Join-Path $Out "pdf"
Run-Check "pdf" "pdf-check.ps1" "-Folder '$PdfFolder' -App '$PdfExe' -Out '$d'" $PdfExe $d
$d = Join-Path $Out "scaffold"
Run-Check "scaffold" "scaffold-check.ps1" "-Zant '$ZantExe' -Out '$d'" $ZantExe $d
$stillDir = Join-Path $env:TEMP "zant-checks/smoke"
Run-Check "smoke" "smoke-stillmd.ps1" "-App '$StillExe'" $StillExe $stillDir
$d = Join-Path $Out "docs"
Run-Check "docs" "docs-check.ps1" "-App '$StillExe' -Out '$d'" $StillExe $d

Record "done" "" ""
Write-Output "--- summary ---"
Get-Content -Encoding UTF8 $summary | Out-String
