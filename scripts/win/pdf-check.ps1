# ui.pdfPreview on Windows (PDFium): for every PDF in a folder, probe the page
# count and text extraction (first and middle page), then render the first
# and middle page in the zant-pdf example and snapshot them. Run in the
# logged-in desktop session:
#   powershell -File scripts/win/run-desktop.ps1 -Exe powershell.exe `
#     -Arguments "-NoProfile -ExecutionPolicy Bypass -File <repo>\scripts\win\pdf-check.ps1 -Folder <pdf folder>" -Wait 1200
param(
    [Parameter(Mandatory = $true)][string]$Folder,
    [string]$App = "",
    [string]$Out = ""
)
# defaults here: $PSScriptRoot is empty inside param() on Windows PowerShell 5.1
if (!$App) { $App = (Join-Path (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)) "zig-out\bin\zant-pdf.exe") }
if (!$Out) { $Out = (Join-Path $env:TEMP "zant-checks\pdf") }
$ErrorActionPreference = "Stop"
Remove-Item -Recurse -Force -ErrorAction SilentlyContinue $Out
New-Item -ItemType Directory -Force $Out | Out-Null
$report = Join-Path $Out "report.txt"

function Run([string]$file, [hashtable]$envs, [int]$timeout) {
    $psi = New-Object Diagnostics.ProcessStartInfo $App
    $psi.Arguments = "`"$file`""
    $psi.UseShellExecute = $false
    $psi.RedirectStandardError = $true
    $psi.RedirectStandardOutput = $true
    foreach ($k in $envs.Keys) { $psi.EnvironmentVariables[$k] = $envs[$k] }
    $p = [Diagnostics.Process]::Start($psi)
    $err = $p.StandardError.ReadToEndAsync()
    $std = $p.StandardOutput.ReadToEndAsync()
    $done = $p.WaitForExit($timeout)
    if (!$done) { $p.Kill(); $p.WaitForExit() }
    [pscustomobject]@{ Done = $done; Code = $p.ExitCode; Text = $std.Result + $err.Result }
}

$i = 0
# "._*" are macOS AppleDouble files, not PDFs
foreach ($file in (Get-ChildItem $Folder -Filter *.pdf | Where-Object { $_.Name -notlike "._*" } | Sort-Object Name)) {
    $i++
    $probe = Run $file.FullName @{ ZANT_PDF_PROBE = "1" } 60000
    $pages = 0; $t0 = 0; $tm = 0
    if ($probe.Text -match "pages=(\d+) text0=(\d+) text_mid=(\d+)") { $pages = [int]$Matches[1]; $t0 = [int]$Matches[2]; $tm = [int]$Matches[3] }
    $problems = @()
    if (!$probe.Done) { $problems += "probe timeout" }
    elseif ($probe.Code -ne 0) { $problems += "probe exit $($probe.Code)" }
    if ($pages -eq 0) { $problems += "no pages" }
    $mid = [math]::Floor($pages / 2)
    foreach ($view in @(@{ Name = "p1"; Page = "0" }, @{ Name = "mid"; Page = "$mid" })) {
        if ($view.Name -eq "mid" -and $pages -lt 2) { continue }
        $png = Join-Path $Out ("{0:D2}-{1}.png" -f $i, $view.Name)
        $r = Run $file.FullName @{ ZANT_SNAPSHOT = $png; ZANT_SNAPSHOT_EXIT = "1"; ZANT_PDF_PAGE = $view.Page } 60000
        if (!$r.Done) { $problems += "$($view.Name): timeout" }
        elseif ($r.Code -ne 0) { $problems += "$($view.Name): exit $($r.Code)" }
        if (!(Test-Path $png)) { $problems += "$($view.Name): no snapshot" }
        if ($r.Text -match "panic|error\(|leaked") { $problems += "$($view.Name): " + (($r.Text -split "`n" | Where-Object { $_ -match "panic|error\(|leaked" } | Select-Object -First 1).Trim()) }
    }
    $status = if ($problems.Count -eq 0) { "PASS" } else { "FAIL" }
    Add-Content -Encoding UTF8 $report ("{0} {1:D2} pages={2} text_p1={3} text_mid={4} {5} {6}" -f $status, $i, $pages, $t0, $tm, $file.Name, ($problems -join "; "))
}
# interactive: "复制文本" puts the page text on the clipboard, "›" turns the page
. (Join-Path $PSScriptRoot "ui-auto.ps1")
$script:ResultFile = $report
# the first PDF with a text layer of a few pages
$sample = Get-ChildItem $Folder -Filter *.pdf | Where-Object { $_.Name -notlike "._*" } | Sort-Object Length | Where-Object {
    (Run $_.FullName @{ ZANT_PDF_PROBE = "1" } 60000).Text -match "pages=([3-9]|\d\d+) text0=[1-9]"
} | Select-Object -First 1
if ($sample) {
    Set-Clipboard -Value "before"
    $viewer = Start-ZantApp $App $Out -Arguments "`"$($sample.FullName)`""
    Click-Key $viewer "pdf-copy" | Out-Null
    Start-Sleep -Milliseconds 300
    $clip = Get-Clipboard -Raw
    Write-Result ($clip -ne $null -and $clip -ne "before" -and $clip.Length -gt 20 -and $clip.Contains("`r`n")) "interactive: 复制文本 copies page 1 of $($sample.Name) (CRLF on the clipboard)"
    Click-Key $viewer "pdf-next" | Out-Null
    Click-Key $viewer "pdf-next" | Out-Null
    Shot $viewer "interactive-page3"
    Click-Key $viewer "pdf-copy" | Out-Null
    Start-Sleep -Milliseconds 300
    $clip3 = Get-Clipboard -Raw
    Write-Result ($clip3 -ne $null -and $clip3 -ne $clip -and $clip3.Length -gt 100) "interactive: › twice, then 复制文本 copies page 3"
    Click-Key $viewer "pdf-last" | Out-Null
    Shot $viewer "interactive-last"
    Click-Key $viewer "pdf-first" | Out-Null
    Shot $viewer "interactive-first"
    Write-Result (!$viewer.Process.HasExited) "interactive: navigation buttons keep the viewer running"
    Stop-ZantApp $viewer
}
Add-Content -Encoding UTF8 $report "done"
